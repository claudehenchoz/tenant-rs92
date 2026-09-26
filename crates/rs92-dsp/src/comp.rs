//! Feed-forward, stereo-linked compressor: 4:1, 6 dB soft knee, 3 ms / 120 ms.

use crate::math::{db_to_gain, smooth_coef};

const RATIO: f32 = 4.0;
const KNEE_DB: f32 = 6.0;
const ATTACK_MS: f32 = 3.0;
const RELEASE_MS: f32 = 120.0;

/// Static gain reduction in dB (<= 0) for an input level in dB.
#[inline]
pub fn gain_reduction_db(level_db: f32, thresh_db: f32) -> f32 {
    let over = level_db - thresh_db;
    let slope = 1.0 / RATIO - 1.0;
    if 2.0 * over < -KNEE_DB {
        0.0
    } else if 2.0 * over.abs() <= KNEE_DB {
        let x = over + KNEE_DB * 0.5;
        slope * x * x / (2.0 * KNEE_DB)
    } else {
        slope * over
    }
}

#[derive(Clone, Debug)]
pub struct Compressor {
    thresh_db: f32,
    makeup: f32,
    att: f32,
    rel: f32,
    gr_db: f32,
}

impl Default for Compressor {
    fn default() -> Self {
        let mut c = Compressor {
            thresh_db: 0.0,
            makeup: 1.0,
            att: 0.0,
            rel: 0.0,
            gr_db: 0.0,
        };
        c.set(-9.0, 48_000.0);
        c
    }
}

impl Compressor {
    pub fn set(&mut self, thresh_db: f32, fs: f32) {
        self.thresh_db = thresh_db;
        // Auto makeup: half of the gain reduction a full-scale signal receives.
        self.makeup = db_to_gain(-0.5 * gain_reduction_db(0.0, thresh_db));
        self.att = smooth_coef(ATTACK_MS, fs);
        self.rel = smooth_coef(RELEASE_MS, fs);
    }

    pub fn reset(&mut self) {
        self.gr_db = 0.0;
    }

    /// Current smoothed gain reduction in dB (for metering).
    pub fn gain_reduction(&self) -> f32 {
        self.gr_db
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        for (a, b) in l.iter_mut().zip(r.iter_mut()) {
            let peak = a.abs().max(b.abs());
            let target = if peak > 1e-6 {
                gain_reduction_db(20.0 * peak.log10(), self.thresh_db)
            } else {
                0.0
            };
            let coef = if target < self.gr_db {
                self.att
            } else {
                self.rel
            };
            self.gr_db = target + (self.gr_db - target) * coef;
            let g = if self.gr_db > -1e-4 {
                self.makeup
            } else {
                db_to_gain(self.gr_db) * self.makeup
            };
            *a *= g;
            *b *= g;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_curve() {
        assert_eq!(gain_reduction_db(-30.0, -9.0), 0.0);
        // Well above the knee: 4:1.
        assert!((gain_reduction_db(3.0, -9.0) + 9.0).abs() < 1e-4);
        // Continuous at the knee edges.
        let a = gain_reduction_db(-12.0 + 1e-3, -9.0);
        assert!(a.abs() < 1e-3);
        let b = gain_reduction_db(-6.0, -9.0);
        assert!((b + 0.75 * 3.0).abs() < 1e-3);
    }
}
