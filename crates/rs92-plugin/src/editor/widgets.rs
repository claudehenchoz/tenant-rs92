//! Panel layout and the interactive controls.
//!
//! Every control is a plain function that allocates a rect, reads its parameter, handles
//! input for this frame and paints. Knobs: vertical drag 200 px = full range, Shift 10×
//! finer, wheel 2 %, double-click reset, right-click menu.

use super::displays;
use super::layout::{self as L, keys};
use super::program;
use super::theme::{self, hex, pill, r, F, T};
use super::{Cx, VfdMode, MAX_SCALE, MIN_SCALE, SIZES};
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
    let pos = ui.ctx().pointer_latest_pos().unwrap_or(L::center(L::VFD));
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
    ui.label(egui::RichText::new("UI SIZE").font(F::Label.id(10.0)));
    let now = window_scale(ui.ctx()) as f64;
    for s in SIZES {
        if ui
            .radio(
                (now - s).abs() < 0.01,
                menu_label(&format!("{:.0}%", s * 100.0)),
            )
            .clicked()
        {
            resize_to(cx, s);
            cx.st.prefs.scale = s;
            cx.st.prefs.save();
            ui.close_menu();
        }
    }
}

/// Logical window pixels per panel point in this frame. Not `zoom_factor()`: `fit_zoom`
/// may already have set next frame's zoom, while this frame's pointer positions still use
/// the current one.
fn window_scale(ctx: &egui::Context) -> f32 {
    ctx.pixels_per_point() / ctx.native_pixels_per_point().unwrap_or(1.0)
}

/// Asks the host for the window size of UI scale `s` (1.0 = `layout::W` × `layout::H`).
fn resize_to(cx: &mut Cx, s: f64) {
    let s = s.clamp(MIN_SCALE, MAX_SCALE);
    let size = (
        (L::W as f64 * s).round() as u32,
        (L::H as f64 * s).round() as u32,
    );
    if cx.st.requested_size != Some(size) {
        cx.st.requested_size = Some(size);
        cx.host.request_size(size.0, size.1);
    }
}

