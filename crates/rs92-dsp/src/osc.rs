//! Operator waveforms and the OP1 → PM → OP2 pair.

use crate::math::{fract_x8, sin_cycles_x8, wrap_once_x8, wrap_x8};
use crate::patch::Wave;
use crate::pd::{res_ratio, PdKnee};
use wide::{f32x8, CmpGt, CmpLt};

/// Control-rate description of one operator's waveform.
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    pub wave: Wave,
    knee: PdKnee,
    /// Resonance ratio for the Res waves.
    res_r: f32,
    /// Apply PolyBLEP (only when the phase is neither warped nor modulated).
    blep: bool,
}

impl Shape {
    /// `pd` is the panel value (-100..100); `modulated` disables PolyBLEP.
    pub fn new(wave: Wave, pd: f32, modulated: bool) -> Self {
        Shape {
            wave,
            knee: PdKnee::new(pd),
            res_r: res_ratio(pd),
            blep: pd == 0.0 && !modulated,
        }
    }

    /// Evaluates the waveform at raw phase `p` in [0, 1); `dt` is the phase increment.
    #[inline]
    pub fn eval(&self, p: f32x8, dt: f32x8) -> f32x8 {
        let dt = Blep::new(dt);
        match self.wave {
            Wave::Sine => self.eval_c::<0>(p, dt),
            Wave::Tri => self.eval_c::<1>(p, dt),
            Wave::Saw => self.eval_c::<2>(p, dt),
            Wave::Square => self.eval_c::<3>(p, dt),
            Wave::Pulse => self.eval_c::<4>(p, dt),
            Wave::ResI => self.eval_c::<5>(p, dt),
            Wave::ResII => self.eval_c::<6>(p, dt),
            Wave::ResIII => self.eval_c::<7>(p, dt),
            Wave::Organ => self.eval_c::<8>(p, dt),
            Wave::Bell => self.eval_c::<9>(p, dt),
        }
    }

    /// Waveform evaluation with the wave index `W` known at compile time, so block loops
    /// can be specialised per waveform.
    #[inline(always)]
    pub fn eval_c<const W: u8>(&self, p: f32x8, dt: Blep) -> f32x8 {
        match W {
            5 => return res_wave::<1>(p, self.res_r),
            6 => return res_wave::<2>(p, self.res_r),
            7 => return res_wave::<3>(p, self.res_r),
            _ => {}
        }
        let w = if self.blep { p } else { self.knee.warp_x8(p) };
        match W {
            0 => sin_cycles_x8(w),
            1 => {
                let t = fract_x8(w + f32x8::splat(0.25)) - f32x8::splat(0.5);
                f32x8::ONE - f32x8::splat(4.0) * t.abs()
            }
            2 => {
                let s = w.mul_add(f32x8::splat(2.0), -f32x8::ONE);
                if self.blep {
                    s - poly_blep(w, dt)
                } else {
                    s
                }
            }
            3 => pulse(w, 0.5, dt, self.blep),
            4 => pulse(w, 0.25, dt, self.blep),
            8 => {
                sin_cycles_x8(w) * f32x8::splat(0.6)
                    + sin_cycles_x8(w * f32x8::splat(2.0)) * f32x8::splat(0.5)
                    + sin_cycles_x8(w * f32x8::splat(3.0)) * f32x8::splat(0.2)
                    + sin_cycles_x8(w * f32x8::splat(4.0)) * f32x8::splat(0.3)
            }
            _ => {
                sin_cycles_x8(w) * f32x8::splat(0.7)
                    + sin_cycles_x8(w * f32x8::splat(2.76)) * f32x8::splat(0.35)
                    + sin_cycles_x8(w * f32x8::splat(5.4)) * f32x8::splat(0.15)
            }
        }
    }
}

/// Windowed resonant sine: a sine at `r` cycles per period times a window.
#[inline(always)]
fn res_wave<const K: u8>(p: f32x8, r: f32) -> f32x8 {
    let s = sin_cycles_x8(p * f32x8::splat(r));
    let window = match K {
        // Falling saw.
        1 => f32x8::ONE - p,
        // Triangle.
        2 => f32x8::ONE - (p.mul_add(f32x8::splat(2.0), -f32x8::ONE)).abs(),
        // Trapezoid: flat for the first half, then falling.
        _ => (f32x8::splat(2.0) * (f32x8::ONE - p)).min(f32x8::ONE),
    };
    s * window
}

