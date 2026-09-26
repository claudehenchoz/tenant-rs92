//! Vintage converter emulation: anti-alias one-pole, sample-and-hold, quantizer.

/// Quantizes to `bits` with mid-tread rounding and no dither. Output has exactly
/// `2^bits` levels over [-1, 1).
#[inline]
pub fn quantize(x: f32, bits: u32) -> f32 {
    let half_levels = (1u32 << (bits - 1)) as f32;
    let q = (x * half_levels)
        .round()
        .clamp(-half_levels, half_levels - 1.0);
    // `+ 0.0` folds -0.0 into 0.0.
    q / half_levels + 0.0
}

/// TPT one-pole low-pass.
#[derive(Clone, Copy, Debug, Default)]
pub struct OnePoleLp {
    g: f32,
    s: f32,
}

impl OnePoleLp {
    pub fn set(&mut self, fc: f32, fs: f32) {
        let fc = fc.clamp(10.0, 0.49 * fs);
        let g = (std::f32::consts::PI * fc / fs).tan();
        self.g = g / (1.0 + g);
    }

    #[inline]
    pub fn tick(&mut self, x: f32) -> f32 {
        let v = (x - self.s) * self.g;
        let y = v + self.s;
        self.s = y + v;
        y
    }

    pub fn reset(&mut self) {
        self.s = 0.0;
    }
}

/// Stereo crusher running at the host rate.
#[derive(Clone, Debug, Default)]
pub struct Crusher {
    aa: [OnePoleLp; 2],
    hold: [f32; 2],
    phase: f32,
    inc: f32,
    bits: Option<u32>,
    mix: f32,
    /// Number of sample-and-hold updates since reset (for tests and metering).
    pub updates: u64,
}

impl Crusher {
    pub fn set(&mut self, rate: f32, bits: Option<u32>, mix: f32, fs: f32) {
        for f in &mut self.aa {
            f.set(0.45 * rate, fs);
        }
        self.inc = (rate / fs).min(1.0);
        self.bits = bits;
        self.mix = mix.clamp(0.0, 1.0);
    }

    pub fn reset(&mut self) {
        for f in &mut self.aa {
            f.reset();
        }
        self.hold = [0.0; 2];
        self.phase = 0.0;
        self.updates = 0;
    }

    /// Crushes one stereo sample; returns the wet signal only.
    #[inline]
    pub fn wet(&mut self, l: f32, r: f32) -> (f32, f32) {
        let fl = self.aa[0].tick(l);
        let fr = self.aa[1].tick(r);
        self.phase += self.inc;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            self.updates += 1;
            self.hold = match self.bits {
                Some(b) => [quantize(fl, b), quantize(fr, b)],
                None => [fl, fr],
            };
        }
        (self.hold[0], self.hold[1])
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let m = self.mix;
        if m <= 0.0 {
            return;
        }
        for (a, b) in l.iter_mut().zip(r.iter_mut()) {
            let (wl, wr) = self.wet(*a, *b);
            *a += (wl - *a) * m;
            *b += (wr - *b) * m;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_bit_has_256_levels() {
        let mut levels = std::collections::BTreeSet::new();
        for i in -20_000..=20_000 {
            let x = i as f32 / 10_000.0;
            levels.insert(quantize(x, 8).to_bits());
        }
        assert_eq!(levels.len(), 256);
    }

    #[test]
    fn sample_and_hold_rate_is_exact() {
        let mut c = Crusher::default();
        let fs = 48_000.0;
        for rate in [4_000.0, 22_050.0, 26_000.0, 32_000.0] {
            c.set(rate, Some(12), 1.0, fs);
            c.reset();
            for _ in 0..48_000 {
                c.wet(0.3, 0.3);
            }
            let err = (c.updates as f32 - rate).abs() / rate;
            assert!(err < 0.001, "{rate}: {}", c.updates);
        }
    }
}
