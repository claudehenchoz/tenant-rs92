//! ADSR envelopes: linear attack, exponential decay and release.

use crate::math::decay_coef;

/// Level under which an envelope counts as silent (-96 dB).
pub const SILENCE: f32 = 1.584_893_2e-5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Stage {
    #[default]
    Idle,
    Attack,
    Decay,
    Release,
}

/// Envelope timing in samples-derived coefficients.
#[derive(Clone, Copy, Debug)]
pub struct AdsrCoefs {
    attack_inc: f32,
    decay: f32,
    sustain: f32,
    release: f32,
    // Inputs, to recompute only on change.
    key: [f32; 5],
}

impl Default for AdsrCoefs {
    fn default() -> Self {
        let mut c = AdsrCoefs {
            attack_inc: 1.0,
            decay: 0.0,
            sustain: 0.0,
            release: 0.0,
            key: [f32::NAN; 5],
        };
        c.update(0.0, 100.0, 0.0, 100.0, 48_000.0);
        c
    }
}

impl AdsrCoefs {
    /// Times in ms (D and R are the time to fall 60 dB), sustain 0..1.
    /// Recomputes only when an input changed.
    pub fn update(&mut self, a: f32, d: f32, s: f32, r: f32, fs: f32) {
        let key = [a, d, s, r, fs];
        if key == self.key {
            return;
        }
        self.key = key;
        let a_samples = a * 0.001 * fs;
        self.attack_inc = if a_samples < 1.0 {
            1.0
        } else {
            1.0 / a_samples
        };
        self.decay = decay_coef(d, fs);
        self.sustain = s.clamp(0.0, 1.0);
        self.release = decay_coef(r, fs);
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Adsr {
    pub stage: Stage,
    pub level: f32,
}

impl Adsr {
    /// Starts (or restarts) the attack from the current level.
    pub fn trigger(&mut self) {
        self.stage = Stage::Attack;
    }

    pub fn release(&mut self) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
        }
    }

    pub fn reset(&mut self) {
        *self = Adsr::default();
    }

    pub fn is_active(&self) -> bool {
        self.stage != Stage::Idle
    }

    /// Fills `out` with successive envelope values, one tight loop per stage.
    pub fn fill(&mut self, c: &AdsrCoefs, out: &mut [f32]) {
        let mut i = 0;
        let n = out.len();
        while i < n {
            match self.stage {
                Stage::Idle => {
                    out[i..].fill(0.0);
                    return;
                }
                Stage::Attack => {
                    self.next(c);
                    out[i] = self.level;
                    i += 1;
                }
                Stage::Decay => {
                    let (s, k) = (c.sustain, c.decay);
                    let mut level = self.level;
                    for o in &mut out[i..] {
                        level = s + (level - s) * k;
                        *o = level;
                    }
                    self.level = level;
                    if s < SILENCE && level < SILENCE {
                        self.level = 0.0;
                        self.stage = Stage::Idle;
                    }
                    return;
                }
                Stage::Release => {
                    let k = c.release;
                    let mut level = self.level;
                    for o in &mut out[i..] {
                        level *= k;
                        *o = level;
                    }
                    self.level = level;
                    if level < SILENCE {
                        self.level = 0.0;
                        self.stage = Stage::Idle;
                    }
                    return;
                }
            }
        }
    }

    /// Advances by `n` samples without producing output (control-rate envelopes).
    pub fn advance(&mut self, c: &AdsrCoefs, n: usize) {
        match self.stage {
            Stage::Idle => {}
            Stage::Attack => {
                for _ in 0..n {
                    self.next(c);
                }
            }
            Stage::Decay => {
                let k = c.decay.powi(n as i32);
                self.level = c.sustain + (self.level - c.sustain) * k;
                if c.sustain < SILENCE && self.level < SILENCE {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
            }
            Stage::Release => {
                self.level *= c.release.powi(n as i32);
                if self.level < SILENCE {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        }
    }

    #[inline]
    pub fn next(&mut self, c: &AdsrCoefs) -> f32 {
        match self.stage {
            Stage::Idle => {}
            Stage::Attack => {
                self.level += c.attack_inc;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                self.level = c.sustain + (self.level - c.sustain) * c.decay;
                if c.sustain < SILENCE && self.level < SILENCE {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
            }
            Stage::Release => {
                self.level *= c.release;
                if self.level < SILENCE {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        }
        self.level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decay_hits_minus_60_db_on_time() {
        let fs = 48_000.0;
        for d in [50.0, 380.0, 2000.0] {
            let mut c = AdsrCoefs::default();
            c.update(0.0, d, 0.0, 100.0, fs);
            let mut e = Adsr::default();
            e.trigger();
            let mut n = 0usize;
            // Reach the top first.
            while e.stage == Stage::Attack {
                e.next(&c);
            }
            while e.level > 0.001 {
                e.next(&c);
                n += 1;
            }
            let ms = n as f32 / fs * 1000.0;
            assert!((ms - d).abs() <= d * 0.01, "decay {d}: {ms}");
        }
    }

    #[test]
    fn retrigger_never_drops() {
        let mut c = AdsrCoefs::default();
        c.update(20.0, 300.0, 0.2, 200.0, 48_000.0);
        let mut e = Adsr::default();
        e.trigger();
        for _ in 0..5000 {
            e.next(&c);
        }
        e.release();
        for _ in 0..2000 {
            e.next(&c);
        }
        let before = e.level;
        e.trigger();
        let mut min = f32::MAX;
        for _ in 0..2000 {
            min = min.min(e.next(&c));
        }
        assert!(20.0 * (min / before).log10() > -1.0);
    }
}
