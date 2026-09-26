//! Headless UI tests: the real editor panel driven by synthetic mouse and keyboard input
//! against a mock parameter host. Nothing is shown on screen.

use egui_kittest::Harness;
use nih_plug_egui::egui::{self, Event, Key, Modifiers, PointerButton, Pos2, Vec2};
use rs92::editor::{draw_panel, EditorState, MockHost, ParamHost, VfdMode};
use rs92::GuiNote;
use rs92_presets::Library;
use std::time::{Duration, Instant};

type State = (EditorState, MockHost);

fn harness() -> Harness<'static, State> {
    let library = Library {
        presets: rs92_presets::preset::factory(),
    };
    let current = library.find("A", "LANDLORD '91");
    let state = (EditorState::new(library, current), MockHost::default());
    let mut h = Harness::builder()
        .with_size(Vec2::new(1280.0, 800.0))
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

const RANDOM: Pos2 = Pos2::new(1168.0, 128.0);
const E4_KEY: Pos2 = Pos2::new(618.0, 350.0);

#[test]
fn renders_the_panel() {
    let mut h = harness();
    h.step();
    let img = h.render().expect("wgpu render");
    assert_eq!((img.width(), img.height()), (1280, 800));
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
    std::fs::create_dir_all(&dir).unwrap();
    img.save(dir.join("panel.png")).unwrap();
}

#[test]
fn knob_drag_moves_the_parameter_with_one_gesture() {
    let mut h = harness();
    let before = host(&h).norm("op2_pm");
    let c = Pos2::new(304.0, 386.0);
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
    let c = Pos2::new(304.0, 386.0);
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
    drag(&mut h, E4_KEY, E4_KEY + Vec2::new(26.0, 0.0), 2);
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
    click(&mut h, Pos2::new(491.0, 263.0)); // MAJ
    assert_eq!(host(&h).patch().chord_type, rs92_dsp::ChordType::Maj);
    let hyper = host(&h).patch().hyper;
    click(&mut h, Pos2::new(990.0, 671.0)); // HYPER STAB
    assert_eq!(host(&h).patch().hyper, !hyper);
    click(&mut h, Pos2::new(576.0, 563.0)); // BITS 8
    assert_eq!(host(&h).patch().crush_bits, rs92_dsp::CrushBits::B8);
    click(&mut h, Pos2::new(446.0, 392.0)); // SUB -1 OCT
    assert!(!host(&h).patch().chord_sub1);
    click(&mut h, Pos2::new(110.0, 303.0)); // OP1 wave, right half: next
    assert_eq!(host(&h).patch().op1_wave, rs92_dsp::Wave::Organ);
    click(&mut h, Pos2::new(40.0, 303.0)); // left third: previous
    assert_eq!(host(&h).patch().op1_wave, rs92_dsp::Wave::ResIII);
    click(&mut h, Pos2::new(880.0, 288.0)); // F1 type chip
    assert_eq!(host(&h).patch().filters[0].ty, rs92_dsp::FilterType::Lp24);
    click(&mut h, Pos2::new(41.0, 189.0)); // 01 lock tab
    assert_eq!(host(&h).lock_mask(), 1);
}

#[test]
fn quick_repeated_button_clicks_all_register() {
    let mut h = harness();
    let start = st(&h).current.unwrap();
    for _ in 0..5 {
        click(&mut h, Pos2::new(1216.0, 44.0)); // NEXT
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
fn jog_drag_steps_presets_and_turns() {
    let mut h = harness();
    let start = st(&h).current.unwrap();
    let angle = st(&h).jog_angle;
    let c = Pos2::new(1008.0, 77.0);
    drag(&mut h, c, c + Vec2::new(0.0, 74.0), 8);
    assert_eq!(st(&h).current, Some(start + 3));
    assert!(st(&h).jog_angle > angle);
}

#[test]
fn browser_opens_navigates_and_loads() {
    let mut h = harness();
    click(&mut h, Pos2::new(560.0, 80.0)); // preset name
    assert!(matches!(st(&h).vfd_mode, VfdMode::Browser { .. }));
    h.press_key(Key::ArrowDown);
    h.press_key(Key::ArrowDown);
    h.press_key(Key::Enter);
    h.step();
    assert_eq!(st(&h).vfd_mode, VfdMode::Normal);
    assert_eq!(host(&h).program().preset_name, "HOUSE PIANO");
}

#[test]
fn store_writes_a_user_preset() {
    let dir = std::env::temp_dir().join(format!("rs92-ui-store-{}", std::process::id()));
    std::env::set_var("RS92_USER_DIR", &dir);
    let mut h = harness();
    click(&mut h, Pos2::new(1120.0, 86.0)); // STORE
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
    let btn = Pos2::new(428.0, 263.0); // MIN
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
        screen_rect: Some(egui::Rect::from_min_size(
            Pos2::ZERO,
            Vec2::new(1280.0, 800.0),
        )),
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
        Pos2::new(304.0, 386.0),
        Pos2::new(110.0, 303.0),
        Pos2::new(491.0, 263.0),
        Pos2::new(100.0, 60.0),
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
