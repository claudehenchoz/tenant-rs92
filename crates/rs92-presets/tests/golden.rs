//! Golden renders: each core preset plays a fixed 2-bar riff; its log-magnitude
//! spectrogram must stay within tolerance of the stored snapshot.
//!
//! Regenerate on purpose only, then review by ear (WAVs land in target/golden-wav):
//!     cargo test -p rs92-presets --release --test golden -- --ignored regen_golden

use realfft::RealFftPlanner;
use rs92_presets::gate::{render_notes, SR};
use rs92_presets::preset::factory;
use std::path::PathBuf;

const BANDS: usize = 24;
const FRAME: usize = 4096;
/// Mean absolute difference allowed, in dB.
const TOLERANCE_DB: f32 = 1.5;

/// 2 bars at 128 BPM: E4 stabs with a G4 and D4 answer.
fn riff() -> Vec<(f32, f32, u8, f32)> {
    let s = 60_000.0 / 128.0 / 4.0; // one 16th in ms
    let hits = [
        (0, 64, 1.0),
        (3, 64, 0.8),
        (6, 64, 0.9),
        (10, 67, 0.8),
        (16, 64, 1.0),
        (19, 64, 0.8),
        (22, 62, 0.9),
        (26, 64, 0.7),
    ];
    hits.iter()
        .map(|&(step, n, v)| (step as f32 * s, s * 1.5, n, v))
        .collect()
}

fn spectrogram(l: &[f32], r: &[f32]) -> Vec<f32> {
    let mono: Vec<f32> = l.iter().zip(r).map(|(a, b)| 0.5 * (a + b)).collect();
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FRAME);
    let mut out = vec![];
    for frame in mono.chunks_exact(FRAME) {
        let mut buf: Vec<f32> = frame
            .iter()
            .enumerate()
            .map(|(i, v)| v * (0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / FRAME as f32).cos()))
            .collect();
        let mut spec = fft.make_output_vec();
        fft.process(&mut buf, &mut spec).unwrap();
        // Log-spaced bands from 40 Hz to 16 kHz.
        for b in 0..BANDS {
            let f0 = 40.0 * 400f32.powf(b as f32 / BANDS as f32);
            let f1 = 40.0 * 400f32.powf((b + 1) as f32 / BANDS as f32);
            let (i0, i1) = (
                (f0 / SR * FRAME as f32) as usize,
                ((f1 / SR * FRAME as f32) as usize).max((f0 / SR * FRAME as f32) as usize + 1),
            );
            let e: f32 = spec[i0..i1.min(spec.len())]
                .iter()
                .map(|c| c.norm_sqr())
                .sum();
            out.push(10.0 * (e + 1e-12).log10());
        }
    }
    out
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn file_for(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn core_presets() -> Vec<rs92_presets::Preset> {
    let names: Vec<&str> = rs92_presets::recipes::recipes()
        .iter()
        .map(|r| r.name)
        .collect();
    factory()
        .into_iter()
        .filter(|p| names.contains(&p.name.as_str()))
        .collect()
}

fn render(p: &rs92_presets::Preset) -> (Vec<f32>, Vec<f32>) {
    render_notes(&p.patch, &riff(), 4000.0)
}

fn write_wav(path: &std::path::Path, l: &[f32], r: &[f32]) {
    let mut data = Vec::with_capacity(l.len() * 4);
    for (a, b) in l.iter().zip(r) {
        for s in [a, b] {
            data.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
        }
    }
    let mut w = Vec::new();
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&(SR as u32).to_le_bytes());
    w.extend_from_slice(&(SR as u32 * 4).to_le_bytes());
    w.extend_from_slice(&4u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(data.len() as u32).to_le_bytes());
    w.extend_from_slice(&data);
    std::fs::write(path, w).unwrap();
}

#[test]
fn golden_renders_match() {
    let presets = core_presets();
    assert_eq!(presets.len(), 18);
    for p in presets {
        let path = golden_dir().join(format!("{}.json", file_for(&p.name)));
        let stored: Vec<f32> =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|_| {
                panic!("missing snapshot {}; run regen_golden", path.display())
            }))
            .unwrap();
        let (l, r) = render(&p);
        let now = spectrogram(&l, &r);
        assert_eq!(now.len(), stored.len());
        // Compare only bands within 60 dB of the loudest, so noise-floor changes do not count.
        let top = stored.iter().cloned().fold(f32::MIN, f32::max);
        let (mut sum, mut n) = (0.0, 0);
        for (a, b) in now.iter().zip(&stored) {
            if *b > top - 60.0 {
                sum += (a - b).abs();
                n += 1;
            }
        }
        let mean = sum / n.max(1) as f32;
        assert!(
            mean <= TOLERANCE_DB,
            "{}: spectrogram differs by {mean:.2} dB",
            p.name
        );
    }
}

#[test]
#[ignore = "regenerates the stored snapshots; review the WAVs by ear before committing"]
fn regen_golden() {
    let wav_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/golden-wav");
    std::fs::create_dir_all(&wav_dir).unwrap();
    std::fs::create_dir_all(golden_dir()).unwrap();
    for p in core_presets() {
        let (l, r) = render(&p);
        let spec: Vec<f32> = spectrogram(&l, &r)
            .iter()
            .map(|v| (v * 100.0).round() / 100.0)
            .collect();
        let f = file_for(&p.name);
        std::fs::write(
            golden_dir().join(format!("{f}.json")),
            serde_json::to_string(&spec).unwrap(),
        )
        .unwrap();
        write_wav(&wav_dir.join(format!("{f}.wav")), &l, &r);
        println!("{}", p.name);
    }
}
