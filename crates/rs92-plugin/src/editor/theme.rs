//! Theme tokens, bundled fonts and bitmaps, and egui painter helpers.
//!
//! The panel is laid out in the design points of `layout`; the editor sets the
//! egui zoom so those points fill the window at any size.

use nih_plug_egui::egui::{
    self, epaint, text::LayoutJob, Color32, FontData, FontDefinitions, FontFamily, FontId, Mesh,
    Painter, Pos2, Rect, Shape, Stroke, TextFormat, TextureHandle, TextureOptions, Vec2,
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ThemeKind {
    #[default]
    Silver,
    BlackEs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum VfdColor {
    #[default]
    Cyan,
    Amber,
    Green,
}

impl VfdColor {
    pub fn color(self) -> Color32 {
        match self {
            VfdColor::Cyan => hex(0x7ff5e6),
            VfdColor::Amber => hex(0xffb347),
            VfdColor::Green => hex(0xa8ff92),
        }
    }
}

pub const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

pub fn alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (c.a() as f32 * a) as u8)
}

/// Colour tokens (design doc section 7).
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub kind: ThemeKind,
    pub panel_solid: Color32,
    pub ink: Color32,
    pub ink_soft: Color32,
    pub groove_dark: Color32,
    pub groove_light: Color32,
    pub arc_on: Color32,
    pub arc_off: Color32,
    pub btn_top: Color32,
    pub btn_bottom: Color32,
    pub btn_on_top: Color32,
    pub btn_on_bottom: Color32,
    pub btn_border: Color32,
    pub btn_ink: Color32,
    pub btn_on_ink: Color32,
    pub btn_highlight: f32,
    pub indicator: Color32,
    pub led_on: Color32,
    pub led_off: Color32,
    pub key_white: Color32,
    pub key_black: Color32,
    pub badge: Color32,
    pub vfd: Color32,
    pub bezel_top: Color32,
    pub bezel_bottom: Color32,
}

impl Theme {
    pub fn new(kind: ThemeKind, vfd: VfdColor) -> Self {
        let vfd = vfd.color();
        match kind {
            ThemeKind::Silver => Theme {
                kind,
                panel_solid: hex(0xd6d9db),
                ink: hex(0x1b1d1f),
                ink_soft: hex(0x464a4e),
                groove_dark: hex(0x9ea3a7),
                groove_light: hex(0xf6f7f7),
                arc_on: hex(0xd2500f),
                arc_off: hex(0xb0b4b7),
                btn_top: hex(0xf7f8f9),
                btn_bottom: hex(0xcdd0d3),
                btn_on_top: hex(0x2c2e31),
                btn_on_bottom: hex(0x17181a),
                btn_border: hex(0x979ca0),
                btn_ink: hex(0x1b1d1f),
                btn_on_ink: hex(0xf2f3f4),
                btn_highlight: 0.7,
                indicator: hex(0x1b1d1f),
                led_on: hex(0xff6a1f),
                led_off: hex(0x7a4a38),
                key_white: hex(0xfbfbfa),
                key_black: hex(0x1a1b1d),
                badge: hex(0xb83d0c),
                vfd,
                bezel_top: hex(0x2a2c2f),
                bezel_bottom: hex(0x3c3f42),
            },
            ThemeKind::BlackEs => Theme {
                kind,
                panel_solid: hex(0x202124),
                ink: hex(0xe4d6b2),
                ink_soft: hex(0xb6a987),
                groove_dark: hex(0x08090a),
                groove_light: hex(0x393a3f),
                arc_on: hex(0xe0a84e),
                arc_off: hex(0x35363a),
                btn_top: hex(0x3b3c40),
                btn_bottom: hex(0x26272a),
                btn_on_top: hex(0xe4d6b2),
                btn_on_bottom: hex(0xbba97f),
                btn_border: hex(0x070708),
                btn_ink: hex(0xe4d6b2),
                btn_on_ink: hex(0x18191b),
                btn_highlight: 0.12,
                indicator: hex(0xe4d6b2),
                led_on: hex(0xff6a1f),
                led_off: hex(0x3b2016),
                key_white: hex(0xe9e4d8),
                key_black: hex(0x0b0b0c),
                badge: hex(0xb83d0c),
                vfd,
                bezel_top: hex(0x0d0d0e),
                bezel_bottom: hex(0x1a1b1d),
            },
        }
    }