/// Bottom-right grip: drag to resize, aspect locked. Works in logical window pixels (not
/// panel points) so the zoom change during the drag does not feed back into it.
fn grip(ui: &mut Ui, cx: &mut Cx) {
    let rect = L::GRIP;
    let resp = ui.interact(rect, Id::new("grip"), Sense::drag());
    let ctx = ui.ctx().clone();
    let k = window_scale(&ctx);
    let panel = Vec2::new(L::W, L::H);
    if resp.drag_started() {
        if let Some(p) = resp.interact_pointer_pos() {
            cx.st.grip = Some(panel * k - p.to_vec2() * k);
        }
    }
    if resp.dragged() {
        if let (Some(off), Some(p)) = (cx.st.grip, ctx.pointer_latest_pos()) {
            let want = p.to_vec2() * k + off;
            let s = (want.x / L::W + want.y / L::H) as f64 * 0.5;
            cx.st.grip_scale = s.clamp(MIN_SCALE, MAX_SCALE);
            resize_to(cx, s);
        }
    }
    if resp.drag_stopped() {
        cx.st.grip = None;
        cx.st.prefs.scale = (cx.st.grip_scale * 100.0).round() / 100.0;
        cx.st.prefs.save();
    }
    if resp.hovered() || resp.dragged() {
        ctx.set_cursor_icon(CursorIcon::ResizeNwSe);
    }
    let p = ui.painter();
    let th = cx.th;
    let (x1, y1) = (rect.right() - 3.0, rect.bottom() - 3.0);
    for d in [4.0, 8.0, 12.0] {
        theme::line(
            p,
            Pos2::new(x1 - d, y1 + 0.5),
            Pos2::new(x1 + 0.5, y1 - d),
            th.groove_dark,
            1.0,
        );
        theme::line(
            p,
            Pos2::new(x1 - d + 1.0, y1 + 1.5),
            Pos2::new(x1 + 1.5, y1 - d + 1.0),
            th.groove_light,
            1.0,
        );
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
        .fixed_pos(Pos2::new(pos.x.min(L::W - 160.0), pos.y.min(L::H - 40.0)))
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
            Size::S => 32.0,
            Size::M => 40.0,
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
    let w = if size == Size::M { 52.0 } else { 50.0 };
    let rect = r(x, y, w, size.ring() + 24.0);
    let resp = ui.interact(rect, Id::new(("knob", id)), Sense::click_and_drag());
    value_control(ui, cx, id, &resp);

    let p = ui.painter();
    let th = cx.th;
    let norm = cx.host.norm(id);
    let ring = size.ring();
    let c = Pos2::new(x + w * 0.5, y + ring * 0.5);
    let rad = ring * 0.5 - 1.25;
    // 0 = 12 o'clock, clockwise; arc from -135° over 270°.
    let ang = |v: f32| (-90.0 - 135.0 + 270.0 * v).to_radians();
    theme::polyline(p, theme::arc(c, rad, ang(0.0), ang(1.0)), th.arc_off, 2.5);
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
        theme::polyline(p, theme::arc(c, rad, a0, a1), th.arc_on, 2.5);
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
        egui::Stroke::new(1.8, th.indicator),
    );
    theme::text(
        p,
        T::new(F::Label, 8.0, 0.12, th.ink).center(),
        c.x,
        y + ring + 9.5,
        label,
    );
    let value = display_value(cx, id, norm);
    theme::text(
        p,
        T::new(F::DotoBlack, 9.5, 0.0, th.ink_soft).center(),
        c.x,
        y + ring + 20.0,
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
    const SLOT: f32 = 84.0;
    const TRAVEL: f32 = 74.0;
    let rect = r(x, y, 22.0, 108.0);
    let resp = ui.interact(rect, Id::new(("fader", id)), Sense::click_and_drag());
    value_control(ui, cx, id, &resp);
    let p = ui.painter();
    let th = cx.th;
    let norm = cx.host.norm(id);
    theme::fill_rrect(p, r(x + 8.5, y, 5.0, SLOT), 2.5, hex(0x121315));
    theme::hline(p, x + 9.0, x + 13.0, y + SLOT + 0.5, th.groove_light, 1.0);
    let top = y + SLOT - 10.0 - norm * TRAVEL;
    let cap = r(x + 1.0, top, 20.0, 10.0);
    theme::shadow(p, cap, 2.0, 2, 3, 90);
    theme::grad_rrect(p, cap, 2.0, th.btn_top, th.btn_bottom);
    theme::stroke_rrect(p, cap, 2.0, th.btn_border, 1.0);
    theme::hline(p, x + 4.0, x + 18.0, top + 4.5, th.indicator, 1.6);
    theme::text(
        p,
        T::new(F::Title, 9.0, 0.0, th.ink).center(),
        x + 11.0,
        y + SLOT + 11.0,
        label,
    );
    let v = short_value(&display_value(cx, id, norm));
    theme::text(
        p,
        T::new(F::DotoBlack, 9.0, 0.0, th.ink_soft).center(),
        x + 11.0,
        y + SLOT + 22.0,
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
        8.5,
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
        10.0,
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
        8.5,
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
        9.0,
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
fn lock_tab(ui: &mut Ui, cx: &mut Cx, section: u8, rect: Rect) {
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
                2.6,
                std::f32::consts::PI,
                std::f32::consts::TAU,
            ),
            th.ink,
            1.3,
        );
        theme::fill_rrect(p, r(c.x - 4.0, c.y - 1.5, 8.0, 6.0), 1.0, th.ink);
    } else {
        theme::fill_rrect(p, rect, 3.0, th.ink);
        theme::text(
            p,
            T::new(F::Title, 9.0, 0.0, th.panel_solid).center(),
            rect.center().x,
            rect.center().y + 3.2,
            &format!("{:02}", section + 1),
        );
    }
}

// -----------------------------------------------------------------------------------------
// Keyboard
// -----------------------------------------------------------------------------------------

