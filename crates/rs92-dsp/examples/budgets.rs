//! Prints CPU use against the design doc's section 6 budgets.
use rs92_dsp::bake::render_bake;
use rs92_dsp::*;
use std::time::Instant;

const SR: f32 = 48_000.0;

fn run(name: &str, e: &mut Engine, budget: f32) {
    let mut l = vec![0.0f32; 128];
    let mut r = vec![0.0f32; 128];
    let buffers = 3000; // 8 s of audio
    let t = Instant::now();
    for _ in 0..buffers {
        for (a, b) in l.chunks_mut(MAX_SUB_BLOCK).zip(r.chunks_mut(MAX_SUB_BLOCK)) {
            e.render(a, b);
        }
    }
    let pct = t.elapsed().as_secs_f32() / (buffers as f32 * 128.0 / SR) * 100.0;
    let ok = if pct < budget { "PASS" } else { "FAIL" };
    println!("{name:<28} {pct:>6.2} % of one core   (budget < {budget} %)  {ok}");
}

fn live(os: OsFactor) -> Engine {
    let mut e = Engine::without_worker(SR);
    e.set_patch(&Patch {
        sampler_on: false,
        chord_type: ChordType::Min9,
        voices: 16,
        amp_s: 100.0,
        os_factor: os,
        ..Patch::default()
    });
    for i in 0..16 {
        e.note_on(40 + i * 2, 0.8);
    }
    e
}

fn main() {
    run("LIVE 16 x 7 partials, 2x OS", &mut live(OsFactor::X2), 5.0);
    run("LIVE 16 x 7 partials, 4x OS", &mut live(OsFactor::X4), 9.0);

    let mut e = Engine::new(SR);
    e.set_patch(&Patch {
        tail_rel: 5000.0,
        voices: 16,
        amp_r: 3000.0,
        ..Patch::default()
    });
    let (mut l, mut r) = (vec![0.0f32; 32], vec![0.0f32; 32]);
    while e.bake_pending() {
        e.render(&mut l, &mut r);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    for i in 0..16 {
        e.note_on(40 + i * 2, 0.8);
    }
    run("SAMPLER 16 voices", &mut e, 1.0);
    run("Idle", &mut Engine::without_worker(SR), 0.1);

    let p = Patch {
        amp_r: 2000.0,
        tail_rel: 2000.0,
        ..Patch::default()
    };
    let t = Instant::now();
    let b = render_bake(&p, SR);
    let ms = t.elapsed().as_secs_f32() * 1000.0;
    let secs = b.len() as f32 / SR;
    println!(
        "Bake ({secs:.2} s buffer)            {ms:>6.2} ms             (budget < 30 ms) {}",
        if ms < 30.0 { "PASS" } else { "FAIL" }
    );
}
