//! Headless UI tests: the real editor panel driven by synthetic mouse and keyboard input
//! against a mock parameter host. Nothing is shown on screen.

use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use nih_plug_egui::egui::{self, Event, Key, Modifiers, PointerButton, Pos2, Rect, Vec2};
use rs92::editor::layout::{self as L, center, keys};
use rs92::editor::{draw_panel, EditorState, MockHost, ParamHost, VfdMode};
use rs92::GuiNote;
use rs92_presets::Library;
use std::time::{Duration, Instant};

type State = (EditorState, MockHost);

fn harness() -> Harness<'static, State> {
    // Keep the grip and size menu away from the real preferences file.
    std::env::set_var(
        "RS92_PREFS_DIR",
        std::env::temp_dir().join(format!("rs92-ui-prefs-{}", std::process::id())),
    );
    let library = Library {
        presets: rs92_presets::preset::factory(),
    };
    let current = library.find("A", "LANDLORD '91");
    let state = (EditorState::new(library, current), MockHost::default());
    let mut h = Harness::builder()
        .with_size(Vec2::new(L::W, L::H))
        .with_step_dt(1.0 / 60.0)
        .build_state(
            |ctx, (st, host): &mut State| draw_panel(ctx, st, host),
            state,
        );
    // First frames install fonts and textures.
    h.step();
    h.step();
    h
}

fn push(h: &mut Harness<State>, ev: Event) {
    h.input_mut().events.push(ev);
}

fn press(h: &mut Harness<State>, pos: Pos2, down: bool, modifiers: Modifiers) {
    push(h, Event::PointerMoved(pos));
    push(
        h,
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: down,
            modifiers,
        },
    );
}

fn click_mod(h: &mut Harness<State>, pos: Pos2, modifiers: Modifiers) {
    // Hover first, like a real pointer: a widget that appeared in the last frame (the
    // browser after a click on the name) is hit-tested against the previous frame.
    push(h, Event::PointerMoved(pos));
    h.step();
    press(h, pos, true, modifiers);
    h.step();
    press(h, pos, false, modifiers);
    h.step();
}

fn click(h: &mut Harness<State>, pos: Pos2) {
    click_mod(h, pos, Modifiers::NONE);
}

fn drag(h: &mut Harness<State>, from: Pos2, to: Pos2, steps: usize) {
    press(h, from, true, Modifiers::NONE);
    h.step();
    for k in 1..=steps {
        let p = from + (to - from) * (k as f32 / steps as f32);
        push(h, Event::PointerMoved(p));
        h.step();
    }
    press(h, to, false, Modifiers::NONE);
    h.step();
}

fn host<'a>(h: &'a Harness<'_, State>) -> &'a MockHost {
    &h.state().1
}

fn st<'a>(h: &'a Harness<'_, State>) -> &'a EditorState {
    &h.state().0
}

const RANDOM: Pos2 = center(L::RANDOM);
/// E4 is the 17th white key from C2.
const E4_KEY: Pos2 = keys::white_center(16);
/// OP2 PM knob (third M knob of the OP 2 row).
const OP2_PM: Pos2 = Pos2::new(L::OP_KNOB_X[2] + 26.0, L::OP2_KNOBS_Y + 20.0);

fn vfd(r: Rect) -> Pos2 {
    center(r.translate(L::VFD.min.to_vec2()))
}

/// First visible row of the browser's preset list.
fn first_row() -> Pos2 {
    Pos2::new(L::VFD.left() + 200.0, L::VFD.top() + 13.0)
}

#[test]
fn renders_the_panel() {
    let mut h = harness();
    h.step();
    let img = h.render().expect("wgpu render");
    assert_eq!((img.width(), img.height()), (L::W as u32, L::H as u32));
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
    std::fs::create_dir_all(&dir).unwrap();
    img.save(dir.join("panel.png")).unwrap();
}

#[test]
fn knob_drag_moves_the_parameter_with_one_gesture() {
    let mut h = harness();
    let before = host(&h).norm("op2_pm");
    let c = OP2_PM;
    drag(&mut h, c, c - Vec2::new(0.0, 100.0), 10);
    let after = host(&h).norm("op2_pm");
    assert!(
        (after - (before + 0.5).min(1.0)).abs() < 0.02,
        "{before} -> {after}"
    );
    let g = host(&h).gestures.borrow();
    let pm: Vec<_> = g
        .iter()
        .filter(|(id, _)| id == "op2_pm")
        .map(|(_, k)| *k)
        .collect();
    assert_eq!(pm.first(), Some(&"begin"));
    assert_eq!(pm.last(), Some(&"end"));
    assert_eq!(pm.iter().filter(|k| **k == "begin").count(), 1);
}

