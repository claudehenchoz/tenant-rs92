//! LIVE chord voice: one played key, up to 8 partials in SIMD lanes.

use crate::chord::{ChordLayout, MAX_PARTIALS};
use crate::env::{Adsr, AdsrCoefs, SILENCE};
use crate::filter::{prewarp_x8, FilterCoefs, FilterState};
use crate::math::exp2_x8;
use crate::osc::{OpParams, OpState};
use crate::patch::{FilterType, Patch};
use wide::{f32x8, CmpGe};

/// Level of each partial before the finish chain, scaled by 1/sqrt(count).
const PARTIAL_GAIN: f32 = 0.24;
/// Fade-out time of a stolen voice.
pub const STEAL_FADE_MS: f32 = 3.0;
/// Per-partial onset ramp, so strummed partials start without a click.
const ONSET_MS: f32 = 1.0;
/// Largest block at the oversampled rate (32 host samples at 4×).
pub const MAX_OS_BLOCK: usize = 128;

static ONES: [f32x8; MAX_OS_BLOCK] = [f32x8::ONE; MAX_OS_BLOCK];

/// Per-block work buffers, shared by all voices of a bank so nothing is zeroed or
/// allocated per voice.
pub struct VoiceScratch {
    x: [f32x8; MAX_OS_BLOCK],
    run: [f32x8; MAX_OS_BLOCK],
    onset: [f32x8; MAX_OS_BLOCK],
    amp: [f32; MAX_OS_BLOCK],
}

impl VoiceScratch {
    pub fn new_boxed() -> Box<Self> {
        Box::new(VoiceScratch {
            x: [f32x8::ZERO; MAX_OS_BLOCK],
            run: [f32x8::ONE; MAX_OS_BLOCK],
            onset: [f32x8::ONE; MAX_OS_BLOCK],
            amp: [0.0; MAX_OS_BLOCK],
        })
    }
}

/// Control-rate values shared by every voice for one sub-block.
#[derive(Clone, Copy, Debug)]
pub struct VoiceShared {
    pub ops: OpParams,
    pub amp: AdsrCoefs,
    pub flt: AdsrCoefs,
    pub filter_ty: [FilterType; 3],
    pub filter_base_oct: [f32; 3],
    pub filter_reso: [f32; 3],
    pub filter_env: [f32; 3],
    pub filter_key: [f32; 3],
    pub vel_tone: f32,
    pub vel_amp: f32,
    /// Oversampled rate.
    pub fs: f32,
    /// Pitch bend in semitones.
    pub bend: f32,
}

impl Default for VoiceShared {
    fn default() -> Self {
        let mut s = VoiceShared {
            ops: OpParams::from_patch(&Patch::default()),
            amp: AdsrCoefs::default(),
            flt: AdsrCoefs::default(),
            filter_ty: [FilterType::Off; 3],
            filter_base_oct: [0.0; 3],
            filter_reso: [0.0; 3],
            filter_env: [0.0; 3],
            filter_key: [0.0; 3],
            vel_tone: 0.0,
            vel_amp: 0.0,
            fs: 96_000.0,
            bend: 0.0,
        };
        s.update(&Patch::default(), 96_000.0, 0.0);
        s
    }
}

impl VoiceShared {
    pub fn update(&mut self, p: &Patch, fs: f32, bend: f32) {
        self.ops = OpParams::from_patch(p);
        self.amp
            .update(p.amp_a, p.amp_d, p.amp_s / 100.0, p.amp_r, fs);
        self.flt
            .update(p.flt_a, p.flt_d, p.flt_s / 100.0, p.flt_r, fs);
        for (i, f) in p.filters.iter().enumerate() {
            self.filter_ty[i] = f.ty;
            self.filter_base_oct[i] = f.cutoff.max(1.0).log2();
            self.filter_reso[i] = f.reso;
            self.filter_env[i] = f.env / 100.0;
            self.filter_key[i] = f.key / 100.0;
        }
        self.vel_tone = p.vel_tone / 100.0;
        self.vel_amp = p.vel_amp / 100.0;
        self.fs = fs;
        self.bend = bend;
    }
}

