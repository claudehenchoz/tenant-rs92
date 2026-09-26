//! The 18 core factory recipes (design doc section 8). Each starts from the
//! LANDLORD '91 defaults and changes only what the table lists.

use crate::preset::bank_name;
use crate::Preset;
use rs92_dsp::*;

pub struct Recipe {
    pub file: &'static str,
    pub name: &'static str,
    pub bank: &'static str,
    pub description: &'static str,
    pub archetype: &'static str,
    pub patch: Patch,
}

fn lp(ty: FilterType, cutoff: f32) -> FilterSlot {
    FilterSlot {
        ty,
        cutoff,
        reso: 0.0,
        env: 0.0,
        key: 100.0,
    }
}

pub fn recipes() -> Vec<Recipe> {
    let d = Patch::default;
    let mut v = vec![];

    v.push(Recipe {
        file: "A01_landlord_91",
        name: "LANDLORD '91",
        bank: "A",
        description: "Early-90s FM house stab, resampled at 12-bit",
        archetype: "FM Stab",
        patch: d(),
    });

    let mut p = d();
    p.op1_wave = Wave::Organ;
    p.op2_wave = Wave::Sine;
    p.op2_tune = 12;
    p.op2_pm = 10.0;
    p.op2_pd = 0.0;
    p.op1_pd = 0.0;
    p.chord_type = ChordType::Min7;
    p.chord_sub2 = false;
    p.filters[0] = FilterSlot {
        env: 20.0,
        ..lp(FilterType::Lp12, 5000.0)
    };
    p.amp_d = 700.0;
    p.amp_s = 25.0;
    p.crush_bits = CrushBits::B16;
    p.crush_rate = 32_000.0;
    p.crush_mix = 20.0;
    v.push(Recipe {
        file: "A02_m1_organ",
        name: "M1 ORGAN",
        bank: "A",
        description: "90s house organ stab",
        archetype: "Organ",
        patch: p,
    });

    let mut p = d();
    p.op1_wave = Wave::Sine;
    p.op2_wave = Wave::Bell;
    p.op2_tune = 12;
    p.op2_pm = 22.0;
    p.op1_pd = 0.0;
    p.op2_pd = 0.0;
    p.chord_type = ChordType::Maj;
    p.chord_sub2 = false;
    p.filters[0] = FilterSlot {
        key: 50.0,
        ..lp(FilterType::Lp12, 6000.0)
    };
    p.amp_d = 900.0;
    p.crush_rate = 32_000.0;
    v.push(Recipe {
        file: "A03_house_piano",
        name: "HOUSE PIANO",
        bank: "A",
        description: "Piano-house stab",
        archetype: "Piano",
        patch: p,
    });

    let mut p = d();
    p.op1_wave = Wave::Saw;
    p.op2_wave = Wave::Saw;
    p.op2_tune = 7;
    p.op2_pm = 65.0;
    p.op1_fdbk = 55.0;
    p.op1_pd = 0.0;
    p.op2_pd = 0.0;
    p.chord_type = ChordType::Fifth;
    p.filters[0] = FilterSlot {
        env: 40.0,
        ..lp(FilterType::Lp24, 2500.0)
    };
    p.amp_d = 450.0;
    p.crush_bits = CrushBits::B8;
    p.crush_rate = 16_000.0;
    p.crush_mix = 60.0;
    p.comp_thresh = -12.0;
    v.push(Recipe {
        file: "A04_orch_hit",
        name: "ORCH HIT",
        bank: "A",
        description: "80s sampler orchestra hit",
        archetype: "Orch Hit",
        patch: p,
    });

    let mut p = d();
    p.op1_wave = Wave::Saw;
    p.op2_wave = Wave::Square;
    p.op2_tune = 0;
    p.op2_pm = 0.0;
    p.op1_level = 80.0;
    p.op1_pd = 0.0;
    p.op2_pd = 0.0;
    p.chord_type = ChordType::Min7;
    p.chord_spread = 40.0;
    p.chord_detune = 10.0;
    p.filters[0] = FilterSlot {
        reso: 30.0,
        env: 55.0,
        ..lp(FilterType::Lp24, 900.0)
    };
    p.flt_d = 180.0;
    p.amp_d = 260.0;
    p.crush_bits = CrushBits::Off;
    p.sampler_on = false;
    v.push(Recipe {
        file: "A05_detroit_min7",
        name: "DETROIT MIN7",
        bank: "A",
        description: "Late-80s techno chord",
        archetype: "Analog Chord",
        patch: p,
    });

    let mut p = d();
    p.op1_wave = Wave::Sine;
    p.op2_wave = Wave::Square;
    p.op2_tune = 24;
    p.op2_pm = 20.0;
    p.op1_pd = 0.0;
    p.op2_pd = 0.0;
    p.chord_type = ChordType::Fifth;
    p.chord_sub1 = false;
    p.chord_sub2 = false;
    p.filters[0] = FilterSlot {
        reso: 55.0,
        env: 0.0,
        ..lp(FilterType::Bp12, 1200.0)
    };
    p.amp_d = 120.0;
    p.sampler_on = false;
    v.push(Recipe {
        file: "A06_bleep_91",
        name: "BLEEP 91",
        bank: "A",
        description: "Bleep techno",
        archetype: "Bleep",
        patch: p,
    });

    let mut p = d();
    p.op1_wave = Wave::ResII;
    p.op2_wave = Wave::Saw;
    p.op2_tune = 12;
    p.op2_pm = 55.0;
    p.chord_type = ChordType::Min9;
    p.crush_bits = CrushBits::B8;
    p.crush_rate = 22_050.0;
    p.comp_thresh = -14.0;
    p.hyper = true;
    v.push(Recipe {
        file: "A07_hardcore_min9",
        name: "HARDCORE MIN9",
        bank: "A",
        description: "Breakbeat hardcore chord",
        archetype: "Hardcore",
        patch: p,
    });

    let mut p = d();
    p.op1_wave = Wave::Saw;
    p.op2_wave = Wave::Saw;
    p.op2_tune = 0;
    p.op2_pm = 0.0;
    p.op1_pd = 0.0;
    p.op2_pd = 0.0;
    p.chord_type = ChordType::Maj;
    p.chord_detune = 15.0;
    p.filters[0] = FilterSlot {
        env: 70.0,
        ..lp(FilterType::Lp24, 800.0)
    };
    p.flt_a = 15.0;
    p.flt_d = 250.0;
    p.amp_a = 5.0;
    p.amp_d = 400.0;
    p.amp_s = 30.0;
    p.crush_bits = CrushBits::Off;
    p.tape_level = -12.0;
    p.sampler_on = false;
    v.push(Recipe {
        file: "A08_italo_brass",
        name: "ITALO BRASS",
        bank: "A",
        description: "Italo/house brass stab",
        archetype: "Analog Chord",
        patch: p,
    });

    let mut p = d();
    p.op1_wave = Wave::Organ;
    p.op2_wave = Wave::Sine;
    p.op2_tune = 24;
    p.op2_pm = 18.0;
    p.op1_pd = 0.0;
    p.op2_pd = 0.0;
    p.chord_type = ChordType::Min7;
    p.chord_sub1 = false;
    p.chord_sub2 = false;
    p.filters[1] = FilterSlot {
        ty: FilterType::Hp12,
        cutoff: 250.0,
        reso: 0.0,
        env: 0.0,
        key: 0.0,
    };
    p.amp_d = 220.0;
    v.push(Recipe {
        file: "A09_garage_organ",
        name: "GARAGE ORGAN",
        bank: "A",
        description: "UK garage organ stab",
        archetype: "Organ",
        patch: p,
    });

    let mut p = d();
    p.op1_wave = Wave::Square;
    p.op2_wave = Wave::Saw;
    p.op2_tune = 12;
    p.op2_pm = 30.0;
    p.op1_pd = 0.0;
    p.chord_type = ChordType::Fifth;
    p.chord_sub2 = false;
    p.filters[0] = FilterSlot {
        env: 50.0,
        ..lp(FilterType::Lp18, 1400.0)
    };
    p.amp_d = 180.0;
    p.crush_rate = 32_000.0;
    v.push(Recipe {
        file: "A10_jackin_fifths",
        name: "JACKIN FIFTHS",
        bank: "A",
        description: "Chicago fifths stab",
        archetype: "FM Stab",
        patch: p,
    });

    let mut p = d();
    p.op1_wave = Wave::Bell;
    p.op2_wave = Wave::Saw;
    p.op2_tune = 12;
    p.op2_pm = 48.0;
    v.push(Recipe {
        file: "B01_tenants_assoc",
        name: "TENANTS ASSOC",
        bank: "B",
        description: "The Landlord chain with a bell operator",
        archetype: "FM Stab",
        patch: p,
    });

    let mut p = d();
    p.op1_wave = Wave::ResI;
    p.op2_wave = Wave::Saw;
    p.op2_tune = 19;
    p.op2_pm = 70.0;
    p.chord_type = ChordType::Maj;
    p.filters[0] = FilterSlot {
        env: 70.0,
        reso: 12.0,
        ..lp(FilterType::Lp18, 900.0)
    };
    p.crush_rate = 22_050.0;
    v.push(Recipe {
        file: "B02_sublet",
        name: "SUBLET",
        bank: "B",
        description: "Res I with a twelfth above, major",
        archetype: "FM Stab",
        patch: p,
    });

    let mut p = d();
    p.op1_wave = Wave::ResIII;
    p.op2_wave = Wave::Pulse;
    p.op2_tune = 12;
    p.op2_pm = 60.0;
    p.crush_bits = CrushBits::B8;
    p.crush_rate = 12_000.0;
    p.hyper = true;
    v.push(Recipe {
        file: "B03_rent_strike",
        name: "RENT STRIKE",
        bank: "B",
        description: "Pulse carrier, 8-bit at 12 kHz",
        archetype: "Hardcore",
        patch: p,
    });

    let mut p = d();
    p.op1_wave = Wave::Tri;
    p.op2_wave = Wave::Saw;
    p.op2_tune = 0;
    p.op2_pm = 15.0;
    p.op1_pd = 0.0;
    p.op2_pd = 0.0;
    p.chord_type = ChordType::Min9;
    p.chord_spread = 50.0;
    p.filters[0] = FilterSlot {
        reso: 40.0,
        env: 45.0,
        ..lp(FilterType::Lp24, 700.0)
    };
    p.flt_d = 300.0;
    p.amp_d = 500.0;
    p.amp_s = 10.0;
    p.crush_bits = CrushBits::B16;
    p.tape_level = -12.0;
    p.sampler_on = false;
    v.push(Recipe {
        file: "C01_deep_min9",
        name: "DEEP MIN9",
        bank: "C",
        description: "Deep house chord",
        archetype: "Deep Chord",
        patch: p,
    });

    let mut p = d();
    p.op1_wave = Wave::Saw;
    p.op2_wave = Wave::Saw;
    p.op2_tune = 0;
    p.op2_pm = 5.0;
    p.op1_pd = 0.0;
    p.op2_pd = 0.0;
    p.chord_type = ChordType::Min7;
    p.chord_spread = 60.0;
    p.filters[0] = FilterSlot {
        reso: 45.0,
        env: 50.0,
        ..lp(FilterType::Lp24, 500.0)
    };
    p.amp_d = 350.0;
    p.tail_lp = 250.0;
    p.tail_rel = 3000.0;
    p.crush_bits = CrushBits::B16;
    p.crush_rate = 32_000.0;
    v.push(Recipe {
        file: "C02_dub_chord",
        name: "DUB CHORD",
        bank: "C",
        description: "Dub-techno chord with a long filtered tail",
        archetype: "Deep Chord",
        patch: p,
    });

    let mut p = d();
    p.op1_wave = Wave::Square;
    p.op2_wave = Wave::Sine;
    p.op2_tune = 12;
    p.op2_pm = 25.0;
    p.op1_pd = 0.0;
    p.op2_pd = 0.0;
    p.chord_type = ChordType::Sus4;
    p.chord_sub2 = false;
    p.filters[0] = FilterSlot {
        env: 45.0,
        ..lp(FilterType::Lp18, 1100.0)
    };
    p.amp_d = 240.0;
    v.push(Recipe {
        file: "C03_factory_sus",
        name: "FACTORY SUS",
        bank: "C",
        description: "Tech house sus stab",
        archetype: "Deep Chord",
        patch: p,
    });

    let mut p = d();
    p.op1_wave = Wave::ResIII;
    p.op2_wave = Wave::Saw;
    p.op2_tune = 12;
    p.op2_pm = 52.0;
    p.smp_transpose = -12;
    p.crush_bits = CrushBits::B8;
    p.crush_rate = 16_000.0;
    p.comp_thresh = -16.0;
    p.hyper = true;
    v.push(Recipe {
        file: "D01_jungle_min",
        name: "JUNGLE MIN",
        bank: "D",
        description: "Pitched-down jungle stab",
        archetype: "Hardcore",
        patch: p,
    });

    let mut p = d();
    p.op1_wave = Wave::Saw;
    p.op2_wave = Wave::Saw;
    p.op2_tune = 0;
    p.op2_pm = 0.0;
    p.op1_pd = 0.0;
    p.op2_pd = 0.0;
    p.chord_detune = 25.0;
    p.chord_type = ChordType::Fifth;
    p.filters[0] = FilterSlot {
        env: 30.0,
        ..lp(FilterType::Lp24, 1800.0)
    };
    p.crush_rate = 22_050.0;
    p.tape_level = -8.0;
    p.sampler_on = false;
    v.push(Recipe {
        file: "D02_hoover_ish",
        name: "HOOVER-ISH",
        bank: "D",
        description: "Hoover-flavoured detuned fifths",
        archetype: "Hardcore",
        patch: p,
    });

    v
}

pub fn to_preset(r: &Recipe) -> Preset {
    Preset {
        name: r.name.into(),
        bank: r.bank.into(),
        category: bank_name(r.bank).into(),
        author: "TENANT".into(),
        description: r.description.into(),
        patch: r.patch,
        seed: None,
        archetype: Some(r.archetype.into()),
        path: None,
    }
}