#[test]
fn double_click_resets_a_knob() {
    let mut h = harness();
    let c = OP2_PM;
    drag(&mut h, c, c - Vec2::new(0.0, 60.0), 4);
    assert!((host(&h).norm("op2_pm") - 0.48).abs() > 0.1);
    click(&mut h, c);
    click(&mut h, c);
    assert!((host(&h).plain("op2_pm") - 48.0).abs() < 0.5);
}

#[test]
fn every_rapid_key_click_sounds() {
    let mut h = harness();
    // Ten clicks, one frame apart.
    for _ in 0..10 {
        click(&mut h, E4_KEY);
    }
    // Ten more, with press and release landing in the same frame.
    for _ in 0..10 {
        press(&mut h, E4_KEY, true, Modifiers::NONE);
        press(&mut h, E4_KEY, false, Modifiers::NONE);
        h.step();
    }
    let notes = host(&h).take_notes();
    let ons = notes
        .iter()
        .filter(|n| matches!(n, GuiNote::On { note: 64, .. }))
        .count();
    let offs = notes
        .iter()
        .filter(|n| matches!(n, GuiNote::Off { note: 64 }))
        .count();
    assert_eq!((ons, offs), (20, 20), "{notes:?}");
}

#[test]
fn sliding_across_keys_plays_each_key() {
    let mut h = harness();
    drag(
        &mut h,
        E4_KEY,
        E4_KEY + Vec2::new(2.0 * keys::PITCH, 0.0),
        2,
    );
    let notes = host(&h).take_notes();
    let ons: Vec<u8> = notes
        .iter()
        .filter_map(|n| {
            if let GuiNote::On { note, .. } = n {
                Some(*note)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(ons, vec![64, 65, 67]);
    assert!(matches!(notes.last(), Some(GuiNote::Off { note: 67 })));
}

#[test]
fn buttons_set_their_parameters() {
    let mut h = harness();
    click(&mut h, center(L::chord_pill(1))); // MAJ
    assert_eq!(host(&h).patch().chord_type, rs92_dsp::ChordType::Maj);
    let hyper = host(&h).patch().hyper;
    click(&mut h, center(L::HYPER));
    assert_eq!(host(&h).patch().hyper, !hyper);
    click(&mut h, center(L::bits_pill(0))); // BITS 8
    assert_eq!(host(&h).patch().crush_bits, rs92_dsp::CrushBits::B8);
    click(&mut h, center(L::SUB1));
    assert!(!host(&h).patch().chord_sub1);
    let wave_y = L::OP1_WAVE.center().y;
    click(&mut h, Pos2::new(L::OP1_WAVE.right() - 10.0, wave_y)); // right: next
    assert_eq!(host(&h).patch().op1_wave, rs92_dsp::Wave::Organ);
    click(&mut h, Pos2::new(L::OP1_WAVE.left() + 8.0, wave_y)); // left third: previous
    assert_eq!(host(&h).patch().op1_wave, rs92_dsp::Wave::ResIII);
    click(&mut h, center(L::filter_type(0))); // F1 type chip
    assert_eq!(host(&h).patch().filters[0].ty, rs92_dsp::FilterType::Lp24);
    click(&mut h, center(L::tab(0))); // 01 lock tab
    assert_eq!(host(&h).lock_mask(), 1);
}

#[test]
fn quick_repeated_button_clicks_all_register() {
    let mut h = harness();
    let start = st(&h).current.unwrap();
    for _ in 0..5 {
        click(&mut h, center(L::PROG_DOWN)); // next
    }
    assert_eq!(st(&h).current, Some(start + 5));
}

fn wait_for_random(h: &mut Harness<State>) {
    let t = Instant::now();
    while st(h).random_job.is_some() {
        assert!(
            t.elapsed() < Duration::from_secs(20),
            "random never finished"
        );
        std::thread::sleep(Duration::from_millis(5));
        h.step();
    }
    h.step();
}

#[test]
fn random_applies_a_patch_and_undo_restores() {
    let mut h = harness();
    let before = host(&h).patch();
    click(&mut h, RANDOM);
    wait_for_random(&mut h);
    let after = host(&h).patch();
    assert_ne!(before, after);
    assert!(host(&h).program().random_seed.is_some());
    assert!(st(&h).seed.is_some());

    // Shift-click mutates.
    click_mod(&mut h, RANDOM, Modifiers::SHIFT);
    wait_for_random(&mut h);
    assert_ne!(host(&h).patch(), after);

    // Ctrl/Cmd-Z twice goes back to the start.
    for _ in 0..2 {
        push(
            &mut h,
            Event::Key {
                key: Key::Z,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            },
        );
        h.step();
    }
    assert_eq!(host(&h).patch(), before);
}

#[test]
fn arrows_step_presets() {
    let mut h = harness();
    let start = st(&h).current.unwrap();
    click(&mut h, center(L::PROG_DOWN));
    click(&mut h, center(L::PROG_DOWN));
    click(&mut h, center(L::PROG_UP));
    assert_eq!(st(&h).current, Some(start + 1));
    // Up from the first preset wraps to the end of the library.
    click(&mut h, center(L::PROG_UP));
    click(&mut h, center(L::PROG_UP));
    assert_eq!(st(&h).current, Some(st(&h).library.presets.len() - 1));
}

#[test]
fn browser_opens_navigates_and_loads() {
    let mut h = harness();
    click(&mut h, vfd(L::vfd::NAME_HIT)); // preset name
    assert!(matches!(st(&h).vfd_mode, VfdMode::Browser { .. }));
    h.press_key(Key::ArrowDown);
    h.press_key(Key::ArrowDown);
    h.press_key(Key::Enter);
    h.step();
    assert_eq!(host(&h).program().preset_name, "HOUSE PIANO");
    // The browser stays open, on the loaded preset.
    assert!(matches!(st(&h).vfd_mode, VfdMode::Browser { sel: 2, .. }));
}

#[test]
fn browser_stays_open_while_loading() {
    let mut h = harness();
    click(&mut h, vfd(L::vfd::NAME_HIT));
    h.press_key(Key::ArrowDown);
    h.press_key(Key::Enter);
    h.step();
    assert_eq!(host(&h).program().preset_name, "M1 ORGAN");
    // Clicking a row loads it too, still open.
    click(&mut h, first_row());
    assert_eq!(host(&h).program().preset_name, "LANDLORD '91");
    assert!(matches!(st(&h).vfd_mode, VfdMode::Browser { sel: 0, .. }));
    // The arrows step and the browser follows.
    click(&mut h, center(L::PROG_DOWN));
    click(&mut h, center(L::PROG_DOWN));
    assert_eq!(host(&h).program().preset_name, "HOUSE PIANO");
    assert!(matches!(st(&h).vfd_mode, VfdMode::Browser { sel: 2, .. }));
    // Bank column, bank B, into its presets, load the first: still open.
    for key in [Key::ArrowLeft, Key::ArrowDown, Key::Enter, Key::Enter] {
        h.press_key(key);
        h.step();
    }
    assert_eq!(host(&h).program().bank, "B");
    assert!(matches!(st(&h).vfd_mode, VfdMode::Browser { .. }));
}

#[test]
fn browser_close_button_and_escape() {
    let mut h = harness();
    click(&mut h, vfd(L::vfd::NAME_HIT));
    assert!(matches!(st(&h).vfd_mode, VfdMode::Browser { .. }));
    click(&mut h, vfd(L::vfd::CLOSE));
    assert_eq!(st(&h).vfd_mode, VfdMode::Normal);
    click(&mut h, vfd(L::vfd::NAME_HIT));
    h.press_key(Key::Escape);
    h.step();
    assert_eq!(st(&h).vfd_mode, VfdMode::Normal);
}

#[test]
fn grip_drag_requests_aspect_locked_size() {
    let mut h = harness();
    let c = center(L::GRIP);
    drag(&mut h, c, c - Vec2::new(L::W * 0.25, L::H * 0.25), 6);
    let req = host(&h).size_requests.borrow().clone();
    assert!(req.len() > 1, "{req:?}");
    for (w, h) in &req {
        assert!(
            (*w as f32 / *h as f32 - L::W / L::H).abs() < 0.01,
            "{w}x{h}"
        );
    }
    // A quarter of the panel inwards on both axes: 75 %.
    let want = ((L::W * 0.75).round() as u32, (L::H * 0.75).round() as u32);
    assert_eq!(req.last(), Some(&want));
    assert!((st(&h).prefs.scale - 0.75).abs() < 1e-9);
}

#[test]
fn size_menu_applies_immediately() {
    let mut h = harness();
    let pos = center(L::BRAND);
    push(&mut h, Event::PointerMoved(pos));
    for pressed in [true, false] {
        push(
            &mut h,
            Event::PointerButton {
                pos,
                button: PointerButton::Secondary,
                pressed,
                modifiers: Modifiers::NONE,
            },
        );
    }
    h.step();
    h.step();
    h.get_by_label("150%").click();
    h.step();
    h.step();
    let want = ((L::W * 1.5).round() as u32, (L::H * 1.5).round() as u32);
    assert_eq!(host(&h).size_requests.borrow().last(), Some(&want));
    assert_eq!(st(&h).prefs.scale, 1.5);
}

#[test]
fn store_writes_a_user_preset() {
    let dir = std::env::temp_dir().join(format!("rs92-ui-store-{}", std::process::id()));
    std::env::set_var("RS92_USER_DIR", &dir);
    let mut h = harness();
    click(&mut h, center(L::STORE));
    assert_eq!(st(&h).vfd_mode, VfdMode::Store);
    h.step();
    // Replace the name.
    for _ in 0..20 {
        h.press_key(Key::Backspace);
    }
    push(&mut h, Event::Text("my stab".into()));
    h.step();
    h.press_key(Key::Enter);
    h.step();
    assert_eq!(st(&h).vfd_mode, VfdMode::Normal);
    assert_eq!(host(&h).program().preset_name, "MY STAB");
    assert!(dir.join("MY_STAB.json").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn chord_learn_by_long_press() {
    let mut h = harness();
    // Hold C4, E4, G4 on the "MIDI keyboard".
    for n in [60u8, 64, 67] {
        let w = &host(&h).bridge.held_keys[(n / 64) as usize];
        w.fetch_or(1 << (n % 64), std::sync::atomic::Ordering::Relaxed);
    }
    let btn = center(L::chord_pill(0)); // MIN
    press(&mut h, btn, true, Modifiers::NONE);
    h.step();
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(700) {
        std::thread::sleep(Duration::from_millis(20));
        h.step();
    }
    press(&mut h, btn, false, Modifiers::NONE);
    h.step();
    assert_eq!(host(&h).custom_chords()[0], vec![0, 4, 7]);
    assert_eq!(host(&h).patch().chord_type, rs92_dsp::ChordType::Custom1);
}

#[test]
fn frame_time_is_small() {
    let ctx = egui::Context::default();
    let library = Library {
        presets: rs92_presets::preset::factory(),
    };
    let mut state = EditorState::new(library, Some(0));
    let host = MockHost::default();
    let input = || egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(L::W, L::H))),
        ..Default::default()
    };
    for _ in 0..5 {
        let out = ctx.run(input(), |ctx| draw_panel(ctx, &mut state, &host));
        ctx.tessellate(out.shapes, out.pixels_per_point);
    }
    let frames = 300;
    let t = Instant::now();
    for _ in 0..frames {
        let out = ctx.run(input(), |ctx| draw_panel(ctx, &mut state, &host));
        let _ = ctx.tessellate(out.shapes, out.pixels_per_point);
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0 / frames as f64;
    println!("frame: {ms:.2} ms");
    let budget = if cfg!(debug_assertions) { 40.0 } else { 4.0 };
    assert!(ms < budget, "{ms:.2} ms per frame");
}

#[test]
fn context_menus_and_black_theme_render() {
    let mut h = harness();
    for pos in [
        OP2_PM,
        center(L::OP1_WAVE),
        center(L::chord_pill(1)),
        center(L::BRAND),
    ] {
        push(&mut h, Event::PointerMoved(pos));
        push(
            &mut h,
            Event::PointerButton {
                pos,
                button: PointerButton::Secondary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        );
        push(
            &mut h,
            Event::PointerButton {
                pos,
                button: PointerButton::Secondary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
        );
        h.step();
        h.step();
        assert!(h.ctx.is_context_menu_open(), "menu at {pos:?}");
        h.render().expect("render with menu");
        h.press_key(Key::Escape);
        h.step();
        h.step();
    }
    h.state_mut().0.prefs.theme = rs92::editor::theme::ThemeKind::BlackEs;
    h.step();
    let img = h.render().expect("black theme render");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
    std::fs::create_dir_all(&dir).unwrap();
    img.save(dir.join("panel-black.png")).unwrap();
}
