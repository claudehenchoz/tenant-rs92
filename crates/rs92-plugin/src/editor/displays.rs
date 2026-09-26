//! Faceplate, VFDs, graphs, meters and footer.

use super::theme::{self, alpha, hex, r, F, T};
use super::widgets::{self, step_index};
use super::{browser, program, Cx, VfdMode, PEEK_HOLD};
use nih_plug_egui::egui::{self, Color32, CursorIcon, Id, Key, Painter, Pos2, Rect, Sense, Ui};
use rs92_dsp::chord::ChordLayout;
use rs92_dsp::filter::magnitude;
use rs92_dsp::{ChordType, CrushBits, Interp};
use std::sync::atomic::Ordering;
use std::time::Instant;

pub fn note_name(n: i32) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!(
        "{}{}",
        NAMES[n.rem_euclid(12) as usize],
        n.div_euclid(12) - 1
    )
}

pub fn chord_label(t: ChordType) -> &'static str {
    match t {
        ChordType::Min => "MIN",
        ChordType::Maj => "MAJ",
        ChordType::Min7 => "MIN7",
        ChordType::Min9 => "MIN9",
        ChordType::Sus4 => "SUS4",
        ChordType::Fifth => "5TH",
        ChordType::Custom1 => "CUST 1",
        ChordType::Custom2 => "CUST 2",
        ChordType::Custom3 => "CUST 3",
        ChordType::Custom4 => "CUST 4",
        ChordType::Single => "SINGLE",
    }
}

/// Engraved rule: dark line with a light line under it.
fn rule(p: &Painter, th: &theme::Theme, x0: f32, x1: f32, y: f32) {
    theme::hline(p, x0, x1, y + 0.5, th.groove_dark, 1.0);
    theme::hline(p, x0, x1, y + 1.5, th.groove_light, 1.0);
}

/// (title, subtitle, right edge) per section.
const SECTIONS: [(&str, &str, f32); 6] = [
    ("SOURCE", "2-OP PHASE MOD", 362.0),
    ("CHORD MEMORY", "ONE KEY, WHOLE CHORD", 772.0),
    ("FILTER CHAIN", "3 IN SERIES", 1250.0),
    ("ENVELOPES", "AMP + FILTER", 472.0),
    ("RESAMPLE", "VINTAGE SAMPLER ENGINE", 882.0),
    ("FINISH", "COMP + TAPE", 1250.0),
];

/// Plate bitmap, brand, section headers and fixed labels.
pub fn faceplate(ui: &mut Ui, cx: &mut Cx) {
    let p = ui.painter();
    let th = cx.th;
    let t = th.is_black() as usize;
    theme::image(
        p,
        &cx.tex.plate[t][cx.hi as usize],
        r(0.0, 0.0, 1280.0, 800.0),
    );

    theme::text(
        p,
        T::new(F::Logo, 50.0, -0.03, th.ink),
        20.0,
        78.0,
        "TENANT",
    );
    let badge = T::new(F::Badge, 13.0, 0.04, Color32::WHITE);
    let bw = theme::text_width(p, badge, "RS-92") + 14.0;
    theme::fill_rrect(p, r(22.0, 90.0, bw, 20.0), 3.0, th.badge);
    theme::text(p, badge, 29.0, 105.0, "RS-92");
    theme::text(
        p,
        T::new(F::Label, 11.0, 0.22, th.ink),
        22.0 + bw + 8.0,
        104.5,
        "RAVE STAB SYNTHESIZER",
    );
    let chip = T::new(F::Chip, 9.0, 0.08, th.ink);
    let mut x = 22.0;
    for label in ["DIGITAL PHASE MOD", "12-BIT RESAMPLING"] {
        let w = theme::text_width(p, chip, label) + 10.0;
        theme::stroke_rrect(p, r(x, 120.0, w + 3.0, 17.0), 3.0, th.ink, 1.5);
        theme::text(p, chip, x + 6.0, 131.5, label);
        x += w + 9.0;
    }

    for (i, (title, sub, right)) in SECTIONS.iter().enumerate() {
        let (tx, ty) = widgets::SECTION_TABS[i];
        let cy = ty + 8.5;
        let x0 = tx + 30.0;
        let tw = theme::text(p, T::new(F::Title, 11.0, 0.18, th.ink), x0, cy + 4.0, title);
        let x1 = x0 + tw + 8.0;
        let sw = theme::text(p, T::new(F::Sub, 9.0, 0.14, th.ink_soft), x1, cy + 3.3, sub);
        rule(p, &th, x1 + sw + 8.0, *right, cy - 0.5);
    }
    for (y, name) in [(281.0, "OP 1"), (381.0, "OP 2")] {
        theme::text(p, T::new(F::Title, 11.0, 0.14, th.ink), 30.0, y, name);
        theme::text(
            p,
            T::new(F::Sub, 8.5, 0.14, th.ink_soft),
            30.0,
            y + 48.0,
            "WAVE",
        );
    }
    rule(p, &th, 30.0, 362.0, 350.5);
    for (i, x0) in [810.0f32, 962.0, 1114.0].into_iter().enumerate() {
        theme::text(
            p,
            T::new(F::Title, 11.0, 0.1, th.ink),
            x0,
            292.0,
            ["F1", "F2", "F3"][i],
        );
        if i > 0 {
            let (ax, ay) = (x0 - 14.0, 283.0);
            let s = egui::Stroke::new(1.5, th.ink_soft);
            p.line_segment([Pos2::new(ax, ay + 5.0), Pos2::new(ax + 10.0, ay + 5.0)], s);
            p.line_segment(
                [
                    Pos2::new(ax + 6.0, ay + 1.0),
                    Pos2::new(ax + 10.0, ay + 5.0),
                ],
                s,
            );
            p.line_segment(
                [
                    Pos2::new(ax + 10.0, ay + 5.0),
                    Pos2::new(ax + 6.0, ay + 9.0),
                ],
                s,
            );
        }
    }
    for (x, name) in [(194.0, "AMP"), (332.0, "FILTER")] {
        theme::text(p, T::new(F::Title, 9.5, 0.16, th.ink), x, 557.0, name);
        theme::hline(p, x, x + 124.0, 562.5, th.groove_dark, 1.0);
    }
    theme::text(
        p,
        T::new(F::Label, 9.0, 0.14, th.ink_soft),
        510.0,
        566.0,
        "BITS",
    );
}