#[inline(always)]
fn pulse(w: f32x8, width: f32, dt: Blep, blep: bool) -> f32x8 {
    let v = w.cmp_lt(f32x8::splat(width)).blend(f32x8::ONE, -f32x8::ONE);
    if blep {
        v + poly_blep(w, dt) - poly_blep(fract_x8(w + f32x8::splat(1.0 - width)), dt)
    } else {
        v
    }
}

/// Phase increment and its reciprocal, precomputed once per block for PolyBLEP.
#[derive(Clone, Copy, Debug)]
pub struct Blep {
    dt: f32x8,
    inv: f32x8,
}

impl Blep {
    #[inline]
    pub fn new(dt: f32x8) -> Self {
        let dt = dt.max(f32x8::splat(1e-6));
        Blep {
            dt,
            inv: dt.recip(),
        }
    }
}

/// Two-sample polynomial band-limited step residual.
#[inline(always)]
fn poly_blep(t: f32x8, b: Blep) -> f32x8 {
    let x0 = t * b.inv;
    let start = x0 + x0 - x0 * x0 - f32x8::ONE;
    let x1 = (t - f32x8::ONE) * b.inv;
    let end = x1 * x1 + x1 + x1 + f32x8::ONE;
    let r = t.cmp_lt(b.dt).blend(start, f32x8::ZERO);
    t.cmp_gt(f32x8::ONE - b.dt).blend(end, r)
}

/// Control-rate settings of the operator pair.
#[derive(Clone, Copy, Debug)]
pub struct OpParams {
    pub shape1: Shape,
    pub shape2: Shape,
    /// OP1 self-feedback amount β (cycles).
    pub beta: f32,
    /// PM index I (cycles).
    pub index: f32,
    /// OP1 direct level, 0..1.
    pub level1: f32,
    /// OP2 frequency ratio 2^(tune/12).
    pub ratio2: f32,
}

impl OpParams {
    pub fn from_patch(p: &crate::patch::Patch) -> Self {
        let beta = 0.25 * (p.op1_fdbk / 100.0).powi(2);
        let index = 1.5 * (p.op2_pm / 100.0).powi(2);
        OpParams {
            shape1: Shape::new(p.op1_wave, p.op1_pd, beta > 0.0),
            shape2: Shape::new(p.op2_wave, p.op2_pd, index > 0.0),
            beta,
            index,
            level1: p.op1_level / 100.0,
            ratio2: 2f32.powf(p.op2_tune as f32 / 12.0),
        }
    }
}

/// Phase state of the operator pair for 8 partials.
#[derive(Clone, Copy, Debug, Default)]
pub struct OpState {
    pub phase1: f32x8,
    pub phase2: f32x8,
    fb1: f32x8,
    fb2: f32x8,
}

macro_rules! dispatch_wave {
    ($wave:expr, $f:ident, $($arg:expr),*) => {
        match $wave {
            Wave::Sine => $f::<0>($($arg),*),
            Wave::Tri => $f::<1>($($arg),*),
            Wave::Saw => $f::<2>($($arg),*),
            Wave::Square => $f::<3>($($arg),*),
            Wave::Pulse => $f::<4>($($arg),*),
            Wave::ResI => $f::<5>($($arg),*),
            Wave::ResII => $f::<6>($($arg),*),
            Wave::ResIII => $f::<7>($($arg),*),
            Wave::Organ => $f::<8>($($arg),*),
            Wave::Bell => $f::<9>($($arg),*),
        }
    };
}

