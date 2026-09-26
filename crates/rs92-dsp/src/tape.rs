//! Tape stage: pre-emphasis, drive, asymmetric soft clip, de-emphasis, head bump,
//! 16 kHz roll-off and DC blocker. The non-linear part runs at 2× oversampling.

use crate::crush::OnePoleLp;
use crate::math::{db_to_gain, tanh_approx};
use crate::oversample::{Downsampler2x, Upsampler2x};

/// RBJ biquad, transposed direct form II.
#[derive(Clone, Copy, Debug, Default)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    fn from_coefs(b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) -> Self {
        Biquad {
            b0: (b0 / a0) as f32,
            b1: (b1 / a0) as f32,
            b2: (b2 / a0) as f32,
            a1: (a1 / a0) as f32,
            a2: (a2 / a0) as f32,
            z1: 0.0,
            z2: 0.0,
        }
    }

    pub fn high_shelf(f: f32, gain_db: f32, fs: f32) -> Self {
        let a = 10f64.powf(gain_db as f64 / 40.0);
        let w = std::f64::consts::TAU * (f as f64 / fs as f64).min(0.49);
        let (sn, cs) = w.sin_cos();
        let alpha = sn / 2.0 * std::f64::consts::SQRT_2; // S = 1
        let sa = 2.0 * a.sqrt() * alpha;
        Biquad::from_coefs(
            a * ((a + 1.0) + (a - 1.0) * cs + sa),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * cs),
            a * ((a + 1.0) + (a - 1.0) * cs - sa),
            (a + 1.0) - (a - 1.0) * cs + sa,
            2.0 * ((a - 1.0) - (a + 1.0) * cs),
            (a + 1.0) - (a - 1.0) * cs - sa,
        )
    }

    pub fn peak(f: f32, gain_db: f32, q: f32, fs: f32) -> Self {
        let a = 10f64.powf(gain_db as f64 / 40.0);
        let w = std::f64::consts::TAU * (f as f64 / fs as f64).min(0.49);
        let (sn, cs) = w.sin_cos();
        let alpha = sn / (2.0 * q as f64);
        Biquad::from_coefs(
            1.0 + alpha * a,
            -2.0 * cs,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * cs,
            1.0 - alpha / a,
        )
    }

    #[inline]
    pub fn tick(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }

    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    /// Copies the coefficients of `other`, keeping this filter's state.
    pub fn set_coefs(&mut self, other: &Biquad) {
        let (z1, z2) = (self.z1, self.z2);
        *self = *other;
        self.z1 = z1;
        self.z2 = z2;
    }
}

const BIAS: f32 = 0.1;

#[derive(Clone, Copy, Debug, Default)]
struct TapeChannel {
    up: Upsampler2x,
    down: Downsampler2x,
    pre: Biquad,
    de: Biquad,
    bump: Biquad,
    lp: OnePoleLp,
    dc_x: f32,
    dc_y: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Tape {
    ch: [TapeChannel; 2],
    drive: f32,
    inv_drive: f32,
    bias_offset: f32,
    dc_r: f32,
    key: [f32; 3],
}

impl Tape {
    /// `level_db` is the TAPE level (-24..-6 dBFS); `extra_drive_db` is added by HYPER.
    /// Coefficients are recomputed only when an input changed; filter state is kept.
    pub fn set(&mut self, level_db: f32, extra_drive_db: f32, fs: f32) {
        let key = [level_db, extra_drive_db, fs];
        if key == self.key {
            return;
        }
        let fs_changed = fs != self.key[2];
        self.key = key;
        if fs_changed {
            let fs2 = fs * 2.0;
            for c in &mut self.ch {
                c.pre.set_coefs(&Biquad::high_shelf(3000.0, 3.0, fs2));
                c.de.set_coefs(&Biquad::high_shelf(3000.0, -3.0, fs2));
                c.bump.set_coefs(&Biquad::peak(80.0, 1.5, 0.9, fs));
                c.lp.set(16_000.0, fs);
            }
            self.dc_r = 1.0 - std::f32::consts::TAU * 10.0 / fs;
        }
        self.drive = db_to_gain(-level_db + extra_drive_db);
        // No make-up: a signal at the TAPE level hits the clipper at 0 dBFS, like
        // lining up a tape machine so 0 VU sits at `level` dBFS.
        self.inv_drive = 1.0;
        self.bias_offset = tanh_approx(BIAS);
    }

    pub fn reset(&mut self) {
        for c in &mut self.ch {
            c.up.reset();
            c.down.reset();
            c.pre.reset();
            c.de.reset();
            c.bump.reset();
            c.lp.reset();
            c.dc_x = 0.0;
            c.dc_y = 0.0;
        }
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let (drive, inv, bo, dc_r) = (self.drive, self.inv_drive, self.bias_offset, self.dc_r);
        for (c, buf) in self.ch.iter_mut().zip([l, r]) {
            for s in buf.iter_mut() {
                let (u0, u1) = c.up.process(*s);
                let mut clip = |x: f32| {
                    let e = c.pre.tick(x);
                    let y = (tanh_approx(e * drive + BIAS) - bo) * inv;
                    c.de.tick(y)
                };
                let y0 = clip(u0);
                let y1 = clip(u1);
                let y = c.down.process(y0, y1);
                let y = c.lp.tick(c.bump.tick(y));
                // DC blocker.
                let out = y - c.dc_x + dc_r * c.dc_y;
                c.dc_x = y;
                c.dc_y = out;
                *s = out;
            }
        }
    }
}
