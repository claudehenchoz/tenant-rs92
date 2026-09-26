//! Preset file format, factory bank and user bank.

use crate::ids::{self, Kind};
use include_dir::{include_dir, Dir};
use rs92_dsp::{CustomChord, Patch};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

pub const FORMAT: u32 = 1;
/// Longest preset name that fits the VFD.
pub const MAX_NAME_LEN: usize = 14;

static FACTORY_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../assets/presets/factory");

/// Bank letters and names.
pub const BANKS: &[(&str, &str)] = &[
    ("A", "CLASSIC RAVE"),
    ("B", "TENANTS"),
    ("C", "WAREHOUSE"),
    ("D", "HARDCORE"),
    ("U", "USER"),
];

pub fn bank_name(bank: &str) -> &'static str {
    BANKS
        .iter()
        .find(|(b, _)| *b == bank)
        .map(|(_, n)| *n)
        .unwrap_or("USER")
}

/// On-disk representation. Unknown keys are ignored so old versions load new files.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PresetFile {
    #[serde(default = "default_format")]
    pub format: u32,
    pub name: String,
    #[serde(default)]
    pub bank: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub params: Map<String, Value>,
    #[serde(default)]
    pub custom_chords: Vec<Vec<u8>>,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archetype: Option<String>,
}

fn default_format() -> u32 {
    FORMAT
}

/// A loaded preset.
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    pub name: String,
    pub bank: String,
    pub category: String,
    pub author: String,
    pub description: String,
    pub patch: Patch,
    pub seed: Option<u64>,
    pub archetype: Option<String>,
    /// File the preset came from (user presets only).
    pub path: Option<PathBuf>,
}

/// Cleans a name for the VFD: uppercase ASCII, at most 14 characters.
pub fn sanitize_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| c.to_ascii_uppercase())
        .filter(|c| c.is_ascii_graphic() || *c == ' ')
        .take(MAX_NAME_LEN)
        .collect();
    let s = s.trim().to_string();
    if s.is_empty() {
        "INIT".into()
    } else {
        s
    }
}

/// Converts a patch into the `params` object: plain values keyed by parameter ID,
/// enums as their string IDs, booleans as `true`/`false`.
pub fn params_to_json(p: &Patch) -> Map<String, Value> {
    let mut m = Map::new();
    for d in ids::all() {
        let v = (d.get)(p);
        let json = match d.kind {
            Kind::Float => {
                // Round to 4 significant decimals so files stay readable.
                let r = (v as f64 * 10_000.0).round() / 10_000.0;
                serde_json::Number::from_f64(r)
                    .map(Value::Number)
                    .unwrap_or(Value::Null)
            }
            Kind::Int => Value::from(v.round() as i64),
            Kind::Bool => Value::Bool(v >= 0.5),
            Kind::Enum(names) => Value::String(names[v.round() as usize].to_string()),
        };
        m.insert(d.id.to_string(), json);
    }
    m
}

/// Applies a `params` object over `base`. Unknown IDs and bad values are ignored.
pub fn params_from_json(base: &Patch, m: &Map<String, Value>) -> Patch {
    let mut p = *base;
    for (id, v) in m {
        let Some(d) = ids::find(id) else { continue };
        let plain = match (d.kind, v) {
            (Kind::Enum(names), Value::String(s)) => names
                .iter()
                .position(|n| n.eq_ignore_ascii_case(s))
                .map(|i| i as f32),
            (_, Value::Bool(b)) => Some(*b as u8 as f32),
            (_, Value::Number(n)) => n.as_f64().map(|x| x as f32),
            _ => None,
        };
        if let Some(x) = plain {
            ids::set_plain(&mut p, id, x);
        }
    }
    p
}

impl Preset {
    pub fn from_file(f: &PresetFile) -> Self {
        let mut patch = params_from_json(&Patch::default(), &f.params);
        for (i, c) in f.custom_chords.iter().take(4).enumerate() {
            patch.custom_chords[i] = chord_from_intervals(c);
        }
        Preset {
            name: sanitize_name(&f.name),
            bank: f.bank.clone(),
            category: f.category.clone(),
            author: f.author.clone(),
            description: f.description.clone(),
            patch,
            seed: f.seed,
            archetype: f.archetype.clone(),
            path: None,
        }
    }

