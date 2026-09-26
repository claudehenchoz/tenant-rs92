//! Plugin parameters. The `#[id]` strings are part of the saved-project contract and
//! must never change after the first release.

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use rs92_dsp as dsp;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// Version of the persisted non-parameter state; see `Rs92::filter_state`.
pub const STATE_VERSION: u32 = 1;

#[derive(Enum, Debug, PartialEq, Eq, Clone, Copy)]
pub enum WaveP {
    Sine,
    Tri,
    Saw,
    Square,
    Pulse,
    #[name = "Res I"]
    ResI,
    #[name = "Res II"]
    ResII,
    #[name = "Res III"]
    ResIII,
    Organ,
    Bell,
}

#[derive(Enum, Debug, PartialEq, Eq, Clone, Copy)]
pub enum ChordP {
    Min,
    Maj,
    Min7,
    Min9,
    Sus4,
    #[name = "5th"]
    Fifth,
    #[name = "Custom 1"]
    Custom1,
    #[name = "Custom 2"]
    Custom2,
    #[name = "Custom 3"]
    Custom3,
    #[name = "Custom 4"]
    Custom4,
    Single,
}

#[derive(Enum, Debug, PartialEq, Eq, Clone, Copy)]
pub enum FilterP {
    #[name = "LP12"]
    Lp12,
    #[name = "LP18"]
    Lp18,
    #[name = "LP24"]
    Lp24,
    #[name = "HP12"]
    Hp12,
    #[name = "HP24"]
    Hp24,
    #[name = "BP12"]
    Bp12,
    Off,
}

#[derive(Enum, Debug, PartialEq, Eq, Clone, Copy)]
pub enum BitsP {
    #[name = "8"]
    B8,
    #[name = "12"]
    B12,
    #[name = "16"]
    B16,
    Off,
}

#[derive(Enum, Debug, PartialEq, Eq, Clone, Copy)]
pub enum OsP {
    #[name = "2x"]
    X2,
    #[name = "4x"]
    X4,
}

#[derive(Enum, Debug, PartialEq, Eq, Clone, Copy)]
pub enum InterpP {
    Vintage,
    Clean,
}

/// Maps a plugin-side enum onto the DSP enum with the same variant order.
fn conv<A: Enum, B: Copy>(v: A, all: &[B]) -> B {
    all[v.to_index().min(all.len() - 1)]
}

/// Non-parameter state saved with the project.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ProgramState {
    pub preset_name: String,
    pub bank: String,
    pub index: usize,
    pub random_seed: Option<u64>,
    pub archetype: Option<String>,
}

#[derive(Params)]
pub struct Rs92Params {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,
    #[persist = "state-version"]
    pub state_version: RwLock<u32>,
    /// Learned chords, as MIDI intervals relative to the lowest note.
    #[persist = "custom-chords"]
    pub custom_chords: RwLock<[Vec<u8>; 4]>,
    #[persist = "program"]
    pub program: RwLock<ProgramState>,
    /// Randomizer section locks, bit n = section 0n+1.
    #[persist = "lock-mask"]
    pub lock_mask: RwLock<u8>,

    // 01 Source
    #[id = "op1_wave"]
    pub op1_wave: EnumParam<WaveP>,
    #[id = "op1_pd"]
    pub op1_pd: FloatParam,
    #[id = "op1_level"]
    pub op1_level: FloatParam,
    #[id = "op1_fdbk"]
    pub op1_fdbk: FloatParam,
    #[id = "op2_wave"]
    pub op2_wave: EnumParam<WaveP>,
    #[id = "op2_tune"]
    pub op2_tune: IntParam,
    #[id = "op2_pd"]
    pub op2_pd: FloatParam,
    #[id = "op2_pm"]
    pub op2_pm: FloatParam,

