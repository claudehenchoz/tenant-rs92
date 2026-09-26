//! Filter magnitude at cutoff and one octave above matches the analytic response.

use rs92_dsp::filter::{magnitude, prewarp_x8, FilterCoefs, FilterState};
use rs92_dsp::FilterType;
use wide::f32x8;

fn measured(ty: FilterType, cutoff: f32, reso: f32, freq: f32, fs: f32) -> f32 {
    let c = FilterCoefs::new(ty, prewarp_x8(f32x8::splat(cutoff), fs), reso);
    let mut s = FilterState::default();
    let n = (fs as usize) / 2;
    let settle = n / 2;
    let w = std::f64::consts::TAU * freq as f64 / fs as f64;
    let (mut e_in, mut e_out) = (0.0f64, 0.0f64);
    for i in 0..n {
        let x = (w * i as f64).sin() as f32;
        let y = s.tick(&c, f32x8::splat(x)).as_array_ref()[0];
        if i >= settle {
            e_in += (x as f64).powi(2);
            e_out += (y as f64).powi(2);
        }
    }
    (e_out / e_in).sqrt() as f32
}

#[test]
fn magnitude_matches_analytic_response() {
    let types = [
        FilterType::Lp12,
        FilterType::Lp18,
        FilterType::Lp24,
        FilterType::Hp12,
        FilterType::Hp24,
        FilterType::Bp12,
    ];
    for fs in [44_100.0, 48_000.0, 96_000.0] {
        for ty in types {
            for cutoff in [200.0, 1000.0, 5000.0] {
                for reso in [0.0, 50.0] {
                    for freq in [cutoff, cutoff * 2.0] {
                        let m = measured(ty, cutoff, reso, freq, fs);
                        let a = magnitude(ty, cutoff, reso, freq, fs);
                        let err_db = 20.0 * (m / a).log10();
                        assert!(
                            err_db.abs() <= 1.0,
                            "{ty:?} fc {cutoff} reso {reso} f {freq} fs {fs}: measured {m}, analytic {a} ({err_db} dB)"
                        );
                    }
                }
            }
        }
    }
}
