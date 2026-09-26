//! Panel layout and the interactive controls.
//!
//! Every control is a plain function that allocates a rect, reads its parameter, handles
//! input for this frame and paints. Knobs: vertical drag 200 px = full range, Shift 10×
//! finer, wheel 2 %, double-click reset, right-click menu.

use super::displays;
use super::program;
use super::theme::{self, alpha, hex, pill, r, Al, F, T};
use super::{browser, Cx, VfdMode, SCALES};
use crate::GuiNote;
use nih_plug_egui::egui::{
    self, Color32, CursorIcon, Event, Id, PointerButton, Pos2, Rect, Response, Sense, Ui, Vec2,
};
use rs92_dsp::{ChordType, FilterType, Wave};
use std::time::Instant;

// -----------------------------------------------------------------------------------------
// Value formatting
// -----------------------------------------------------------------------------------------

/// Display string: unit attached, uppercase, "%" without a space.
pub fn display_value(cx: &Cx, id: &str, norm: f32) -> String {
    cx.host
        .to_text(id, norm)
        .to_uppercase()
        .replace(" %", "%")
        .replace(" ST", "")
        .replace(" DBFS", " DB")
}

fn peek(cx: &mut Cx, id: &str, norm: f32) {
    let text = format!(
        "{} {}",
        cx.host.name(id).to_uppercase(),
        display_value(cx, id, norm)
    );
    cx.st.set_peek(text);
}

fn snap(cx: &Cx, id: &str, v: f32) -> f32 {
    match cx.host.steps(id) {
        Some(n) => (v * n as f32).round() / n as f32,
        None => v,
    }
}

/// Index of an enum/int parameter's current value.
pub fn step_index(cx: &Cx, id: &str) -> usize {
    let n = cx.host.steps(id).unwrap_or(1);
    (cx.host.norm(id) * n as f32).round() as usize
}

fn set_index(cx: &mut Cx, id: &str, i: usize) {
    let n = cx.host.steps(id).unwrap_or(1);
    let v = i.min(n) as f32 / n as f32;
    cx.host.set_gesture(id, v);
    peek(cx, id, v);
}

fn step_enum(cx: &mut Cx, id: &str, delta: i32) {
    let n = cx.host.steps(id).unwrap_or(1) as i32;
    let i = (step_index(cx, id) as i32 + delta).rem_euclid(n + 1);
    set_index(cx, id, i as usize);
}

/// One wheel notch per frame with scroll input over `resp`: +1 up, -1 down.
fn wheel(ui: &Ui, resp: &Response) -> i32 {
    if !resp.hovered() {
        return 0;
    }
    let dy = ui.input(|i| i.raw_scroll_delta.y);
    if dy > 0.0 {
        1
    } else if dy < 0.0 {
        -1
    } else {
        0
    }
}

// -----------------------------------------------------------------------------------------
// Context menus
// -----------------------------------------------------------------------------------------

const HIDDEN: [&str; 7] = [
    "voices",
    "bend_range",
    "vel_amp",
    "os_factor",
    "smp_root",
    "smp_gate",
    "smp_interp",
];

fn menu_label(s: &str) -> egui::RichText {
    egui::RichText::new(s).font(F::Title.id(11.0))
}

/// Reset / Type value / enum values / More.
pub fn param_menu(ui: &mut Ui, cx: &mut Cx, id: &str) {
    ui.set_min_width(210.0);
    if cx.st.menu_more {
        more_menu(ui, cx);
        return;
    }
    ui.label(egui::RichText::new(cx.host.name(id).to_uppercase()).font(F::Label.id(10.0)));
    if ui.button(menu_label("RESET")).clicked() {
        let d = cx.host.default_norm(id);
        cx.host.set_gesture(id, d);
        peek(cx, id, d);
        ui.close_menu();
    }
    if ui.button(menu_label("TYPE VALUE...")).clicked() {
        open_type_value(ui, cx, id);
        ui.close_menu();
    }
    if let Some(n) = cx.host.steps(id).filter(|n| *n <= 16) {
        ui.separator();
        let cur = step_index(cx, id);
        for i in 0..=n {
            let label = display_value(cx, id, i as f32 / n as f32);
            if ui.radio(cur == i, menu_label(&label)).clicked() {
                set_index(cx, id, i);
                ui.close_menu();
            }
        }
    }
    ui.separator();
    // Shown in place (egui's submenu arrow glyph is not in the bundled fonts).
    if ui.button(menu_label("MORE...")).clicked() {
        cx.st.menu_more = true;
    }
}

fn open_type_value(ui: &Ui, cx: &mut Cx, id: &str) {
    let pos = ui
        .ctx()
        .pointer_latest_pos()
        .unwrap_or(Pos2::new(600.0, 400.0));
    let text = cx.host.to_text(id, cx.host.norm(id));
    cx.st.type_value = Some((id.to_string(), text, pos));
    cx.st.type_value_focus = true;
}

/// Hidden parameters and global preferences.
fn more_menu(ui: &mut Ui, cx: &mut Cx) {
    for id in HIDDEN {
        let label = format!(
            "{}: {}",
            cx.host.name(id).to_uppercase(),
            display_value(cx, id, cx.host.norm(id))
        );
        if ui.button(menu_label(&label)).clicked() {
            match cx.host.steps(id) {
                Some(n) if n <= 2 => step_enum(cx, id, 1),
                _ => open_type_value(ui, cx, id),
            }
            ui.close_menu();
        }
    }
    ui.separator();
    setup_menu(ui, cx);
}