    // 02 Chord memory
    #[id = "chord_type"]
    pub chord_type: EnumParam<ChordP>,
    #[id = "chord_sub1"]
    pub chord_sub1: BoolParam,
    #[id = "chord_sub2"]
    pub chord_sub2: BoolParam,
    #[id = "chord_spread"]
    pub chord_spread: FloatParam,
    #[id = "chord_strum"]
    pub chord_strum: FloatParam,
    #[id = "chord_detune"]
    pub chord_detune: FloatParam,
    #[id = "vel_tone"]
    pub vel_tone: FloatParam,

    // 03 Filter chain
    #[nested(id_prefix = "f1", group = "F1")]
    pub f1: FilterParams,
    #[nested(id_prefix = "f2", group = "F2")]
    pub f2: FilterParams,
    #[nested(id_prefix = "f3", group = "F3")]
    pub f3: FilterParams,

    // 04 Envelopes
    #[id = "amp_a"]
    pub amp_a: FloatParam,
    #[id = "amp_d"]
    pub amp_d: FloatParam,
    #[id = "amp_s"]
    pub amp_s: FloatParam,
    #[id = "amp_r"]
    pub amp_r: FloatParam,
    #[id = "flt_a"]
    pub flt_a: FloatParam,
    #[id = "flt_d"]
    pub flt_d: FloatParam,
    #[id = "flt_s"]
    pub flt_s: FloatParam,
    #[id = "flt_r"]
    pub flt_r: FloatParam,

    // 05 Resample
    #[id = "crush_bits"]
    pub crush_bits: EnumParam<BitsP>,
    #[id = "crush_rate"]
    pub crush_rate: FloatParam,
    #[id = "crush_mix"]
    pub crush_mix: FloatParam,
    #[id = "sampler_on"]
    pub sampler_on: BoolParam,
    #[id = "tail_lp"]
    pub tail_lp: FloatParam,
    #[id = "tail_rel"]
    pub tail_rel: FloatParam,
    #[id = "smp_transpose"]
    pub smp_transpose: IntParam,

    // 06 Finish
    #[id = "comp_thresh"]
    pub comp_thresh: FloatParam,
    #[id = "tape_level"]
    pub tape_level: FloatParam,
    #[id = "hyper"]
    pub hyper: BoolParam,
    #[id = "width"]
    pub width: FloatParam,
    #[id = "output"]
    pub output: FloatParam,

    // Hidden
    #[id = "voices"]
    pub voices: IntParam,
    #[id = "bend_range"]
    pub bend_range: IntParam,
    #[id = "vel_amp"]
    pub vel_amp: FloatParam,
    #[id = "os_factor"]
    pub os_factor: EnumParam<OsP>,
    #[id = "smp_root"]
    pub smp_root: IntParam,
    #[id = "smp_gate"]
    pub smp_gate: FloatParam,
    #[id = "smp_interp"]
    pub smp_interp: EnumParam<InterpP>,
}

#[derive(Params)]
pub struct FilterParams {
    #[id = "type"]
    pub ty: EnumParam<FilterP>,
    #[id = "cutoff"]
    pub cutoff: FloatParam,
    #[id = "reso"]
    pub reso: FloatParam,
    #[id = "env"]
    pub env: FloatParam,
    #[id = "key"]
    pub key: FloatParam,
}

fn pct(name: &str, default: f32) -> FloatParam {
    FloatParam::new(
        name,
        default,
        FloatRange::Linear {
            min: 0.0,
            max: 100.0,
        },
    )
    .with_unit(" %")
    .with_step_size(0.1)
    .with_value_to_string(formatters_v2s(0))
}

fn bipolar(name: &str, default: f32) -> FloatParam {
    FloatParam::new(
        name,
        default,
        FloatRange::Linear {
            min: -100.0,
            max: 100.0,
        },
    )
    .with_step_size(0.1)
    .with_value_to_string(Arc::new(signed))
}

/// "+26", "-100", and a bare "0".
fn signed(v: f32) -> String {
    let r = v.round();
    if r == 0.0 {
        "0".into()
    } else {
        format!("{r:+.0}")
    }
}

