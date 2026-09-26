//! Smart randomizer (design doc section 9): archetype → constrained sampling → quality
//! gate → name. Deterministic for a given seed and inputs.

use crate::gate::{analyse, render_hit};
use crate::ids::{self, Kind};
use rand::distributions::WeightedIndex;
use rand::prelude::*;
use rand_pcg::Pcg64;
use rs92_dsp::*;

/// Seeds tried before settling for the best-scoring patch.
pub const MAX_TRIES: usize = 12;
/// Random patches are normalised to this peak.
pub const TARGET_PEAK_DB: f32 = -6.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Archetype {
    FmStab,
    Organ,
    Piano,
    OrchHit,
    AnalogChord,
    Bleep,
    DeepChord,
    Hardcore,
}

impl Archetype {
    pub const ALL: [Archetype; 8] = [
        Archetype::FmStab,
        Archetype::Organ,
        Archetype::Piano,
        Archetype::OrchHit,
        Archetype::AnalogChord,
        Archetype::Bleep,
        Archetype::DeepChord,
        Archetype::Hardcore,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Archetype::FmStab => "FM Stab",
            Archetype::Organ => "Organ",
            Archetype::Piano => "Piano",
            Archetype::OrchHit => "Orch Hit",
            Archetype::AnalogChord => "Analog Chord",
            Archetype::Bleep => "Bleep",
            Archetype::DeepChord => "Deep Chord",
            Archetype::Hardcore => "Hardcore",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|a| a.name().eq_ignore_ascii_case(s))
    }
}

/// Distributions of one archetype. The centre patches are the section 8 recipes.
struct Template {
    /// (wave, weight) for OP1 and OP2.
    op1: &'static [(Wave, u32)],
    op2: &'static [(Wave, u32)],
    pm: f32,
    pd1: f32,
    pd2: f32,
    level1: f32,
    f1_type: FilterType,
    /// F1 cutoff range for the log-uniform draw.
    f1_range: (f32, f32),
    reso_max: f32,
    reso_min: f32,
    f2: (FilterType, f32),
}

fn template(a: Archetype) -> Template {
    use FilterType::*;
    use Wave::*;
    match a {
        Archetype::FmStab => Template {
            op1: &[
                (ResIII, 30),
                (ResII, 20),
                (Bell, 15),
                (Saw, 15),
                (ResI, 10),
                (Square, 5),
                (Sine, 5),
            ],
            op2: &[(Saw, 50), (Square, 15), (Pulse, 15), (Sine, 10), (Tri, 10)],
            pm: 48.0,
            pd1: 26.0,
            pd2: -60.0,
            level1: 58.0,
            f1_type: Lp18,
            f1_range: (400.0, 2000.0),
            reso_max: 40.0,
            reso_min: 0.0,
            f2: (Hp12, 110.0),
        },
        Archetype::Organ => Template {
            op1: &[(Organ, 70), (Sine, 15), (Tri, 15)],
            op2: &[(Sine, 60), (Tri, 25), (Organ, 15)],
            pm: 14.0,
            pd1: 0.0,
            pd2: 0.0,
            level1: 70.0,
            f1_type: Lp12,
            f1_range: (2500.0, 8000.0),
            reso_max: 20.0,
            reso_min: 0.0,
            f2: (Hp12, 150.0),
        },
        Archetype::Piano => Template {
            op1: &[(Sine, 50), (Tri, 30), (Bell, 20)],
            op2: &[(Bell, 50), (Sine, 30), (Tri, 20)],
            pm: 22.0,
            pd1: 0.0,
            pd2: 0.0,
            level1: 60.0,
            f1_type: Lp12,
            f1_range: (3000.0, 9000.0),
            reso_max: 15.0,
            reso_min: 0.0,
            f2: (Hp12, 110.0),
        },
        Archetype::OrchHit => Template {
            op1: &[(Saw, 60), (Square, 20), (ResII, 20)],
            op2: &[(Saw, 70), (Square, 30)],
            pm: 55.0,
            pd1: 0.0,
            pd2: 0.0,
            level1: 60.0,
            f1_type: Lp24,
            f1_range: (1500.0, 4000.0),
            reso_max: 30.0,
            reso_min: 0.0,
            f2: (Hp12, 150.0),
        },
        Archetype::AnalogChord => Template {
            op1: &[(Saw, 60), (Square, 25), (Pulse, 15)],
            op2: &[(Square, 40), (Saw, 40), (Pulse, 20)],
            pm: 6.0,
            pd1: 0.0,
            pd2: 0.0,
            level1: 80.0,
            f1_type: Lp24,
            f1_range: (600.0, 2000.0),
            reso_max: 45.0,
            reso_min: 10.0,
            f2: (Hp12, 90.0),
        },
        Archetype::Bleep => Template {
            op1: &[(Sine, 60), (Tri, 25), (Square, 15)],
            op2: &[(Square, 50), (Sine, 30), (Pulse, 20)],
            pm: 20.0,
            pd1: 0.0,
            pd2: 0.0,
            level1: 60.0,
            f1_type: Bp12,
            f1_range: (800.0, 2500.0),
            reso_max: 75.0,
            reso_min: 40.0,
            f2: (Hp12, 150.0),
        },
        Archetype::DeepChord => Template {
            op1: &[(Tri, 35), (Saw, 35), (Square, 15), (Sine, 15)],
            op2: &[(Saw, 60), (Sine, 20), (Square, 20)],
            pm: 12.0,
            pd1: 0.0,
            pd2: 0.0,
            level1: 70.0,
            f1_type: Lp24,
            f1_range: (500.0, 1400.0),
            reso_max: 50.0,
            reso_min: 15.0,
            f2: (Hp12, 90.0),
        },
        Archetype::Hardcore => Template {
            op1: &[(ResIII, 35), (ResII, 30), (ResI, 15), (Saw, 20)],
            op2: &[(Saw, 60), (Pulse, 20), (Square, 20)],
            pm: 55.0,
            pd1: 26.0,
            pd2: -60.0,
            level1: 58.0,
            f1_type: Lp18,
            f1_range: (500.0, 2200.0),
            reso_max: 35.0,
            reso_min: 0.0,
            f2: (Hp12, 120.0),
        },
    }
}

