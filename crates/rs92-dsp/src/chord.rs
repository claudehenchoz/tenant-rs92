//! Chord memory: builds the partial list of a chord voice.

use crate::patch::{ChordType, Patch};

pub const MAX_PARTIALS: usize = 8;

/// Intervals of the fixed chord types, relative to the played note.
pub fn intervals(ty: ChordType) -> &'static [u8] {
    match ty {
        ChordType::Min => &[0, 3, 7],
        ChordType::Maj => &[0, 4, 7],
        ChordType::Min7 => &[0, 3, 7, 10],
        ChordType::Min9 => &[0, 3, 7, 10, 14],
        ChordType::Sus4 => &[0, 5, 7],
        ChordType::Fifth => &[0, 7],
        ChordType::Single => &[0],
        // Custom chords are looked up in the patch.
        ChordType::Custom1 | ChordType::Custom2 | ChordType::Custom3 | ChordType::Custom4 => &[0],
    }
}

/// The partials of one chord voice, sorted by pitch (lowest first).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChordLayout {
    pub count: usize,
    /// MIDI note numbers (integer part only, before detune).
    pub notes: [i32; MAX_PARTIALS],
    /// Detune offset in cents.
    pub detune_cents: [f32; MAX_PARTIALS],
    pub pan_l: [f32; MAX_PARTIALS],
    pub pan_r: [f32; MAX_PARTIALS],
    /// Start delay in ms (strum).
    pub delay_ms: [f32; MAX_PARTIALS],
}

impl ChordLayout {
    /// Builds the layout for played note `note` from the patch's chord settings.
    pub fn build(note: i32, patch: &Patch) -> Self {
        let mut notes = [0i32; 16];
        let mut n = 0;
        if patch.chord_sub2 {
            notes[n] = note - 24;
            n += 1;
        }
        if patch.chord_sub1 {
            notes[n] = note - 12;
            n += 1;
        }
        let custom;
        let ints: &[u8] = match patch.custom_index() {
            Some(i) if patch.custom_chords[i].len > 0 => {
                custom = patch.custom_chords[i];
                custom.intervals()
            }
            _ => intervals(patch.chord_type),
        };
        for &i in ints {
            if n < notes.len() {
                notes[n] = note + i as i32;
                n += 1;
            }
        }
        let list = &mut notes[..n];
        list.sort_unstable();
        let count = n.min(MAX_PARTIALS);

        let mut out = ChordLayout {
            count,
            ..Default::default()
        };
        out.notes[..count].copy_from_slice(&list[..count]);
        let spread = patch.chord_spread / 100.0;
        for i in 0..count {
            // Lowest partial at 0, then alternating +d, -d, +d ...
            out.detune_cents[i] = if i == 0 {
                0.0
            } else if i % 2 == 1 {
                patch.chord_detune
            } else {
                -patch.chord_detune
            };
            let pos = if count > 1 {
                i as f32 / (count - 1) as f32 * 2.0 - 1.0
            } else {
                0.0
            };
            let theta = (pos * spread + 1.0) * std::f32::consts::FRAC_PI_4;
            out.pan_l[i] = theta.cos();
            out.pan_r[i] = theta.sin();
            out.delay_ms[i] = if count > 1 {
                i as f32 * patch.chord_strum / (count - 1) as f32
            } else {
                0.0
            };
        }
        out
    }

    pub fn notes(&self) -> &[i32] {
        &self.notes[..self.count]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn landlord_e4_plays_e2_e3_e4_g4_b4() {
        let l = ChordLayout::build(64, &Patch::default());
        assert_eq!(l.notes(), &[40, 52, 64, 67, 71]);
    }

    #[test]
    fn every_chord_type() {
        let mut p = Patch {
            chord_sub1: false,
            chord_sub2: false,
            ..Patch::default()
        };
        let cases: &[(ChordType, &[i32])] = &[
            (ChordType::Min, &[60, 63, 67]),
            (ChordType::Maj, &[60, 64, 67]),
            (ChordType::Min7, &[60, 63, 67, 70]),
            (ChordType::Min9, &[60, 63, 67, 70, 74]),
            (ChordType::Sus4, &[60, 65, 67]),
            (ChordType::Fifth, &[60, 67]),
            (ChordType::Single, &[60]),
        ];
        for (ty, expect) in cases {
            p.chord_type = *ty;
            assert_eq!(ChordLayout::build(60, &p).notes(), *expect, "{ty:?}");
        }
        p.chord_type = ChordType::Min9;
        p.chord_sub1 = true;
        p.chord_sub2 = true;
        assert_eq!(ChordLayout::build(60, &p).count, 7);
    }

    #[test]
    fn custom_chord_uses_all_lanes() {
        let mut p = Patch {
            chord_type: ChordType::Custom2,
            ..Patch::default()
        };
        p.custom_chords[1] = crate::patch::CustomChord::from_notes(&[60, 63, 67, 70, 74, 77]);
        let l = ChordLayout::build(60, &p);
        assert_eq!(l.notes(), &[36, 48, 60, 63, 67, 70, 74, 77]);
    }

    #[test]
    fn spread_and_detune() {
        let l = ChordLayout::build(
            64,
            &Patch {
                chord_spread: 100.0,
                ..Patch::default()
            },
        );
        assert!((l.pan_l[0] - 1.0).abs() < 1e-6 && l.pan_r[0].abs() < 1e-6);
        assert!((l.pan_r[4] - 1.0).abs() < 1e-6);
        assert_eq!(&l.detune_cents[..5], &[0.0, 6.0, -6.0, 6.0, -6.0]);
    }
}