fn formatters_v2s(digits: usize) -> Arc<dyn Fn(f32) -> String + Send + Sync> {
    Arc::new(move |v| format!("{v:.digits$}"))
}

/// Log-like frequency range: the skew puts the geometric mean at the knob centre.
fn freq(name: &str, default: f32, min: f32, max: f32) -> FloatParam {
    let factor = 0.5f32.ln() / (((min * max).sqrt() - min) / (max - min)).ln();
    FloatParam::new(name, default, FloatRange::Skewed { min, max, factor })
        .with_value_to_string(Arc::new(|v| {
            if v >= 1000.0 {
                format!("{:.1}k", v / 1000.0)
            } else {
                format!("{v:.0} Hz")
            }
        }))
        .with_string_to_value(Arc::new(|s| {
            let s = s
                .trim()
                .trim_end_matches("Hz")
                .trim_end_matches("hz")
                .trim();
            if let Some(k) = s.strip_suffix(['k', 'K']) {
                k.trim().parse::<f32>().ok().map(|v| v * 1000.0)
            } else {
                s.parse().ok()
            }
        }))
}

fn ms(name: &str, default: f32, min: f32, max: f32) -> FloatParam {
    FloatParam::new(
        name,
        default,
        FloatRange::Skewed {
            min,
            max,
            factor: FloatRange::skew_factor(-2.0),
        },
    )
    .with_step_size(0.1)
    .with_value_to_string(Arc::new(|v| {
        if v >= 1000.0 {
            format!("{:.2} s", v / 1000.0)
        } else {
            format!("{v:.0} ms")
        }
    }))
    .with_string_to_value(Arc::new(|s| {
        let s = s.trim();
        if let Some(sec) = s.strip_suffix('s').filter(|x| !x.ends_with('m')) {
            sec.trim().parse::<f32>().ok().map(|v| v * 1000.0)
        } else {
            s.trim_end_matches("ms").trim().parse().ok()
        }
    }))
}

fn db(name: &str, default: f32, min: f32, max: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min, max })
        .with_unit(" dB")
        .with_step_size(0.1)
        .with_value_to_string(Arc::new(|v| format!("{v:.1}")))
}

impl FilterParams {
    fn new(n: usize, ty: FilterP, cutoff: f32, reso: f32, env: f32, key: f32) -> Self {
        FilterParams {
            ty: EnumParam::new(format!("F{n} Type"), ty),
            cutoff: freq(&format!("F{n} Cutoff"), cutoff, 20.0, 20_000.0),
            reso: pct(&format!("F{n} Reso"), reso),
            env: bipolar(&format!("F{n} Env"), env).with_unit(" %"),
            key: pct(&format!("F{n} Key"), key),
        }
    }
}

impl Default for Rs92Params {
    fn default() -> Self {
        Rs92Params {
            editor_state: crate::editor::default_state(),
            state_version: RwLock::new(STATE_VERSION),
            custom_chords: RwLock::new(Default::default()),
            program: RwLock::new(ProgramState {
                preset_name: "LANDLORD '91".into(),
                bank: "A".into(),
                index: 0,
                random_seed: None,
                archetype: None,
            }),
            lock_mask: RwLock::new(0),

            op1_wave: EnumParam::new("OP1 Wave", WaveP::ResIII),
            op1_pd: bipolar("OP1 PD", 26.0),
            op1_level: pct("OP1 Level", 58.0),
            op1_fdbk: pct("OP1 Fdbk", 0.0),
            op2_wave: EnumParam::new("OP2 Wave", WaveP::Saw),
            op2_tune: IntParam::new("OP2 Tune", 12, IntRange::Linear { min: -24, max: 24 })
                .with_unit(" st")
                .with_value_to_string(Arc::new(|v| signed(v as f32))),
            op2_pd: bipolar("OP2 PD", -100.0),
            op2_pm: pct("OP2 PM", 48.0),

            chord_type: EnumParam::new("Chord", ChordP::Min),
            chord_sub1: BoolParam::new("Sub -1 Oct", true),
            chord_sub2: BoolParam::new("Sub -2 Oct", true),
            chord_spread: pct("Spread", 18.0),
            chord_strum: FloatParam::new(
                "Strum",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 60.0,
                },
            )
            .with_unit(" ms")
            .with_step_size(0.1)
            .with_value_to_string(formatters_v2s(0)),
            chord_detune: FloatParam::new(
                "Detune",
                6.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 25.0,
                },
            )
            .with_unit(" ct")
            .with_step_size(0.1)
            .with_value_to_string(formatters_v2s(0)),
            vel_tone: pct("Vel>Tone", 40.0),