/// OP1 with self-feedback over a block. `run[i]` is 1.0 for lanes whose phase advances.
fn op1_block<const W: u8>(
    st: &mut OpState,
    prm: &OpParams,
    dt1: f32x8,
    run: &[f32x8],
    out: &mut [f32x8],
) {
    let beta = f32x8::splat(prm.beta * 0.5);
    let blep = Blep::new(dt1);
    let (mut ph, mut fb1, mut fb2) = (st.phase1, st.fb1, st.fb2);
    if prm.beta == 0.0 {
        for (o, &r) in out.iter_mut().zip(run) {
            *o = prm.shape1.eval_c::<W>(ph, blep);
            ph = wrap_once_x8(dt1.mul_add(r, ph));
        }
    } else {
        for (o, &r) in out.iter_mut().zip(run) {
            // Feedback averaged over the last two samples.
            let p = wrap_x8((fb1 + fb2).mul_add(beta, ph));
            let v = prm.shape1.eval_c::<W>(p, blep);
            fb2 = fb1;
            fb1 = v;
            *o = v;
            ph = wrap_once_x8(dt1.mul_add(r, ph));
        }
    }
    st.phase1 = ph;
    st.fb1 = fb1;
    st.fb2 = fb2;
}

/// OP2 phase-modulated by OP1, mixed to `x = 0.5 (level1 * o1 + o2)` in place.
fn op2_block<const W: u8>(
    st: &mut OpState,
    prm: &OpParams,
    dt2: f32x8,
    run: &[f32x8],
    io: &mut [f32x8],
) {
    let index = f32x8::splat(prm.index);
    let level1 = f32x8::splat(prm.level1 * 0.5);
    let half = f32x8::splat(0.5);
    let blep = Blep::new(dt2);
    let mut ph = st.phase2;
    for (x, &r) in io.iter_mut().zip(run) {
        let o1 = *x;
        let p = wrap_x8(o1.mul_add(index, ph));
        let o2 = prm.shape2.eval_c::<W>(p, blep);
        *x = o1.mul_add(level1, o2 * half);
        ph = wrap_once_x8(dt2.mul_add(r, ph));
    }
    st.phase2 = ph;
}

impl OpState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Renders `out.len()` samples of the operator pair.
    pub fn render_block(&mut self, prm: &OpParams, dt1: f32x8, run: &[f32x8], out: &mut [f32x8]) {
        dispatch_wave!(prm.shape1.wave, op1_block, self, prm, dt1, run, out);
        let dt2 = dt1 * f32x8::splat(prm.ratio2);
        dispatch_wave!(prm.shape2.wave, op2_block, self, prm, dt2, run, out);
    }

    /// Renders one sample of `x = 0.5 (level1 * o1 + o2)` for all lanes.
    #[inline]
    pub fn tick(&mut self, prm: &OpParams, dt1: f32x8, run: f32x8) -> f32x8 {
        let mut out = [f32x8::ZERO];
        self.render_block(prm, dt1, &[run], &mut out);
        out[0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::lane;

    fn plain(wave: Wave, p: f32) -> f32 {
        match wave {
            Wave::Sine => (p * std::f32::consts::TAU).sin(),
            Wave::Saw => 2.0 * p - 1.0,
            Wave::Square => {
                if p < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            Wave::Tri => {
                let t = (p + 0.25).fract() - 0.5;
                1.0 - 4.0 * t.abs()
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn pd_zero_equals_plain_waveform() {
        for wave in [Wave::Sine, Wave::Saw, Wave::Square, Wave::Tri] {
            // `modulated = true` disables PolyBLEP so we compare the naive shape.
            let s = Shape::new(wave, 0.0, true);
            for i in 0..997 {
                let p = i as f32 / 997.0;
                let v = lane(s.eval(f32x8::splat(p), f32x8::splat(0.001)), 0);
                assert!((v - plain(wave, p)).abs() < 1e-4, "{wave:?} at {p}: {v}");
            }
        }
    }

    #[test]
    fn waves_are_bounded() {
        for &wave in Wave::ALL {
            for pd in [-100.0, 0.0, 26.0, 100.0] {
                let s = Shape::new(wave, pd, false);
                for i in 0..1000 {
                    let p = i as f32 / 1000.0;
                    let v = lane(s.eval(f32x8::splat(p), f32x8::splat(0.01)), 0);
                    assert!(v.is_finite() && v.abs() <= 1.61, "{wave:?} {pd} {p} {v}");
                }
            }
        }
    }
}
