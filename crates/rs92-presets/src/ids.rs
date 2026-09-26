//! The one table that maps parameter IDs to [`Patch`] fields.
//!
//! Used by the preset files (JSON), by the plugin to push presets through its parameter
//! setter, and by the randomizer. Enum values are exchanged as their index (plain value
//! of an nih-plug `EnumParam`); booleans as 0/1.

use rs92_dsp::*;

/// How a parameter is represented in preset files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Float,
    Int,
    Bool,
    /// Enum with the given variant IDs, in parameter order.
    Enum(&'static [&'static str]),
}

/// Randomizer section a parameter belongs to (the panel's 01–06 tabs, 0 = hidden).
pub type Section = u8;

pub struct ParamDef {
    pub id: &'static str,
    pub kind: Kind,
    pub min: f32,
    pub max: f32,
    pub section: Section,
    pub get: fn(&Patch) -> f32,
    pub set: fn(&mut Patch, f32),
}

// `id()` is not const; mirror the variant IDs here in the same order.
const WAVE_IDS: &[&str] = &[
    "Sine", "Tri", "Saw", "Square", "Pulse", "ResI", "ResII", "ResIII", "Organ", "Bell",
];
const CHORD_IDS: &[&str] = &[
    "Min", "Maj", "Min7", "Min9", "Sus4", "5th", "Custom1", "Custom2", "Custom3", "Custom4",
    "Single",
];
const FILTER_IDS: &[&str] = &["LP12", "LP18", "LP24", "HP12", "HP24", "BP12", "Off"];
const BITS_IDS: &[&str] = &["8", "12", "16", "Off"];
const OS_IDS: &[&str] = &["2x", "4x"];
const INTERP_IDS: &[&str] = &["Vintage", "Clean"];

macro_rules! f {
    ($id:literal, $field:ident, $min:expr, $max:expr, $sec:expr) => {
        ParamDef {
            id: $id,
            kind: Kind::Float,
            min: $min,
            max: $max,
            section: $sec,
            get: |p| p.$field,
            set: |p, v| p.$field = v,
        }
    };
}

macro_rules! i {
    ($id:literal, $field:ident, $min:expr, $max:expr, $sec:expr, $ty:ty) => {
        ParamDef {
            id: $id,
            kind: Kind::Int,
            min: $min as f32,
            max: $max as f32,
            section: $sec,
            get: |p| p.$field as f32,
            set: |p, v| p.$field = v.round() as $ty,
        }
    };
}

macro_rules! b {
    ($id:literal, $field:ident, $sec:expr) => {
        ParamDef {
            id: $id,
            kind: Kind::Bool,
            min: 0.0,
            max: 1.0,
            section: $sec,
            get: |p| p.$field as u8 as f32,
            set: |p, v| p.$field = v >= 0.5,
        }
    };
}

macro_rules! e {
    ($id:literal, $field:ident, $ty:ty, $names:expr, $sec:expr) => {
        ParamDef {
            id: $id,
            kind: Kind::Enum($names),
            min: 0.0,
            max: ($names.len() - 1) as f32,
            section: $sec,
            get: |p| p.$field.index() as f32,
            set: |p, v| p.$field = <$ty>::from_index(v.round().max(0.0) as usize),
        }
    };
}

macro_rules! flt {
    ($n:literal, $slot:literal) => {
        [
            ParamDef {
                id: concat!("f", $n, "_type"),
                kind: Kind::Enum(FILTER_IDS),
                min: 0.0,
                max: 6.0,
                section: 3,
                get: |p| p.filters[$slot].ty.index() as f32,
                set: |p, v| {
                    p.filters[$slot].ty = FilterType::from_index(v.round().max(0.0) as usize)
                },
            },
            ParamDef {
                id: concat!("f", $n, "_cutoff"),
                kind: Kind::Float,
                min: 20.0,
                max: 20_000.0,
                section: 3,
                get: |p| p.filters[$slot].cutoff,
                set: |p, v| p.filters[$slot].cutoff = v,
            },
            ParamDef {
                id: concat!("f", $n, "_reso"),
                kind: Kind::Float,
                min: 0.0,
                max: 100.0,
                section: 3,
                get: |p| p.filters[$slot].reso,
                set: |p, v| p.filters[$slot].reso = v,
            },
            ParamDef {
                id: concat!("f", $n, "_env"),
                kind: Kind::Float,
                min: -100.0,
                max: 100.0,
                section: 3,
                get: |p| p.filters[$slot].env,
                set: |p, v| p.filters[$slot].env = v,
            },
            ParamDef {
                id: concat!("f", $n, "_key"),
                kind: Kind::Float,
                min: 0.0,
                max: 100.0,
                section: 3,
                get: |p| p.filters[$slot].key,
                set: |p, v| p.filters[$slot].key = v,
            },
        ]
    };
}