    pub fn is_black(&self) -> bool {
        self.kind == ThemeKind::BlackEs
    }

    pub fn ghost(&self) -> Color32 {
        alpha(self.vfd, 0.14)
    }
}

// -----------------------------------------------------------------------------------------
// Fonts
// -----------------------------------------------------------------------------------------

/// Font faces used on the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum F {
    /// Archivo semi-condensed 800: titles, buttons.
    Title,
    /// Archivo condensed 600: sub-titles.
    Sub,
    /// Archivo condensed 700: knob labels.
    Label,
    /// Archivo 700: VU scale.
    Bold,
    /// Archivo expanded black italic: logo.
    Logo,
    /// Archivo semi-expanded 800 italic: model badge.
    Badge,
    /// Archivo semi-expanded 900 italic: HYPER STAB, VU.
    Hyper,
    /// Archivo semi-condensed 800 italic: feature chips.
    Chip,
    /// Doto 800 / 900: VFD and value readouts.
    Doto,
    DotoBlack,
}

impl F {
    fn key(self) -> &'static str {
        match self {
            F::Title => "title",
            F::Sub => "sub",
            F::Label => "label",
            F::Bold => "bold",
            F::Logo => "logo",
            F::Badge => "badge",
            F::Hyper => "hyper",
            F::Chip => "chip",
            F::Doto => "doto",
            F::DotoBlack => "doto_black",
        }
    }

    pub fn id(self, size: f32) -> FontId {
        FontId::new(size, FontFamily::Name(self.key().into()))
    }
}

macro_rules! asset {
    ($dir:literal, $f:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/",
            $dir,
            "/",
            $f
        ))
    };
}

/// Registers the bundled faces. egui's own widgets (menus, text fields) use Archivo and Doto.
pub fn install_fonts(ctx: &egui::Context) {
    let mut defs = FontDefinitions::empty();
    let faces: [(F, &'static [u8]); 10] = [
        (
            F::Title,
            asset!("fonts", "Archivo-SemiCondensed-ExtraBold.ttf"),
        ),
        (F::Sub, asset!("fonts", "Archivo-Condensed-SemiBold.ttf")),
        (F::Label, asset!("fonts", "Archivo-Condensed-Bold.ttf")),
        (F::Bold, asset!("fonts", "Archivo-Bold.ttf")),
        (F::Logo, asset!("fonts", "Archivo-Expanded-BlackItalic.ttf")),
        (
            F::Badge,
            asset!("fonts", "Archivo-SemiExpanded-ExtraBoldItalic.ttf"),
        ),
        (
            F::Hyper,
            asset!("fonts", "Archivo-SemiExpanded-BlackItalic.ttf"),
        ),
        (
            F::Chip,
            asset!("fonts", "Archivo-SemiCondensed-ExtraBoldItalic.ttf"),
        ),
        (F::Doto, asset!("fonts", "Doto-ExtraBold.ttf")),
        (F::DotoBlack, asset!("fonts", "Doto-Black.ttf")),
    ];
    for (f, bytes) in faces {
        defs.font_data
            .insert(f.key().into(), Arc::new(FontData::from_static(bytes)));
    }
    for (f, _) in faces {
        defs.families.insert(
            FontFamily::Name(f.key().into()),
            vec![f.key().into(), "bold".into()],
        );
    }
    defs.families.insert(
        FontFamily::Proportional,
        vec!["title".into(), "bold".into()],
    );
    defs.families.insert(
        FontFamily::Monospace,
        vec!["doto_black".into(), "bold".into()],
    );
    ctx.set_fonts(defs);
}

// -----------------------------------------------------------------------------------------
// Bitmaps
// -----------------------------------------------------------------------------------------

/// Pre-rendered bitmaps, indexed [theme][2x].
pub struct Textures {
    pub plate: [[TextureHandle; 2]; 2],
    /// [size S/M][theme][2x].
    pub knob: [[[TextureHandle; 2]; 2]; 2],
}

/// Padding baked into the knob images for their drop shadows.
pub const KNOB_IMG_PAD: f32 = 5.0;

fn decode(name: &str, bytes: &[u8], ctx: &egui::Context) -> TextureHandle {
    let decoder = png::Decoder::new(bytes);
    let mut reader = decoder.read_info().expect("bundled png");
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("bundled png");
    let data = &buf[..info.buffer_size()];
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .chunks(3)
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        other => panic!("unsupported png colour type {other:?}"),
    };
    let img = egui::ColorImage::from_rgba_unmultiplied(
        [info.width as usize, info.height as usize],
        &rgba,
    );
    ctx.load_texture(name, img, TextureOptions::LINEAR)
}