/// Finish, VFD colour and UI scale.
pub fn setup_menu(ui: &mut Ui, cx: &mut Cx) {
    ui.set_min_width(210.0);
    use super::theme::{ThemeKind, VfdColor};
    let prefs = cx.st.prefs.clone();
    ui.label(egui::RichText::new("FINISH").font(F::Label.id(10.0)));
    for (k, n) in [
        (ThemeKind::Silver, "SILVER"),
        (ThemeKind::BlackEs, "BLACK ES"),
    ] {
        if ui.radio(prefs.theme == k, menu_label(n)).clicked() {
            cx.st.prefs.theme = k;
            cx.st.prefs.save();
            ui.close_menu();
        }
    }
    ui.label(egui::RichText::new("VFD").font(F::Label.id(10.0)));
    for (c, n) in [
        (VfdColor::Cyan, "CYAN"),
        (VfdColor::Amber, "AMBER"),
        (VfdColor::Green, "GREEN"),
    ] {
        if ui.radio(prefs.vfd == c, menu_label(n)).clicked() {
            cx.st.prefs.vfd = c;
            cx.st.prefs.save();
            ui.close_menu();
        }
    }
    ui.label(egui::RichText::new("UI SCALE (NEXT OPEN)").font(F::Label.id(10.0)));
    for s in SCALES {
        if ui
            .radio(
                (prefs.scale - s).abs() < 1e-3,
                menu_label(&format!("{:.0}%", s * 100.0)),
            )
            .clicked()
        {
            cx.st.prefs.scale = s;
            cx.st.prefs.save();
            cx.st
                .set_status(format!("SCALE {:.0}% ON NEXT OPEN", s * 100.0));
            ui.close_menu();
        }
    }
}

/// The floating "type value" box. Enter applies, Escape or clicking away cancels.
fn type_value_box(ui: &mut Ui, cx: &mut Cx) {
    let Some((id, mut text, pos)) = cx.st.type_value.take() else {
        return;
    };
    let mut keep = true;
    let th = cx.th;
    egui::Area::new(Id::new("type-value"))
        .order(egui::Order::Foreground)
        .fixed_pos(Pos2::new(
            pos.x.min(1280.0 - 160.0),
            pos.y.min(800.0 - 40.0),
        ))
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(hex(0x060808))
                .stroke(egui::Stroke::new(1.0, th.vfd))
                .corner_radius(4.0)
                .inner_margin(6.0)
                .show(ui, |ui| {
                    let edit = egui::TextEdit::singleline(&mut text)
                        .font(F::DotoBlack.id(14.0))
                        .text_color(th.vfd)
                        .frame(false)
                        .desired_width(140.0);
                    let resp = ui.add(edit);
                    if cx.st.type_value_focus {
                        resp.request_focus();
                        cx.st.type_value_focus = false;
                    }
                    if resp.lost_focus() {
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            if let Some(n) = cx.host.parse_text(&id, &text) {
                                cx.host.set_gesture(&id, n);
                                peek(cx, &id, n);
                            }
                        }
                        keep = false;
                    }
                });
        });
    if keep {
        cx.st.type_value = Some((id, text, pos));
    }
}

// -----------------------------------------------------------------------------------------
// Knobs and faders
// -----------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    S,
    M,
}

impl Size {
    fn ring(self) -> f32 {
        match self {
            Size::S => 38.0,
            Size::M => 48.0,
        }
    }
    fn index(self) -> usize {
        self as usize
    }
}

fn is_bipolar(id: &str) -> bool {
    id.ends_with("_pd") || id.ends_with("_env") || id == "op2_tune" || id == "smp_transpose"
}

/// Shared drag/wheel/double-click/menu handling for knobs and faders.
fn value_control(ui: &mut Ui, cx: &mut Cx, id: &str, resp: &Response) {
    if resp.drag_started() {
        cx.host.begin(id);
        cx.st.drag_value = cx.host.norm(id);
    }
    if resp.dragged() {
        let fine = if ui.input(|i| i.modifiers.shift) {
            0.1
        } else {
            1.0
        };
        cx.st.drag_value = (cx.st.drag_value - resp.drag_delta().y / 200.0 * fine).clamp(0.0, 1.0);
        let v = snap(cx, id, cx.st.drag_value);
        if (v - cx.host.norm(id)).abs() > 1e-7 {
            cx.host.set(id, v);
        }
        peek(cx, id, v);
    }
    if resp.drag_stopped() {
        cx.host.end(id);
    }
    if resp.double_clicked() {
        let d = cx.host.default_norm(id);
        cx.host.set_gesture(id, d);
        peek(cx, id, d);
    }
    let w = wheel(ui, resp);
    if w != 0 {
        let v = cx.host.norm(id);
        let n = match cx.host.steps(id) {
            Some(steps) => (v + w as f32 / steps as f32).clamp(0.0, 1.0),
            None => {
                let fine = if ui.input(|i| i.modifiers.shift) {
                    0.1
                } else {
                    1.0
                };
                (v + 0.02 * w as f32 * fine).clamp(0.0, 1.0)
            }
        };
        cx.host.set_gesture(id, n);
        peek(cx, id, n);
    }
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
    }
    resp.context_menu(|ui| param_menu(ui, cx, id));
}

