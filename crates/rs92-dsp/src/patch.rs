//! Plain-value patch description shared by the engine, presets and the plugin.
//!
//! Every field is stored in panel units (percent, ms, Hz, dB, semitones), matching the
//! parameter table in the design document. The plugin converts its parameters into a
//! [`Patch`] once per sub-block; the preset crate serialises it by parameter ID.

use std::hash::{Hash, Hasher};

macro_rules! simple_enum {
    ($(#[$m:meta])* $name:ident { $($variant:ident = $id:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
        #[repr(u8)]
        pub enum $name { #[default] $($variant),+ }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            /// Stable identifier used in preset files.
            pub fn id(self) -> &'static str {
                match self { $($name::$variant => $id),+ }
            }

            pub fn from_id(id: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|v| v.id().eq_ignore_ascii_case(id))
            }

            pub fn index(self) -> usize {
                self as usize
            }

            pub fn from_index(i: usize) -> Self {
                Self::ALL[i.min(Self::ALL.len() - 1)]
            }
        }
    };
}

simple_enum!(
    /// Operator waveform.
    Wave {
        Sine = "Sine",
        Tri = "Tri",
        Saw = "Saw",
        Square = "Square",
        Pulse = "Pulse",
        ResI = "ResI",
        ResII = "ResII",
        ResIII = "ResIII",
        Organ = "Organ",
        Bell = "Bell",
    }
);

impl Wave {
    pub fn is_res(self) -> bool {
        matches!(self, Wave::ResI | Wave::ResII | Wave::ResIII)
    }
}

simple_enum!(
    /// Chord memory type.
    ChordType {
        Min = "Min",
        Maj = "Maj",
        Min7 = "Min7",
        Min9 = "Min9",
        Sus4 = "Sus4",
        Fifth = "5th",
        Custom1 = "Custom1",
        Custom2 = "Custom2",
        Custom3 = "Custom3",
        Custom4 = "Custom4",
        Single = "Single",
    }
);

simple_enum!(
    /// Filter slot type.
    FilterType {
        Lp12 = "LP12",
        Lp18 = "LP18",
        Lp24 = "LP24",
        Hp12 = "HP12",
        Hp24 = "HP24",
        Bp12 = "BP12",
        Off = "Off",
    }
);

simple_enum!(
    /// Crusher bit depth.
    CrushBits {
        B8 = "8",
        B12 = "12",
        B16 = "16",
        Off = "Off",
    }
);

impl CrushBits {
    pub fn bits(self) -> Option<u32> {
        match self {
            CrushBits::B8 => Some(8),
            CrushBits::B12 => Some(12),
            CrushBits::B16 => Some(16),
            CrushBits::Off => None,
        }
    }
}

simple_enum!(
    /// Oversampling factor for operators and filters.
    OsFactor {
        X2 = "2x",
        X4 = "4x",
    }
);

impl OsFactor {
    pub fn factor(self) -> usize {
        match self {
            OsFactor::X2 => 2,
            OsFactor::X4 => 4,
        }
    }
}

simple_enum!(
    /// Sampler playback interpolation.
    Interp {
        Vintage = "Vintage",
        Clean = "Clean",
    }
);

/// A learned chord: intervals in semitones relative to the lowest note.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct CustomChord {
    pub len: u8,
    pub notes: [u8; 8],
}

impl CustomChord {
    pub fn from_notes(notes: &[u8]) -> Self {
        let mut sorted: Vec<u8> = notes.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        sorted.truncate(8);
        let mut out = CustomChord::default();
        if let Some(&lowest) = sorted.first() {
            for (i, n) in sorted.iter().enumerate() {
                out.notes[i] = n - lowest;
            }
            out.len = sorted.len() as u8;
        }
        out
    }

    pub fn intervals(&self) -> &[u8] {
        &self.notes[..self.len as usize]
    }
}

/// Settings of one filter slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FilterSlot {
    pub ty: FilterType,
    /// Hz.
    pub cutoff: f32,
    /// 0..100 %.
    pub reso: f32,
    /// -100..100 %.
    pub env: f32,
    /// 0..100 %.
    pub key: f32,
}

/// The complete sound-defining state of the instrument, in panel units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Patch {
    pub op1_wave: Wave,
    pub op1_pd: f32,
    pub op1_level: f32,
    pub op1_fdbk: f32,
    pub op2_wave: Wave,
    pub op2_tune: i32,
    pub op2_pd: f32,
    pub op2_pm: f32,

    pub chord_type: ChordType,
    pub chord_sub1: bool,
    pub chord_sub2: bool,
    pub chord_spread: f32,
    pub chord_strum: f32,
    pub chord_detune: f32,
    pub vel_tone: f32,

    pub filters: [FilterSlot; 3],

    pub amp_a: f32,
    pub amp_d: f32,
    pub amp_s: f32,
    pub amp_r: f32,
    pub flt_a: f32,
    pub flt_d: f32,
    pub flt_s: f32,
    pub flt_r: f32,

    pub crush_bits: CrushBits,
    /// Hz.
    pub crush_rate: f32,
    pub crush_mix: f32,

    pub sampler_on: bool,
    pub tail_lp: f32,
    pub tail_rel: f32,
    pub smp_transpose: i32,

    pub comp_thresh: f32,
    pub tape_level: f32,
    pub hyper: bool,
    pub width: f32,
    pub output: f32,

    // Hidden parameters.
    pub voices: u32,
    pub bend_range: f32,
    pub vel_amp: f32,
    pub os_factor: OsFactor,
    pub smp_root: u8,
    pub smp_gate: f32,
    pub smp_interp: Interp,

    /// Learned chords (persisted state, not parameters).
    pub custom_chords: [CustomChord; 4],
}

