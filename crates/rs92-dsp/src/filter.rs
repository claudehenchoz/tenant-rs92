//! Zero-delay-feedback TPT filters (Zavalishin state-variable filter and one-pole),
//! processed 8 partials at a time.

use crate::math::{tan_approx_x8, PI};
use crate::patch::FilterType;
use wide::f32x8;

/// Q of a 12 dB/oct slot for a RESO value of 0..100 %: 0.5 … 12.
#[inline]
pub fn q_12(reso: f32) -> f32 {
    0.5 * 24f32.powf((reso / 100.0).clamp(0.0, 1.0))
}

/// Q of the resonant (second) stage of a 24 dB/oct slot: 1.31 … 12.
#[inline]
pub fn q_24(reso: f32) -> f32 {
    1.31 * (12.0 / 1.31f32).powf((reso / 100.0).clamp(0.0, 1.0))
}

pub const Q_24_FIRST: f32 = 0.54;

/// Prewarped integrator gain `g = tan(π fc / fs)` with the cutoff clamped to
/// 20 Hz … 0.45 fs.
#[inline]
pub fn prewarp_x8(fc: f32x8, fs: f32) -> f32x8 {
    let fc = fc.max(f32x8::splat(20.0)).min(f32x8::splat(0.45 * fs));
    tan_approx_x8(fc * f32x8::splat(PI / fs))
}

#[derive(Clone, Copy, Debug)]
struct SvfCoefs {
    k: f32x8,
    a1: f32x8,
    a2: f32x8,
    a3: f32x8,
}

impl SvfCoefs {
    #[inline]
    fn new(g: f32x8, q: f32) -> Self {
        let k = f32x8::splat(1.0 / q);
        let a1 = (f32x8::ONE + g * (g + k)).recip();
        let a2 = g * a1;
        let a3 = g * a2;
        SvfCoefs { k, a1, a2, a3 }
    }
}