pub fn knob(
    ui: &mut Ui,
    cx: &mut Cx,
    x: f32,
    y: f32,
    id: &str,
    size: Size,
    label: &str,
) -> Response {
    let w = if size == Size::M { 60.0 } else { 58.0 };
    let rect = r(x, y, w, size.ring() + 32.0);
    let resp = ui.interact(rect, Id::new(("knob", id)), Sense::click_and_drag());
    value_control(ui, cx, id, &resp);

    let p = ui.painter();
    let th = cx.th;
    let norm = cx.host.norm(id);
    let ring = size.ring();
    let c = Pos2::new(x + w * 0.5, y + ring * 0.5);
    let rad = ring * 0.5 - 1.5;
    // 0 = 12 o'clock, clockwise; arc from -135° over 270°.
    let ang = |v: f32| (-90.0 - 135.0 + 270.0 * v).to_radians();
    theme::polyline(p, theme::arc(c, rad, ang(0.0), ang(1.0)), th.arc_off, 3.0);
    let (a0, a1) = if is_bipolar(id) {
        let (m, v) = (ang(0.5), ang(norm));
        if v < m {
            (v, m)
        } else {
            (m, v)
        }
    } else {
        (ang(0.0), ang(norm))
    };
    if (a1 - a0).abs() > 1e-3 {
        theme::polyline(p, theme::arc(c, rad, a0, a1), th.arc_on, 3.0);
    }
    // Body image: not rotated, so its lighting stays fixed.
    let inset = (ring * 0.13).round() + 1.0;
    let d = ring - 2.0 * inset;
    let img = theme::KNOB_IMG_PAD;
    let t = (th.kind == super::theme::ThemeKind::BlackEs) as usize;
    let tex = &cx.tex.knob[size.index()][t][cx.hi as usize];
    theme::image(
        p,
        tex,
        r(
            c.x - d * 0.5 - img,
            y + inset - (img - 1.0),
            d + 2.0 * img,
            d + 2.0 * img,
        ),
    );
    // Indicator.
    let a = ang(norm);
    let (s, co) = a.sin_cos();
    let (r0, r1) = (d * 0.5 - 2.0, d * 0.5 - 2.0 - d * 0.36);
    let bc = Pos2::new(c.x, y + ring * 0.5);
    p.line_segment(
        [
            Pos2::new(bc.x + co * r0, bc.y + s * r0),
            Pos2::new(bc.x + co * r1, bc.y + s * r1),
        ],
        egui::Stroke::new(2.0, th.indicator),
    );
    theme::text(
        p,
        T::new(F::Label, 9.0, 0.14, th.ink).center(),
        c.x,
        y + ring + 12.0,
        label,
    );
    let value = display_value(cx, id, norm);
    theme::text(
        p,
        T::new(F::DotoBlack, 12.0, 0.0, th.ink_soft).center(),
        c.x,
        y + ring + 27.0,
        &value,
    );
    resp
}

/// Short fader readout: "0MS", "380", "12%", "1.50S".
fn short_value(v: &str) -> String {
    if let Some(n) = v.strip_suffix(" MS") {
        if n == "0" {
            "0MS".into()
        } else {
            n.into()
        }
    } else if let Some(s) = v.strip_suffix(" S") {
        format!("{s}S")
    } else {
        v.replace(' ', "")
    }
}

pub fn fader(ui: &mut Ui, cx: &mut Cx, x: f32, y: f32, id: &str, label: &str) -> Response {
    const SLOT: f32 = 96.0;
    const TRAVEL: f32 = 84.0;
    let rect = r(x, y, 28.0, 131.0);
    let resp = ui.interact(rect, Id::new(("fader", id)), Sense::click_and_drag());
    value_control(ui, cx, id, &resp);
    let p = ui.painter();
    let th = cx.th;
    let norm = cx.host.norm(id);
    theme::fill_rrect(p, r(x + 11.0, y, 6.0, SLOT), 3.0, hex(0x121315));
    theme::hline(p, x + 11.5, x + 16.5, y + SLOT + 0.5, th.groove_light, 1.0);
    let top = y + SLOT - 12.0 - norm * TRAVEL;
    let cap = r(x + 2.0, top, 24.0, 12.0);
    theme::shadow(p, cap, 2.0, 2, 3, 90);
    theme::grad_rrect(p, cap, 2.0, th.btn_top, th.btn_bottom);
    theme::stroke_rrect(p, cap, 2.0, th.btn_border, 1.0);
    theme::hline(p, x + 5.0, x + 23.0, top + 5.0, th.indicator, 2.0);
    theme::text(
        p,
        T::new(F::Title, 10.0, 0.0, th.ink).center(),
        x + 14.0,
        y + SLOT + 14.0,
        label,
    );
    let v = short_value(&display_value(cx, id, norm));
    theme::text(
        p,
        T::new(F::DotoBlack, 10.0, 0.0, th.ink_soft).center(),
        x + 14.0,
        y + SLOT + 28.0,
        &v,
    );
    resp
}

// -----------------------------------------------------------------------------------------
// Buttons and selectors
// -----------------------------------------------------------------------------------------

