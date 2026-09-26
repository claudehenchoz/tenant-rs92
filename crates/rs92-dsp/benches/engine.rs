//! Performance budgets from design doc section 6 (48 kHz, 128-sample buffers).
//!
//! Criterion reports time per 128-sample buffer; the budget in % of one core is
//! time / 2.667 ms. `cargo run --release --example budgets` prints the percentages.

use criterion::{criterion_group, criterion_main, Criterion};
use rs92_dsp::bake::render_bake;
use rs92_dsp::*;
use std::hint::black_box;

const SR: f32 = 48_000.0;
const BUF: usize = 128;

fn live_patch(os: OsFactor) -> Patch {
    Patch {
        sampler_on: false,
        chord_type: ChordType::Min9,
        voices: 16,
        amp_s: 100.0,
        os_factor: os,
        ..Patch::default()
    }
}

pub fn live_engine(os: OsFactor) -> Engine {
    let mut e = Engine::without_worker(SR);
    e.set_patch(&live_patch(os));
    for i in 0..16 {
        e.note_on(40 + i * 2, 0.8);
    }
    e
}

fn process(e: &mut Engine, l: &mut [f32], r: &mut [f32]) {
    for (a, b) in l.chunks_mut(MAX_SUB_BLOCK).zip(r.chunks_mut(MAX_SUB_BLOCK)) {
        e.render(a, b);
    }
}

fn benches(c: &mut Criterion) {
    let mut l = vec![0.0f32; BUF];
    let mut r = vec![0.0f32; BUF];

    let mut e = live_engine(OsFactor::X2);
    c.bench_function("live_16x7_os2", |b| {
        b.iter(|| process(&mut e, &mut l, &mut r))
    });

    let mut e = live_engine(OsFactor::X4);
    c.bench_function("live_16x7_os4", |b| {
        b.iter(|| process(&mut e, &mut l, &mut r))
    });

    let mut e = Engine::new(SR);
    e.set_patch(&Patch {
        tail_rel: 5000.0,
        voices: 16,
        ..Patch::default()
    });
    while e.bake_pending() {
        process(&mut e, &mut l, &mut r);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    for i in 0..16 {
        e.note_on(40 + i * 2, 0.8);
    }
    c.bench_function("sampler_16", |b| b.iter(|| process(&mut e, &mut l, &mut r)));

    let mut e = Engine::without_worker(SR);
    c.bench_function("idle", |b| b.iter(|| process(&mut e, &mut l, &mut r)));

    let p = Patch {
        amp_r: 2000.0,
        tail_rel: 2000.0,
        ..Patch::default()
    };
    c.bench_function("bake", |b| b.iter(|| black_box(render_bake(&p, SR))));
}

criterion_group!(name = g; config = Criterion::default().sample_size(20); targets = benches);
criterion_main!(g);
