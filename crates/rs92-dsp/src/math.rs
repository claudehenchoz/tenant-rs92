//! Cheap math used in the audio loop.

use std::sync::LazyLock;
use wide::{f32x8, CmpGe, CmpLt};

pub const TAU: f32 = std::f32::consts::TAU;
pub const PI: f32 = std::f32::consts::PI;

/// Size of the shared sine table.
pub const SINE_TABLE_SIZE: usize = 4096;

/// Shared 4096-point sine table (plus one guard point) for scalar lookups.
pub static SINE_TABLE: LazyLock<Box<[f32; SINE_TABLE_SIZE + 1]>> = LazyLock::new(|| {
    let mut t = Box::new([0.0f32; SINE_TABLE_SIZE + 1]);
    for (i, v) in t.iter_mut().enumerate() {
        *v = (i as f64 / SINE_TABLE_SIZE as f64 * std::f64::consts::TAU).sin() as f32;
    }
    t
});

/// Scalar sine of a phase in cycles, via the shared table with linear interpolation.
#[inline]
pub fn sin_cycles(phase: f32) -> f32 {
    let p = phase - phase.floor();
    let x = p * SINE_TABLE_SIZE as f32;
    let i = (x as usize).min(SINE_TABLE_SIZE - 1);
    let f = x - i as f32;
    let t = &**SINE_TABLE;
    t[i] + (t[i + 1] - t[i]) * f
}

/// 8-lane sine of a phase in cycles.
///
/// The SIMD path uses a 9th-order odd polynomial rather than table gathers, which `wide`
/// cannot vectorise. Its error is below 1e-6, well under the table's interpolation error.
#[inline(always)]
pub fn sin_cycles_x8(phase: f32x8) -> f32x8 {
    // r in [-0.5, 0.5]; sin(2πr) = sign(r) · sin(2π b) with b = 0.25 - |(|r| - 0.25)|
    // in [0, 0.25]. `fast_round_int` is a single SSE2 instruction.
    let r = phase - f32x8::from_i32x8(phase.fast_round_int());
    let quarter = f32x8::splat(0.25);
    let b = quarter - (r.abs() - quarter).abs();
    let t = b * f32x8::splat(TAU);
    let t2 = t * t;
    // Minimax polynomial for sin on [0, π/2].
    let p = f32x8::splat(2.600_054_4e-6);
    let p = p.mul_add(t2, f32x8::splat(-1.980_960_8e-4));
    let p = p.mul_add(t2, f32x8::splat(8.333_066e-3));
    let p = p.mul_add(t2, f32x8::splat(-0.166_666_4));
    let p = p.mul_add(t2, f32x8::ONE);
    (p * t).flip_signs(r)
}

/// Wraps a phase into [0, 1).
#[inline]
pub fn fract_x8(x: f32x8) -> f32x8 {
    // Truncation is a single SSE2 instruction; `floor()` is not without SSE4.1.
    let f = x - f32x8::from_i32x8(x.fast_trunc_int());
    let f = f.cmp_lt(f32x8::ZERO).blend(f + f32x8::ONE, f);
    // Guard against rounding to exactly 1.0 for tiny negative inputs.
    f.cmp_ge(f32x8::ONE).blend(f32x8::ZERO, f)
}

/// Fast wrap into [0, 1] for any phase: `x - floor(x)` with floor computed as
/// round-to-nearest of `x - 0.5`. Exact integers may map to 1.0 instead of 0.0, which
/// every waveform treats as the same point of the cycle.
#[inline(always)]
pub fn wrap_x8(x: f32x8) -> f32x8 {
    x - f32x8::from_i32x8((x - f32x8::splat(0.5)).fast_round_int())
}

/// Wraps an accumulator that is known to be in [0, 2) back into [0, 1).
#[inline(always)]
pub fn wrap_once_x8(x: f32x8) -> f32x8 {
    x - (x.cmp_ge(f32x8::ONE) & f32x8::ONE)
}

/// Rational (5,4) Padé approximation of `tan(x)`, accurate to ~0.1 % for |x| < 1.45.
#[inline]
pub fn tan_approx(x: f32) -> f32 {
    let x2 = x * x;
    x * (945.0 + x2 * (-105.0 + x2)) / (945.0 + x2 * (-420.0 + x2 * 15.0))
}

#[inline]
pub fn tan_approx_x8(x: f32x8) -> f32x8 {
    let x2 = x * x;
    let num = x2.mul_add(x2 - f32x8::splat(105.0), f32x8::splat(945.0));
    let den = x2.mul_add(
        x2.mul_add(f32x8::splat(15.0), f32x8::splat(-420.0)),
        f32x8::splat(945.0),
    );
    x * num / den
}

/// Rational tanh approximation, exact at 0, saturating to ±1 at |x| >= 3.
#[inline]
pub fn tanh_approx(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

/// Fast `2^x` for control-rate use (per sub-block, per lane).
#[inline]
pub fn exp2_x8(x: f32x8) -> f32x8 {
    (x * f32x8::splat(std::f32::consts::LN_2)).exp()
}

#[inline]
pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db * 0.05)
}

#[inline]
pub fn gain_to_db(g: f32) -> f32 {
    20.0 * g.max(1e-12).log10()
}

#[inline]
pub fn midi_to_hz(note: f32) -> f32 {
    440.0 * 2f32.powf((note - 69.0) / 12.0)
}

/// Per-sample coefficient of an exponential segment that falls 60 dB in `ms` milliseconds.
#[inline]
pub fn decay_coef(ms: f32, fs: f32) -> f32 {
    let samples = (ms * 0.001 * fs).max(1.0);
    (0.001f32.ln() / samples).exp()
}

/// One-pole smoothing coefficient for a time constant in ms.
#[inline]
pub fn smooth_coef(ms: f32, fs: f32) -> f32 {
    let samples = (ms * 0.001 * fs).max(1.0);
    (-1.0 / samples).exp()
}

/// Extracts one lane of an `f32x8`.
#[inline]
pub fn lane(v: f32x8, i: usize) -> f32 {
    v.as_array_ref()[i]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simd_sine_matches_std() {
        for i in -2000..2000 {
            let p = i as f32 * 0.00137;
            let v = sin_cycles_x8(f32x8::splat(p));
            let r = (p as f64 * std::f64::consts::TAU).sin() as f32;
            assert!((lane(v, 0) - r).abs() < 2e-6, "{p}: {} vs {r}", lane(v, 0));
            assert!((sin_cycles(p) - r).abs() < 5e-6);
        }
    }

    #[test]
    fn tan_approximation_is_accurate() {
        for i in 1..1000 {
            let x = i as f32 * 1.41 / 1000.0;
            let rel = (tan_approx(x) - x.tan()).abs() / x.tan();
            assert!(rel < 2e-3, "{x}: {rel}");
            assert!(
                (lane(tan_approx_x8(f32x8::splat(x)), 3) - tan_approx(x)).abs() < 1e-4 * x.tan()
            );
        }
    }
}