/// Pill button body + centred label with an optional LED. Returns the response.
#[allow(clippy::too_many_arguments)]
fn pill_button(
    ui: &mut Ui,
    cx: &Cx,
    rect: Rect,
    key: impl std::hash::Hash,
    label: &str,
    on: bool,
    led: Option<bool>,
    font: F,
    size: f32,
) -> Response {
    let resp = ui.interact(rect, Id::new(key), Sense::click());
    let p = ui.painter();
    let th = cx.th;
    let pressed = resp.is_pointer_button_down_on();
    pill(p, &th, rect, on, pressed);
    let ink = if on { th.btn_on_ink } else { th.btn_ink };
    let t = T::new(font, size, if font == F::Hyper { 0.06 } else { 0.08 }, ink);
    let tw = theme::text_width(p, t, label);
    let led_w = if led.is_some() { size + 1.0 } else { 0.0 };
    let x0 = rect.center().x - (tw + led_w) * 0.5;
    let cy = rect.center().y;
    if let Some(l) = led {
        theme::led(p, Pos2::new(x0 + size * 0.3, cy), size * 0.3, l, &th);
    }
    theme::text(p, t, x0 + led_w, cy + size * 0.36, label);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp
}

/// Toggle bound to a boolean parameter (LED shows the state).
fn toggle(ui: &mut Ui, cx: &mut Cx, rect: Rect, id: &str, label: &str) {
    let on = cx.host.norm(id) >= 0.5;
    let resp = pill_button(
        ui,
        cx,
        rect,
        ("toggle", id),
        label,
        false,
        Some(on),
        F::Title,
        9.5,
    );
    if resp.clicked() {
        let v = if on { 0.0 } else { 1.0 };
        cx.host.set_gesture(id, v);
        peek(cx, id, v);
    }
    resp.context_menu(|ui| param_menu(ui, cx, id));
}

fn hyper_button(ui: &mut Ui, cx: &mut Cx, rect: Rect) {
    let on = cx.host.norm("hyper") >= 0.5;
    let resp = pill_button(
        ui,
        cx,
        rect,
        "hyper",
        "HYPER STAB",
        on,
        Some(on),
        F::Hyper,
        11.0,
    );
    if resp.clicked() {
        let v = if on { 0.0 } else { 1.0 };
        cx.host.set_gesture("hyper", v);
        peek(cx, "hyper", v);
    }
    resp.context_menu(|ui| param_menu(ui, cx, "hyper"));
}

/// Chord button: click selects; long-press while holding keys learns a custom chord.
fn chord_button(ui: &mut Ui, cx: &mut Cx, rect: Rect, index: usize, label: &str) {
    let on = step_index(cx, "chord_type") == index;
    let resp = pill_button(
        ui,
        cx,
        rect,
        ("chord", index),
        label,
        on,
        Some(on),
        F::Title,
        10.0,
    );
    let down = resp.is_pointer_button_down_on();
    match cx.st.chord_press {
        Some((i, since, learned)) if i == index => {
            if down && !learned && since.elapsed().as_millis() >= program::LEARN_HOLD_MS {
                let ok = program::learn_chord(cx.st, cx.host);
                cx.st.chord_press = Some((i, since, ok));
            }
            if !down {
                if !learned && resp.clicked() {
                    set_index(cx, "chord_type", index);
                }
                cx.st.chord_press = None;
            }
        }
        _ if down => cx.st.chord_press = Some((index, Instant::now(), false)),
        _ => {
            if resp.clicked() {
                set_index(cx, "chord_type", index);
            }
        }
    }
    resp.context_menu(|ui| chord_menu(ui, cx));
}

fn chord_menu(ui: &mut Ui, cx: &mut Cx) {
    let cur = step_index(cx, "chord_type");
    for (i, t) in ChordType::ALL.iter().enumerate() {
        if ui
            .radio(cur == i, menu_label(displays::chord_label(*t)))
            .clicked()
        {
            set_index(cx, "chord_type", i);
            ui.close_menu();
        }
    }
}

fn bits_button(ui: &mut Ui, cx: &mut Cx, rect: Rect, index: usize, label: &str) {
    let on = step_index(cx, "crush_bits") == index;
    let resp = pill_button(
        ui,
        cx,
        rect,
        ("bits", index),
        label,
        on,
        None,
        F::Title,
        10.0,
    );
    if resp.clicked() {
        set_index(cx, "crush_bits", index);
    }
}

pub fn wave_label(w: Wave) -> &'static str {
    match w {
        Wave::Sine => "SINE",
        Wave::Tri => "TRI",
        Wave::Saw => "SAW",
        Wave::Square => "SQUARE",
        Wave::Pulse => "PULSE",
        Wave::ResI => "RES I",
        Wave::ResII => "RES II",
        Wave::ResIII => "RES III",
        Wave::Organ => "ORGAN",
        Wave::Bell => "BELL",
    }
}

pub fn filter_label(t: FilterType) -> &'static str {
    match t {
        FilterType::Lp12 => "LP12",
        FilterType::Lp18 => "LP18",
        FilterType::Lp24 => "LP24",
        FilterType::Hp12 => "HP12",
        FilterType::Hp24 => "HP24",
        FilterType::Bp12 => "BP12",
        FilterType::Off => "OFF",
    }
}

