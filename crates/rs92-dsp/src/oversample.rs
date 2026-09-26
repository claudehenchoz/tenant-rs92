//! Polyphase allpass halfband IIR resamplers (the HIIR design by Laurent de Soras).
//!
//! Each stage uses 8 allpass coefficients designed for at least 90 dB of stopband
//! rejection. The filters are minimum phase, so the plugin reports zero latency.

use std::sync::LazyLock;

pub const NUM_COEFS: usize = 8;

/// Target stopband rejection of one stage.
pub const STOPBAND_DB: f64 = 90.0;

/// Coefficients shared by every halfband stage.
pub static HALFBAND_COEFS: LazyLock<[f32; NUM_COEFS]> = LazyLock::new(|| {
    let tbw = transition_for_attenuation(NUM_COEFS, STOPBAND_DB);
    let c = design(NUM_COEFS, tbw);
    let mut out = [0.0f32; NUM_COEFS];
    for (o, v) in out.iter_mut().zip(c) {
        *o = v as f32;
    }
    out
});

fn transition_param(transition: f64) -> (f64, f64) {
    let mut k = ((1.0 - transition * 2.0) * std::f64::consts::PI / 4.0).tan();
    k *= k;
    let kksqrt = (1.0 - k * k).powf(0.25);
    let e = 0.5 * (1.0 - kksqrt) / (1.0 + kksqrt);
    let e2 = e * e;
    let e4 = e2 * e2;
    let q = e * (1.0 + e4 * (2.0 + e4 * (15.0 + 150.0 * e4)));
    (k, q)
}

/// Stopband attenuation in dB of a design with `nbr_coefs` coefficients and the given
/// normalised transition bandwidth.
pub fn attenuation(nbr_coefs: usize, transition: f64) -> f64 {
    let (_, q) = transition_param(transition);
    let order = (nbr_coefs * 2 + 1) as f64;
    let a = 4.0 * (order * 0.5 * q.ln()).exp();
    let attn_p2 = a / (1.0 + a);
    -10.0 * attn_p2.log10()
}

/// Smallest transition bandwidth (in steps of 0.0005) reaching `atten_db`.
pub fn transition_for_attenuation(nbr_coefs: usize, atten_db: f64) -> f64 {
    let mut t = 0.001;
    while t < 0.49 && attenuation(nbr_coefs, t) < atten_db {
        t += 0.0005;
    }
    t
}

fn acc_num(q: f64, order: usize, c: usize) -> f64 {
    let mut acc = 0.0;
    let mut j = 1.0;
    let mut i = 0i32;
    loop {
        let term = q.powi(i * (i + 1))
            * (((i * 2 + 1) as f64) * c as f64 * std::f64::consts::PI / order as f64).sin()
            * j;
        acc += term;
        j = -j;
        i += 1;
        if term.abs() <= 1e-100 || i > 1000 {
            break;
        }
    }
    acc
}

fn acc_den(q: f64, order: usize, c: usize) -> f64 {
    let mut acc = 0.0;
    let mut j = -1.0;
    let mut i = 1i32;
    loop {
        let term = q.powi(i * i)
            * ((i * 2) as f64 * c as f64 * std::f64::consts::PI / order as f64).cos()
            * j;
        acc += term;
        j = -j;
        i += 1;
        if term.abs() <= 1e-100 || i > 1000 {
            break;
        }
    }
    acc
}

/// Computes the allpass coefficients for a given transition bandwidth.
pub fn design(nbr_coefs: usize, transition: f64) -> Vec<f64> {
    let (k, q) = transition_param(transition);
    let order = nbr_coefs * 2 + 1;
    (0..nbr_coefs)
        .map(|index| {
            let c = index + 1;
            let num = acc_num(q, order, c) * q.powf(0.25);
            let den = acc_den(q, order, c) + 0.5;
            let ww = num / den;
            let wwsq = ww * ww;
            let x = ((1.0 - wwsq * k) * (1.0 - wwsq / k)).sqrt() / (1.0 + wwsq);
            (1.0 - x) / (1.0 + x)
        })
        .collect()
}

/// One chain of first-order allpass sections running at the low rate.
#[derive(Clone, Copy, Debug, Default)]
struct AllpassChain {
    x: [f32; NUM_COEFS / 2],
    y: [f32; NUM_COEFS / 2],
}