// -----------------------------------------------------------------------------------------
// Main VFD
// -----------------------------------------------------------------------------------------

/// Dim 5 × 7 dot cells behind VFD text: the unlit segments of a dot-matrix display.
fn ghost_cells(p: &Painter, th: &theme::Theme, x: f32, baseline: f32, size: f32, cells: usize) {
    let cell_w = size * 0.62;
    let (px, cap) = (cell_w / 6.0, size * 0.72);
    let py = cap / 6.0;
    let col = alpha(th.vfd, 0.05);
    for i in 0..cells {
        for gx in 0..5 {
            for gy in 0..7 {
                let c = Pos2::new(
                    x + i as f32 * cell_w + gx as f32 * px + px * 0.5,
                    baseline - cap + gy as f32 * py,
                );
                p.circle_filled(c, px * 0.32, col);
            }
        }
    }
}

fn top_line(cx: &Cx) -> String {
    for msg in [&cx.st.peek, &cx.st.status].into_iter().flatten() {
        if msg.1.elapsed() < PEEK_HOLD {
            return msg.0.clone();
        }
    }
    if let Some(seed) = cx.st.seed {
        return format!("RND {:04X}", seed & 0xffff);
    }
    let prog = cx.host.program();
    let voices = cx.host.bridge().active_voices.load(Ordering::Relaxed);
    format!(
        "CAT:{}  VOICE {:02}/{:02}",
        rs92_presets::preset::bank_name(&prog.bank),
        voices,
        cx.patch.voices
    )
}

pub fn main_vfd(ui: &mut Ui, cx: &mut Cx, rect: Rect) {
    theme::main_glass(ui.painter(), rect);
    match cx.st.vfd_mode.clone() {
        VfdMode::Normal => vfd_normal(ui, cx, rect),
        VfdMode::Browser { .. } => vfd_browser(ui, cx, rect),
        VfdMode::Store => vfd_store(ui, cx, rect),
    }
}