impl Default for SvfCoefs {
    fn default() -> Self {
        SvfCoefs::new(f32x8::splat(0.1), 0.707)
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Svf {
    ic1: f32x8,
    ic2: f32x8,
}

struct SvfOut {
    lp: f32x8,
    bp: f32x8,
    hp: f32x8,
}

impl Svf {
    #[inline(always)]
    fn tick(&mut self, c: &SvfCoefs, v0: f32x8) -> SvfOut {
        let v3 = v0 - self.ic2;
        let v1 = c.a1 * self.ic1 + c.a2 * v3;
        let v2 = self.ic2 + c.a2 * self.ic1 + c.a3 * v3;
        self.ic1 = v1 + v1 - self.ic1;
        self.ic2 = v2 + v2 - self.ic2;
        SvfOut {
            lp: v2,
            bp: v1,
            hp: v0 - c.k * v1 - v2,
        }
    }
}

/// Control-rate coefficients of one filter slot.
#[derive(Clone, Copy, Debug)]
pub struct FilterCoefs {
    pub ty: FilterType,
    s1: SvfCoefs,
    s2: SvfCoefs,
    /// One-pole gain G = g / (1 + g).
    op: f32x8,
}

impl Default for FilterCoefs {
    fn default() -> Self {
        FilterCoefs {
            ty: FilterType::Off,
            s1: SvfCoefs::default(),
            s2: SvfCoefs::default(),
            op: f32x8::splat(0.5),
        }
    }
}

impl FilterCoefs {
    /// `g` is the prewarped gain per lane, `reso` the panel value in percent.
    pub fn new(ty: FilterType, g: f32x8, reso: f32) -> Self {
        let mut c = FilterCoefs {
            ty,
            ..Default::default()
        };
        match ty {
            FilterType::Lp12 | FilterType::Hp12 | FilterType::Bp12 => {
                c.s1 = SvfCoefs::new(g, q_12(reso));
            }
            FilterType::Lp18 => {
                c.s1 = SvfCoefs::new(g, q_12(reso));
                c.op = g / (f32x8::ONE + g);
            }
            FilterType::Lp24 | FilterType::Hp24 => {
                c.s1 = SvfCoefs::new(g, Q_24_FIRST);
                c.s2 = SvfCoefs::new(g, q_24(reso));
            }
            FilterType::Off => {}
        }
        c
    }
}

/// Per-partial state of one filter slot.
#[derive(Clone, Copy, Debug, Default)]
pub struct FilterState {
    s1: Svf,
    s2: Svf,
    op: f32x8,
}

impl FilterState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Filters a block in place, with the type dispatch outside the sample loop.
    pub fn process_block(&mut self, c: &FilterCoefs, buf: &mut [f32x8]) {
        let (mut s1, mut s2, mut op) = (self.s1, self.s2, self.op);
        match c.ty {
            FilterType::Off => {}
            FilterType::Lp12 => buf.iter_mut().for_each(|x| *x = s1.tick(&c.s1, *x).lp),
            FilterType::Hp12 => buf.iter_mut().for_each(|x| *x = s1.tick(&c.s1, *x).hp),
            FilterType::Bp12 => buf
                .iter_mut()
                .for_each(|x| *x = s1.tick(&c.s1, *x).bp * c.s1.k),
            FilterType::Lp18 => buf.iter_mut().for_each(|x| {
                let y = s1.tick(&c.s1, *x).lp;
                let v = (y - op) * c.op;
                let out = v + op;
                op = out + v;
                *x = out;
            }),
            FilterType::Lp24 => buf.iter_mut().for_each(|x| {
                let y = s1.tick(&c.s1, *x).lp;
                *x = s2.tick(&c.s2, y).lp;
            }),
            FilterType::Hp24 => buf.iter_mut().for_each(|x| {
                let y = s1.tick(&c.s1, *x).hp;
                *x = s2.tick(&c.s2, y).hp;
            }),
        }
        self.s1 = s1;
        self.s2 = s2;
        self.op = op;
    }

    #[inline]
    pub fn tick(&mut self, c: &FilterCoefs, x: f32x8) -> f32x8 {
        match c.ty {
            FilterType::Off => x,
            FilterType::Lp12 => self.s1.tick(&c.s1, x).lp,
            FilterType::Hp12 => self.s1.tick(&c.s1, x).hp,
            // Normalised band-pass: unity gain at the centre.
            FilterType::Bp12 => self.s1.tick(&c.s1, x).bp * c.s1.k,
            FilterType::Lp18 => {
                let y = self.s1.tick(&c.s1, x).lp;
                let v = (y - self.op) * c.op;
                let out = v + self.op;
                self.op = out + v;
                out
            }
            FilterType::Lp24 => {
                let y = self.s1.tick(&c.s1, x).lp;
                self.s2.tick(&c.s2, y).lp
            }
            FilterType::Hp24 => {
                let y = self.s1.tick(&c.s1, x).hp;
                self.s2.tick(&c.s2, y).hp
            }
        }
    }
}

/// Analog-prototype magnitude of a filter slot, evaluated at the bilinear-warped
/// frequency. This is the exact response of the TPT filter and is used by the
/// response-curve display and the tests.
pub fn magnitude(ty: FilterType, cutoff: f32, reso: f32, freq: f32, fs: f32) -> f32 {
    let fc = cutoff.clamp(20.0, 0.45 * fs);
    let wc = (PI * fc / fs).tan();
    let w = (PI * freq.min(0.4999 * fs) / fs).tan() / wc;
    let s = num_complex_jw(w);
    let lp2 = |q: f32| 1.0 / c_abs(c_add(c_add(c_mul(s, s), c_scale(s, 1.0 / q)), (1.0, 0.0)));
    let hp2 = |q: f32| w * w * lp2(q);
    match ty {
        FilterType::Off => 1.0,
        FilterType::Lp12 => lp2(q_12(reso)),
        FilterType::Hp12 => hp2(q_12(reso)),
        FilterType::Bp12 => {
            let q = q_12(reso);
            (w / q) * lp2(q)
        }
        FilterType::Lp18 => lp2(q_12(reso)) / (1.0 + w * w).sqrt(),
        FilterType::Lp24 => lp2(Q_24_FIRST) * lp2(q_24(reso)),
        FilterType::Hp24 => hp2(Q_24_FIRST) * hp2(q_24(reso)),
    }
}

type C = (f32, f32);
fn num_complex_jw(w: f32) -> C {
    (0.0, w)
}
fn c_add(a: C, b: C) -> C {
    (a.0 + b.0, a.1 + b.1)
}
fn c_mul(a: C, b: C) -> C {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}
fn c_scale(a: C, s: f32) -> C {
    (a.0 * s, a.1 * s)
}
fn c_abs(a: C) -> f32 {
    (a.0 * a.0 + a.1 * a.1).sqrt()
}