impl Textures {
    pub fn load(ctx: &egui::Context) -> Self {
        macro_rules! pair {
            ($a:literal, $b:literal) => {
                [
                    decode($a, asset!("images", $a), ctx),
                    decode($b, asset!("images", $b), ctx),
                ]
            };
        }
        Textures {
            plate: [
                pair!("plate_silver@1x.png", "plate_silver@2x.png"),
                pair!("plate_black@1x.png", "plate_black@2x.png"),
            ],
            knob: [
                [
                    pair!("knob_s_silver@1x.png", "knob_s_silver@2x.png"),
                    pair!("knob_s_black@1x.png", "knob_s_black@2x.png"),
                ],
                [
                    pair!("knob_m_silver@1x.png", "knob_m_silver@2x.png"),
                    pair!("knob_m_black@1x.png", "knob_m_black@2x.png"),
                ],
            ],
        }
    }
}

pub fn image(p: &Painter, tex: &TextureHandle, rect: Rect) {
    p.image(
        tex.id(),
        rect,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
}

// -----------------------------------------------------------------------------------------
// Drawing helpers
// -----------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Al {
    Left,
    Center,
    Right,
}

/// Text style: face, size, tracking (em), colour, alignment.
#[derive(Clone, Copy)]
pub struct T {
    pub font: F,
    pub size: f32,
    pub track: f32,
    pub color: Color32,
    pub align: Al,
}

impl T {
    pub fn new(font: F, size: f32, track: f32, color: Color32) -> Self {
        T {
            font,
            size,
            track,
            color,
            align: Al::Left,
        }
    }
    pub fn center(mut self) -> Self {
        self.align = Al::Center;
        self
    }
    pub fn right(mut self) -> Self {
        self.align = Al::Right;
        self
    }
    pub fn color(mut self, c: Color32) -> Self {
        self.color = c;
        self
    }
}

fn galley(p: &Painter, t: T, s: &str) -> Arc<epaint::Galley> {
    let mut job = LayoutJob::default();
    job.append(
        s,
        0.0,
        TextFormat {
            font_id: t.font.id(t.size),
            extra_letter_spacing: t.track * t.size,
            color: t.color,
            ..Default::default()
        },
    );
    p.layout_job(job)
}

pub fn text_width(p: &Painter, t: T, s: &str) -> f32 {
    // Trailing letter spacing is not part of the visual width.
    (galley(p, t, s).size().x - t.track * t.size).max(0.0)
}

