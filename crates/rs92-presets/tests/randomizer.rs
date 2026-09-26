//! Randomizer acceptance: >= 95 % of 1,000 seeds pass the gate on the first or second
//! try, and the same seed gives the same patch.

use rs92_dsp::Patch;
use rs92_presets::random::{generate, mutate};

#[test]
fn same_seed_same_patch() {
    let base = Patch::default();
    let a = generate(42, Some("FM Stab"), &base, 0);
    let b = generate(42, Some("FM Stab"), &base, 0);
    assert_eq!(a.patch, b.patch);
    assert_eq!(a.name, b.name);
    let m1 = mutate(7, &base, Some("FM Stab"), 0.2, 0);
    let m2 = mutate(7, &base, Some("FM Stab"), 0.2, 0);
    assert_eq!(m1.patch, m2.patch);
}

#[test]
fn locked_sections_are_untouched() {
    let base = Patch::default();
    // Lock 01 SOURCE and 06 FINISH.
    let r = generate(9, None, &base, 0b10_0001);
    assert_eq!(r.patch.op1_wave, base.op1_wave);
    assert_eq!(r.patch.op2_pm, base.op2_pm);
    assert_eq!(r.patch.comp_thresh, base.comp_thresh);
    assert_eq!(r.patch.output, base.output);
}

#[test]
fn names_fit_the_vfd() {
    for seed in 0..200 {
        let r = generate(seed, None, &Patch::default(), 0);
        assert!(r.name.len() <= 14, "{}", r.name);
    }
}

#[test]
#[ignore = "slow: 1,000 gated renders; run with --release -- --ignored"]
fn gate_pass_rate_over_1000_seeds() {
    let base = Patch::default();
    let mut ok = 0;
    for seed in 0..1000u64 {
        let r = generate(seed.wrapping_mul(0x9e37_79b9) + 1, None, &base, 0);
        if matches!(r.passed_on, Some(1 | 2)) {
            ok += 1;
        }
    }
    println!("pass on 1st/2nd try: {ok}/1000");
    assert!(ok >= 950, "{ok}/1000");
}