    pub fn to_file(&self) -> PresetFile {
        PresetFile {
            format: FORMAT,
            name: sanitize_name(&self.name),
            bank: self.bank.clone(),
            category: self.category.clone(),
            author: self.author.clone(),
            description: self.description.clone(),
            params: params_to_json(&self.patch),
            custom_chords: self
                .patch
                .custom_chords
                .iter()
                .map(|c| c.intervals().to_vec())
                .collect(),
            seed: self.seed,
            archetype: self.archetype.clone(),
        }
    }

    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        Ok(Self::from_file(&serde_json::from_str(json)?))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.to_file()).expect("preset serialises")
    }
}

pub fn chord_from_intervals(iv: &[u8]) -> CustomChord {
    let mut c = CustomChord::default();
    for (i, &n) in iv.iter().take(8).enumerate() {
        c.notes[i] = n;
        c.len = i as u8 + 1;
    }
    c
}

/// All factory presets, sorted by bank then file name.
pub fn factory() -> Vec<Preset> {
    let mut files: Vec<_> = FACTORY_DIR
        .files()
        .filter(|f| f.path().extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort_by_key(|f| f.path().to_path_buf());
    files
        .iter()
        .filter_map(|f| {
            let text = f.contents_utf8()?;
            match Preset::parse(text) {
                Ok(p) => Some(p),
                Err(e) => {
                    eprintln!("bad factory preset {}: {e}", f.path().display());
                    None
                }
            }
        })
        .collect()
}

/// `~/Library/Application Support/Tenant RS-92/Presets`, `%APPDATA%\Tenant RS-92\Presets`, …
/// `RS92_USER_DIR` overrides the location (used by tests).
pub fn user_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("RS92_USER_DIR") {
        return Some(PathBuf::from(dir));
    }
    directories::BaseDirs::new().map(|d| d.data_dir().join("Tenant RS-92").join("Presets"))
}

/// Loads every `.json` preset in `dir` into the user bank.
pub fn load_dir(dir: &Path) -> Vec<Preset> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut out: Vec<Preset> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            let mut p = Preset::parse(&text).ok()?;
            p.bank = "U".into();
            p.path = Some(path);
            Some(p)
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// File name for a preset name.
pub fn file_name(name: &str) -> String {
    let base: String = sanitize_name(name)
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("{base}.json")
}

/// Saves a preset into `dir`, returning the written path.
pub fn save_to(dir: &Path, preset: &Preset) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(file_name(&preset.name));
    std::fs::write(&path, preset.to_json())?;
    Ok(path)
}

/// Factory and user presets in browsing order.
#[derive(Clone, Debug, Default)]
pub struct Library {
    pub presets: Vec<Preset>,
}

impl Library {
    pub fn load() -> Self {
        let mut presets = factory();
        if let Some(dir) = user_dir() {
            presets.extend(load_dir(&dir));
        }
        Library { presets }
    }

    pub fn banks(&self) -> Vec<&'static str> {
        BANKS
            .iter()
            .map(|(b, _)| *b)
            .filter(|b| self.presets.iter().any(|p| p.bank == *b))
            .collect()
    }

    pub fn in_bank<'a>(&'a self, bank: &'a str) -> impl Iterator<Item = (usize, &'a Preset)> + 'a {
        self.presets
            .iter()
            .enumerate()
            .filter(move |(_, p)| p.bank == bank)
    }

    pub fn find(&self, bank: &str, name: &str) -> Option<usize> {
        self.presets
            .iter()
            .position(|p| p.bank == bank && p.name == name)
    }

    /// Index within its bank (1-based program number).
    pub fn program_number(&self, index: usize) -> usize {
        let Some(p) = self.presets.get(index) else {
            return 0;
        };
        self.presets[..index]
            .iter()
            .filter(|q| q.bank == p.bank)
            .count()
            + 1
    }
}