/// Draws text with its baseline at `y`; returns its width.
pub fn text(p: &Painter, t: T, x: f32, y: f32, s: &str) -> f32 {
    if s.is_empty() {
        return 0.0;
    }
    let g = galley(p, t, s);
    let base = g
        .rows
        .first()
        .and_then(|r| r.glyphs.first())
        .map(|gl| gl.pos.y)
        .unwrap_or(t.size * 0.8);
    let w = (g.size().x - t.track * t.size).max(0.0);
    let x0 = match t.align {
        Al::Left => x,
        Al::Center => x - w * 0.5,
        Al::Right => x - w,
    };
    p.galley(Pos2::new(x0, y - base), g, t.color);
    w
}

/// VFD text: a soft glow pass, then the lit text.
pub fn vfd_text(p: &Painter, t: T, x: f32, y: f32, s: &str) -> f32 {
    let glow = T {
        color: alpha(t.color, 0.22),
        ..t
    };
    text(p, glow, x - 0.8, y, s);
    text(p, glow, x + 0.8, y + 0.4, s);
    text(p, t, x, y, s)
}

pub fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h))
}

pub fn line(p: &Painter, a: Pos2, b: Pos2, color: Color32, w: f32) {
    p.line_segment([a, b], Stroke::new(w, color));
}

pub fn hline(p: &Painter, x0: f32, x1: f32, y: f32, color: Color32, w: f32) {
    line(p, Pos2::new(x0, y), Pos2::new(x1, y), color, w);
}

/// Outline points of a rounded rectangle, clockwise.
fn rrect_points(rect: Rect, rad: f32) -> Vec<Pos2> {
    let rad = rad
        .min(rect.width() * 0.5)
        .min(rect.height() * 0.5)
        .max(0.0);
    let mut pts = Vec::with_capacity(40);
    let corners = [
        (rect.right() - rad, rect.top() + rad, -90.0f32),
        (rect.right() - rad, rect.bottom() - rad, 0.0),
        (rect.left() + rad, rect.bottom() - rad, 90.0),
        (rect.left() + rad, rect.top() + rad, 180.0),
    ];
    for (cx, cy, a0) in corners {
        for k in 0..=8 {
            let a = (a0 + 90.0 * k as f32 / 8.0).to_radians();
            pts.push(Pos2::new(cx + rad * a.cos(), cy + rad * a.sin()));
        }
    }
    pts
}

pub fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        l(a.r(), b.r()),
        l(a.g(), b.g()),
        l(a.b(), b.b()),
        l(a.a(), b.a()),
    )
}

