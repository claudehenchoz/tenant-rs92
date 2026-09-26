//! The full process loop never allocates: 60 s of random notes and automation.
// assert_no_alloc only checks in debug builds.
#![cfg(debug_assertions)]

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use rand::{Rng, SeedableRng};
use rand_pcg::Pcg64;
use rs92_dsp::*;

#[global_allocator]
static A: AllocDisabler = AllocDisabler;

#[test]
fn process_loop_does_not_allocate() {
    let sr = 48_000.0;
    let mut e = Engine::new(sr);
    let mut rng = Pcg64::seed_from_u64(1);
    let mut l = vec![0.0f32; MAX_SUB_BLOCK];
    let mut r = vec![0.0f32; MAX_SUB_BLOCK];
    let mut patch = Patch::default();
    let blocks = (60.0 * sr) as usize / MAX_SUB_BLOCK;
    for b in 0..blocks {
        // Automation: nudge a few parameters, sometimes switching modes.
        if b % 50 == 0 {
            patch.op2_pm = rng.gen_range(0.0..=100.0);
            patch.filters[0].cutoff = rng.gen_range(100.0..=8000.0);
            patch.width = rng.gen_range(0.0..=200.0);
            patch.chord_type = ChordType::from_index(rng.gen_range(0..ChordType::ALL.len()));
            if rng.gen_bool(0.1) {
                patch.sampler_on = !patch.sampler_on;
            }
            if rng.gen_bool(0.05) {
                patch.os_factor = OsFactor::from_index(rng.gen_range(0..2));
            }
        }
        let ev = rng.gen_range(0..40);
        assert_no_alloc(|| {
            e.set_patch(&patch);
            match ev {
                0 => e.note_on(rng.gen_range(36..=84), rng.gen()),
                1 => e.note_off(rng.gen_range(36..=84)),
                2 => e.pitch_bend(rng.gen_range(-1.0..=1.0)),
                3 => e.sustain(rng.gen()),
                4 => e.all_notes_off(),
                _ => {}
            }
            e.render(&mut l, &mut r);
        });
        if b % 200 == 0 {
            // Give the bake worker a chance to deliver, as a real audio callback would.
            std::thread::yield_now();
        }
        assert!(l.iter().chain(&r).all(|s| s.is_finite()));
    }
}