fn vfd_normal(ui: &mut Ui, cx: &mut Cx, rect: Rect) {
    let (x, y) = (rect.left(), rect.top());
    // Clicking the preset name opens the browser.
    let name_hit = ui.interact(
        r(x + 100.0, y + 36.0, 320.0, 50.0),
        Id::new("vfd-name"),
        Sense::click(),
    );
    if name_hit.clicked() {
        browser::open(cx.st);
    }
    if name_hit.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let name_hit = name_hit.on_hover_text("Open the preset browser");
    let _ = name_hit;

    let p = ui.painter();
    let th = cx.th;
    let prog = cx.host.program();
    let small = T::new(F::Doto, 11.0, 0.08, th.vfd);
    let bank = if prog.random_seed.is_some() {
        "RND".to_string()
    } else {
        format!("BANK {}", prog.bank)
    };
    theme::vfd_text(p, small, x + 16.0, y + 22.0, &bank);
    ghost_cells(p, &th, x + 16.0, y + 81.0, 50.0, 2);
    let number = cx
        .st
        .current
        .map(|i| cx.st.library.program_number(i))
        .unwrap_or(0);
    let num = if number > 0 {
        format!("{number:02}")
    } else {
        "--".into()
    };
    theme::vfd_text(
        p,
        T::new(F::DotoBlack, 50.0, 0.0, th.vfd),
        x + 16.0,
        y + 81.0,
        &num,
    );
    theme::vfd_text(p, small, x + 16.0, y + 115.0, "PROG");

    theme::vfd_text(p, small, x + 104.0, y + 22.0, &top_line(cx));
    ghost_cells(p, &th, x + 104.0, y + 74.0, 33.3, 14);
    let name = if cx.st.comparing {
        format!("{}*", prog.preset_name)
    } else {
        prog.preset_name.clone()
    };
    theme::vfd_text(
        p,
        T::new(F::DotoBlack, 32.0, 0.04, th.vfd),
        x + 104.0,
        y + 74.0,
        &name,
    );

    let pt = &cx.patch;
    let bits = match pt.crush_bits {
        CrushBits::Off => "CLEAN".to_string(),
        b => format!("{}BIT", b.bits().unwrap_or(0)),
    };
    let chips = [
        ("POLY", true),
        (bits.as_str(), pt.crush_bits != CrushBits::Off),
        ("RESAMPLE", pt.sampler_on),
        ("CHORD", pt.chord_type != ChordType::Single),
        ("HYPER", pt.hyper),
    ];
    let ct = T::new(F::DotoBlack, 10.0, 0.04, th.vfd);
    let mut cxp = x + 104.0;
    for (label, on) in chips {
        let w = theme::text_width(p, ct, label) + 8.0;
        let col = if on { th.vfd } else { th.ghost() };
        theme::stroke_rrect(p, r(cxp, y + 100.0, w, 16.0), 2.0, col, 1.0);
        if on {
            theme::vfd_text(p, ct, cxp + 4.0, y + 111.5, label);
        } else {
            theme::text(p, ct.color(col), cxp + 4.0, y + 111.5, label);
        }
        cxp += w + 6.0;
    }
    scope(p, cx, x + 432.0, y + 12.0);
    meters(p, cx, x + 432.0, y + 90.0);
}

fn scope(p: &Painter, cx: &Cx, x: f32, y: f32) {
    let th = cx.th;
    let (w, h) = (160.0, 62.0);
    let ghost = th.ghost();
    theme::stroke_rrect(p, r(x, y, w, h), 0.0, ghost, 1.0);
    theme::hline(p, x, x + w, y + h * 0.5, ghost, 1.0);
    for gx in [40.0, 80.0, 120.0] {
        theme::line(
            p,
            Pos2::new(x + gx, y),
            Pos2::new(x + gx, y + h),
            ghost,
            1.0,
        );
    }
    let ring = &cx.st.scope.ring;
    let n = ring.len();
    let span = 320usize;
    let base = cx.st.scope.pos + n * 2 - span - 64;
    // Trigger on a rising zero crossing so the trace stands still.
    let start = (0..64)
        .map(|k| (base + k) % n)
        .find(|&i| ring[i] <= 0.0 && ring[(i + 1) % n] > 0.0)
        .unwrap_or(base % n);
    let pts: Vec<Pos2> = (0..=160)
        .map(|px| {
            let v = ring[(start + px * span / 160) % n].clamp(-1.0, 1.0);
            Pos2::new(x + px as f32, y + h * 0.5 - v * (h * 0.5 - 3.0))
        })
        .collect();
    theme::polyline(p, pts.clone(), alpha(th.vfd, 0.25), 4.0);
    theme::polyline(p, pts, th.vfd, 1.6);
}

fn meters(p: &Painter, cx: &Cx, x: f32, y: f32) {
    let th = cx.th;
    for (row, label) in ["L", "R"].into_iter().enumerate() {
        let cy = y + row as f32 * 12.0;
        theme::vfd_text(
            p,
            T::new(F::DotoBlack, 10.0, 0.0, th.vfd),
            x,
            cy + 4.0,
            label,
        );
        let db = 20.0 * cx.st.scope.level[row].max(1e-6).log10();
        let lit = (((db + 48.0) / 3.0).floor() as i32).clamp(0, 16) as usize;
        for i in 0..16 {
            let sx = x + 13.0 + i as f32 * 9.0;
            let seg = r(sx, cy - 3.5, 6.0, 7.0);
            if i < lit {
                let col = if i >= 13 { hex(0xff7a3a) } else { th.vfd };
                p.rect_filled(seg.expand(1.5), 2.0, alpha(col, 0.25));
                p.rect_filled(seg, 0.0, col);
            } else {
                p.rect_filled(seg, 0.0, th.ghost());
            }
        }
    }
}

const ROW_H: f32 = 20.0;
const LIST_X: f32 = 132.0;

