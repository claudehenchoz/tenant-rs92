//! Offline rendering and the randomizer's quality gate (design doc section 9, step 3).

use realfft::RealFftPlanner;
use rs92_dsp::bake::render_bake;
use rs92_dsp::{Engine, Patch};
use std::sync::Arc;

pub const SR: f32 = 48_000.0;
pub const GATE_MS: f32 = 120.0;
pub const TOTAL_MS: f32 = 800.0;
pub const VELOCITY: f32 = 100.0 / 127.0;
pub const NOTE: u8 = 64;

/// Renders one chord hit on E4 at velocity 100: 120 ms gate, 800 ms total, 48 kHz.
/// SAMPLER-mode patches are baked first and played back, as in the plugin.
pub fn render_hit(patch: &Patch) -> (Vec<f32>, Vec<f32>) {
    render_notes(patch, &[(0.0, GATE_MS, NOTE, VELOCITY)], TOTAL_MS)
}

/// Renders a list of (start ms, length ms, note, velocity) events.
pub fn render_notes(
    patch: &Patch,
    notes: &[(f32, f32, u8, f32)],
    total_ms: f32,
) -> (Vec<f32>, Vec<f32>) {
    let mut e = Engine::without_worker(SR);
    e.set_patch(patch);
    if patch.sampler_on {
        e.set_sample(Arc::new(render_bake(patch, SR)));
    }
    let n = (total_ms * 0.001 * SR) as usize;
    let mut l = vec![0.0; n];
    let mut r = vec![0.0; n];
    // Event list in samples.
    let mut ev: Vec<(usize, bool, u8, f32)> = vec![];
    for &(start, len, note, vel) in notes {
        let s = (start * 0.001 * SR) as usize;
        ev.push((s, true, note, vel));
        ev.push((s + (len * 0.001 * SR) as usize, false, note, vel));
    }
    ev.sort_by_key(|e| (e.0, e.1));
    let mut pos = 0;
    let mut k = 0;
    while pos < n {
        while k < ev.len() && ev[k].0 <= pos {
            let (_, on, note, vel) = ev[k];
            if on {
                e.note_on(note, vel);
            } else {
                e.note_off(note);
            }
            k += 1;
        }
        let next = ev.get(k).map(|x| x.0).unwrap_or(n).min(n);
        let end = (pos + rs92_dsp::MAX_SUB_BLOCK)
            .min(n)
            .min(next.max(pos + 1));
        e.render(&mut l[pos..end], &mut r[pos..end]);
        pos = end;
    }
    (l, r)
}

/// Measurements of a rendered hit.
#[derive(Clone, Copy, Debug, Default)]
pub struct Analysis {
    pub valid: bool,
    pub dc: f32,
    pub peak_db: f32,
    /// Energy of the last 100 ms relative to the first 100 ms.
    pub tail_ratio: f32,
    pub crest_db: f32,
    pub centroid_hz: f32,
    pub flatness: f32,
}

fn energy(x: &[f32]) -> f64 {
    x.iter().map(|v| (*v as f64).powi(2)).sum()
}

pub fn analyse(l: &[f32], r: &[f32]) -> Analysis {
    let mono: Vec<f32> = l.iter().zip(r).map(|(a, b)| 0.5 * (a + b)).collect();
    let valid = l.iter().chain(r).all(|v| v.is_finite());
    if !valid || mono.is_empty() {
        return Analysis::default();
    }
    let ms = |t: f32| ((t * 0.001 * SR) as usize).min(mono.len());
    let dc = (mono.iter().map(|v| *v as f64).sum::<f64>() / mono.len() as f64) as f32;
    let peak = l.iter().chain(r).fold(0.0f32, |m, v| m.max(v.abs()));
    let peak_db = 20.0 * peak.max(1e-9).log10();

    let first = energy(&mono[..ms(100.0)]);
    let last = energy(&mono[mono.len() - ms(100.0)..]);
    let tail_ratio = if first > 0.0 {
        (last / first) as f32
    } else {
        1.0
    };

    let punch = &mono[..ms(150.0)];
    let p = punch.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let rms = (energy(punch) / punch.len() as f64).sqrt() as f32;
    let crest_db = if rms > 0.0 {
        20.0 * (p / rms).log10()
    } else {
        0.0
    };

    // Spectrum of the first 200 ms, Hann-windowed.
    let seg = &mono[..ms(200.0)];
    let n = seg.len();
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(n);
    let mut input: Vec<f32> = seg
        .iter()
        .enumerate()
        .map(|(i, v)| v * (0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos()))
        .collect();
    let mut spec = fft.make_output_vec();
    let _ = fft.process(&mut input, &mut spec);
    let power: Vec<f64> = spec
        .iter()
        .skip(1)
        .map(|c| (c.norm_sqr() as f64) + 1e-20)
        .collect();
    let bin_hz = SR as f64 / n as f64;
    let total: f64 = power.iter().sum();
    let centroid = power
        .iter()
        .enumerate()
        .map(|(i, p)| (i + 1) as f64 * bin_hz * p)
        .sum::<f64>()
        / total;
    let log_mean = power.iter().map(|p| p.ln()).sum::<f64>() / power.len() as f64;
    let flatness = (log_mean.exp() / (total / power.len() as f64)) as f32;

    Analysis {
        valid,
        dc,
        peak_db,
        tail_ratio,
        crest_db,
        centroid_hz: centroid as f32,
        flatness,
    }
}

impl Analysis {
    /// Distance outside the pass ranges; 0 means the patch passes every check.
    pub fn score(&self) -> f32 {
        if !self.valid {
            return f32::INFINITY;
        }
        let mut s = 0.0;
        s += (self.dc.abs() - 0.01).max(0.0) * 100.0;
        s += (-24.0 - self.peak_db).max(0.0) / 6.0 + (self.peak_db + 0.5).max(0.0) / 6.0;
        s += (self.tail_ratio - 0.25).max(0.0) * 4.0;
        s += (6.0 - self.crest_db).max(0.0) / 3.0;
        s += (500.0f32.log2() - self.centroid_hz.max(1.0).log2()).max(0.0)
            + (self.centroid_hz.max(1.0).log2() - 6000.0f32.log2()).max(0.0);
        s += (self.flatness - 0.5).max(0.0) * 4.0;
        s
    }

    pub fn passes(&self) -> bool {
        self.score() == 0.0
    }
}
