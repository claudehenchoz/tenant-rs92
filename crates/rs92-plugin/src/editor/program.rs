//! Program operations: preset load/step, STORE, COMPARE, RANDOM (on a worker thread),
//! undo, chord learning, keyboard shortcuts.

use super::{Cx, EditorState, ParamHost, VfdMode};
use crate::params::ProgramState;
use nih_plug_egui::egui::{self, Key};
use rs92_presets::random::{self, RandomResult};
use rs92_presets::Preset;
use std::sync::{Arc, Mutex};

/// Undo depth for random results.
pub const UNDO_DEPTH: usize = 16;
/// Hold time for chord learning.
pub const LEARN_HOLD_MS: u128 = 600;

/// Loads library preset `idx`. The patch before it becomes the COMPARE target. An open
/// browser stays open and follows the loaded preset, so presets can be auditioned.
pub fn load_preset(st: &mut EditorState, host: &dyn ParamHost, idx: usize) {
    let Some(preset) = st.library.presets.get(idx).cloned() else {
        return;
    };
    st.compare = Some(host.patch());
    host.apply_patch(&preset.patch);
    host.set_program(ProgramState {
        preset_name: preset.name.clone(),
        bank: preset.bank.clone(),
        index: idx,
        random_seed: None,
        archetype: preset.archetype.clone(),
    });
    st.current = Some(idx);
    st.comparing = false;
    st.seed = None;
    if super::browser::is_open(st) {
        super::browser::open(st);
    } else {
        st.vfd_mode = VfdMode::Normal;
    }
}

/// The ▲ / ▼ buttons: steps through the whole library, wrapping.
pub fn step(st: &mut EditorState, host: &dyn ParamHost, delta: i32) {
    let n = st.library.presets.len();
    if n == 0 {
        return;
    }
    let next = match st.current {
        Some(i) => (i as i32 + delta).rem_euclid(n as i32) as usize,
        None if delta < 0 => n - 1,
        None => 0,
    };
    load_preset(st, host, next);
}

pub fn store_begin(st: &mut EditorState, host: &dyn ParamHost) {
    st.store_name = host.program().preset_name;
    st.store_focus = true;
    st.vfd_mode = VfdMode::Store;
}

/// Saves the current patch to the user bank (or `dir`, for tests).
pub fn store_commit(st: &mut EditorState, host: &dyn ParamHost, dir: Option<std::path::PathBuf>) {
    let name = rs92_presets::preset::sanitize_name(&st.store_name);
    let prog = host.program();
    let preset = Preset {
        name: name.clone(),
        bank: "U".into(),
        category: "USER".into(),
        author: "USER".into(),
        description: String::new(),
        patch: host.patch(),
        seed: prog.random_seed,
        archetype: prog.archetype.clone(),
        path: None,
    };
    let dir = dir.or_else(rs92_presets::preset::user_dir);
    st.vfd_mode = VfdMode::Normal;
    match dir.map(|d| rs92_presets::preset::save_to(&d, &preset)) {
        Some(Ok(path)) => {
            if !st
                .library
                .presets
                .iter()
                .any(|p| p.bank == "U" && p.name == name)
            {
                let mut p = preset;
                p.path = Some(path);
                st.library.presets.push(p);
            }
            let idx = st.library.find("U", &name);
            st.current = idx;
            st.seed = None;
            host.set_program(ProgramState {
                preset_name: name.clone(),
                bank: "U".into(),
                index: idx.unwrap_or(0),
                random_seed: None,
                archetype: prog.archetype,
            });
            st.set_status(format!("SAVED U:{name}"));
        }
        Some(Err(e)) => st.set_status(format!("SAVE FAILED: {e}").to_uppercase()),
        None => st.set_status("NO USER FOLDER"),
    }
}

/// COMPARE: swap between the current patch and the one before the last load/random.
pub fn compare(st: &mut EditorState, host: &dyn ParamHost) {
    match st.compare {
        Some(other) => {
            let now = host.patch();
            host.apply_patch(&other);
            st.compare = Some(now);
            st.comparing = !st.comparing;
            st.set_status(if st.comparing {
                "COMPARE: BEFORE"
            } else {
                "COMPARE: CURRENT"
            });
        }
        None => st.set_status("NOTHING TO COMPARE"),
    }
}