fn vfd_browser(ui: &mut Ui, cx: &mut Cx, rect: Rect) {
    let VfdMode::Browser {
        bank,
        sel,
        focus_presets,
    } = cx.st.vfd_mode
    else {
        return;
    };
    let (x, y) = (rect.left(), rect.top() + 14.0);
    // Keyboard navigation; every press counts, even several in one frame.
    let (up, down, side, enter, esc) = ui.input_mut(|i| {
        let mut c = |k| i.count_and_consume_key(egui::Modifiers::NONE, k) as i32;
        (
            c(Key::ArrowUp),
            c(Key::ArrowDown),
            c(Key::ArrowLeft) + c(Key::ArrowRight),
            c(Key::Enter),
            c(Key::Escape),
        )
    });
    if down - up != 0 {
        browser::scroll(cx.st, down - up);
    }
    if side % 2 == 1 {
        browser::switch_column(cx.st);
    }
    if enter > 0 {
        if let Some(idx) = browser::enter(cx.st) {
            program::load_preset(cx.st, cx.host, idx);
            return;
        }
    }
    if esc > 0 {
        browser::close(cx.st);
        return;
    }
    let area = ui.interact(rect, Id::new("vfd-browser"), Sense::hover());
    if area.hovered() {
        let dy = ui.input(|i| i.raw_scroll_delta.y);
        if dy != 0.0 {
            browser::scroll(cx.st, if dy > 0.0 { -1 } else { 1 });
        }
    }

    let banks = browser::banks(cx.st);
    for i in 0..banks.len() {
        let row = r(x + 12.0, y + i as f32 * ROW_H - 1.0, 108.0, ROW_H - 2.0);
        if ui
            .interact(row, Id::new(("bank", i)), Sense::click())
            .clicked()
        {
            browser::select_bank(cx.st, i);
        }
    }
    let list = browser::presets_in(cx.st, bank);
    let start = browser::window_start(sel, list.len());
    for (row, &idx) in list.iter().enumerate().skip(start).take(browser::ROWS) {
        let rr = r(
            x + LIST_X - 4.0,
            y + (row - start) as f32 * ROW_H - 1.0,
            452.0,
            ROW_H - 2.0,
        );
        let resp = ui.interact(rr, Id::new(("preset-row", idx)), Sense::click());
        if resp.clicked() {
            program::load_preset(cx.st, cx.host, idx);
            return;
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
    }

    let p = ui.painter();
    let th = cx.th;
    let dark = hex(0x050707);
    let bt = T::new(F::DotoBlack, 11.0, 0.06, th.vfd);
    for (i, b) in banks.iter().enumerate() {
        let yy = y + i as f32 * ROW_H;
        let label: String = format!("{b} {}", rs92_presets::preset::bank_name(b))
            .chars()
            .take(12)
            .collect();
        if i == bank {
            theme::fill_rrect(
                p,
                r(x + 12.0, yy - 1.0, 108.0, ROW_H - 2.0),
                2.0,
                if focus_presets { th.ghost() } else { th.vfd },
            );
            theme::text(
                p,
                bt.color(if focus_presets { th.vfd } else { dark }),
                x + 16.0,
                yy + 13.0,
                &label,
            );
        } else {
            theme::vfd_text(p, bt, x + 16.0, yy + 13.0, &label);
        }
    }
    theme::line(
        p,
        Pos2::new(x + LIST_X - 10.0, rect.top() + 10.0),
        Pos2::new(x + LIST_X - 10.0, rect.bottom() - 10.0),
        th.ghost(),
        1.0,
    );
    let pt = T::new(F::DotoBlack, 13.0, 0.04, th.vfd);
    for (row, &idx) in list.iter().enumerate().skip(start).take(browser::ROWS) {
        let yy = y + (row - start) as f32 * ROW_H;
        let label = format!("{:02} {}", row + 1, cx.st.library.presets[idx].name);
        if row == sel {
            theme::fill_rrect(
                p,
                r(x + LIST_X - 4.0, yy - 1.0, 452.0, ROW_H - 2.0),
                2.0,
                if focus_presets { th.vfd } else { th.ghost() },
            );
            theme::text(
                p,
                pt.color(if focus_presets { dark } else { th.vfd }),
                x + LIST_X,
                yy + 14.0,
                &label,
            );
        } else {
            theme::vfd_text(p, pt, x + LIST_X, yy + 14.0, &label);
        }
        if cx.st.current == Some(idx) {
            // Marks the loaded preset (drawn: the glyph is not in Doto).
            let (mx, my) = (x + 586.0, yy + 9.0);
            let tri = vec![
                Pos2::new(mx - 6.0, my),
                Pos2::new(mx, my - 4.5),
                Pos2::new(mx, my + 4.5),
            ];
            p.add(egui::Shape::convex_polygon(tri, th.vfd, egui::Stroke::NONE));
        }
    }
    if list.len() > browser::ROWS {
        let track = browser::ROWS as f32 * ROW_H;
        let h = track * browser::ROWS as f32 / list.len() as f32;
        let ty = y + track * start as f32 / list.len() as f32;
        theme::fill_rrect(p, r(x + 598.0, ty, 3.0, h), 1.5, th.ghost());
    }
}

fn vfd_store(ui: &mut Ui, cx: &mut Cx, rect: Rect) {
    let th = cx.th;
    let (x, y) = (rect.left(), rect.top());
    {
        let p = ui.painter();
        let small = T::new(F::Doto, 11.0, 0.08, th.vfd);
        theme::vfd_text(p, small, x + 16.0, y + 22.0, "STORE TO USER BANK");
        theme::vfd_text(p, small, x + 16.0, y + 115.0, "ENTER SAVE   ESC CANCEL");
        ghost_cells(p, &th, x + 16.0, y + 74.0, 33.3, 14);
    }
    let edit = egui::TextEdit::singleline(&mut cx.st.store_name)
        .font(F::DotoBlack.id(32.0))
        .text_color(th.vfd)
        .frame(false)
        .char_limit(rs92_presets::preset::MAX_NAME_LEN)
        .desired_width(560.0);
    let resp = ui.put(r(x + 14.0, y + 42.0, 580.0, 40.0), edit);
    if cx.st.store_focus {
        resp.request_focus();
        cx.st.store_focus = false;
    }
    cx.st.store_name = cx.st.store_name.to_uppercase();
    if resp.lost_focus() {
        if ui.input(|i| i.key_pressed(Key::Enter)) {
            program::store_commit(cx.st, cx.host, None);
        } else {
            cx.st.vfd_mode = VfdMode::Normal;
        }
    }
}

// -----------------------------------------------------------------------------------------
// Small readouts
// -----------------------------------------------------------------------------------------

/// OP1 → PM → OP2 → FILTER diagram; paths light up with the amounts.
pub fn source_diagram(ui: &mut Ui, cx: &mut Cx, rect: Rect) {
    let p = ui.painter();
    let th = cx.th;
    let pt = &cx.patch;
    theme::glass(p, rect, 5.0);
    let (x, y) = (rect.left(), rect.top());
    let v = th.vfd;
    let dim = th.ghost();
    let s = |c: Color32| egui::Stroke::new(1.4, c);
    for bx in [12.0, 128.0] {
        theme::stroke_rrect(p, r(x + bx, y + 12.0, 50.0, 22.0), 2.0, v, 1.4);
    }
    let t11 = T::new(F::DotoBlack, 11.0, 0.0, v).center();
    theme::vfd_text(p, t11, x + 37.0, y + 27.5, "OP1");
    theme::vfd_text(p, t11, x + 153.0, y + 27.5, "OP2");
    theme::vfd_text(p, t11, x + 286.0, y + 27.5, "FILTER");
    let arrow = |x0: f32, x1: f32, c: Color32| {
        p.line_segment(
            [Pos2::new(x + x0, y + 23.0), Pos2::new(x + x1, y + 23.0)],
            s(c),
        );
        p.line_segment(
            [
                Pos2::new(x + x1 - 6.0, y + 19.0),
                Pos2::new(x + x1 + 1.0, y + 23.0),
            ],
            s(c),
        );
        p.line_segment(
            [
                Pos2::new(x + x1 + 1.0, y + 23.0),
                Pos2::new(x + x1 - 6.0, y + 27.0),
            ],
            s(c),
        );
    };
    arrow(62.0, 126.0, if pt.op2_pm > 0.0 { v } else { dim });
    theme::vfd_text(
        p,
        T::new(F::DotoBlack, 10.0, 0.0, v).center(),
        x + 94.0,
        y + 17.0,
        "PM",
    );
    let fb: Vec<Pos2> = (0..=16)
        .map(|k| {
            let a = std::f32::consts::PI * k as f32 / 16.0;
            Pos2::new(x + 153.0 + 17.0 * a.cos(), y + 12.0 - 8.0 * a.sin())
        })
        .collect();
    theme::dashed(p, &fb, if pt.op2_pd != 0.0 { v } else { dim }, 1.2, 2.0);
    theme::vfd_text(
        p,
        T::new(F::DotoBlack, 9.0, 0.0, v),
        x + 190.0,
        y + 12.0,
        "PD",
    );
    arrow(178.0, 244.0, v);
    let lvl = if pt.op1_level > 0.0 {
        alpha(v, 0.35 + 0.65 * pt.op1_level / 100.0)
    } else {
        dim
    };
    p.line_segment(
        [Pos2::new(x + 37.0, y + 34.0), Pos2::new(x + 37.0, y + 40.0)],
        s(lvl),
    );
    p.line_segment(
        [
            Pos2::new(x + 37.0, y + 40.0),
            Pos2::new(x + 250.0, y + 40.0),
        ],
        s(lvl),
    );
}

/// "E MIN   E2 E3 E4 G4 B4". Click steps through every chord type; right-click lists them.
pub fn chord_readout(ui: &mut Ui, cx: &mut Cx, rect: Rect) {
    if let Some(n) = (0u8..128).find(|&n| cx.host.bridge().is_held(n)) {
        cx.st.chord_root = n as i32;
    }
    let resp = ui.interact(rect, Id::new("chord-readout"), Sense::click());
    if resp.clicked() {
        let n = ChordType::ALL.len();
        let i = (step_index(cx, "chord_type") + 1) % n;
        cx.host.set_gesture("chord_type", i as f32 / (n - 1) as f32);
    }
    resp.context_menu(|ui| {
        let cur = step_index(cx, "chord_type");
        for (i, t) in ChordType::ALL.iter().enumerate() {
            if ui
                .radio(
                    cur == i,
                    egui::RichText::new(chord_label(*t)).font(F::Title.id(11.0)),
                )
                .clicked()
            {
                cx.host
                    .set_gesture("chord_type", i as f32 / (ChordType::ALL.len() - 1) as f32);
                ui.close_menu();
            }
        }
    });
    let th = cx.th;
    let root = cx.st.chord_root;
    let layout = ChordLayout::build(root, &cx.patch);
    let notes: Vec<String> = layout.notes().iter().map(|&n| note_name(n)).collect();
    let name = note_name(root);
    let name = name.trim_end_matches(|c: char| c.is_ascii_digit() || c == '-');
    let p = ui.painter();
    theme::glass(p, rect, 4.0);
    let cy = rect.center().y;
    theme::vfd_text(
        p,
        T::new(F::DotoBlack, 16.0, 0.0, th.vfd),
        rect.left() + 10.0,
        cy + 5.5,
        &format!("{name} {}", chord_label(cx.patch.chord_type)),
    );
    theme::vfd_text(
        p,
        T::new(F::DotoBlack, 11.0, 0.0, th.vfd).right(),
        rect.right() - 10.0,
        cy + 4.0,
        &notes.join(" "),
    );
}

pub fn sampler_status(ui: &mut Ui, cx: &mut Cx, rect: Rect) {
    let pt = &cx.patch;
    let text = if !pt.sampler_on {
        "LIVE  DIRECT SYNTHESIS  NO RESAMPLING".to_string()
    } else if cx.host.bridge().bake_pending.load(Ordering::Relaxed) {
        "SMP BAKING...".to_string()
    } else {
        let bits = match pt.crush_bits {
            CrushBits::Off => "CLEAN".to_string(),
            b => format!("{}BIT", b.bits().unwrap_or(0)),
        };
        let alias = if pt.smp_interp == Interp::Vintage {
            "ALIAS ON"
        } else {
            "ALIAS OFF"
        };
        let tail = if pt.tail_lp >= 1000.0 {
            format!("{:.1}KHZ", pt.tail_lp / 1000.0)
        } else {
            format!("{:.0}HZ", pt.tail_lp)
        };
        format!(
            "SMP {:.1}KHZ  {bits}  {alias}  TAIL LP12 {tail}",
            pt.crush_rate / 1000.0
        )
    };
    let p = ui.painter();
    theme::glass(p, rect, 4.0);
    theme::vfd_text(
        p,
        T::new(F::DotoBlack, 11.0, 0.0, cx.th.vfd),
        rect.left() + 10.0,
        rect.center().y + 4.0,
        &text,
    );
}

/// Combined F1 → F2 → F3 magnitude at the base cutoffs.
pub fn response_curve(ui: &mut Ui, cx: &mut Cx, rect: Rect) {
    let p = ui.painter();
    let th = cx.th;
    let pt = &cx.patch;
    theme::glass(p, rect, 5.0);
    let (x, y, w, h) = (rect.left(), rect.top(), rect.width(), rect.height());
    let fs = 48_000.0 * pt.os_factor.factor() as f32;
    let x_of = |hz: f32| x + w * (hz / 20.0).log10() / 3.0;
    let ghost = th.ghost();
    for hz in [100.0, 1000.0, 10_000.0] {
        let gx = x_of(hz).round() + 0.5;
        theme::line(p, Pos2::new(gx, y), Pos2::new(gx, y + h), ghost, 1.0);
    }
    theme::dashed(
        p,
        &[Pos2::new(x, y + 14.5), Pos2::new(x + w, y + 14.5)],
        ghost,
        1.0,
        2.5,
    );
    let mut pts = Vec::with_capacity(222);
    let mut px = 0.0;
    while px <= w {
        let hz = 20.0 * 1000f32.powf(px / w);
        let mag: f32 = pt
            .filters
            .iter()
            .map(|s| magnitude(s.ty, s.cutoff, s.reso, hz, fs))
            .product();
        let db = 20.0 * mag.max(1e-6).log10();
        pts.push(Pos2::new(
            x + px,
            y + (14.0 - db * (h - 14.0) / 42.0).clamp(2.0, h),
        ));
        px += 2.0;
    }
    // Fill under the curve with thin vertical strips.
    for q in &pts {
        p.line_segment(
            [*q, Pos2::new(q.x, y + h)],
            egui::Stroke::new(2.0, alpha(th.vfd, 0.10)),
        );
    }
    theme::polyline(p, pts.clone(), alpha(th.vfd, 0.25), 4.0);
    theme::polyline(p, pts, th.vfd, 1.6);
    let t = T::new(F::DotoBlack, 8.0, 0.0, th.vfd);
    for (hz, label) in [(100.0, "100"), (1000.0, "1K"), (10_000.0, "10K")] {
        theme::vfd_text(p, t, x_of(hz).round() + 3.0, y + 54.0, label);
    }
}

fn env_points(x: f32, y: f32, a: f32, d: f32, s: f32, r_ms: f32, hold: f32) -> Vec<Pos2> {
    let px = |ms: f32| 3.2 * ms.max(0.0).sqrt();
    let (top, bot) = (y + 26.0, y + 140.0);
    let ly = |l: f32| bot - l * (bot - top);
    let (mut pa, mut pd, mut pr) = (px(a), px(d), px(r_ms));
    let total = pa + pd + hold + pr;
    if total > 146.0 {
        let k = (146.0 - hold) / (pa + pd + pr);
        pa *= k;
        pd *= k;
        pr *= k;
    }
    let mut pts = vec![Pos2::new(x + 2.0, bot), Pos2::new(x + 2.0 + pa, ly(1.0))];
    for i in 1..=16 {
        let k = i as f32 / 16.0;
        pts.push(Pos2::new(
            x + 2.0 + pa + pd * k,
            ly(s + (1.0 - s) * (-6.9 * k).exp()),
        ));
    }
    let x2 = x + 2.0 + pa + pd + hold;
    pts.push(Pos2::new(x2, ly(s)));
    for i in 1..=10 {
        let k = i as f32 / 10.0;
        pts.push(Pos2::new(x2 + pr * k, ly(s * (-6.9 * k).exp())));
    }
    pts
}

/// AMP solid, FILTER dashed.
pub fn env_graph(ui: &mut Ui, cx: &mut Cx, rect: Rect) {
    let p = ui.painter();
    let th = cx.th;
    let pt = &cx.patch;
    let (x, y) = (rect.left(), rect.top());
    theme::glass(p, rect, 5.0);
    let ghost = th.ghost();
    for gy in [50.5, 90.5] {
        theme::hline(p, x, x + 150.0, y + gy, ghost, 1.0);
    }
    for gx in [50.5, 100.5] {
        theme::line(
            p,
            Pos2::new(x + gx, y),
            Pos2::new(x + gx, y + 150.0),
            ghost,
            1.0,
        );
    }
    let amp = env_points(x, y, pt.amp_a, pt.amp_d, pt.amp_s / 100.0, pt.amp_r, 10.0);
    let flt = env_points(x, y, pt.flt_a, pt.flt_d, pt.flt_s / 100.0, pt.flt_r, 40.0);
    theme::polyline(p, amp.clone(), alpha(th.vfd, 0.25), 4.5);
    theme::polyline(p, amp, th.vfd, 1.8);
    theme::dashed(p, &flt, th.vfd, 1.4, 3.0);
    theme::hline(p, x + 8.0, x + 20.0, y + 12.0, th.vfd, 1.8);
    let t = T::new(F::DotoBlack, 9.0, 0.0, th.vfd);
    theme::vfd_text(p, t, x + 24.0, y + 15.0, "AMP");
    theme::dashed(
        p,
        &[Pos2::new(x + 62.0, y + 12.0), Pos2::new(x + 74.0, y + 12.0)],
        th.vfd,
        1.4,
        3.0,
    );
    theme::vfd_text(p, t, x + 78.0, y + 15.0, "FLT");
}

// -----------------------------------------------------------------------------------------
// VU meter and footer
// -----------------------------------------------------------------------------------------

/// Scale marks: (label, dB, needle angle in degrees from vertical).
const TICKS: [(&str, f32, f32); 7] = [
    ("-20", -20.0, -46.0),
    ("10", -10.0, -28.0),
    ("7", -7.0, -18.0),
    ("5", -5.0, -8.0),
    ("3", -3.0, 4.0),
    ("0", 0.0, 22.0),
    ("+3", 3.0, 44.0),
];
/// 0 VU corresponds to this output peak level.
const VU_REF_DBFS: f32 = -12.0;

fn vu_angle(db: f32) -> f32 {
    if db <= TICKS[0].1 {
        return TICKS[0].2 - 4.0 * ((TICKS[0].1 - db) / 10.0).min(1.0);
    }
    for w in TICKS.windows(2) {
        if db <= w[1].1 {
            let t = (db - w[0].1) / (w[1].1 - w[0].1);
            return w[0].2 + t * (w[1].2 - w[0].2);
        }
    }
    (TICKS[6].2 + (db - 3.0) * 3.0).min(50.0)
}

pub fn vu(ui: &mut Ui, cx: &mut Cx, rect: Rect) {
    // 300 ms VU ballistics, frame-rate independent.
    let dt = ui.input(|i| i.stable_dt).clamp(0.001, 0.1);
    let br = cx.host.bridge();
    let peak = br.peak[0]
        .load(Ordering::Relaxed)
        .max(br.peak[1].load(Ordering::Relaxed));
    let target = (20.0 * peak.max(1e-5).log10() - VU_REF_DBFS).max(-30.0);
    let k = 1.0 - (-dt / 0.3 * 2.2).exp();
    cx.st.vu += (target - cx.st.vu) * k;

    let p = ui.painter();
    let th = cx.th;
    theme::grad_rrect(p, rect, 5.0, th.bezel_top, th.bezel_bottom);
    theme::hline(
        p,
        rect.left() + 5.0,
        rect.right() - 5.0,
        rect.bottom() + 0.5,
        th.groove_light,
        1.0,
    );
    let face = rect.shrink(4.0);
    theme::grad_rrect(p, face, 3.0, hex(0xf7e3ad), hex(0xe2b95e));
    let fp = p.with_clip_rect(face);
    let (c, rad) = (Pos2::new(face.left() + 66.0, face.top() + 104.0), 80.0);
    let pt = |a: f32, rr: f32| {
        let a = a.to_radians();
        Pos2::new(c.x + rr * a.sin(), c.y - rr * a.cos())
    };
    let ink = hex(0x2a2217);
    let deg = |d: f32| (d - 90.0).to_radians();
    theme::polyline(&fp, theme::arc(c, rad, deg(-46.0), deg(22.0)), ink, 1.2);
    theme::polyline(
        &fp,
        theme::arc(c, rad, deg(22.0), deg(44.0)),
        hex(0xc2410c),
        3.5,
    );
    for (label, _, a) in TICKS {
        fp.line_segment([pt(a, rad), pt(a, rad - 7.0)], egui::Stroke::new(1.2, ink));
        let l = pt(a, rad - 16.0);
        theme::text(
            &fp,
            T::new(F::Bold, 7.5, 0.0, ink).center(),
            l.x,
            l.y + 3.0,
            label,
        );
    }
    theme::text(
        &fp,
        T::new(F::Hyper, 11.0, 0.0, ink).center(),
        face.left() + 66.0,
        face.top() + 70.0,
        "VU",
    );
    fp.line_segment(
        [c, pt(vu_angle(cx.st.vu), rad + 2.0)],
        egui::Stroke::new(1.4, hex(0x1a1410)),
    );
}

pub fn footer(ui: &mut Ui, cx: &mut Cx, rect: Rect) {
    // Click the footer to clear the CLIP latch.
    if ui
        .interact(rect, Id::new("footer"), Sense::click())
        .clicked()
    {
        cx.host.bridge().clip.store(false, Ordering::Relaxed);
    }
    let br = cx.host.bridge();
    let m = br.midi_activity.load(Ordering::Relaxed);
    if m != cx.st.midi_seen {
        cx.st.midi_seen = m;
        cx.st.midi_flash = Some(Instant::now());
    }
    let midi = cx
        .st
        .midi_flash
        .is_some_and(|t| t.elapsed().as_millis() < 150);
    let clip = br.clip.load(Ordering::Relaxed);
    let os = cx.patch.os_factor.factor();
    let status = format!(
        "VOICES {:02}/{:02}  \u{b7}  CPU {:.1}%  \u{b7}  {:.1} KHZ  \u{b7}  {}\u{d7} OS",
        br.active_voices.load(Ordering::Relaxed),
        cx.patch.voices,
        br.cpu.load(Ordering::Relaxed) * 100.0,
        br.sample_rate.load(Ordering::Relaxed) / 1000.0,
        os
    );
    let p = ui.painter();
    let th = cx.th;
    let t = T::new(F::Label, 9.0, 0.2, th.ink_soft);
    let (x0, y) = (rect.left() + 6.0, rect.top() + 27.0);
    let lw = theme::text(
        p,
        t,
        x0,
        y,
        "MODEL RS-92  \u{b7}  DIGITAL PHASE-MODULATION RAVE STAB SYNTHESIZER",
    );
    let mut x = rect.right() - 6.0;
    for (label, on) in [("OVERSAMPLE", os == 4), ("CLIP", clip), ("MIDI", midi)] {
        let w = theme::text(p, t.right(), x, y, label);
        theme::led(p, Pos2::new(x - w - 9.5, y - 3.2), 3.5, on, &th);
        x -= w + 27.0;
    }
    let sw = theme::text(p, t.right(), x + 4.0, y, &status);
    let (a, b) = (x0 + lw + 16.0, x + 4.0 - sw - 16.0);
    theme::hline(p, a, b, y - 6.0, th.groove_dark, 1.0);
    theme::hline(p, a, b, y - 1.0, th.groove_dark, 1.0);
}