            f1: FilterParams::new(1, FilterP::Lp18, 640.0, 12.0, 62.0, 100.0),
            f2: FilterParams::new(2, FilterP::Hp12, 110.0, 0.0, -20.0, 0.0),
            f3: FilterParams::new(3, FilterP::Lp24, 12_400.0, 0.0, 0.0, 0.0),

            amp_a: ms("Amp A", 0.0, 0.0, 5000.0),
            amp_d: ms("Amp D", 380.0, 0.0, 5000.0),
            amp_s: pct("Amp S", 0.0),
            amp_r: ms("Amp R", 120.0, 0.0, 5000.0),
            flt_a: ms("Filter A", 0.0, 0.0, 5000.0),
            flt_d: ms("Filter D", 210.0, 0.0, 5000.0),
            flt_s: pct("Filter S", 12.0),
            flt_r: ms("Filter R", 90.0, 0.0, 5000.0),

            crush_bits: EnumParam::new("Bits", BitsP::B12),
            crush_rate: freq("Rate", 26_000.0, 4000.0, 48_000.0),
            crush_mix: pct("Crush Mix", 35.0),
            sampler_on: BoolParam::new("Sampler Pitch", true),
            tail_lp: freq("Tail LP", 411.0, 40.0, 20_000.0),
            tail_rel: ms("Tail Rel", 1500.0, 10.0, 5000.0),
            smp_transpose: IntParam::new("Transpose", 0, IntRange::Linear { min: -24, max: 24 })
                .with_unit(" st")
                .with_value_to_string(Arc::new(|v| signed(v as f32))),

            comp_thresh: db("Comp", -9.0, -30.0, 0.0),
            tape_level: db("Tape", -18.0, -24.0, -6.0).with_unit(" dBFS"),
            hyper: BoolParam::new("Hyper Stab", true),
            width: FloatParam::new(
                "Width",
                110.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 200.0,
                },
            )
            .with_unit(" %")
            .with_step_size(0.1)
            .with_value_to_string(formatters_v2s(0)),
            output: db("Output", -3.0, -60.0, 6.0),

            voices: IntParam::new("Voices", 8, IntRange::Linear { min: 1, max: 16 }),
            bend_range: IntParam::new("Bend Range", 2, IntRange::Linear { min: 0, max: 12 })
                .with_unit(" st"),
            vel_amp: pct("Vel>Amp", 40.0),
            os_factor: EnumParam::new("Oversampling", OsP::X2),
            smp_root: IntParam::new("Sample Root", 64, IntRange::Linear { min: 36, max: 84 })
                .with_value_to_string(Arc::new(|v| note_name(v as u8)))
                .with_string_to_value(Arc::new(parse_note)),
            smp_gate: FloatParam::new(
                "Sample Gate",
                120.0,
                FloatRange::Linear {
                    min: 40.0,
                    max: 500.0,
                },
            )
            .with_unit(" ms")
            .with_step_size(1.0),
            smp_interp: EnumParam::new("Sample Interp", InterpP::Vintage),
        }
    }
}