#[derive(Clone, Debug, Default)]
pub struct ChordVoice {
    pub active: bool,
    pub note: u8,
    /// Key is physically held.
    pub held: bool,
    /// Release deferred by the sustain pedal.
    pub sustained: bool,
    /// Allocation order, for stealing.
    pub started_at: u64,
    pub released_at: u64,
    amp_vel: f32,
    tone_vel: f32,

    layout: ChordLayout,
    /// Partial pitch (MIDI, incl. detune) per lane.
    pitch: f32x8,
    pan_l: f32x8,
    pan_r: f32x8,
    lane_gain: f32x8,
    delay: f32x8,
    max_delay: f32,
    onset_inc: f32,
    counter: f32,

    ops: OpState,
    filters: [FilterState; 3],
    fcoefs: [FilterCoefs; 3],
    amp_env: Adsr,
    flt_env: Adsr,

    fading: bool,
    fade: f32,
    fade_dec: f32,
}

impl ChordVoice {
    /// Starts the voice. `amp_vel` scales the level, `tone_vel` drives VEL>TONE.
    pub fn start(
        &mut self,
        note: u8,
        amp_vel: f32,
        tone_vel: f32,
        patch: &Patch,
        fs: f32,
        now: u64,
    ) {
        self.active = true;
        self.note = note;
        self.held = true;
        self.sustained = false;
        self.started_at = now;
        self.released_at = u64::MAX;
        self.amp_vel = amp_vel;
        self.tone_vel = tone_vel;
        self.layout = ChordLayout::build(note as i32, patch);
        let l = &self.layout;
        let mut pitch = [0.0; MAX_PARTIALS];
        let mut pan_l = [0.0; MAX_PARTIALS];
        let mut pan_r = [0.0; MAX_PARTIALS];
        let mut gain = [0.0; MAX_PARTIALS];
        let mut delay = [0.0; MAX_PARTIALS];
        let g = PARTIAL_GAIN / (l.count.max(1) as f32).sqrt();
        for i in 0..MAX_PARTIALS {
            if i < l.count {
                pitch[i] = l.notes[i] as f32 + l.detune_cents[i] / 100.0;
                pan_l[i] = l.pan_l[i];
                pan_r[i] = l.pan_r[i];
                gain[i] = g;
                delay[i] = l.delay_ms[i] * 0.001 * fs;
            } else {
                // Unused lanes: silent, parked at a harmless pitch.
                pitch[i] = 60.0;
            }
        }
        self.pitch = f32x8::from(pitch);
        self.pan_l = f32x8::from(pan_l);
        self.pan_r = f32x8::from(pan_r);
        self.lane_gain = f32x8::from(gain);
        self.delay = f32x8::from(delay);
        self.max_delay = delay.iter().fold(0.0f32, |m, &d| m.max(d));
        self.onset_inc = 1.0 / (ONSET_MS * 0.001 * fs);
        self.counter = 0.0;
        self.ops.reset();
        for f in &mut self.filters {
            f.reset();
        }
        self.amp_env.reset();
        self.flt_env.reset();
        self.amp_env.trigger();
        self.flt_env.trigger();
        self.fading = false;
        self.fade = 1.0;
    }

    pub fn release(&mut self, now: u64) {
        self.held = false;
        self.released_at = now;
        self.amp_env.release();
        self.flt_env.release();
    }

    pub fn is_released(&self) -> bool {
        !self.held && !self.sustained
    }

    /// Starts a 3 ms fade-out, after which the voice frees itself.
    pub fn steal(&mut self, fs: f32) {
        if !self.fading {
            self.fading = true;
            self.fade_dec = 1.0 / (STEAL_FADE_MS * 0.001 * fs);
        }
    }

    pub fn is_fading(&self) -> bool {
        self.fading
    }

    pub fn layout(&self) -> &ChordLayout {
        &self.layout
    }