impl Default for Patch {
    /// The `LANDLORD '91` patch.
    fn default() -> Self {
        Patch {
            op1_wave: Wave::ResIII,
            op1_pd: 26.0,
            op1_level: 58.0,
            op1_fdbk: 0.0,
            op2_wave: Wave::Saw,
            op2_tune: 12,
            op2_pd: -100.0,
            op2_pm: 48.0,

            chord_type: ChordType::Min,
            chord_sub1: true,
            chord_sub2: true,
            chord_spread: 18.0,
            chord_strum: 0.0,
            chord_detune: 6.0,
            vel_tone: 40.0,

            filters: [
                FilterSlot {
                    ty: FilterType::Lp18,
                    cutoff: 640.0,
                    reso: 12.0,
                    env: 62.0,
                    key: 100.0,
                },
                FilterSlot {
                    ty: FilterType::Hp12,
                    cutoff: 110.0,
                    reso: 0.0,
                    env: -20.0,
                    key: 0.0,
                },
                FilterSlot {
                    ty: FilterType::Lp24,
                    cutoff: 12_400.0,
                    reso: 0.0,
                    env: 0.0,
                    key: 0.0,
                },
            ],

            amp_a: 0.0,
            amp_d: 380.0,
            amp_s: 0.0,
            amp_r: 120.0,
            flt_a: 0.0,
            flt_d: 210.0,
            flt_s: 12.0,
            flt_r: 90.0,

            crush_bits: CrushBits::B12,
            crush_rate: 26_000.0,
            crush_mix: 35.0,

            sampler_on: true,
            tail_lp: 411.0,
            tail_rel: 1500.0,
            smp_transpose: 0,

            comp_thresh: -9.0,
            tape_level: -18.0,
            hyper: true,
            width: 110.0,
            output: -3.0,

            voices: 8,
            bend_range: 2.0,
            vel_amp: 40.0,
            os_factor: OsFactor::X2,
            smp_root: 64,
            smp_gate: 120.0,
            smp_interp: Interp::Vintage,

            custom_chords: [CustomChord::default(); 4],
        }
    }
}

/// Quantizes a float for hashing so that tiny automation jitter still hits the cache.
fn q(v: f32) -> i64 {
    (v as f64 * 1000.0).round() as i64
}

impl Patch {
    /// Hash of every parameter that changes the baked sample, plus the host sample rate.
    ///
    /// `crush_mix` is marked as a bake parameter in the spec, but it only selects the
    /// playback crossfade between the two stored buffers, so it is left out of the key:
    /// changing it re-requests a bake that is always a cache hit.
    pub fn bake_key(&self, sample_rate: f32) -> u64 {
        // Fixed seeds: deterministic keys, and no lazily allocated random state on the
        // audio thread.
        use std::hash::BuildHasher;
        let mut h =
            ahash::RandomState::with_seeds(0x5253_3932, 0x7465_6e61, 0x6e74_2031, 0x3939_3120)
                .build_hasher();
        q(sample_rate).hash(&mut h);
        self.op1_wave.hash(&mut h);
        q(self.op1_pd).hash(&mut h);
        q(self.op1_level).hash(&mut h);
        q(self.op1_fdbk).hash(&mut h);
        self.op2_wave.hash(&mut h);
        self.op2_tune.hash(&mut h);
        q(self.op2_pd).hash(&mut h);
        q(self.op2_pm).hash(&mut h);
        self.chord_type.hash(&mut h);
        self.chord_sub1.hash(&mut h);
        self.chord_sub2.hash(&mut h);
        q(self.chord_spread).hash(&mut h);
        q(self.chord_strum).hash(&mut h);
        q(self.chord_detune).hash(&mut h);
        for f in &self.filters {
            f.ty.hash(&mut h);
            q(f.cutoff).hash(&mut h);
            q(f.reso).hash(&mut h);
            q(f.env).hash(&mut h);
            q(f.key).hash(&mut h);
        }
        for v in [
            self.amp_a,
            self.amp_d,
            self.amp_s,
            self.amp_r,
            self.flt_a,
            self.flt_d,
            self.flt_s,
            self.flt_r,
            self.crush_rate,
            self.comp_thresh,
            self.tape_level,
            self.smp_gate,
        ] {
            q(v).hash(&mut h);
        }
        self.crush_bits.hash(&mut h);
        self.hyper.hash(&mut h);
        self.os_factor.hash(&mut h);
        self.smp_root.hash(&mut h);
        if let Some(i) = self.custom_index() {
            self.custom_chords[i].hash(&mut h);
        }
        h.finish()
    }

    pub fn custom_index(&self) -> Option<usize> {
        match self.chord_type {
            ChordType::Custom1 => Some(0),
            ChordType::Custom2 => Some(1),
            ChordType::Custom3 => Some(2),
            ChordType::Custom4 => Some(3),
            _ => None,
        }
    }
}