/// Cmd/Ctrl-Z: steps back through the last random results.
pub fn undo(st: &mut EditorState, host: &dyn ParamHost) {
    if let Some((patch, prog)) = st.undo.pop() {
        host.apply_patch(&patch);
        st.current = st.library.find(&prog.bank, &prog.preset_name);
        host.set_program(prog);
        st.seed = None;
        st.set_status("UNDO RANDOM");
    }
}

fn entropy() -> u64 {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mut z = t.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Starts a random patch on a worker thread; `mutate` is the Shift/Alt amount.
pub fn start_random(
    st: &mut EditorState,
    host: &dyn ParamHost,
    mutate: Option<f32>,
    seed: Option<u64>,
) {
    if st.random_job.is_some() {
        return;
    }
    let base = host.patch();
    let archetype = host.program().archetype;
    let locks = host.lock_mask();
    let seed = seed.unwrap_or_else(entropy);
    let slot = Arc::new(Mutex::new(None));
    let out = slot.clone();
    let spawned = std::thread::Builder::new()
        .name("rs92-random".into())
        .spawn(move || {
            let r = match mutate {
                Some(amount) => random::mutate(seed, &base, archetype.as_deref(), amount, locks),
                None => random::generate(seed, archetype.as_deref(), &base, locks),
            };
            if let Ok(mut s) = out.lock() {
                *s = Some(r);
            }
        });
    if spawned.is_ok() {
        st.random_job = Some(slot);
        st.set_status("RANDOM...");
    }
}

pub fn random_busy(st: &EditorState) -> bool {
    st.random_job.is_some()
}

/// Applies a finished random result. Called every frame.
pub fn poll_random(st: &mut EditorState, host: &dyn ParamHost) {
    let done = st
        .random_job
        .as_ref()
        .and_then(|j| j.lock().ok().and_then(|mut s| s.take()));
    if let Some(r) = done {
        st.random_job = None;
        apply_random(st, host, r);
    }
}

fn apply_random(st: &mut EditorState, host: &dyn ParamHost, r: RandomResult) {
    let before = host.patch();
    st.undo.push((before, host.program()));
    if st.undo.len() > UNDO_DEPTH {
        st.undo.remove(0);
    }
    host.apply_patch(&r.patch);
    host.set_program(ProgramState {
        preset_name: r.name.clone(),
        bank: "U".into(),
        index: 0,
        random_seed: Some(r.seed),
        archetype: Some(r.archetype.to_string()),
    });
    st.compare = Some(before);
    st.comparing = false;
    st.current = None;
    st.seed = Some(r.seed);
    st.status = None;
}

/// Stores the held keys as a custom chord: into the selected Custom slot, else the
/// first empty one, else Custom 1. Returns false when no keys are held.
pub fn learn_chord(st: &mut EditorState, host: &dyn ParamHost) -> bool {
    let held: Vec<u8> = (0u8..128).filter(|&n| host.bridge().is_held(n)).collect();
    if held.is_empty() {
        st.set_status("HOLD KEYS TO LEARN");
        return false;
    }
    let patch = host.patch();
    let subs = patch.chord_sub1 as usize + patch.chord_sub2 as usize;
    let mut notes = held;
    notes.truncate(8usize.saturating_sub(subs).max(1));
    let chord = rs92_dsp::CustomChord::from_notes(&notes);
    let slot = match patch.custom_index() {
        Some(i) => i,
        None => host
            .custom_chords()
            .iter()
            .position(|c| c.is_empty())
            .unwrap_or(0),
    };
    let mut chords = host.custom_chords();
    chords[slot] = chord.intervals().to_vec();
    host.set_custom_chords(chords);
    let steps = host.steps("chord_type").unwrap_or(10) as f32;
    host.set_gesture(
        "chord_type",
        (rs92_dsp::ChordType::Custom1.index() + slot) as f32 / steps,
    );
    st.set_status(format!("LEARNED CUSTOM {}", slot + 1));
    true
}

/// Global shortcuts: Cmd/Ctrl-Z undo.
pub fn shortcuts(ui: &mut egui::Ui, cx: &mut Cx) {
    let undo_pressed = ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::Z));
    if undo_pressed {
        undo(cx.st, cx.host);
    }
}
