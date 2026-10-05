//! TENANT RS-92 editor: the compact hi-fi faceplate (`layout`), drawn with egui.
//!
//! egui is immediate mode: every frame reads the parameters and the audio bridge and
//! repaints, so nothing can go stale. All plugin access goes through [`ParamHost`], which
//! lets the whole panel run headless in tests (`tests/ui.rs`).
//!
//! The panel is drawn in fixed design points (`layout`) and zoomed to fill the window, so
//! resizing only changes the window size: the grip and the size menu ask the host for a
//! new one through [`ParamHost::request_size`].

use crate::prefs::Prefs;
use crate::{Rs92Params, UiBridge};
use nih_plug::prelude::Editor;
use nih_plug_egui::egui::{self, Context, Pos2};
use nih_plug_egui::{create_egui_editor, EguiState};
use rs92_dsp::Patch;
use rs92_presets::random::RandomResult;
use rs92_presets::Library;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub mod browser;
pub mod displays;
pub mod host;
pub mod layout;
pub mod program;
pub mod theme;
pub mod widgets;

pub use host::{MockHost, ParamHost, ParamTable, PluginHost};
use theme::{Textures, Theme};

/// Window sizes offered in the setup menu (1.0 = `layout::W` × `layout::H`).
pub const SIZES: [f64; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];
pub const MIN_SCALE: f64 = 0.5;
pub const MAX_SCALE: f64 = 2.5;
/// How long the parameter peek and status messages stay on the VFD.
pub const PEEK_HOLD: Duration = Duration::from_millis(1500);

pub fn default_state() -> Arc<EguiState> {
    let s = Prefs::load().scale.clamp(MIN_SCALE, MAX_SCALE);
    EguiState::from_size(
        (layout::W as f64 * s).round() as u32,
        (layout::H as f64 * s).round() as u32,
    )
}

/// Main-VFD modes.
#[derive(Clone, Debug, PartialEq)]
pub enum VfdMode {
    Normal,
    /// Preset browser inside the glass: bank column and preset column.
    Browser {
        bank: usize,
        sel: usize,
        focus_presets: bool,
    },
    /// Inline name editor for STORE.
    Store,
}

/// GUI-side copy of the decimated output, and meter levels with GUI-side release.
pub struct ScopeState {
    pub ring: Vec<f32>,
    pub pos: usize,
    pub level: [f32; 2],
}

impl Default for ScopeState {
    fn default() -> Self {
        ScopeState {
            ring: vec![0.0; crate::SCOPE_LEN],
            pos: 0,
            level: [0.0; 2],
        }
    }
}

impl ScopeState {
    pub fn pull(&mut self, bridge: &UiBridge) {
        if let Some(mut rx) = bridge.scope_rx.try_lock() {
            while let Ok(v) = rx.pop() {
                self.ring[self.pos] = v;
                self.pos = (self.pos + 1) % self.ring.len();
            }
        }
        for ch in 0..2 {
            let p = bridge.peak[ch].load(std::sync::atomic::Ordering::Relaxed);
            self.level[ch] = if p > self.level[ch] {
                p
            } else {
                self.level[ch] * 0.9
            };
        }
    }
}

/// Editor state that lives as long as the editor.
pub struct EditorState {
    pub prefs: Prefs,
    pub library: Library,
    /// Index of the loaded preset in `library`.
    pub current: Option<usize>,
    pub vfd_mode: VfdMode,
    /// The patch before the last load / random result, for COMPARE.
    pub compare: Option<Patch>,
    pub comparing: bool,
    /// Random results for Cmd/Ctrl-Z, newest last.
    pub undo: Vec<(Patch, crate::params::ProgramState)>,
    pub seed: Option<u64>,
    pub peek: Option<(String, Instant)>,
    pub status: Option<(String, Instant)>,
    pub random_job: Option<Arc<Mutex<Option<RandomResult>>>>,
    pub scope: ScopeState,
    /// VU needle in dB relative to 0 VU.
    pub vu: f32,
    /// Resize grip drag: window corner minus pointer, in logical pixels.
    pub grip: Option<egui::Vec2>,
    /// Scale the grip drag last asked for.
    pub grip_scale: f64,
    /// Last window size requested from the host.
    pub requested_size: Option<(u32, u32)>,
    /// Knob / fader drag accumulators by parameter ID.
    pub drag_value: f32,
    /// Key held on the on-screen keyboard.
    pub playing_key: Option<u8>,
    /// Chord button being held: (index, since, learned).
    pub chord_press: Option<(usize, Instant, bool)>,
    /// Open "type value" box: (parameter ID, text, position).
    pub type_value: Option<(String, String, Pos2)>,
    pub type_value_focus: bool,
    pub store_name: String,
    pub store_focus: bool,
    pub chord_root: i32,
    pub midi_seen: u32,
    pub midi_flash: Option<Instant>,
    /// The open parameter menu shows the MORE page.
    pub menu_more: bool,
    tex: Option<Arc<Textures>>,
}

