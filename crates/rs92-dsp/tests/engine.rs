//! Engine-level tests: chord voices, bake determinism and caching, fuzzing.

use rand::{Rng, SeedableRng};
use rand_pcg::Pcg64;
use rs92_dsp::bake::render_bake;
use rs92_dsp::*;
use std::time::{Duration, Instant};

fn render(engine: &mut Engine, secs: f32) -> (Vec<f32>, Vec<f32>) {
    let n = (secs * engine.sample_rate()) as usize;
    let mut l = vec![0.0; n];
    let mut r = vec![0.0; n];
    engine.render_offline(&mut l, &mut r);
    (l, r)
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}

#[test]
fn live_landlord_hit_is_audible_and_bounded() {
    let mut e = Engine::without_worker(48_000.0);
    e.set_patch(&Patch {
        sampler_on: false,
        ..Patch::default()
    });
    e.note_on(64, 100.0 / 127.0);
    let (l, r) = render(&mut e, 0.12);
    e.note_off(64);
    let (l2, _) = render(&mut e, 1.0);
    let p = peak(&l).max(peak(&r));
    println!("LIVE landlord peak {:.1} dBFS", 20.0 * p.log10());
    assert!(p > 0.05 && p < 1.0, "peak {p}");
    // Released: decays.
    assert!(peak(&l2[40_000..]) < p * 0.05);
    // All voices free after the tail.
    assert!(!e.bank.any_active());
}

#[test]
fn e4_lights_e2_e3_e4_g4_b4() {
    let mut e = Engine::without_worker(48_000.0);
    e.set_patch(&Patch {
        sampler_on: false,
        ..Patch::default()
    });
    e.note_on(64, 1.0);
    let mut lit = vec![];
    e.for_each_lit_note(|n| lit.push(n));
    assert_eq!(lit, vec![40, 52, 64, 67, 71]);
}

#[test]
fn voice_limit_steals_oldest() {
    let mut e = Engine::without_worker(48_000.0);
    e.set_patch(&Patch {
        sampler_on: false,
        voices: 2,
        ..Patch::default()
    });
    e.note_on(60, 1.0);
    e.note_on(62, 1.0);
    e.note_on(64, 1.0);
    render(&mut e, 0.01);
    let notes: Vec<u8> = e
        .bank
        .voices
        .iter()
        .filter(|v| v.active)
        .map(|v| v.note)
        .collect();
    assert_eq!(notes.len(), 2, "{notes:?}");
    assert!(!notes.contains(&60));
}

#[test]
fn bake_is_deterministic() {
    let p = Patch::default();
    let a = render_bake(&p, 48_000.0);
    let b = render_bake(&p, 48_000.0);
    assert!(a.len() > 4800);
    assert_eq!(a.dry, b.dry);
    assert_eq!(a.crushed, b.crushed);
    let expected = a.len() as f32 * 26_000.0 / 48_000.0;
    assert!((a.crushed[0].len() as f32 - expected).abs() < 2.0);
    // Crushed buffer is quantized to 12 bits.
    for &s in &a.crushed[0] {
        assert_eq!((s * 2048.0).fract(), 0.0);
    }
}

