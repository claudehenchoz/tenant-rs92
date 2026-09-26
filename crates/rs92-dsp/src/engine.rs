//! The complete instrument: LIVE voice bank, finish chain, SAMPLER playback and the
//! bake worker handoff.

use crate::bake::{BakeClient, BakeWorker, BakedSample};
use crate::comp::Compressor;
use crate::crush::Crusher;
use crate::math::{db_to_gain, smooth_coef};
use crate::oversample::StereoDecimator;
use crate::patch::Patch;
use crate::sampler::{SamplerShared, SamplerVoice};
use crate::voice::{ChordVoice, VoiceScratch, VoiceShared};
use std::sync::Arc;
use wide::f32x8;

/// Largest sub-block, in host-rate samples.
pub const MAX_SUB_BLOCK: usize = 32;
/// Upper bound of the `voices` parameter.
pub const MAX_VOICES: usize = 16;
/// Extra slots so stolen voices can fade out while new ones start.
const SPARE_VOICES: usize = 4;
/// Re-bake this long after the last bake-parameter change.
pub const BAKE_DEBOUNCE_MS: f32 = 40.0;
/// Skip the finish chain after this much silence with no voices.
const IDLE_SKIP_S: f32 = 2.0;
/// HYPER STAB offsets.
pub const HYPER_COMP_DB: f32 = -6.0;
pub const HYPER_TAPE_DB: f32 = 4.0;
pub const HYPER_TRIM_DB: f32 = -2.0;

/// Crush → Comp → Tape, at the host rate.
#[derive(Clone, Debug, Default)]
pub struct Finish {
    pub crusher: Crusher,
    pub comp: Compressor,
    pub tape: crate::tape::Tape,
}

impl Finish {
    /// `crush_mix` overrides the patch's mix (0..1) when set.
    pub fn set(&mut self, p: &Patch, fs: f32, crush_mix: Option<f32>) {
        let mix = crush_mix.unwrap_or(p.crush_mix / 100.0);
        self.crusher.set(p.crush_rate, p.crush_bits.bits(), mix, fs);
        let hyper = p.hyper as u8 as f32;
        self.comp.set(p.comp_thresh + HYPER_COMP_DB * hyper, fs);
        self.tape.set(p.tape_level, HYPER_TAPE_DB * hyper, fs);
    }

    pub fn reset(&mut self) {
        self.crusher.reset();
        self.comp.reset();
        self.tape.reset();
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        self.crusher.process(l, r);
        self.comp.process(l, r);
        self.tape.process(l, r);
    }
}

/// LIVE chord voices, rendered at the oversampled rate and decimated to the host rate.
pub struct VoiceBank {
    pub voices: Vec<ChordVoice>,
    shared: VoiceShared,
    os_l: Vec<f32>,
    os_r: Vec<f32>,
    acc_l: Vec<f32x8>,
    acc_r: Vec<f32x8>,
    scratch: Box<VoiceScratch>,
    dec: StereoDecimator,
    os: usize,
    sr: f32,
    limit: usize,
}

impl VoiceBank {
    pub fn new(sr: f32) -> Self {
        crate::init_tables();
        VoiceBank {
            voices: vec![ChordVoice::default(); MAX_VOICES + SPARE_VOICES],
            shared: VoiceShared::default(),
            os_l: vec![0.0; MAX_SUB_BLOCK * 4],
            os_r: vec![0.0; MAX_SUB_BLOCK * 4],
            acc_l: vec![f32x8::ZERO; MAX_SUB_BLOCK * 4],
            acc_r: vec![f32x8::ZERO; MAX_SUB_BLOCK * 4],
            scratch: VoiceScratch::new_boxed(),
            dec: StereoDecimator::default(),
            os: 2,
            sr,
            limit: 8,
        }
    }

    pub fn set(&mut self, p: &Patch, bend: f32) {
        let os = p.os_factor.factor();
        if os != self.os {
            self.os = os;
            self.dec.reset();
        }
        self.limit = (p.voices as usize).clamp(1, MAX_VOICES);
        self.shared.update(p, self.sr * self.os as f32, bend);
    }

    pub fn any_active(&self) -> bool {
        self.voices.iter().any(|v| v.active)
    }

    pub fn note_on(&mut self, note: u8, amp_vel: f32, tone_vel: f32, patch: &Patch, now: u64) {
        let fs_os = self.sr * self.os as f32;
        let slot = allocate(
            &mut self.voices,
            self.limit,
            fs_os,
            |v| v.active,
            |v| v.is_fading(),
            |v| (v.is_released(), v.released_at, v.started_at),
            |v, fs| v.steal(fs),
        );
        self.voices[slot].start(note, amp_vel, tone_vel, patch, fs_os, now);
    }

