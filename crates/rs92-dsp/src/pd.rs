//! CZ-style phase distortion.

use wide::{f32x8, CmpLt};

/// Precomputed phase-distortion knee for one operator.
#[derive(Clone, Copy, Debug)]
pub struct PdKnee {
    pub k: f32,
    lo_scale: f32,
    hi_scale: f32,
}

impl PdKnee {
    /// `pd` is the panel value, -100..100.
    pub fn new(pd: f32) -> Self {
        let d = (pd / 100.0).clamp(-1.0, 1.0);
        let k = 0.5 - 0.49 * d;
        PdKnee {
            k,
            lo_scale: 0.5 / k,
            hi_scale: 0.5 / (1.0 - k),
        }
    }

    #[inline]
    pub fn warp(&self, p: f32) -> f32 {
        if p < self.k {
            p * self.lo_scale
        } else {
            0.5 + (p - self.k) * self.hi_scale
        }
    }

    #[inline]
    pub fn warp_x8(&self, p: f32x8) -> f32x8 {
        let k = f32x8::splat(self.k);
        let lo = p * f32x8::splat(self.lo_scale);
        let hi = (p - k).mul_add(f32x8::splat(self.hi_scale), f32x8::splat(0.5));
        p.cmp_lt(k).blend(lo, hi)
    }
}

/// Resonance ratio of the Res waves: PD sets R = 1 + 15 * (pd + 100) / 200.
#[inline]
pub fn res_ratio(pd: f32) -> f32 {
    1.0 + 15.0 * (pd.clamp(-100.0, 100.0) + 100.0) / 200.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_pd_is_identity() {
        let k = PdKnee::new(0.0);
        for i in 0..1000 {
            let p = i as f32 / 1000.0;
            assert!((k.warp(p) - p).abs() < 1e-6);
        }
    }

    #[test]
    fn warp_is_continuous_and_monotonic() {
        for pd in [-100.0, -40.0, 26.0, 100.0] {
            let k = PdKnee::new(pd);
            let mut prev = k.warp(0.0);
            assert_eq!(prev, 0.0);
            for i in 1..=10_000 {
                let v = k.warp(i as f32 / 10_000.0);
                assert!(v >= prev && v - prev < 0.06, "pd {pd} jump at {i}");
                prev = v;
            }
            assert!((prev - 1.0).abs() < 1e-5);
        }
    }
}