fn wait_for_sample(e: &mut Engine) {
    let t = Instant::now();
    while e.bake_pending() {
        render(e, 0.01);
        assert!(t.elapsed() < Duration::from_secs(10), "bake never arrived");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn sampler_mode_plays_baked_chord_and_caches() {
    let mut e = Engine::new(48_000.0);
    e.set_patch(&Patch::default());
    wait_for_sample(&mut e);
    let stats = e.bake_stats().unwrap();
    assert_eq!(stats.renders.load(std::sync::atomic::Ordering::Relaxed), 1);
    println!(
        "bake took {} us",
        stats
            .last_render_us
            .load(std::sync::atomic::Ordering::Relaxed)
    );

    e.note_on(64, 100.0 / 127.0);
    let (l, _) = render(&mut e, 0.3);
    let p = peak(&l);
    println!("SAMPLER landlord peak {:.1} dBFS", 20.0 * p.log10());
    assert!(p > 0.05 && p < 1.0);

    // Change a bake parameter, then change it back: the second bake is a cache hit.
    let other = Patch {
        op2_pm: 20.0,
        ..Patch::default()
    };
    e.set_patch(&other);
    wait_for_sample(&mut e);
    e.set_patch(&Patch::default());
    wait_for_sample(&mut e);
    assert_eq!(stats.renders.load(std::sync::atomic::Ordering::Relaxed), 2);
    assert!(stats.cache_hits.load(std::sync::atomic::Ordering::Relaxed) >= 1);
}

/// A knob sweep changes a bake parameter every few milliseconds. New notes must not wait
/// for the sweep to end: the sample has to keep following the knob while it moves.
#[test]
fn bake_follows_a_continuous_knob_sweep() {
    let mut e = Engine::new(48_000.0);
    let mut p = Patch::default();
    e.set_patch(&p);
    wait_for_sample(&mut e);
    let first = e.current_sample().unwrap().key;

    // ~600 ms of wall time, cutoff moving every 10 ms (faster than a 60 fps GUI).
    let mut keys_seen = std::collections::HashSet::new();
    for step in 0..60 {
        p.filters[0].cutoff = 300.0 + step as f32 * 40.0;
        e.set_patch(&p);
        render(&mut e, 0.01);
        keys_seen.insert(e.current_sample().unwrap().key);
        std::thread::sleep(Duration::from_millis(10));
    }
    keys_seen.remove(&first);
    assert!(
        keys_seen.len() >= 3,
        "only {} new bakes arrived during the sweep",
        keys_seen.len()
    );

    // Once the knob stops, the final value is baked promptly.
    let t = Instant::now();
    wait_for_sample(&mut e);
    assert!(
        t.elapsed() < Duration::from_millis(300),
        "{:?}",
        t.elapsed()
    );
    assert_eq!(e.current_sample().unwrap().key, p.bake_key(48_000.0));
}

pub fn random_patch(rng: &mut Pcg64) -> Patch {
    let mut p = Patch::default();
    let pick = |rng: &mut Pcg64, n: usize| rng.gen_range(0..n);
    p.op1_wave = Wave::from_index(pick(rng, Wave::ALL.len()));
    p.op2_wave = Wave::from_index(pick(rng, Wave::ALL.len()));
    p.op1_pd = rng.gen_range(-100.0..=100.0);
    p.op2_pd = rng.gen_range(-100.0..=100.0);
    p.op1_level = rng.gen_range(0.0..=100.0);
    p.op1_fdbk = rng.gen_range(0.0..=100.0);
    p.op2_tune = rng.gen_range(-24..=24);
    p.op2_pm = rng.gen_range(0.0..=100.0);
    p.chord_type = ChordType::from_index(pick(rng, ChordType::ALL.len()));
    p.chord_sub1 = rng.gen();
    p.chord_sub2 = rng.gen();
    p.chord_spread = rng.gen_range(0.0..=100.0);
    p.chord_strum = rng.gen_range(0.0..=60.0);
    p.chord_detune = rng.gen_range(0.0..=25.0);
    p.vel_tone = rng.gen_range(0.0..=100.0);
    for f in &mut p.filters {
        f.ty = FilterType::from_index(pick(rng, FilterType::ALL.len()));
        f.cutoff = 20.0 * 1000f32.powf(rng.gen());
        f.reso = rng.gen_range(0.0..=100.0);
        f.env = rng.gen_range(-100.0..=100.0);
        f.key = rng.gen_range(0.0..=100.0);
    }
    let ms = |rng: &mut Pcg64| 5000.0 * rng.gen::<f32>().powi(3);
    p.amp_a = ms(rng);
    p.amp_d = ms(rng);
    p.amp_r = ms(rng);
    p.amp_s = rng.gen_range(0.0..=100.0);
    p.flt_a = ms(rng);
    p.flt_d = ms(rng);
    p.flt_r = ms(rng);
    p.flt_s = rng.gen_range(0.0..=100.0);
    p.crush_bits = CrushBits::from_index(pick(rng, 4));
    p.crush_rate = 4000.0 * 12f32.powf(rng.gen());
    p.crush_mix = rng.gen_range(0.0..=100.0);
    p.comp_thresh = rng.gen_range(-30.0..=0.0);
    p.tape_level = rng.gen_range(-24.0..=-6.0);
    p.hyper = rng.gen();
    p.width = rng.gen_range(0.0..=200.0);
    p.output = rng.gen_range(-60.0..=6.0);
    p.bend_range = rng.gen_range(0.0..=12.0);
    p.vel_amp = rng.gen_range(0.0..=100.0);
    p.os_factor = OsFactor::from_index(pick(rng, 2));
    p.voices = rng.gen_range(1..=16);
    p.smp_transpose = rng.gen_range(-24..=24);
    p.tail_lp = 40.0 * 500f32.powf(rng.gen());
    p.tail_rel = 10.0 + 4990.0 * rng.gen::<f32>().powi(3);
    p.smp_interp = Interp::from_index(pick(rng, 2));
    p.sampler_on = false;
    p
}

#[test]
fn fuzz_ten_thousand_patches() {
    let mut rng = Pcg64::seed_from_u64(92);
    let mut e = Engine::without_worker(48_000.0);
    let limit = 10f32.powf(12.0 / 20.0);
    let mut l = vec![0.0; 1536];
    let mut r = vec![0.0; 1536];
    for i in 0..10_000 {
        let p = random_patch(&mut rng);
        e.set_patch(&p);
        let note = rng.gen_range(24..=96);
        e.note_on(note, rng.gen());
        e.pitch_bend(rng.gen_range(-1.0..=1.0));
        e.render_offline(&mut l, &mut r);
        e.note_off(note);
        e.render_offline(&mut l, &mut r);
        for &s in l.iter().chain(&r) {
            assert!(s.is_finite(), "patch {i}: non-finite output {p:?}");
            assert!(s.abs() <= limit, "patch {i}: peak {s} {p:?}");
        }
        if i % 100 == 0 {
            e.all_notes_off();
        }
    }
}

#[test]
fn fuzz_bakes() {
    let mut rng = Pcg64::seed_from_u64(7);
    for _ in 0..40 {
        let mut p = random_patch(&mut rng);
        p.amp_r = p.amp_r.min(500.0);
        let b = render_bake(&p, 48_000.0);
        for s in b.dry.iter().chain(&b.crushed).flatten() {
            assert!(s.is_finite() && s.abs() < 4.0);
        }
    }
}
