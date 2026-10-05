//! Global preferences (theme, VFD colour, UI scale), stored in a small JSON file in the
//! user data folder rather than in the host project.

use crate::editor::theme::{ThemeKind, VfdColor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Prefs {
    pub theme: ThemeKind,
    pub vfd: VfdColor,
    /// Window size for new editors, as a factor of the `layout::W` × `layout::H` panel.
    pub scale: f64,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            theme: ThemeKind::Silver,
            vfd: VfdColor::Cyan,
            scale: 1.0,
        }
    }
}

/// `RS92_PREFS_DIR` overrides the folder (used by tests).
fn path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("RS92_PREFS_DIR") {
        return Some(PathBuf::from(dir).join("prefs.json"));
    }
    directories::BaseDirs::new().map(|d| d.data_dir().join("Tenant RS-92").join("prefs.json"))
}

impl Prefs {
    pub fn load() -> Self {
        path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(p) = path() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(p, serde_json::to_string_pretty(self).unwrap_or_default());
        }
    }
}