/// Small VFD selector: ‹ VALUE ›. Left third steps back, elsewhere forward; wheel steps.
fn selector(ui: &mut Ui, cx: &mut Cx, rect: Rect, id: &str, text: &str, arrows: bool, size: f32) {
    let resp = ui.interact(rect, Id::new(("sel", id)), Sense::click());
    if resp.clicked() {
        let back = arrows
            && resp
                .interact_pointer_pos()
                .is_some_and(|p| p.x < rect.left() + rect.width() * 0.33);
        step_enum(cx, id, if back { -1 } else { 1 });
    }
    let w = wheel(ui, &resp);
    if w != 0 {
        step_enum(cx, id, -w);
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.context_menu(|ui| param_menu(ui, cx, id));
    let p = ui.painter();
    theme::glass(p, rect, 4.0);
    let t = T::new(F::DotoBlack, size, 0.0, cx.th.vfd);
    let cy = rect.center().y + size * 0.35;
    if arrows {
        theme::vfd_text(p, t, rect.left() + 6.0, cy, "\u{2039}");
        theme::vfd_text(p, t.right(), rect.right() - 6.0, cy, "\u{203a}");
    }
    theme::vfd_text(p, t.center(), rect.center().x, cy, text);
}

/// Section tab (01–06): click toggles the randomizer lock.
fn lock_tab(ui: &mut Ui, cx: &mut Cx, section: u8, x: f32, y: f32) {
    let rect = r(x, y, 22.0, 17.0);
    let resp = ui.interact(rect, Id::new(("lock", section)), Sense::click());
    let locked = cx.host.lock_mask() & (1 << section) != 0;
    if resp.clicked() {
        cx.host.set_lock_mask(cx.host.lock_mask() ^ (1 << section));
        let names = [
            "SOURCE",
            "CHORD",
            "FILTER",
            "ENVELOPES",
            "RESAMPLE",
            "FINISH",
        ];
        let state = if locked { "UNLOCKED" } else { "LOCKED" };
        cx.st
            .set_status(format!("{} {state}", names[section as usize]));
    }
    resp.clone().on_hover_text("Lock this section for RANDOM");
    let p = ui.painter();
    let th = cx.th;
    if locked {
        theme::fill_rrect(p, rect, 3.0, th.panel_solid);
        theme::stroke_rrect(p, rect, 3.0, th.ink, 1.0);
        let c = rect.center();
        theme::polyline(
            p,
            theme::arc(
                Pos2::new(c.x, c.y - 1.5),
                3.0,
                std::f32::consts::PI,
                std::f32::consts::TAU,
            ),
            th.ink,
            1.4,
        );
        theme::fill_rrect(p, r(c.x - 4.5, c.y - 1.5, 9.0, 7.0), 1.0, th.ink);
    } else {
        theme::fill_rrect(p, rect, 3.0, th.ink);
        theme::text(
            p,
            T::new(F::Title, 10.0, 0.0, th.panel_solid).center(),
            rect.center().x,
            rect.center().y + 3.6,
            &format!("{:02}", section + 1),
        );
    }
}

// -----------------------------------------------------------------------------------------
// Keyboard
// -----------------------------------------------------------------------------------------

const KEY_LOW: u8 = 36;
const KEY_HIGH: u8 = 83;

fn is_white(n: u8) -> bool {
    [0, 2, 4, 5, 7, 9, 11].contains(&(n % 12))
}

/// Key under a point (black keys win). `origin` is the keyboard's top-left.
pub fn key_at(origin: Pos2, pos: Pos2) -> Option<u8> {
    let (kx, ky) = (pos.x - origin.x - 3.0, pos.y - origin.y - 3.0);
    if !(0.0..364.0).contains(&kx) || !(0.0..72.0).contains(&ky) {
        return None;
    }
    if ky < 44.0 {
        let mut wi = 0;
        for n in KEY_LOW..=KEY_HIGH {
            if is_white(n) {
                wi += 1;
            } else {
                let left = wi as f32 * 13.0 - 4.0;
                if kx >= left && kx < left + 8.0 {
                    return Some(n);
                }
            }
        }
    }
    (KEY_LOW..=KEY_HIGH)
        .filter(|&n| is_white(n))
        .nth((kx / 13.0) as usize)
}

fn velocity(origin: Pos2, pos: Pos2) -> f32 {
    (0.45 + 0.55 * ((pos.y - origin.y) / 72.0)).clamp(0.3, 1.0)
}

/// C2–B5 keyboard. Driven by raw pointer events, so every press sounds even when several
/// clicks land in one frame.
fn keyboard(ui: &mut Ui, cx: &mut Cx, x: f32, y: f32) {
    let rect = r(x, y, 370.0, 78.0);
    let origin = rect.min;
    let _resp = ui.interact(rect, Id::new("keyboard"), Sense::click_and_drag());
    let events = ui.input(|i| i.events.clone());
    let blocked = ui.ctx().is_context_menu_open() || cx.st.type_value.is_some();
    for ev in events {
        match ev {
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                ..
            } if !blocked => {
                if let Some(n) = key_at(origin, pos) {
                    if let Some(old) = cx.st.playing_key.take() {
                        cx.host.send_note(GuiNote::Off { note: old });
                    }
                    cx.host.send_note(GuiNote::On {
                        note: n,
                        velocity: velocity(origin, pos),
                    });
                    cx.st.playing_key = Some(n);
                }
            }
            Event::PointerButton {
                button: PointerButton::Primary,
                pressed: false,
                ..
            }
            | Event::PointerGone => {
                if let Some(old) = cx.st.playing_key.take() {
                    cx.host.send_note(GuiNote::Off { note: old });
                }
            }
            Event::PointerMoved(pos) => {
                if let Some(old) = cx.st.playing_key {
                    let now = key_at(origin, pos);
                    if now != Some(old) {
                        cx.host.send_note(GuiNote::Off { note: old });
                        cx.st.playing_key = None;
                        if let Some(n) = now {
                            cx.host.send_note(GuiNote::On {
                                note: n,
                                velocity: velocity(origin, pos),
                            });
                            cx.st.playing_key = Some(n);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let p = ui.painter();
    let th = cx.th;
    theme::fill_rrect(p, rect, 4.0, hex(0x111214));
    let bridge = cx.host.bridge();
    let lit = |n: u8| bridge.is_lit(n) || cx.st.playing_key == Some(n);
    let mut wi = 0;
    let mut blacks = vec![];
    for n in KEY_LOW..=KEY_HIGH {
        if is_white(n) {
            let kx = x + 3.0 + wi as f32 * 13.0;
            let key = r(kx, y + 3.0, 12.0, 72.0);
            let on = lit(n);
            theme::grad_rrect(
                p,
                key,
                2.0,
                if on { th.led_on } else { th.key_white },
                if on {
                    theme::lerp(th.led_on, Color32::BLACK, 0.2)
                } else {
                    theme::lerp(th.key_white, Color32::BLACK, 0.06)
                },
            );
            if n % 12 == 0 {
                theme::text(
                    p,
                    T::new(F::Label, 6.5, 0.0, hex(0x5b5f63)).center(),
                    kx + 6.0,
                    y + 72.0,
                    &format!("C{}", n / 12 - 1),
                );
            }
            wi += 1;
        } else {
            blacks.push((x + 3.0 + wi as f32 * 13.0 - 4.0, n));
        }
    }
    for (bx, n) in blacks {
        let key = r(bx, y + 3.0, 8.0, 44.0);
        theme::shadow(p, key, 2.0, 2, 3, 128);
        let on = lit(n);
        theme::fill_rrect(p, key, 2.0, if on { th.led_on } else { th.key_black });
        if !on {
            theme::line(
                p,
                Pos2::new(bx + 1.5, y + 4.0),
                Pos2::new(bx + 1.5, y + 44.0),
                Color32::from_white_alpha(20),
                1.0,
            );
        }
    }
}

// -----------------------------------------------------------------------------------------
// Jog dial and program buttons
// -----------------------------------------------------------------------------------------

/// Drag distance per preset step.
const JOG_STEP_PX: f32 = 24.0;

fn jog_step(cx: &mut Cx, delta: i32) {
    if browser::is_open(cx.st) {
        browser::scroll(cx.st, delta);
    } else {
        program::step(cx.st, cx.host, delta);
    }
    cx.st.jog_angle += delta as f32 * 15f32.to_radians();
}

fn jog(ui: &mut Ui, cx: &mut Cx, x: f32, y: f32) {
    let rect = r(x, y, 104.0, 104.0);
    let resp = ui.interact(rect, Id::new("jog"), Sense::click_and_drag());
    if resp.dragged() {
        let dy = resp.drag_delta().y;
        cx.st.jog_acc += dy;
        while cx.st.jog_acc.abs() >= JOG_STEP_PX {
            let d = cx.st.jog_acc.signum() as i32;
            cx.st.jog_acc -= d as f32 * JOG_STEP_PX;
            jog_step(cx, d);
        }
    }
    if resp.drag_stopped() {
        cx.st.jog_acc = 0.0;
    }
    if resp.clicked() {
        if browser::is_open(cx.st) {
            if let Some(idx) = browser::enter(cx.st) {
                program::load_preset(cx.st, cx.host, idx);
            }
        } else {
            browser::open(cx.st);
        }
    }
    let w = wheel(ui, &resp);
    if w != 0 {
        jog_step(cx, -w);
    }
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::Grab);
    }
    let p = ui.painter();
    let th = cx.th;
    let t = (th.is_black()) as usize;
    let pad = theme::JOG_IMG_PAD;
    theme::image(
        p,
        &cx.tex.jog[t][cx.hi as usize],
        r(
            x - pad,
            y - (pad - 3.0),
            104.0 + 2.0 * pad,
            104.0 + 2.0 * pad,
        ),
    );
    // Dimple, rotating with the dial (plus the live part of the drag).
    let c = Pos2::new(x + 52.0, y + 52.0);
    let a = cx.st.jog_angle + cx.st.jog_acc / JOG_STEP_PX * 15f32.to_radians() - 0.8;
    let d = Pos2::new(c.x + 34.0 * a.cos(), c.y + 34.0 * a.sin());
    let (top, bot) = if th.is_black() {
        (hex(0x141516), hex(0x26272a))
    } else {
        (hex(0xa9adb0), hex(0xc9ccce))
    };
    p.circle_filled(
        Pos2::new(d.x, d.y + 1.0),
        9.0,
        Color32::from_white_alpha(80),
    );
    theme::grad_rrect(
        p,
        Rect::from_center_size(d, Vec2::splat(18.0)),
        9.0,
        top,
        bot,
    );
    theme::text(
        p,
        T::new(F::Label, 9.0, 0.2, th.ink_soft).center(),
        x + 52.0,
        142.0,
        "PROGRAM / DATA",
    );
}

fn program_buttons(ui: &mut Ui, cx: &mut Cx) {
    let x0 = 1076.0;
    let th = cx.th;
    // PREV / NEXT
    if pill_button(
        ui,
        cx,
        r(x0, 27.0, 88.0, 34.0),
        "prev",
        "\u{2039} PREV",
        false,
        None,
        F::Title,
        10.0,
    )
    .clicked()
    {
        program::step(cx.st, cx.host, -1);
    }
    if pill_button(
        ui,
        cx,
        r(x0 + 96.0, 27.0, 88.0, 34.0),
        "next",
        "NEXT \u{203a}",
        false,
        None,
        F::Title,
        10.0,
    )
    .clicked()
    {
        program::step(cx.st, cx.host, 1);
    }
    if pill_button(
        ui,
        cx,
        r(x0, 69.0, 88.0, 34.0),
        "store",
        "STORE",
        cx.st.vfd_mode == VfdMode::Store,
        None,
        F::Title,
        10.0,
    )
    .clicked()
    {
        program::store_begin(cx.st, cx.host);
    }
    let comparing = cx.st.comparing;
    if pill_button(
        ui,
        cx,
        r(x0 + 96.0, 69.0, 88.0, 34.0),
        "compare",
        "COMPARE",
        comparing,
        None,
        F::Title,
        10.0,
    )
    .clicked()
    {
        program::compare(cx.st, cx.host);
    }
    // RANDOM: LED blinks while the worker is generating.
    let busy = program::random_busy(cx.st);
    let blink = !busy || (ui.input(|i| i.time) * 6.0) as i64 % 2 == 0;
    let rect = r(x0, 111.0, 184.0, 34.0);
    let resp = pill_button(ui, cx, rect, "random", "", false, None, F::Title, 10.0);
    let p = ui.painter();
    let t = T::new(F::Title, 10.0, 0.14, th.btn_ink);
    let tw = theme::text_width(p, t, "RANDOM");
    let x = rect.center().x - (tw + 40.0) * 0.5;
    let cy = rect.center().y;
    theme::led(p, Pos2::new(x + 3.5, cy), 3.5, blink, &th);
    let die = r(x + 14.0, cy - 7.0, 14.0, 14.0);
    theme::stroke_rrect(p, die, 3.0, th.btn_ink, 1.4);
    for (dx, dy) in [
        (4.0, 4.0),
        (10.0, 10.0),
        (7.0, 7.0),
        (10.0, 4.0),
        (4.0, 10.0),
    ] {
        p.circle_filled(Pos2::new(die.left() + dx, die.top() + dy), 1.2, th.btn_ink);
    }
    theme::text(p, t, x + 40.0, cy + 3.6, "RANDOM");
    if resp.clicked() {
        let m = ui.input(|i| i.modifiers);
        let amount = if m.shift {
            Some(0.2)
        } else if m.alt {
            Some(0.05)
        } else {
            None
        };
        program::start_random(cx.st, cx.host, amount, None);
    }
    resp.on_hover_text(
        "Click: new patch · Shift: mutate 20 % · Alt: mutate 5 % · Ctrl/Cmd-Z: undo",
    );
}

// -----------------------------------------------------------------------------------------
// The panel
// -----------------------------------------------------------------------------------------

/// Section tab positions (01–06).
pub const SECTION_TABS: [(f32, f32); 6] = [
    (30.0, 180.5),
    (400.0, 180.5),
    (810.0, 180.5),
    (30.0, 520.5),
    (510.0, 520.5),
    (920.0, 520.5),
];

pub fn panel(ui: &mut Ui, cx: &mut Cx) {
    displays::faceplate(ui, cx);

    // Brand plate: right-click for setup (finish, VFD colour, scale).
    let brand = ui.interact(
        r(16.0, 16.0, 300.0, 140.0),
        Id::new("brand"),
        Sense::click(),
    );
    brand.context_menu(|ui| setup_menu(ui, cx));

    for (i, (x, y)) in SECTION_TABS.iter().enumerate() {
        lock_tab(ui, cx, i as u8, *x, *y);
    }

    displays::main_vfd(ui, cx, r(332.0, 22.0, 608.0, 128.0));
    jog(ui, cx, 956.0, 25.0);
    program_buttons(ui, cx);

    // 01 SOURCE
    displays::source_diagram(ui, cx, r(30.0, 208.0, 332.0, 44.0));
    let w1 = wave_label(cx.patch.op1_wave);
    selector(
        ui,
        cx,
        r(30.0, 290.0, 96.0, 26.0),
        "op1_wave",
        w1,
        true,
        13.0,
    );
    let w2 = wave_label(cx.patch.op2_wave);
    selector(
        ui,
        cx,
        r(30.0, 390.0, 96.0, 26.0),
        "op2_wave",
        w2,
        true,
        13.0,
    );
    for (i, (id, label)) in [
        ("op1_pd", "PD"),
        ("op1_level", "LEVEL"),
        ("op1_fdbk", "FDBK"),
    ]
    .into_iter()
    .enumerate()
    {
        knob(ui, cx, 138.0 + i as f32 * 68.0, 262.0, id, Size::M, label);
    }
    for (i, (id, label)) in [("op2_tune", "TUNE"), ("op2_pd", "PD"), ("op2_pm", "PM")]
        .into_iter()
        .enumerate()
    {
        knob(ui, cx, 138.0 + i as f32 * 68.0, 362.0, id, Size::M, label);
    }

    // 02 CHORD MEMORY
    displays::chord_readout(ui, cx, r(400.0, 208.0, 372.0, 30.0));
    for (i, label) in ["MIN", "MAJ", "MIN7", "MIN9", "SUS4", "5TH"]
        .into_iter()
        .enumerate()
    {
        chord_button(
            ui,
            cx,
            r(400.0 + i as f32 * 63.0, 248.0, 57.0, 30.0),
            i,
            label,
        );
    }
    keyboard(ui, cx, 401.0, 288.0);
    toggle(
        ui,
        cx,
        r(400.0, 378.0, 92.0, 28.0),
        "chord_sub1",
        "SUB \u{2212}1 OCT",
    );
    toggle(
        ui,
        cx,
        r(400.0, 412.0, 92.0, 28.0),
        "chord_sub2",
        "SUB \u{2212}2 OCT",
    );
    for (i, (id, label)) in [
        ("chord_spread", "SPREAD"),
        ("chord_strum", "STRUM"),
        ("chord_detune", "DETUNE"),
        ("vel_tone", "VEL>TONE"),
    ]
    .into_iter()
    .enumerate()
    {
        knob(ui, cx, 504.0 + i as f32 * 64.0, 376.0, id, Size::S, label);
    }

    // 03 FILTER CHAIN
    displays::response_curve(ui, cx, r(810.0, 208.0, 440.0, 58.0));
    const FILTER_IDS: [[&str; 5]; 3] = [
        ["f1_type", "f1_cutoff", "f1_reso", "f1_env", "f1_key"],
        ["f2_type", "f2_cutoff", "f2_reso", "f2_env", "f2_key"],
        ["f3_type", "f3_cutoff", "f3_reso", "f3_env", "f3_key"],
    ];
    for (f, ids) in FILTER_IDS.iter().enumerate() {
        let x0 = 810.0 + f as f32 * 152.0;
        let label = filter_label(cx.patch.filters[f].ty);
        selector(
            ui,
            cx,
            r(x0 + 22.0, 277.0, 114.0, 22.0),
            ids[0],
            label,
            false,
            12.0,
        );
        knob(ui, cx, x0 + 4.0, 308.0, ids[1], Size::M, "CUTOFF");
        knob(ui, cx, x0 + 73.0, 308.0, ids[2], Size::S, "RESO");
        knob(ui, cx, x0 + 5.0, 393.0, ids[3], Size::S, "ENV");
        knob(ui, cx, x0 + 73.0, 393.0, ids[4], Size::S, "KEY");
    }

    // 04 ENVELOPES
    displays::env_graph(ui, cx, r(30.0, 548.0, 150.0, 150.0));
    const ENV_IDS: [[&str; 4]; 2] = [
        ["amp_a", "amp_d", "amp_s", "amp_r"],
        ["flt_a", "flt_d", "flt_s", "flt_r"],
    ];
    for (g, ids) in ENV_IDS.iter().enumerate() {
        for (i, id) in ids.iter().enumerate() {
            fader(
                ui,
                cx,
                194.0 + g as f32 * 138.0 + i as f32 * 32.0,
                569.0,
                id,
                ["A", "D", "S", "R"][i],
            );
        }
    }

    // 05 RESAMPLE
    for (i, label) in ["8", "12", "16", "OFF"].into_iter().enumerate() {
        bits_button(
            ui,
            cx,
            r(554.0 + i as f32 * 49.0, 548.0, 44.0, 30.0),
            i,
            label,
        );
    }
    toggle(
        ui,
        cx,
        r(756.0, 548.0, 126.0, 30.0),
        "sampler_on",
        "SAMPLER PITCH",
    );
    let res_x = [510.0, 588.5, 667.0, 745.5, 824.0];
    for (i, (id, label)) in [
        ("crush_rate", "RATE"),
        ("crush_mix", "CRUSH MIX"),
        ("tail_lp", "TAIL LP"),
        ("tail_rel", "TAIL REL"),
        ("smp_transpose", "TRANSPOSE"),
    ]
    .into_iter()
    .enumerate()
    {
        knob(ui, cx, res_x[i], 588.0, id, Size::S, label);
    }
    displays::sampler_status(ui, cx, r(510.0, 667.0, 372.0, 26.0));

    // 06 FINISH
    displays::vu(ui, cx, r(920.0, 548.0, 140.0, 96.0));
    hyper_button(ui, cx, r(920.0, 654.0, 140.0, 34.0));
    knob(ui, cx, 1089.0, 548.0, "comp_thresh", Size::S, "COMP");
    knob(ui, cx, 1177.0, 548.0, "tape_level", Size::S, "TAPE");
    knob(ui, cx, 1089.0, 635.0, "width", Size::S, "WIDTH");
    knob(ui, cx, 1176.0, 625.0, "output", Size::M, "OUTPUT");

    displays::footer(ui, cx, r(16.0, 736.0, 1248.0, 48.0));
    type_value_box(ui, cx);
    let _ = (alpha, Al::Left);
}