fn is_white(n: u8) -> bool {
    [0, 2, 4, 5, 7, 9, 11].contains(&(n % 12))
}

/// Key under a point (black keys win). `origin` is the keyboard's top-left.
pub fn key_at(origin: Pos2, pos: Pos2) -> Option<u8> {
    let (kx, ky) = (
        pos.x - origin.x - keys::INSET,
        pos.y - origin.y - keys::INSET,
    );
    let width = keys::WHITES as f32 * keys::PITCH - 1.0;
    if !(0.0..width).contains(&kx) || !(0.0..keys::WHITE_H).contains(&ky) {
        return None;
    }
    if ky < keys::BLACK_H {
        let mut wi = 0;
        for n in keys::LOW..=keys::HIGH {
            if is_white(n) {
                wi += 1;
            } else {
                let left = black_left(wi);
                if kx >= left && kx < left + keys::BLACK_W {
                    return Some(n);
                }
            }
        }
    }
    (keys::LOW..=keys::HIGH)
        .filter(|&n| is_white(n))
        .nth((kx / keys::PITCH) as usize)
}

/// Left edge of the black key after `whites` white keys, relative to the key area.
fn black_left(whites: usize) -> f32 {
    whites as f32 * keys::PITCH - 0.5 - keys::BLACK_W * 0.5
}

fn velocity(origin: Pos2, pos: Pos2) -> f32 {
    (0.45 + 0.55 * ((pos.y - origin.y) / keys::WHITE_H)).clamp(0.3, 1.0)
}

/// C2–B5 keyboard. Driven by raw pointer events, so every press sounds even when several
/// clicks land in one frame.
fn keyboard(ui: &mut Ui, cx: &mut Cx) {
    let rect = keys::RECT;
    let origin = rect.min;
    let (x, y) = (rect.left(), rect.top());
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
    let (kx0, ky0) = (x + keys::INSET, y + keys::INSET);
    let mut wi = 0;
    let mut blacks = vec![];
    for n in keys::LOW..=keys::HIGH {
        if is_white(n) {
            let kx = kx0 + wi as f32 * keys::PITCH;
            let key = r(kx, ky0, keys::WHITE_W, keys::WHITE_H);
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
                    T::new(F::Label, 6.0, 0.0, hex(0x5b5f63)).center(),
                    kx + keys::WHITE_W * 0.5,
                    ky0 + keys::WHITE_H - 3.0,
                    &format!("C{}", n / 12 - 1),
                );
            }
            wi += 1;
        } else {
            blacks.push((kx0 + black_left(wi), n));
        }
    }
    for (bx, n) in blacks {
        let key = r(bx, ky0, keys::BLACK_W, keys::BLACK_H);
        theme::shadow(p, key, 2.0, 2, 3, 128);
        let on = lit(n);
        theme::fill_rrect(p, key, 1.5, if on { th.led_on } else { th.key_black });
        if !on {
            theme::line(
                p,
                Pos2::new(bx + 1.5, ky0 + 1.0),
                Pos2::new(bx + 1.5, ky0 + keys::BLACK_H),
                Color32::from_white_alpha(20),
                1.0,
            );
        }
    }
}

// -----------------------------------------------------------------------------------------
// Program buttons
// -----------------------------------------------------------------------------------------

/// ▲ / ▼ beside the VFD: previous / next preset (the open browser follows).
fn step_arrow(ui: &mut Ui, cx: &mut Cx, rect: Rect, up: bool) {
    let key = if up { "prog-up" } else { "prog-down" };
    let resp = pill_button(ui, cx, rect, key, "", false, None, F::Title, 10.0);
    let p = ui.painter();
    let c = rect.center();
    let (tip, base) = if up { (-3.5, 3.0) } else { (3.5, -3.0) };
    let tri = vec![
        Pos2::new(c.x, c.y + tip),
        Pos2::new(c.x + 5.0, c.y + base),
        Pos2::new(c.x - 5.0, c.y + base),
    ];
    p.add(egui::Shape::convex_polygon(
        tri,
        cx.th.btn_ink,
        egui::Stroke::NONE,
    ));
    if resp.clicked() {
        program::step(cx.st, cx.host, if up { -1 } else { 1 });
    }
    resp.on_hover_text(if up { "Previous preset" } else { "Next preset" });
}

