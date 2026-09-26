//! Rough per-stage cost of the LIVE voice (16 voices x 7 partials, 2x OS).
use rs92_dsp::*;
use std::time::Instant;

fn run(name: &str, p: Patch) {
    let sr = 48_000.0;
    let mut e = Engine::without_worker(sr);
    e.set_patch(&p);
    for i in 0..16 {
        e.note_on(40 + i * 2, 0.8);
    }
    let bs: usize = std::env::var("BS")
        .map(|v| v.parse().unwrap())
        .unwrap_or(32);
    let (mut l, mut r) = (vec![0.0f32; bs], vec![0.0f32; bs]);
    let blocks = 12_000 * 32 / bs;
    let t = Instant::now();
    for _ in 0..blocks {
        e.render(&mut l, &mut r);
    }
    let pct = t.elapsed().as_secs_f32() / (blocks as f32 * bs as f32 / sr) * 100.0;
    println!("{name:<32} {pct:6.2} %");
}

fn main() {
    let base = Patch {
        sampler_on: false,
        chord_type: ChordType::Min9,
        voices: 16,
        amp_s: 100.0,
        ..Patch::default()
    };
    run("full", base);
    let mut nf = base;
    for f in &mut nf.filters {
        f.ty = FilterType::Off;
    }
    run("no filters", nf);
    run(
        "no filters, sine/sine",
        Patch {
            op1_wave: Wave::Sine,
            op2_wave: Wave::Sine,
            ..nf
        },
    );
    run(
        "no filters, saw/saw pd0 nopm",
        Patch {
            op1_wave: Wave::Saw,
            op2_wave: Wave::Saw,
            op1_pd: 0.0,
            op2_pd: 0.0,
            op2_pm: 0.0,
            ..nf
        },
    );
    run(
        "no filters, no finish",
        Patch {
            crush_mix: 0.0,
            ..nf
        },
    );
}
