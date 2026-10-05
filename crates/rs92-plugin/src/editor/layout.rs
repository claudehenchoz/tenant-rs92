//! Every position on the 912 × 508 faceplate, in design points.
//!
//! The section panels and the VFD bezel are also baked into the plate bitmap by
//! `scripts/gen_assets.mjs`; keep `PANELS` and `BEZEL` in sync with its `panels` / `bezel`.

use nih_plug_egui::egui::{Pos2, Rect};

pub const W: f32 = 912.0;
pub const H: f32 = 508.0;

pub const fn rr(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect {
        min: Pos2 { x, y },
        max: Pos2 { x: x + w, y: y + h },
    }
}

pub const fn center(r: Rect) -> Pos2 {
    Pos2 {
        x: (r.min.x + r.max.x) * 0.5,
        y: (r.min.y + r.max.y) * 0.5,
    }
}

// -----------------------------------------------------------------------------------------
// Header
// -----------------------------------------------------------------------------------------

/// Brand plate; right-click opens the setup menu.
pub const BRAND: Rect = rr(10.0, 8.0, 182.0, 80.0);
pub const BEZEL: Rect = rr(200.0, 8.0, 476.0, 80.0);
pub const VFD: Rect = rr(206.0, 14.0, 464.0, 68.0);
/// Preset step arrows beside the VFD.
pub const PROG_UP: Rect = rr(682.0, 8.0, 24.0, 38.0);
pub const PROG_DOWN: Rect = rr(682.0, 50.0, 24.0, 38.0);
pub const STORE: Rect = rr(714.0, 10.0, 90.0, 24.0);
pub const COMPARE: Rect = rr(812.0, 10.0, 90.0, 24.0);
pub const RANDOM: Rect = rr(714.0, 40.0, 188.0, 24.0);
/// MIDI / CLIP / OS LEDs and CPU; click clears the CLIP latch.
pub const STATUS: Rect = rr(714.0, 68.0, 188.0, 18.0);

/// Inside the main VFD, relative to its top-left.
pub mod vfd {
    use super::{rr, Rect};
    /// Preset name: click opens the browser.
    pub const NAME_HIT: Rect = rr(68.0, 20.0, 250.0, 28.0);
    pub const SCOPE: Rect = rr(332.0, 6.0, 124.0, 36.0);
    /// Meter rows (centre lines).
    pub const METER_X: f32 = 332.0;
    pub const METER_Y: f32 = 50.0;
    /// Browser close button.
    pub const CLOSE: Rect = rr(444.0, 4.0, 16.0, 14.0);
}

// -----------------------------------------------------------------------------------------
// Sections
// -----------------------------------------------------------------------------------------

/// Section panels 01–06.
pub const PANELS: [Rect; 6] = [
    rr(10.0, 96.0, 206.0, 230.0),
    rr(224.0, 96.0, 300.0, 230.0),
    rr(532.0, 96.0, 370.0, 230.0),
    rr(10.0, 334.0, 324.0, 164.0),
    rr(342.0, 334.0, 276.0, 164.0),
    rr(626.0, 334.0, 276.0, 164.0),
];

/// Section tab (01–06, the RANDOM lock) inside each panel.
pub const fn tab(section: usize) -> Rect {
    let p = PANELS[section];
    rr(p.min.x + 10.0, p.min.y + 10.0, 20.0, 15.0)
}

// 01 SOURCE
pub const OP1_WAVE: Rect = rr(50.0, 128.0, 100.0, 20.0);
pub const OP2_WAVE: Rect = rr(50.0, 230.0, 100.0, 20.0);
pub const OP1_KNOBS_Y: f32 = 154.0;
pub const OP2_KNOBS_Y: f32 = 256.0;
pub const OP_KNOB_X: [f32; 3] = [20.0, 82.0, 144.0];
pub const OP_RULE_Y: f32 = 221.0;

// 02 CHORD MEMORY
pub const CHORD_READOUT: Rect = rr(234.0, 128.0, 280.0, 22.0);
pub const fn chord_pill(i: usize) -> Rect {
    rr(234.0 + i as f32 * 47.2, 156.0, 44.0, 22.0)
}
pub const SUB1: Rect = rr(234.0, 256.0, 74.0, 22.0);
pub const SUB2: Rect = rr(234.0, 284.0, 74.0, 22.0);
pub const CHORD_KNOBS: (f32, f32, f32) = (314.0, 256.0, 50.0);

/// On-screen keyboard, C2–B5.
pub mod keys {
    use super::{rr, Pos2, Rect};
    pub const LOW: u8 = 36;
    pub const HIGH: u8 = 83;
    pub const WHITES: usize = 28;
    pub const PITCH: f32 = 9.8;
    pub const INSET: f32 = 3.0;
    pub const WHITE_W: f32 = PITCH - 1.0;
    pub const WHITE_H: f32 = 58.0;
    pub const BLACK_W: f32 = 6.0;
    pub const BLACK_H: f32 = 35.0;
    pub const RECT: Rect = rr(
        234.0,
        184.0,
        WHITES as f32 * PITCH + 2.0 * INSET - 1.0,
        WHITE_H + 2.0 * INSET,
    );

    /// Centre of a white key's lower half (below the black keys), by white-key index.
    pub const fn white_center(i: usize) -> Pos2 {
        Pos2 {
            x: RECT.min.x + INSET + i as f32 * PITCH + WHITE_W * 0.5,
            y: RECT.min.y + INSET + (BLACK_H + WHITE_H) * 0.5,
        }
    }
}

// 03 FILTER CHAIN
pub const RESPONSE: Rect = rr(542.0, 128.0, 350.0, 36.0);
pub const fn filter_x(f: usize) -> f32 {
    542.0 + f as f32 * 120.0
}
pub const fn filter_type(f: usize) -> Rect {
    rr(filter_x(f) + 20.0, 172.0, 86.0, 20.0)
}
pub const FILTER_ROW1_Y: f32 = 198.0;
pub const FILTER_ROW2_Y: f32 = 264.0;

// 04 ENVELOPES
pub const ENV_GRAPH: Rect = rr(20.0, 366.0, 92.0, 122.0);
pub const fn fader_x(group: usize, i: usize) -> f32 {
    122.0 + group as f32 * 108.0 + i as f32 * 24.0
}
pub const FADER_Y: f32 = 382.0;

// 05 RESAMPLE
pub const fn bits_pill(i: usize) -> Rect {
    rr(378.0 + i as f32 * 34.0, 372.0, 30.0, 22.0)
}
pub const SAMPLER_ON: Rect = rr(516.0, 372.0, 92.0, 22.0);
pub const RESAMPLE_KNOBS: (f32, f32, f32) = (352.0, 420.0, 51.5);

// 06 FINISH
pub const VU: Rect = rr(636.0, 368.0, 110.0, 72.0);
pub const HYPER: Rect = rr(636.0, 452.0, 110.0, 26.0);
pub const COMP_KNOB: Pos2 = Pos2 { x: 760.0, y: 368.0 };
pub const TAPE_KNOB: Pos2 = Pos2 { x: 836.0, y: 368.0 };
pub const WIDTH_KNOB: Pos2 = Pos2 { x: 760.0, y: 432.0 };
pub const OUTPUT_KNOB: Pos2 = Pos2 { x: 835.0, y: 428.0 };

/// Aspect-locked resize grip in the bottom-right corner.
pub const GRIP: Rect = rr(896.0, 492.0, 16.0, 16.0);