impl AllpassChain {
    #[inline]
    fn process(&mut self, coefs: &[f32; NUM_COEFS], offset: usize, mut s: f32) -> f32 {
        for i in 0..NUM_COEFS / 2 {
            let a = coefs[offset + 2 * i];
            let y = a * (s - self.y[i]) + self.x[i];
            self.x[i] = s;
            self.y[i] = y;
            s = y;
        }
        s
    }
}

/// 2:1 decimator.
#[derive(Clone, Copy, Debug, Default)]
pub struct Downsampler2x {
    even: AllpassChain,
    odd: AllpassChain,
}

impl Downsampler2x {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Consumes two input samples, returns one.
    #[inline]
    pub fn process(&mut self, x0: f32, x1: f32) -> f32 {
        let c = &*HALFBAND_COEFS;
        let a = self.even.process(c, 0, x1);
        let b = self.odd.process(c, 1, x0);
        0.5 * (a + b)
    }
}

/// 1:2 interpolator.
#[derive(Clone, Copy, Debug, Default)]
pub struct Upsampler2x {
    even: AllpassChain,
    odd: AllpassChain,
}

impl Upsampler2x {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Consumes one input sample, returns two.
    #[inline]
    pub fn process(&mut self, x: f32) -> (f32, f32) {
        let c = &*HALFBAND_COEFS;
        let a = self.even.process(c, 0, x);
        let b = self.odd.process(c, 1, x);
        (a, b)
    }
}

/// Stereo decimator from 2× or 4× down to the host rate.
#[derive(Clone, Debug, Default)]
pub struct StereoDecimator {
    stage1: [Downsampler2x; 2],
    stage2: [Downsampler2x; 2],
}

impl StereoDecimator {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Decimates `input` (length `out.len() * factor`) into `out`, for one channel.
    /// `factor` is 2 or 4. Processes in place-friendly order.
    pub fn process_channel(&mut self, ch: usize, factor: usize, input: &[f32], out: &mut [f32]) {
        match factor {
            4 => {
                for (o, x) in out.iter_mut().zip(input.chunks_exact(4)) {
                    let a = self.stage2[ch].process(x[0], x[1]);
                    let b = self.stage2[ch].process(x[2], x[3]);
                    *o = self.stage1[ch].process(a, b);
                }
            }
            _ => {
                for (o, x) in out.iter_mut().zip(input.chunks_exact(2)) {
                    *o = self.stage1[ch].process(x[0], x[1]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_reaches_90_db() {
        let t = transition_for_attenuation(NUM_COEFS, STOPBAND_DB);
        assert!(attenuation(NUM_COEFS, t) >= 90.0);
        for c in HALFBAND_COEFS.iter() {
            assert!(*c > 0.0 && *c < 1.0);
        }
    }

    fn tone_level_after_decimation(freq_norm: f32) -> f32 {
        // freq_norm is relative to the high (input) rate.
        let mut d = Downsampler2x::default();
        let n = 16384;
        let mut peak = 0.0f32;
        for i in 0..n {
            let w = std::f64::consts::TAU * freq_norm as f64;
            let x0 = (w * (2 * i) as f64).sin() as f32;
            let x1 = (w * (2 * i + 1) as f64).sin() as f32;
            let y = d.process(x0, x1);
            if i > n / 2 {
                peak = peak.max(y.abs());
            }
        }
        peak
    }

    #[test]
    fn passband_is_flat_and_stopband_rejects() {
        let t = transition_for_attenuation(NUM_COEFS, STOPBAND_DB) as f32;
        // Passband: up to (0.25 - t) of the high rate.
        for f in [0.01, 0.1, 0.2, 0.25 - t - 0.005] {
            let g = tone_level_after_decimation(f);
            assert!((g - 1.0).abs() < 0.01, "passband {f}: {g}");
        }
        // Stopband: above (0.25 + t).
        for f in [0.25 + t + 0.005, 0.35, 0.45] {
            let g = tone_level_after_decimation(f);
            assert!(
                g < 10f32.powf(-85.0 / 20.0),
                "stopband {f}: {} dB",
                20.0 * g.log10()
            );
        }
    }
}