fn program_buttons(ui: &mut Ui, cx: &mut Cx) {
    let th = cx.th;
    step_arrow(ui, cx, L::PROG_UP, true);
    step_arrow(ui, cx, L::PROG_DOWN, false);
    if pill_button(
        ui,
        cx,
        L::STORE,
        "store",
        "STORE",
        cx.st.vfd_mode == VfdMode::Store,
        None,
        F::Title,
        9.0,
    )
    .clicked()
    {
        program::store_begin(cx.st, cx.host);
    }
    let comparing = cx.st.comparing;
    if pill_button(
        ui,
        cx,
        L::COMPARE,
        "compare",
        "COMPARE",
        comparing,
        None,
        F::Title,
        9.0,
    )
    .clicked()
    {
        program::compare(cx.st, cx.host);
    }
    // RANDOM: LED blinks while the worker is generating.
    let busy = program::random_busy(cx.st);
    let blink = !busy || (ui.input(|i| i.time) * 6.0) as i64 % 2 == 0;
    let rect = L::RANDOM;
    let resp = pill_button(ui, cx, rect, "random", "", false, None, F::Title, 9.0);
    let p = ui.painter();
    let t = T::new(F::Title, 9.0, 0.14, th.btn_ink);
    let tw = theme::text_width(p, t, "RANDOM");
    let x = rect.center().x - (tw + 34.0) * 0.5;
    let cy = rect.center().y;
    theme::led(p, Pos2::new(x + 3.0, cy), 3.0, blink, &th);
    let die = r(x + 12.0, cy - 6.0, 12.0, 12.0);
    theme::stroke_rrect(p, die, 2.5, th.btn_ink, 1.2);
    for (dx, dy) in [(3.5, 3.5), (8.5, 8.5), (6.0, 6.0), (8.5, 3.5), (3.5, 8.5)] {
        p.circle_filled(Pos2::new(die.left() + dx, die.top() + dy), 1.0, th.btn_ink);
    }
    theme::text(p, t, x + 34.0, cy + 3.2, "RANDOM");
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

pub fn panel(ui: &mut Ui, cx: &mut Cx) {
    displays::faceplate(ui, cx);

    // Brand plate: right-click for setup (finish, VFD colour, size).
    let brand = ui.interact(L::BRAND, Id::new("brand"), Sense::click());
    brand.context_menu(|ui| setup_menu(ui, cx));

    for i in 0..6 {
        lock_tab(ui, cx, i as u8, L::tab(i));
    }

    displays::main_vfd(ui, cx, L::VFD);
    program_buttons(ui, cx);
    displays::status_line(ui, cx, L::STATUS);

    // 01 SOURCE
    let w1 = wave_label(cx.patch.op1_wave);
    selector(ui, cx, L::OP1_WAVE, "op1_wave", w1, true, 11.0);
    let w2 = wave_label(cx.patch.op2_wave);
    selector(ui, cx, L::OP2_WAVE, "op2_wave", w2, true, 11.0);
    for (i, (id, label)) in [
        ("op1_pd", "PD"),
        ("op1_level", "LEVEL"),
        ("op1_fdbk", "FDBK"),
    ]
    .into_iter()
    .enumerate()
    {
        knob(ui, cx, L::OP_KNOB_X[i], L::OP1_KNOBS_Y, id, Size::M, label);
    }
    for (i, (id, label)) in [("op2_tune", "TUNE"), ("op2_pd", "PD"), ("op2_pm", "PM")]
        .into_iter()
        .enumerate()
    {
        knob(ui, cx, L::OP_KNOB_X[i], L::OP2_KNOBS_Y, id, Size::M, label);
    }

    // 02 CHORD MEMORY
    displays::chord_readout(ui, cx, L::CHORD_READOUT);
    for (i, label) in ["MIN", "MAJ", "MIN7", "MIN9", "SUS4", "5TH"]
        .into_iter()
        .enumerate()
    {
        chord_button(ui, cx, L::chord_pill(i), i, label);
    }
    keyboard(ui, cx);
    toggle(ui, cx, L::SUB1, "chord_sub1", "SUB \u{2212}1 OCT");
    toggle(ui, cx, L::SUB2, "chord_sub2", "SUB \u{2212}2 OCT");
    let (kx, ky, kp) = L::CHORD_KNOBS;
    for (i, (id, label)) in [
        ("chord_spread", "SPREAD"),
        ("chord_strum", "STRUM"),
        ("chord_detune", "DETUNE"),
        ("vel_tone", "VEL>TONE"),
    ]
    .into_iter()
    .enumerate()
    {
        knob(ui, cx, kx + i as f32 * kp, ky, id, Size::S, label);
    }

    // 03 FILTER CHAIN
    displays::response_curve(ui, cx, L::RESPONSE);
    const FILTER_IDS: [[&str; 5]; 3] = [
        ["f1_type", "f1_cutoff", "f1_reso", "f1_env", "f1_key"],
        ["f2_type", "f2_cutoff", "f2_reso", "f2_env", "f2_key"],
        ["f3_type", "f3_cutoff", "f3_reso", "f3_env", "f3_key"],
    ];
    for (f, ids) in FILTER_IDS.iter().enumerate() {
        let x0 = L::filter_x(f);
        let label = filter_label(cx.patch.filters[f].ty);
        selector(ui, cx, L::filter_type(f), ids[0], label, false, 11.0);
        knob(ui, cx, x0, L::FILTER_ROW1_Y, ids[1], Size::M, "CUTOFF");
        knob(
            ui,
            cx,
            x0 + 60.0,
            L::FILTER_ROW1_Y + 4.0,
            ids[2],
            Size::S,
            "RESO",
        );
        knob(ui, cx, x0 + 1.0, L::FILTER_ROW2_Y, ids[3], Size::S, "ENV");
        knob(ui, cx, x0 + 60.0, L::FILTER_ROW2_Y, ids[4], Size::S, "KEY");
    }

    // 04 ENVELOPES
    displays::env_graph(ui, cx, L::ENV_GRAPH);
    const ENV_IDS: [[&str; 4]; 2] = [
        ["amp_a", "amp_d", "amp_s", "amp_r"],
        ["flt_a", "flt_d", "flt_s", "flt_r"],
    ];
    for (g, ids) in ENV_IDS.iter().enumerate() {
        for (i, id) in ids.iter().enumerate() {
            fader(
                ui,
                cx,
                L::fader_x(g, i),
                L::FADER_Y,
                id,
                ["A", "D", "S", "R"][i],
            );
        }
    }

    // 05 RESAMPLE
    for (i, label) in ["8", "12", "16", "OFF"].into_iter().enumerate() {
        bits_button(ui, cx, L::bits_pill(i), i, label);
    }
    toggle(ui, cx, L::SAMPLER_ON, "sampler_on", "SAMPLER PITCH");
    let (kx, ky, kp) = L::RESAMPLE_KNOBS;
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
        knob(ui, cx, kx + i as f32 * kp, ky, id, Size::S, label);
    }

    // 06 FINISH
    displays::vu(ui, cx, L::VU);
    hyper_button(ui, cx, L::HYPER);
    for (pos, id, size, label) in [
        (L::COMP_KNOB, "comp_thresh", Size::S, "COMP"),
        (L::TAPE_KNOB, "tape_level", Size::S, "TAPE"),
        (L::WIDTH_KNOB, "width", Size::S, "WIDTH"),
        (L::OUTPUT_KNOB, "output", Size::M, "OUTPUT"),
    ] {
        knob(ui, cx, pos.x, pos.y, id, size, label);
    }

    grip(ui, cx);
    type_value_box(ui, cx);
}
