//! Preset browser state inside the main VFD: banks on the left, presets on the right.

use super::{EditorState, VfdMode};

pub const ROWS: usize = 4;

pub fn banks(st: &EditorState) -> Vec<&'static str> {
    st.library.banks()
}

/// Library indices of the presets in bank column `bank`.
pub fn presets_in(st: &EditorState, bank: usize) -> Vec<usize> {
    let banks = banks(st);
    match banks.get(bank) {
        Some(b) => st.library.in_bank(b).map(|(i, _)| i).collect(),
        None => vec![],
    }
}

pub fn open(st: &mut EditorState) {
    let banks = banks(st);
    let (bank, sel) = match st
        .current
        .and_then(|i| st.library.presets.get(i).map(|p| (i, p.bank.clone())))
    {
        Some((i, b)) => (
            banks.iter().position(|x| *x == b).unwrap_or(0),
            st.library
                .in_bank(&b)
                .position(|(j, _)| j == i)
                .unwrap_or(0),
        ),
        None => (0, 0),
    };
    st.vfd_mode = VfdMode::Browser {
        bank,
        sel,
        focus_presets: true,
    };
}

pub fn close(st: &mut EditorState) {
    st.vfd_mode = VfdMode::Normal;
}

pub fn is_open(st: &EditorState) -> bool {
    matches!(st.vfd_mode, VfdMode::Browser { .. })
}

pub fn scroll(st: &mut EditorState, delta: i32) {
    if let VfdMode::Browser {
        mut bank,
        mut sel,
        focus_presets,
    } = st.vfd_mode
    {
        if focus_presets {
            let n = presets_in(st, bank).len() as i32;
            sel = (sel as i32 + delta).clamp(0, (n - 1).max(0)) as usize;
        } else {
            let n = banks(st).len() as i32;
            bank = (bank as i32 + delta).clamp(0, (n - 1).max(0)) as usize;
            sel = 0;
        }
        st.vfd_mode = VfdMode::Browser {
            bank,
            sel,
            focus_presets,
        };
    }
}

pub fn switch_column(st: &mut EditorState) {
    if let VfdMode::Browser {
        bank,
        sel,
        focus_presets,
    } = st.vfd_mode
    {
        st.vfd_mode = VfdMode::Browser {
            bank,
            sel,
            focus_presets: !focus_presets,
        };
    }
}

/// Enter: returns the preset to load, or moves focus to the preset column.
pub fn enter(st: &mut EditorState) -> Option<usize> {
    if let VfdMode::Browser {
        bank,
        sel,
        focus_presets,
    } = st.vfd_mode
    {
        if focus_presets {
            return presets_in(st, bank).get(sel).copied();
        }
        st.vfd_mode = VfdMode::Browser {
            bank,
            sel: 0,
            focus_presets: true,
        };
    }
    None
}

pub fn select_bank(st: &mut EditorState, bank: usize) {
    if bank < banks(st).len() {
        st.vfd_mode = VfdMode::Browser {
            bank,
            sel: 0,
            focus_presets: false,
        };
    }
}

/// First visible row of the preset column.
pub fn window_start(sel: usize, n: usize) -> usize {
    if n <= ROWS {
        0
    } else {
        sel.saturating_sub(ROWS / 2).min(n - ROWS)
    }
}