    pub fn note_off(&mut self, note: u8, sustain: bool, now: u64) {
        for v in self
            .voices
            .iter_mut()
            .filter(|v| v.active && v.held && v.note == note)
        {
            if sustain {
                v.held = false;
                v.sustained = true;
            } else {
                v.release(now);
            }
        }
    }

    pub fn release_sustained(&mut self, now: u64) {
        for v in self.voices.iter_mut().filter(|v| v.active && v.sustained) {
            v.sustained = false;
            v.release(now);
        }
    }

    pub fn release_all(&mut self, now: u64) {
        for v in self.voices.iter_mut().filter(|v| v.active) {
            v.sustained = false;
            v.release(now);
        }
    }

    /// Renders `l.len()` host-rate samples (overwriting `l` and `r`).
    /// Returns false (and leaves the buffers zeroed) when no voice is active.
    pub fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        l.fill(0.0);
        r.fill(0.0);
        if !self.any_active() {
            return false;
        }
        let n = l.len();
        let m = n * self.os;
        let (al, ar) = (&mut self.acc_l[..m], &mut self.acc_r[..m]);
        al.fill(f32x8::ZERO);
        ar.fill(f32x8::ZERO);
        for v in self.voices.iter_mut().filter(|v| v.active) {
            v.render(&self.shared, &mut self.scratch, al, ar);
        }
        let (ol, or) = (&mut self.os_l[..m], &mut self.os_r[..m]);
        for i in 0..m {
            ol[i] = al[i].reduce_add();
            or[i] = ar[i].reduce_add();
        }
        self.dec.process_channel(0, self.os, ol, l);
        self.dec.process_channel(1, self.os, or, r);
        true
    }

    pub fn active_layouts(&self) -> impl Iterator<Item = &ChordVoice> {
        self.voices.iter().filter(|v| v.active && !v.is_fading())
    }
}