/// Parses "E4", "C#2", "Db3" or a MIDI number.
pub fn parse_note(s: &str) -> Option<i32> {
    let s = s.trim();
    if let Ok(n) = s.parse::<i32>() {
        return Some(n);
    }
    let mut chars = s.chars();
    let base = match chars.next()?.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let rest: String = chars.collect();
    let (acc, oct) = if let Some(r) = rest.strip_prefix('#') {
        (1, r)
    } else if let Some(r) = rest.strip_prefix('b') {
        (-1, r)
    } else {
        (0, rest.as_str())
    };
    let octave: i32 = oct.trim().parse().ok()?;
    Some((octave + 1) * 12 + base + acc)
}

pub fn note_name(n: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!("{}{}", NAMES[n as usize % 12], n as i32 / 12 - 1)
}

impl Rs92Params {
    pub fn filters(&self) -> [&FilterParams; 3] {
        [&self.f1, &self.f2, &self.f3]
    }

    /// Snapshot of the current (unsmoothed) parameter values as a DSP patch.
    /// Real-time safe as long as the custom-chord lock is uncontended; falls back to the
    /// previous chords if it is.
    pub fn to_patch(&self, prev: &dsp::Patch) -> dsp::Patch {
        let mut p = dsp::Patch {
            op1_wave: conv(self.op1_wave.value(), dsp::Wave::ALL),
            op1_pd: self.op1_pd.value(),
            op1_level: self.op1_level.value(),
            op1_fdbk: self.op1_fdbk.value(),
            op2_wave: conv(self.op2_wave.value(), dsp::Wave::ALL),
            op2_tune: self.op2_tune.value(),
            op2_pd: self.op2_pd.value(),
            op2_pm: self.op2_pm.value(),
            chord_type: conv(self.chord_type.value(), dsp::ChordType::ALL),
            chord_sub1: self.chord_sub1.value(),
            chord_sub2: self.chord_sub2.value(),
            chord_spread: self.chord_spread.value(),
            chord_strum: self.chord_strum.value(),
            chord_detune: self.chord_detune.value(),
            vel_tone: self.vel_tone.value(),
            filters: [0, 1, 2].map(|i| {
                let f = self.filters()[i];
                dsp::FilterSlot {
                    ty: conv(f.ty.value(), dsp::FilterType::ALL),
                    cutoff: f.cutoff.value(),
                    reso: f.reso.value(),
                    env: f.env.value(),
                    key: f.key.value(),
                }
            }),
            amp_a: self.amp_a.value(),
            amp_d: self.amp_d.value(),
            amp_s: self.amp_s.value(),
            amp_r: self.amp_r.value(),
            flt_a: self.flt_a.value(),
            flt_d: self.flt_d.value(),
            flt_s: self.flt_s.value(),
            flt_r: self.flt_r.value(),
            crush_bits: conv(self.crush_bits.value(), dsp::CrushBits::ALL),
            crush_rate: self.crush_rate.value(),
            crush_mix: self.crush_mix.value(),
            sampler_on: self.sampler_on.value(),
            tail_lp: self.tail_lp.value(),
            tail_rel: self.tail_rel.value(),
            smp_transpose: self.smp_transpose.value(),
            comp_thresh: self.comp_thresh.value(),
            tape_level: self.tape_level.value(),
            hyper: self.hyper.value(),
            width: self.width.value(),
            output: self.output.value(),
            voices: self.voices.value() as u32,
            bend_range: self.bend_range.value() as f32,
            vel_amp: self.vel_amp.value(),
            os_factor: conv(self.os_factor.value(), dsp::OsFactor::ALL),
            smp_root: self.smp_root.value() as u8,
            smp_gate: self.smp_gate.value(),
            smp_interp: conv(self.smp_interp.value(), dsp::Interp::ALL),
            custom_chords: prev.custom_chords,
        };
        if let Ok(chords) = self.custom_chords.try_read() {
            for (dst, src) in p.custom_chords.iter_mut().zip(chords.iter()) {
                // Stored as intervals; `from_notes` would allocate, so copy directly.
                let mut c = dsp::CustomChord::default();
                for (i, &n) in src.iter().take(8).enumerate() {
                    c.notes[i] = n;
                    c.len = i as u8 + 1;
                }
                *dst = c;
            }
        }
        p
    }
}