/// Every parameter, in panel order.
pub fn all() -> &'static [ParamDef] {
    use std::sync::LazyLock;
    static ALL: LazyLock<Vec<ParamDef>> = LazyLock::new(|| {
        let mut v = vec![
            e!("op1_wave", op1_wave, Wave, WAVE_IDS, 1),
            f!("op1_pd", op1_pd, -100.0, 100.0, 1),
            f!("op1_level", op1_level, 0.0, 100.0, 1),
            f!("op1_fdbk", op1_fdbk, 0.0, 100.0, 1),
            e!("op2_wave", op2_wave, Wave, WAVE_IDS, 1),
            i!("op2_tune", op2_tune, -24, 24, 1, i32),
            f!("op2_pd", op2_pd, -100.0, 100.0, 1),
            f!("op2_pm", op2_pm, 0.0, 100.0, 1),
            e!("chord_type", chord_type, ChordType, CHORD_IDS, 2),
            b!("chord_sub1", chord_sub1, 2),
            b!("chord_sub2", chord_sub2, 2),
            f!("chord_spread", chord_spread, 0.0, 100.0, 2),
            f!("chord_strum", chord_strum, 0.0, 60.0, 2),
            f!("chord_detune", chord_detune, 0.0, 25.0, 2),
            f!("vel_tone", vel_tone, 0.0, 100.0, 2),
        ];
        v.extend(flt!("1", 0));
        v.extend(flt!("2", 1));
        v.extend(flt!("3", 2));
        v.extend([
            f!("amp_a", amp_a, 0.0, 5000.0, 4),
            f!("amp_d", amp_d, 0.0, 5000.0, 4),
            f!("amp_s", amp_s, 0.0, 100.0, 4),
            f!("amp_r", amp_r, 0.0, 5000.0, 4),
            f!("flt_a", flt_a, 0.0, 5000.0, 4),
            f!("flt_d", flt_d, 0.0, 5000.0, 4),
            f!("flt_s", flt_s, 0.0, 100.0, 4),
            f!("flt_r", flt_r, 0.0, 5000.0, 4),
            e!("crush_bits", crush_bits, CrushBits, BITS_IDS, 5),
            f!("crush_rate", crush_rate, 4000.0, 48_000.0, 5),
            f!("crush_mix", crush_mix, 0.0, 100.0, 5),
            b!("sampler_on", sampler_on, 5),
            f!("tail_lp", tail_lp, 40.0, 20_000.0, 5),
            f!("tail_rel", tail_rel, 10.0, 5000.0, 5),
            i!("smp_transpose", smp_transpose, -24, 24, 5, i32),
            f!("comp_thresh", comp_thresh, -30.0, 0.0, 6),
            f!("tape_level", tape_level, -24.0, -6.0, 6),
            b!("hyper", hyper, 6),
            f!("width", width, 0.0, 200.0, 6),
            f!("output", output, -60.0, 6.0, 6),
            i!("voices", voices, 1, 16, 0, u32),
            f!("bend_range", bend_range, 0.0, 12.0, 0),
            f!("vel_amp", vel_amp, 0.0, 100.0, 0),
            e!("os_factor", os_factor, OsFactor, OS_IDS, 0),
            i!("smp_root", smp_root, 36, 84, 0, u8),
            f!("smp_gate", smp_gate, 40.0, 500.0, 0),
            e!("smp_interp", smp_interp, Interp, INTERP_IDS, 0),
        ]);
        v
    });
    &ALL
}

pub fn find(id: &str) -> Option<&'static ParamDef> {
    all().iter().find(|d| d.id == id)
}

/// Plain values of every parameter of a patch, clamped to range.
pub fn plain_values(p: &Patch) -> Vec<(&'static str, f32)> {
    all()
        .iter()
        .map(|d| (d.id, (d.get)(p).clamp(d.min, d.max)))
        .collect()
}

/// Sets one parameter by ID. Unknown IDs are ignored; values are clamped.
pub fn set_plain(p: &mut Patch, id: &str, v: f32) -> bool {
    match find(id) {
        Some(d) if v.is_finite() => {
            (d.set)(p, v.clamp(d.min, d.max));
            true
        }
        _ => false,
    }
}