/// Picks a free slot, stealing if the voice limit is reached: oldest released voice
/// first, then the oldest held one, with a short fade-out.
fn allocate<V>(
    voices: &mut [V],
    limit: usize,
    fs: f32,
    active: impl Fn(&V) -> bool,
    fading: impl Fn(&V) -> bool,
    age: impl Fn(&V) -> (bool, u64, u64),
    steal: impl Fn(&mut V, f32),
) -> usize {
    let sounding = voices.iter().filter(|v| active(v) && !fading(v)).count();
    if sounding >= limit {
        // Oldest released first (by release time), else oldest held (by start time).
        let victim = voices
            .iter()
            .enumerate()
            .filter(|(_, v)| active(v) && !fading(v))
            .min_by_key(|(_, v)| {
                let (released, released_at, started_at) = age(v);
                if released {
                    (0, released_at)
                } else {
                    (1, started_at)
                }
            })
            .map(|(i, _)| i);
        if let Some(i) = victim {
            steal(&mut voices[i], fs);
        }
    }
    if let Some(i) = voices.iter().position(|v| !active(v)) {
        return i;
    }
    // Every slot is busy (fading voices included): reuse the oldest.
    voices
        .iter()
        .enumerate()
        .min_by_key(|(_, v)| age(v).2)
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Capacity of the queue that returns finished buffers to the worker.
const RETIRE_PENDING: usize = 64;

/// The full instrument.
pub struct Engine {
    sr: f32,
    patch: Patch,
    pub bank: VoiceBank,
    pub finish: Finish,
    sampler: Vec<SamplerVoice>,
    sampler_shared: SamplerShared,
    /// Most recent baked sample, used by new SAMPLER notes.
    current: Option<Arc<BakedSample>>,
    bake: Option<BakeClient>,
    _worker: Option<BakeWorker>,
    pending_retire: Vec<Arc<BakedSample>>,
    seen_key: u64,
    requested_key: u64,
    debounce: i64,
    bend_norm: f32,
    sustain: bool,
    now: u64,
    silent: usize,
    width: f32,
    gain: f32,
    gain_target: f32,
    gain_coef: f32,
    tmp_l: Vec<f32>,
    tmp_r: Vec<f32>,
}

impl Engine {
    /// Creates an engine with its own bake worker thread.
    pub fn new(sr: f32) -> Self {
        let (worker, client) = crate::bake::spawn_worker();
        let mut e = Self::without_worker(sr);
        e.bake = Some(client);
        e._worker = Some(worker);
        e
    }

    /// An engine without a bake worker: SAMPLER mode falls back to LIVE playback.
    /// Used by the bake renderer itself.
    pub fn without_worker(sr: f32) -> Self {
        let mut e = Engine {
            sr,
            patch: Patch::default(),
            bank: VoiceBank::new(sr),
            finish: Finish::default(),
            sampler: vec![SamplerVoice::default(); MAX_VOICES + SPARE_VOICES],
            sampler_shared: SamplerShared::default(),
            current: None,
            bake: None,
            _worker: None,
            pending_retire: Vec::with_capacity(RETIRE_PENDING),
            seen_key: 0,
            requested_key: 0,
            debounce: 0,
            bend_norm: 0.0,
            sustain: false,
            now: 0,
            silent: usize::MAX / 2,
            width: 1.0,
            gain: 0.0,
            gain_target: 0.0,
            gain_coef: smooth_coef(10.0, sr),
            tmp_l: vec![0.0; MAX_SUB_BLOCK],
            tmp_r: vec![0.0; MAX_SUB_BLOCK],
        };
        e.set_patch(&Patch::default());
        e.gain = e.gain_target;
        e
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    pub fn patch(&self) -> &Patch {
        &self.patch
    }

    /// Stats of the bake worker, if any.
    pub fn bake_stats(&self) -> Option<Arc<crate::bake::BakeStats>> {
        self._worker.as_ref().map(|w| w.stats.clone())
    }

    /// Installs a baked sample directly (offline rendering: golden tests, randomizer
    /// quality gate). Not for the audio thread: the previous sample is dropped here.
    pub fn set_sample(&mut self, sample: Arc<BakedSample>) {
        self.seen_key = sample.key;
        self.requested_key = sample.key;
        self.current = Some(sample);
    }

    /// Number of sounding voices (LIVE and SAMPLER).
    pub fn active_voices(&self) -> usize {
        self.bank.voices.iter().filter(|v| v.active).count()
            + self.sampler.iter().filter(|v| v.active).count()
    }

    /// The sample new SAMPLER notes will play, if one is ready.
    pub fn current_sample(&self) -> Option<&Arc<BakedSample>> {
        self.current.as_ref()
    }

    /// Applies a patch. Call once per sub-block (cheap when nothing changed).
    pub fn set_patch(&mut self, p: &Patch) {
        self.patch = *p;
        let bend = self.bend_norm * p.bend_range;
        self.bank.set(p, bend);
        self.finish.set(p, self.sr, None);
        self.sampler_shared.update(p, self.sr, bend);
        self.width = p.width / 100.0;
        let trim = if p.hyper { HYPER_TRIM_DB } else { 0.0 };
        self.gain_target = db_to_gain(p.output + trim);
    }

    /// Pitch bend, -1..1.
    pub fn pitch_bend(&mut self, value: f32) {
        self.bend_norm = value.clamp(-1.0, 1.0);
        let p = self.patch;
        let bend = self.bend_norm * p.bend_range;
        self.bank.set(&p, bend);
        self.sampler_shared.update(&p, self.sr, bend);
    }

    pub fn sustain(&mut self, on: bool) {
        self.sustain = on;
        if !on {
            self.bank.release_sustained(self.now);
            for v in self.sampler.iter_mut().filter(|v| v.active && v.sustained) {
                v.sustained = false;
                v.release(self.now);
            }
        }
    }

    pub fn all_notes_off(&mut self) {
        self.sustain = false;
        self.bank.release_all(self.now);
        for v in self.sampler.iter_mut().filter(|v| v.active) {
            v.sustained = false;
            v.release(self.now);
        }
    }

    /// `velocity` is 0..1.
    pub fn note_on(&mut self, note: u8, velocity: f32) {
        self.now += 1;
        self.silent = 0;
        let p = self.patch;
        match (&self.current, p.sampler_on) {
            (Some(buf), true) => {
                let buf = buf.clone();
                let slot = allocate(
                    &mut self.sampler,
                    (p.voices as usize).clamp(1, MAX_VOICES),
                    self.sr,
                    |v| v.active,
                    |v| v.is_fading(),
                    |v| (v.is_released(), v.released_at, v.started_at),
                    |v, fs| v.steal(fs),
                );
                // A reused slot may still hold an old buffer.
                if let Some(old) = self.sampler[slot].take_buffer_any() {
                    self.retire(old);
                }
                self.sampler[slot].start(note, velocity, buf, self.now);
            }
            // LIVE mode, or SAMPLER mode before the first bake is ready.
            _ => self.bank.note_on(note, velocity, velocity, &p, self.now),
        }
    }

    pub fn note_off(&mut self, note: u8) {
        self.now += 1;
        self.bank.note_off(note, self.sustain, self.now);
        let (sustain, now) = (self.sustain, self.now);
        for v in self
            .sampler
            .iter_mut()
            .filter(|v| v.active && v.held && v.note == note)
        {
            if sustain {
                v.held = false;
                v.sustained = true;
            } else {
                v.release(now);
            }
        }
    }

    /// Hands a buffer back to the worker for deallocation. Never drops it here.
    fn retire(&mut self, buf: Arc<BakedSample>) {
        if let Some(client) = &mut self.bake {
            if let Err(rtrb::PushError::Full(b)) = client.retire.push(buf) {
                if self.pending_retire.len() < self.pending_retire.capacity() {
                    self.pending_retire.push(b);
                } else {
                    // Should never happen; leaking beats deallocating on the audio thread.
                    std::mem::forget(b);
                }
            }
        }
        // Without a worker we are not on an audio thread; dropping is fine.
    }

    fn service_bake(&mut self, n: usize) {
        let Some(client) = &mut self.bake else { return };
        // Retry pending retirements.
        while let Some(b) = self.pending_retire.pop() {
            if let Err(rtrb::PushError::Full(b)) = client.retire.push(b) {
                self.pending_retire.push(b);
                break;
            }
        }
        // Pick up finished bakes.
        while let Ok(buf) = client.results.pop() {
            if let Some(old) = self.current.replace(buf) {
                if let Err(rtrb::PushError::Full(b)) = client.retire.push(old) {
                    if self.pending_retire.len() < self.pending_retire.capacity() {
                        self.pending_retire.push(b);
                    } else {
                        std::mem::forget(b);
                    }
                }
            }
        }
        if !self.patch.sampler_on {
            return;
        }
        let key = self.patch.bake_key(self.sr);
        if key != self.seen_key {
            self.seen_key = key;
            // The first bake (nothing to play yet) goes out immediately.
            self.debounce = if self.current.is_none() && self.requested_key == 0 {
                0
            } else {
                (BAKE_DEBOUNCE_MS * 0.001 * self.sr) as i64
            };
        } else {
            self.debounce -= n as i64;
        }
        if self.debounce <= 0 && key != self.requested_key {
            let req = crate::bake::BakeRequest {
                patch: self.patch,
                sr: self.sr,
                key,
            };
            if client.requests.push(req).is_ok() {
                self.requested_key = key;
            }
        }
    }

    /// True while a bake is pending for the current patch.
    pub fn bake_pending(&self) -> bool {
        self.patch.sampler_on
            && self.current.as_ref().map(|c| c.key) != Some(self.patch.bake_key(self.sr))
    }

    /// Renders up to [`MAX_SUB_BLOCK`] samples, overwriting `l` and `r`.
    pub fn render(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        debug_assert!(n <= MAX_SUB_BLOCK);
        let (l, r) = (&mut l[..n], &mut r[..n]);
        self.service_bake(n);

        // LIVE voices and the finish chain.
        let live = self.bank.render(l, r);
        if live {
            self.silent = 0;
        } else {
            self.silent = self.silent.saturating_add(n);
        }
        if (self.silent as f32) < IDLE_SKIP_S * self.sr {
            self.finish.process(l, r);
        } else {
            l.fill(0.0);
            r.fill(0.0);
        }

        // SAMPLER playback voices.
        let mut any_sampler = false;
        for i in 0..self.sampler.len() {
            if self.sampler[i].active {
                any_sampler = true;
                self.sampler[i].render(&self.sampler_shared, l, r);
                if let Some(b) = self.sampler[i].take_buffer() {
                    self.retire(b);
                }
            }
        }
        if !live && !any_sampler && self.silent as f32 >= IDLE_SKIP_S * self.sr {
            // Fully idle: skip width and gain work too.
            self.gain = self.gain_target;
            return;
        }

        // Width and output.
        let w = self.width;
        for (a, b) in l.iter_mut().zip(r.iter_mut()) {
            self.gain = self.gain_target + (self.gain - self.gain_target) * self.gain_coef;
            let m = 0.5 * (*a + *b);
            let s = 0.5 * (*a - *b) * w;
            *a = (m + s) * self.gain;
            *b = (m - s) * self.gain;
        }
    }

    /// Convenience for offline rendering: splits into sub-blocks.
    pub fn render_offline(&mut self, l: &mut [f32], r: &mut [f32]) {
        for (cl, cr) in l.chunks_mut(MAX_SUB_BLOCK).zip(r.chunks_mut(MAX_SUB_BLOCK)) {
            self.render(cl, cr);
        }
    }

    /// Scratch buffers for callers that need them without allocating.
    pub fn scratch(&mut self) -> (&mut [f32], &mut [f32]) {
        (&mut self.tmp_l, &mut self.tmp_r)
    }

    /// Notes lit on the keyboard display: partials of every sounding, held voice.
    pub fn for_each_lit_note(&self, mut f: impl FnMut(i32)) {
        for v in self.bank.active_layouts().filter(|v| v.held || v.sustained) {
            for &n in v.layout().notes() {
                f(n);
            }
        }
        for v in self
            .sampler
            .iter()
            .filter(|v| v.active && (v.held || v.sustained))
        {
            let layout = crate::chord::ChordLayout::build(v.note as i32, &self.patch);
            for &n in layout.notes() {
                f(n);
            }
        }
    }
}