    /// Updates the per-lane filter coefficients for this sub-block.
    fn update_filters(&mut self, sh: &VoiceShared) {
        let env = self.flt_env.level;
        let key_oct = (self.pitch - f32x8::splat(60.0)) * f32x8::splat(1.0 / 12.0);
        let vel_oct = sh.vel_tone * (self.tone_vel - 0.5) * 2.0;
        for i in 0..3 {
            let ty = sh.filter_ty[i];
            if ty == FilterType::Off {
                self.fcoefs[i] = FilterCoefs::default();
                continue;
            }
            let oct = f32x8::splat(sh.filter_base_oct[i] + env * sh.filter_env[i] * 7.0 + vel_oct)
                + key_oct * f32x8::splat(sh.filter_key[i]);
            let fc = exp2_x8(oct);
            self.fcoefs[i] = FilterCoefs::new(ty, prewarp_x8(fc, sh.fs), sh.filter_reso[i]);
        }
    }

    /// Adds `acc_l.len()` samples at the oversampled rate into per-lane stereo
    /// accumulators; the bank sums the lanes once for all voices.
    pub fn render(
        &mut self,
        sh: &VoiceShared,
        scratch: &mut VoiceScratch,
        acc_l: &mut [f32x8],
        acc_r: &mut [f32x8],
    ) {
        if !self.active {
            return;
        }
        let n = acc_l.len();
        debug_assert!(n <= MAX_OS_BLOCK);
        self.update_filters(sh);
        let hz = exp2_x8((self.pitch + f32x8::splat(sh.bend - 69.0)) * f32x8::splat(1.0 / 12.0))
            * f32x8::splat(440.0);
        let dt1 = hz * f32x8::splat(1.0 / sh.fs);

        // Strum: which lanes run, and their onset ramps.
        let VoiceScratch { x, run, onset, amp } = scratch;
        let settled = self.counter >= self.max_delay + 1.0 / self.onset_inc;
        if !settled {
            let inc = f32x8::splat(self.onset_inc);
            for i in 0..n {
                let t = f32x8::splat(self.counter + i as f32);
                run[i] = t.cmp_ge(self.delay).blend(f32x8::ONE, f32x8::ZERO);
                onset[i] = ((t - self.delay) * inc).max(f32x8::ZERO).min(f32x8::ONE);
            }
        }
        self.counter += n as f32;
        let run: &[f32x8] = if settled { &ONES[..n] } else { &run[..n] };

        let x = &mut x[..n];
        self.ops.render_block(&sh.ops, dt1, run, x);
        for (f, c) in self.filters.iter_mut().zip(&self.fcoefs) {
            f.process_block(c, x);
        }

        // Envelopes (scalar, per voice).
        let vel_gain = 1.0 - sh.vel_amp * (1.0 - self.amp_vel);
        let amp = &mut amp[..n];
        self.amp_env.fill(&sh.amp, amp);
        if self.fading {
            for a in amp.iter_mut() {
                self.fade = (self.fade - self.fade_dec).max(0.0);
                *a *= self.fade * vel_gain;
            }
        } else {
            amp.iter_mut().for_each(|a| *a *= vel_gain);
        }
        // The filter envelope only feeds control-rate cutoffs.
        self.flt_env.advance(&sh.flt, n);

        let (gl, gr) = (self.lane_gain * self.pan_l, self.lane_gain * self.pan_r);
        if settled {
            for i in 0..n {
                let v = x[i] * f32x8::splat(amp[i]);
                acc_l[i] = v.mul_add(gl, acc_l[i]);
                acc_r[i] = v.mul_add(gr, acc_r[i]);
            }
        } else {
            for i in 0..n {
                let v = x[i] * onset[i] * f32x8::splat(amp[i]);
                acc_l[i] = v.mul_add(gl, acc_l[i]);
                acc_r[i] = v.mul_add(gr, acc_r[i]);
            }
        }

        let done = (self.fading && self.fade <= 0.0)
            || (!self.amp_env.is_active())
            || (self.is_released() && self.amp_env.level < SILENCE);
        if done {
            self.active = false;
        }
    }
}