impl EditorState {
    pub fn new(library: Library, current: Option<usize>) -> Self {
        EditorState {
            prefs: Prefs::load(),
            library,
            current,
            vfd_mode: VfdMode::Normal,
            compare: None,
            comparing: false,
            undo: vec![],
            seed: None,
            peek: None,
            status: None,
            random_job: None,
            scope: ScopeState::default(),
            vu: -30.0,
            grip: None,
            grip_scale: 1.0,
            requested_size: None,
            drag_value: 0.0,
            playing_key: None,
            chord_press: None,
            type_value: None,
            type_value_focus: false,
            store_name: String::new(),
            store_focus: false,
            chord_root: 64,
            midi_seen: 0,
            midi_flash: None,
            menu_more: false,
            tex: None,
        }
    }

    pub fn theme(&self) -> Theme {
        Theme::new(self.prefs.theme, self.prefs.vfd)
    }

    pub fn set_peek(&mut self, text: String) {
        self.peek = Some((text, Instant::now()));
    }

    pub fn set_status(&mut self, text: impl Into<String>) {
        self.status = Some((text.into(), Instant::now()));
    }
}

/// Everything a widget needs for one frame.
pub struct Cx<'a> {
    pub st: &'a mut EditorState,
    pub host: &'a dyn ParamHost,
    pub th: Theme,
    pub tex: Arc<Textures>,
    /// Use the 2× bitmaps.
    pub hi: bool,
    /// Patch snapshot taken at the start of the frame.
    pub patch: Patch,
}

/// One-time setup: fonts, bitmaps, widget style.
pub fn setup(ctx: &Context, st: &mut EditorState) {
    theme::install_fonts(ctx);
    st.tex = Some(Arc::new(Textures::load(ctx)));
    ctx.style_mut(|s| {
        s.interaction.tooltip_delay = 0.6;
        s.spacing.menu_margin = egui::Margin::same(6);
        s.visuals.menu_corner_radius = 6.0.into();
    });
}

/// Scales the panel to the window, keeping its aspect ratio.
fn fit_zoom(ctx: &Context) {
    let ppp = ctx.pixels_per_point();
    let px = ctx.screen_rect().size() * ppp;
    let want = (px.x / layout::W).min(px.y / layout::H).max(0.25);
    let native = ctx.native_pixels_per_point().unwrap_or(1.0);
    let zoom = want / native;
    if (ctx.zoom_factor() - zoom).abs() > 1e-3 {
        ctx.set_zoom_factor(zoom);
    }
}

/// Draws the whole panel. Called every frame by the plugin editor and by the UI tests.
pub fn draw_panel(ctx: &Context, st: &mut EditorState, host: &dyn ParamHost) {
    if st.tex.is_none() {
        setup(ctx, st);
    }
    // egui applies new fonts from the next frame on; draw nothing until they are live.
    let ready = ctx.fonts(|f| {
        f.families()
            .contains(&egui::FontFamily::Name("logo".into()))
    });
    if !ready {
        ctx.request_repaint();
        return;
    }
    fit_zoom(ctx);
    if !ctx.is_context_menu_open() {
        st.menu_more = false;
    }
    let tex = st.tex.clone().expect("textures loaded");
    let hi = ctx.pixels_per_point() > 1.25;
    let th = st.theme();
    // Menus and text fields follow the finish.
    let dark = th.is_black();
    if ctx.style().visuals.dark_mode != dark {
        ctx.set_visuals(if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
    }
    let patch = host.patch();
    program::poll_random(st, host);
    st.scope.pull(host.bridge());

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let mut cx = Cx {
                st,
                host,
                th,
                tex,
                hi,
                patch,
            };
            program::shortcuts(ui, &mut cx);
            widgets::panel(ui, &mut cx);
        });
}

pub fn create(params: Arc<Rs92Params>, bridge: Arc<UiBridge>) -> Option<Box<dyn Editor>> {
    let table = ParamTable::new(params.clone());
    let prog = params.program.read().map(|p| p.clone()).unwrap_or_default();
    let library = Library::load();
    let current = library.find(&prog.bank, &prog.preset_name);
    let state = (table, bridge, EditorState::new(library, current));
    create_egui_editor(
        params.editor_state.clone(),
        state,
        |ctx, (_, _, st)| setup(ctx, st),
        |ctx, setter, (table, bridge, st)| {
            let host = PluginHost {
                table,
                bridge,
                setter,
            };
            draw_panel(ctx, st, &host);
        },
    )
}
