//! Factory presets load, and save → reload gives identical parameters.

use rs92_presets::preset::{factory, load_dir, save_to};
use rs92_presets::Preset;

#[test]
fn factory_presets_load() {
    let presets = factory();
    assert_eq!(presets.len(), 64);
    let a01 = presets
        .iter()
        .find(|p| p.name == "LANDLORD '91")
        .expect("A01");
    assert_eq!(a01.patch, rs92_dsp::Patch::default());
    for p in &presets {
        assert!(
            p.name.len() <= 14 && p.name == p.name.to_ascii_uppercase(),
            "{}",
            p.name
        );
        assert!(
            ["A", "B", "C", "D"].contains(&p.bank.as_str()),
            "{}",
            p.bank
        );
    }
}

#[test]
fn save_then_reload_is_identical() {
    let dir = std::env::temp_dir().join(format!("rs92-preset-test-{}", std::process::id()));
    for p in factory() {
        let mut q = p.clone();
        q.bank = "U".into();
        save_to(&dir, &q).unwrap();
        let text = q.to_json();
        let back = Preset::parse(&text).unwrap();
        assert_eq!(back.patch, q.patch, "{}", q.name);
    }
    let loaded = load_dir(&dir);
    assert_eq!(loaded.len(), factory().len());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unknown_keys_are_ignored() {
    let json = r#"{"format": 9, "name": "future", "params": {"op2_pm": 12, "new_thing": 3}, "shiny": true}"#;
    let p = Preset::parse(json).unwrap();
    assert_eq!(p.name, "FUTURE");
    assert_eq!(p.patch.op2_pm, 12.0);
}
