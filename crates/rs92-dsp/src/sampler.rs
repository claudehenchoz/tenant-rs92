//! SAMPLER playback voice: varispeed read of a baked chord plus the tail filter.

use crate::bake::BakedSample;
use crate::env::{Adsr, AdsrCoefs, SILENCE};
use crate::math::tan_approx;
use crate::patch::{Interp, Patch};
use std::sync::Arc;

/// Tail filter sweep range above TAIL LP while the key is held.
const TAIL_OCTAVES: f32 = 10.0;

/// Control-rate values shared by all playback voices for one sub-block.
#[derive(Clone, Copy, Debug)]
pub struct SamplerShared {
    pub amp: AdsrCoefs,
    pub tail: AdsrCoefs,
    pub tail_lp: f32,
    pub transpose: f32,
    pub bend: f32,
    pub crush_mix: f32,
    pub interp: Interp,
    pub vel_amp: f32,
    pub fs: f32,
}

impl Default for SamplerShared {
    fn default() -> Self {
        let mut s = SamplerShared {
            amp: AdsrCoefs::default(),
            tail: AdsrCoefs::default(),
            tail_lp: 411.0,
            transpose: 0.0,
            bend: 0.0,
            crush_mix: 0.35,
            interp: Interp::Vintage,
            vel_amp: 0.4,
            fs: 48_000.0,
        };
        s.update(&Patch::default(), 48_000.0, 0.0);
        s
    }
}

impl SamplerShared {
    pub fn update(&mut self, p: &Patch, fs: f32, bend: f32) {
        // Fixed playback envelope: 1 ms attack, full sustain, release = TAIL REL.
        self.amp.update(1.0, 100.0, 1.0, p.tail_rel, fs);
        // Tail filter envelope: zero attack, full sustain, release = TAIL REL.
        self.tail.update(0.0, 100.0, 1.0, p.tail_rel, fs);
        self.tail_lp = p.tail_lp;
        self.transpose = p.smp_transpose as f32;
        self.bend = bend;
        self.crush_mix = p.crush_mix / 100.0;
        self.interp = p.smp_interp;
        self.vel_amp = p.vel_amp / 100.0;
        self.fs = fs;
    }
}

/// Scalar TPT state-variable low-pass (stereo), Q 0.707.
#[derive(Clone, Copy, Debug, Default)]
struct TailLp {
    ic1: [f32; 2],
    ic2: [f32; 2],
    a1: f32,
    a2: f32,
    a3: f32,
}