/// Rounded rectangle filled with a vertical gradient.
pub fn grad_rrect(p: &Painter, rect: Rect, rad: f32, top: Color32, bottom: Color32) {
    let pts = rrect_points(rect, rad);
    let mut mesh = Mesh::default();
    let col = |y: f32| {
        lerp(
            top,
            bottom,
            ((y - rect.top()) / rect.height()).clamp(0.0, 1.0),
        )
    };
    mesh.colored_vertex(rect.center(), col(rect.center().y));
    for q in &pts {
        mesh.colored_vertex(*q, col(q.y));
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    p.add(Shape::mesh(mesh));
}

pub fn fill_rrect(p: &Painter, rect: Rect, rad: f32, color: Color32) {
    p.rect_filled(rect, rad, color);
}

pub fn stroke_rrect(p: &Painter, rect: Rect, rad: f32, color: Color32, w: f32) {
    p.rect_stroke(rect, rad, Stroke::new(w, color), egui::StrokeKind::Inside);
}

pub fn shadow(p: &Painter, rect: Rect, rad: f32, dy: i8, blur: u8, a: u8) {
    let s = epaint::Shadow {
        offset: [0, dy],
        blur,
        spread: 0,
        color: Color32::from_black_alpha(a),
    };
    p.add(s.as_shape(rect, rad.min(255.0) as u8));
}

fn inset_shadow(p: &Painter, rect: Rect, rad: f32) {
    let band = Rect::from_min_size(
        rect.min,
        Vec2::new(rect.width(), 8.0_f32.min(rect.height())),
    );
    grad_rrect(
        p,
        band,
        rad,
        Color32::from_black_alpha(200),
        Color32::from_black_alpha(0),
    );
}

/// Black glass window with an inset shadow (small VFDs and graph windows).
pub fn glass(p: &Painter, rect: Rect, rad: f32) {
    fill_rrect(p, rect, rad, hex(0x060808));
    inset_shadow(p, rect, rad);
}

/// Main VFD glass: radial #0d1716 → #050707 with a 7 % white top reflection.
pub fn main_glass(p: &Painter, rect: Rect) {
    fill_rrect(p, rect, 6.0, hex(0x050707));
    let clip = p.with_clip_rect(rect.shrink(1.0));
    let c = Pos2::new(rect.center().x, rect.bottom() + rect.height() * 0.2);
    let (rx, ry) = (rect.width() * 0.75, rect.height() * 1.3);
    let mut mesh = Mesh::default();
    mesh.colored_vertex(c, hex(0x0d1716));
    for k in 0..=48 {
        let a = std::f32::consts::TAU * k as f32 / 48.0;
        mesh.colored_vertex(
            Pos2::new(c.x + rx * a.cos(), c.y + ry * a.sin()),
            hex(0x050707),
        );
    }
    for k in 0..48u32 {
        mesh.add_triangle(0, 1 + k, 2 + k);
    }
    clip.add(Shape::mesh(mesh));
    let refl = Rect::from_min_size(rect.min, Vec2::new(rect.width(), rect.height() * 0.38));
    grad_rrect(
        p,
        refl,
        6.0,
        Color32::from_white_alpha(18),
        Color32::from_white_alpha(0),
    );
    inset_shadow(p, rect, 6.0);
}

/// Round LED with a glow when lit.
pub fn led(p: &Painter, c: Pos2, rad: f32, on: bool, th: &Theme) {
    if on {
        p.circle_filled(c, rad * 2.0, alpha(th.led_on, 0.12));
        p.circle_filled(c, rad * 1.5, alpha(th.led_on, 0.25));
    }
    p.circle_filled(c, rad, if on { th.led_on } else { th.led_off });
}

/// Rubber pill button body.
pub fn pill(p: &Painter, th: &Theme, rect: Rect, on: bool, pressed: bool) {
    // Fully rounded on the short side (tall pills such as the preset arrows too).
    let rad = rect.height().min(rect.width()) * 0.5;
    shadow(p, rect, rad, 2, 4, if th.is_black() { 128 } else { 60 });
    let (t, b) = if on {
        (th.btn_on_top, th.btn_on_bottom)
    } else {
        (th.btn_top, th.btn_bottom)
    };
    let (t, b) = if pressed { (b, t) } else { (t, b) };
    grad_rrect(p, rect, rad, t, b);
    stroke_rrect(p, rect, rad, th.btn_border, 1.0);
    if !on && !pressed && rect.width() > 2.0 * rad + 4.0 {
        let hl = Color32::from_white_alpha((255.0 * th.btn_highlight) as u8);
        hline(
            p,
            rect.left() + rad,
            rect.right() - rad,
            rect.top() + 1.5,
            hl,
            1.0,
        );
    }
}

/// Points along a circular arc; angles in radians, 0 = +x, clockwise (screen space).
pub fn arc(c: Pos2, rad: f32, a0: f32, a1: f32) -> Vec<Pos2> {
    let n = (((a1 - a0).abs() * rad / 3.0).ceil() as usize).clamp(2, 96);
    (0..=n)
        .map(|k| {
            let a = a0 + (a1 - a0) * k as f32 / n as f32;
            Pos2::new(c.x + rad * a.cos(), c.y + rad * a.sin())
        })
        .collect()
}

pub fn polyline(p: &Painter, pts: Vec<Pos2>, color: Color32, w: f32) {
    p.add(Shape::line(pts, Stroke::new(w, color)));
}

pub fn dashed(p: &Painter, pts: &[Pos2], color: Color32, w: f32, dash: f32) {
    p.extend(Shape::dashed_line(pts, Stroke::new(w, color), dash, dash));
}