fn weighted<T: Copy>(rng: &mut Pcg64, items: &[(T, u32)]) -> T {
    let w = WeightedIndex::new(items.iter().map(|x| x.1)).expect("weights");
    items[w.sample(rng)].0
}

fn normal(rng: &mut Pcg64, mean: f32, sigma: f32) -> f32 {
    // Box–Muller.
    let u1: f32 = rng.gen_range(1e-7..1.0);
    let u2: f32 = rng.gen();
    mean + sigma * (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos()
}

fn log_uniform(rng: &mut Pcg64, lo: f32, hi: f32) -> f32 {
    lo * (hi / lo).powf(rng.gen())
}

/// Caps PM so the modulation index times OP2's frequency ratio stays below 1.2.
fn cap_pm(pm: f32, tune: i32) -> f32 {
    let ratio = 2f32.powf(tune as f32 / 12.0);
    let max = 100.0 * (1.2 / (1.5 * ratio)).sqrt();
    pm.clamp(0.0, max.min(100.0))
}

/// Samples a fresh patch for an archetype. Hidden parameters come from `base`.
fn sample(rng: &mut Pcg64, a: Archetype, base: &Patch) -> Patch {
    let t = template(a);
    let mut p = *base;
    p.custom_chords = base.custom_chords;
    p.op1_wave = weighted(rng, t.op1);
    p.op2_wave = weighted(rng, t.op2);
    p.op2_tune = weighted(rng, &[(0, 2), (12, 4), (19, 2), (24, 2), (7, 1), (-12, 1)]);
    p.op2_pm = cap_pm(normal(rng, t.pm, 15.0), p.op2_tune);
    p.op1_pd = normal(rng, t.pd1, 25.0).clamp(-100.0, 100.0);
    p.op2_pd = normal(rng, t.pd2, 25.0).clamp(-100.0, 100.0);
    p.op1_level = normal(rng, t.level1, 12.0).clamp(20.0, 100.0);
    p.op1_fdbk = if rng.gen_bool(0.7) {
        0.0
    } else {
        rng.gen_range(0.0..50.0)
    };

    p.chord_type = weighted(
        rng,
        &[
            (ChordType::Min, 30),
            (ChordType::Min7, 25),
            (ChordType::Min9, 15),
            (ChordType::Maj, 10),
            (ChordType::Sus4, 10),
            (ChordType::Fifth, 10),
        ],
    );
    p.chord_sub1 = rng.gen_bool(0.7);
    p.chord_sub2 = rng.gen_bool(0.7);
    p.chord_spread = rng.gen_range(0.0..50.0);
    p.chord_strum = if rng.gen_bool(0.85) {
        0.0
    } else {
        rng.gen_range(0.0..20.0)
    };
    p.chord_detune = rng.gen_range(0.0..14.0);

    p.filters[0] = FilterSlot {
        ty: t.f1_type,
        cutoff: log_uniform(rng, t.f1_range.0, t.f1_range.1),
        reso: rng.gen_range(t.reso_min..=t.reso_max),
        env: rng.gen_range(30.0..=80.0),
        key: rng.gen_range(50.0..=100.0),
    };
    p.filters[1] = FilterSlot {
        ty: t.f2.0,
        cutoff: (t.f2.1 * rng.gen_range(0.8..1.25f32)).clamp(20.0, 400.0),
        reso: 0.0,
        env: if rng.gen_bool(0.5) { -20.0 } else { 0.0 },
        key: 0.0,
    };
    p.filters[2] = if rng.gen_bool(0.7) {
        FilterSlot {
            ty: FilterType::Lp24,
            cutoff: 18_000.0,
            reso: 0.0,
            env: 0.0,
            key: 0.0,
        }
    } else {
        FilterSlot {
            ty: FilterType::Lp24,
            cutoff: log_uniform(rng, 6_000.0, 14_000.0),
            reso: 0.0,
            env: 0.0,
            key: 0.0,
        }
    };

    p.amp_a = rng.gen_range(0.0..10.0);
    p.amp_d = rng.gen_range(150.0..700.0);
    p.amp_s = rng.gen_range(0.0..20.0);
    p.amp_r = rng.gen_range(60.0..300.0);
    p.flt_a = rng.gen_range(0.0..5.0);
    p.flt_d = p.amp_d * rng.gen_range(0.3..0.9);
    p.flt_s = rng.gen_range(0.0..25.0);
    p.flt_r = rng.gen_range(60.0..200.0);

    p.crush_bits = weighted(
        rng,
        &[
            (CrushBits::B12, 50),
            (CrushBits::B8, 20),
            (CrushBits::B16, 20),
            (CrushBits::Off, 10),
        ],
    );
    p.crush_rate = *[16_000.0, 22_050.0, 26_000.0, 32_000.0, 44_100.0]
        .choose(rng)
        .expect("rates");
    p.crush_mix = rng.gen_range(20.0..60.0);
    p.sampler_on = rng.gen_bool(0.7);
    p.tail_lp = log_uniform(rng, 250.0, 2500.0);
    p.tail_rel = rng.gen_range(p.amp_r.max(300.0)..2500.0);
    p.smp_transpose = if a == Archetype::Hardcore && rng.gen_bool(0.3) {
        -12
    } else {
        0
    };

    p.comp_thresh = rng.gen_range(-14.0..-6.0);
    p.tape_level = rng.gen_range(-22.0..-12.0);
    p.hyper = rng.gen_bool(0.4);
    p.width = rng.gen_range(80.0..140.0);
    p.output = -3.0;
    p
}

/// Normalised position of a parameter (log for frequencies, sqrt for times).
fn to_norm(d: &ids::ParamDef, v: f32) -> f32 {
    if is_log(d.id) {
        (v.max(d.min) / d.min).ln() / (d.max / d.min).ln()
    } else if is_time(d.id) {
        ((v - d.min) / (d.max - d.min)).max(0.0).sqrt()
    } else {
        (v - d.min) / (d.max - d.min)
    }
}

fn from_norm(d: &ids::ParamDef, n: f32) -> f32 {
    let n = n.clamp(0.0, 1.0);
    if is_log(d.id) {
        d.min * (d.max / d.min).powf(n)
    } else if is_time(d.id) {
        d.min + n * n * (d.max - d.min)
    } else {
        d.min + n * (d.max - d.min)
    }
}

fn is_log(id: &str) -> bool {
    id.ends_with("_cutoff") || id == "crush_rate" || id == "tail_lp"
}

fn is_time(id: &str) -> bool {
    matches!(
        id,
        "amp_a" | "amp_d" | "amp_r" | "flt_a" | "flt_d" | "flt_r" | "tail_rel"
    )
}

/// Standard deviation in normalised space at amount 1.0.
const MUTATE_SIGMA: f32 = 0.5;

fn mutate_patch(rng: &mut Pcg64, a: Archetype, base: &Patch, amount: f32, locks: u8) -> Patch {
    let fresh = sample(rng, a, base);
    let mut p = *base;
    for d in ids::all() {
        if d.section == 0 || locks & (1 << (d.section - 1)) != 0 || d.id == "output" {
            continue;
        }
        match d.kind {
            Kind::Float => {
                let n = to_norm(d, (d.get)(base)) + normal(rng, 0.0, amount * MUTATE_SIGMA);
                (d.set)(&mut p, from_norm(d, n));
            }
            Kind::Int | Kind::Bool | Kind::Enum(_) => {
                if rng.gen::<f32>() < amount {
                    (d.set)(&mut p, (d.get)(&fresh));
                }
            }
        }
    }
    p.op2_pm = cap_pm(p.op2_pm, p.op2_tune);
    p
}

/// Copies every parameter of a locked section from `base`.
fn apply_locks(p: &mut Patch, base: &Patch, locks: u8) {
    for d in ids::all() {
        let locked = d.section == 0 || locks & (1 << (d.section - 1)) != 0;
        if locked {
            (d.set)(p, (d.get)(base));
        }
    }
    if locks & (1 << 1) != 0 {
        p.custom_chords = base.custom_chords;
    }
}

const FIRST: &[&str] = &[
    "ATTIC",
    "LODGER",
    "BASEMENT",
    "DEPOSIT",
    "SQUAT",
    "LEASE",
    "SUBLET",
    "BEDSIT",
    "LOFT",
    "ARREARS",
    "BOILER",
    "COUNCIL",
    "STAIRWELL",
    "TOWER",
    "FLAT",
    "RENT",
    "DOORBELL",
    "METER",
    "HALLWAY",
    "ESTATE",
    "KEYS",
    "LANDING",
    "TENURE",
    "EVICT",
];
const SECOND: &[&str] = &[
    "STAB", "HIT", "CHORD", "91", "92", "DUB", "RAVE", "JAM", "PUMP", "MIX", "HOUSE",
];

/// "ATTIC STAB", at most 14 characters.
pub fn make_name(rng: &mut Pcg64) -> String {
    loop {
        let a = FIRST.choose(rng).expect("words");
        let b = SECOND.choose(rng).expect("words");
        let n = format!("{a} {b}");
        if n.len() <= crate::preset::MAX_NAME_LEN {
            return n;
        }
    }
}

#[derive(Clone, Debug)]
pub struct RandomResult {
    pub patch: Patch,
    pub name: String,
    pub seed: u64,
    pub archetype: &'static str,
    /// Try on which the gate passed (1-based), or None if the best failing try was kept.
    pub passed_on: Option<usize>,
}

fn try_seed(seed: u64, k: usize) -> u64 {
    seed ^ (k as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

/// Runs the gate over up to 12 derived seeds and normalises the winner's output level.
fn gated(
    seed: u64,
    archetype: Archetype,
    locks: u8,
    mut make: impl FnMut(&mut Pcg64) -> Patch,
) -> RandomResult {
    let mut best: Option<(f32, Patch, f32)> = None;
    let mut passed_on = None;
    for k in 0..MAX_TRIES {
        let mut rng = Pcg64::seed_from_u64(try_seed(seed, k));
        let p = make(&mut rng);
        let a = analyse_patch(&p);
        let score = a.score();
        if best.as_ref().is_none_or(|b| score < b.0) {
            best = Some((score, p, a.peak_db));
        }
        if score == 0.0 {
            passed_on = Some(k + 1);
            break;
        }
    }
    let (_, mut patch, peak_db) = best.expect("at least one try");
    // OUTPUT belongs to 06 FINISH; a locked section keeps its level.
    if peak_db.is_finite() && locks & (1 << 5) == 0 {
        patch.output = (patch.output + TARGET_PEAK_DB - peak_db).clamp(-60.0, 6.0);
    }
    let mut name_rng = Pcg64::seed_from_u64(seed ^ 0x6e61_6d65);
    RandomResult {
        patch,
        name: make_name(&mut name_rng),
        seed,
        archetype: archetype.name(),
        passed_on,
    }
}

pub fn analyse_patch(p: &Patch) -> crate::gate::Analysis {
    let (l, r) = render_hit(p);
    analyse(&l, &r)
}

/// New patch from a random archetype, weighted 50 % towards the current one.
pub fn generate(seed: u64, current: Option<&str>, base: &Patch, locks: u8) -> RandomResult {
    let mut rng = Pcg64::seed_from_u64(seed);
    let archetype = match current.and_then(Archetype::from_name) {
        Some(a) if rng.gen_bool(0.5) => a,
        _ => *Archetype::ALL.choose(&mut rng).expect("archetypes"),
    };
    gated(seed, archetype, locks, |rng| {
        let mut p = sample(rng, archetype, base);
        apply_locks(&mut p, base, locks);
        p
    })
}

/// Mutates `base` by `amount` (0.2 for Shift-click, 0.05 for Alt-click).
pub fn mutate(
    seed: u64,
    base: &Patch,
    current: Option<&str>,
    amount: f32,
    locks: u8,
) -> RandomResult {
    let archetype = current
        .and_then(Archetype::from_name)
        .unwrap_or(Archetype::FmStab);
    gated(seed, archetype, locks, |rng| {
        mutate_patch(rng, archetype, base, amount, locks)
    })
}