impl TailLp {
    fn set(&mut self, fc: f32, fs: f32) {
        let fc = fc.clamp(20.0, 0.45 * fs);
        let g = tan_approx(std::f32::consts::PI * fc / fs);
        let k = std::f32::consts::SQRT_2;
        self.a1 = 1.0 / (1.0 + g * (g + k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }

    #[inline]
    fn tick(&mut self, ch: usize, v0: f32) -> f32 {
        let v3 = v0 - self.ic2[ch];
        let v1 = self.a1 * self.ic1[ch] + self.a2 * v3;
        let v2 = self.ic2[ch] + self.a2 * self.ic1[ch] + self.a3 * v3;
        self.ic1[ch] = 2.0 * v1 - self.ic1[ch];
        self.ic2[ch] = 2.0 * v2 - self.ic2[ch];
        v2
    }
}

#[inline]
fn hermite(buf: &[f32], pos: f64) -> f32 {
    let i = pos as usize;
    let f = (pos - i as f64) as f32;
    let at = |k: isize| -> f32 {
        let j = i as isize + k;
        if j < 0 || j as usize >= buf.len() {
            0.0
        } else {
            buf[j as usize]
        }
    };
    let (xm1, x0, x1, x2) = (at(-1), at(0), at(1), at(2));
    let c1 = 0.5 * (x1 - xm1);
    let c2 = xm1 - 2.5 * x0 + 2.0 * x1 - 0.5 * x2;
    let c3 = 0.5 * (x2 - xm1) + 1.5 * (x0 - x1);
    ((c3 * f + c2) * f + c1) * f + x0
}

#[derive(Clone, Debug, Default)]
pub struct SamplerVoice {
    pub active: bool,
    pub note: u8,
    pub held: bool,
    pub sustained: bool,
    pub started_at: u64,
    pub released_at: u64,
    buf: Option<Arc<BakedSample>>,
    pos: f64,
    vel_gain: f32,
    amp_env: Adsr,
    tail_env: Adsr,
    tail: TailLp,
    fading: bool,
    fade: f32,
    fade_dec: f32,
}

impl SamplerVoice {
    pub fn start(&mut self, note: u8, velocity: f32, buf: Arc<BakedSample>, now: u64) {
        self.active = true;
        self.note = note;
        self.held = true;
        self.sustained = false;
        self.started_at = now;
        self.released_at = u64::MAX;
        self.buf = Some(buf);
        self.pos = 0.0;
        self.vel_gain = velocity;
        self.amp_env.reset();
        self.tail_env.reset();
        self.amp_env.trigger();
        self.tail_env.trigger();
        self.tail = TailLp::default();
        self.fading = false;
        self.fade = 1.0;
    }

    pub fn release(&mut self, now: u64) {
        self.held = false;
        self.released_at = now;
        self.amp_env.release();
        self.tail_env.release();
    }

    pub fn is_released(&self) -> bool {
        !self.held && !self.sustained
    }

    pub fn steal(&mut self, fs: f32) {
        if !self.fading {
            self.fading = true;
            self.fade_dec = 1.0 / (crate::voice::STEAL_FADE_MS * 0.001 * fs);
        }
    }

    pub fn is_fading(&self) -> bool {
        self.fading
    }

    /// Takes the buffer out of a finished voice so the caller can retire it.
    pub fn take_buffer(&mut self) -> Option<Arc<BakedSample>> {
        if self.active {
            None
        } else {
            self.buf.take()
        }
    }

    /// Takes the buffer regardless of state (when a slot is reused).
    pub fn take_buffer_any(&mut self) -> Option<Arc<BakedSample>> {
        self.buf.take()
    }

    pub fn render(&mut self, sh: &SamplerShared, out_l: &mut [f32], out_r: &mut [f32]) {
        if !self.active {
            return;
        }
        let Some(buf) = self.buf.as_deref() else {
            self.active = false;
            return;
        };
        // Tail filter coefficients once per sub-block.
        let fc = sh.tail_lp * 2f32.powf(TAIL_OCTAVES * self.tail_env.level);
        self.tail.set(fc, sh.fs);

        let semis = self.note as f32 - buf.root as f32 + sh.transpose + sh.bend;
        let speed = 2f64.powf(semis as f64 / 12.0) * (buf.sr as f64 / sh.fs as f64);
        let crush_ratio = buf.crush_rate as f64 / buf.sr as f64;
        let vel = 1.0 - sh.vel_amp * (1.0 - self.vel_gain);
        let mix = sh.crush_mix;
        let len = buf.len() as f64;
        let clen = buf.crushed[0].len();

        for (l, r) in out_l.iter_mut().zip(out_r.iter_mut()) {
            if self.pos >= len - 1.0 {
                self.active = false;
                break;
            }
            let cpos = self.pos * crush_ratio;
            let mut s = [0.0f32; 2];
            for (ch, v) in s.iter_mut().enumerate() {
                let dry = hermite(&buf.dry[ch], self.pos);
                let wet = match sh.interp {
                    // Drop-sample: the grit of pitched early samplers.
                    Interp::Vintage => buf.crushed[ch][(cpos as usize).min(clen.saturating_sub(1))],
                    Interp::Clean => hermite(&buf.crushed[ch], cpos),
                };
                *v = dry + (wet - dry) * mix;
            }
            self.pos += speed;
            let mut amp = self.amp_env.next(&sh.amp) * vel;
            self.tail_env.next(&sh.tail);
            if self.fading {
                self.fade = (self.fade - self.fade_dec).max(0.0);
                amp *= self.fade;
            }
            *l += self.tail.tick(0, s[0]) * amp;
            *r += self.tail.tick(1, s[1]) * amp;
        }
        if (self.fading && self.fade <= 0.0)
            || !self.amp_env.is_active()
            || (self.is_released() && self.amp_env.level < SILENCE)
        {
            self.active = false;
        }
    }
}
