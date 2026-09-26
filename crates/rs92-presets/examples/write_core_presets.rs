//! Writes the 18 core factory presets and the 46 bank variations as JSON.
//!
//!     cargo run --release -p rs92-presets --example write_core_presets
//!
//! Variations are 20-30 % mutations of each bank's recipes that pass the randomizer's
//! quality gate. The JSON files are the source of truth afterwards; tune them by ear.

use rs92_presets::random::mutate;
use rs92_presets::recipes::{recipes, to_preset};
use std::path::Path;

const NAMES: [&[&str]; 4] = [
    &[
        "SECOND SUMMER",
        "ACID HOUSE HIT",
        "PIANO ANTHEM",
        "BREAKDOWN MAJ",
        "WAREHOUSE 88",
        "HANDS UP",
    ],
    &[
        "ATTIC FLAT",
        "BEDSIT",
        "DAMP PROOF",
        "GROUND RENT",
        "HOUSING LIST",
        "NOTICE PERIOD",
        "SERVICE CHG",
        "FLAT SHARE",
        "GUARANTOR",
        "INVENTORY",
        "BOX ROOM",
        "TOP FLOOR",
        "SPARE KEYS",
    ],
    &[
        "DOCKLANDS",
        "NIGHT BUS",
        "FOG MACHINE",
        "STROBE MIN7",
        "CONCRETE",
        "LOADING BAY",
        "BERLIN SUS",
        "DUB SIREN",
        "SUB BASEMENT",
        "TAPE ECHO MIN",
        "DEEP FREEZE",
        "GRID MIN9",
        "LOW CEILING",
    ],
    &[
        "HOOVER DAMP",
        "AMEN STAB",
        "PIRATE RADIO",
        "DARKSIDE MIN",
        "BREAKBEAT 92",
        "RUFF MIN9",
        "MENTAL MIN",
        "RAGGA STAB",
        "ROLLER MIN",
        "PLUR CHORD",
        "ESSEX 92",
        "RAVE SIGNAL",
        "JUNGLIST HIT",
        "SUBSONIC",
    ],
];

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/presets/factory");
    let all = recipes();
    for r in &all {
        std::fs::write(dir.join(format!("{}.json", r.file)), to_preset(r).to_json()).unwrap();
    }
    for (b, bank) in ["A", "B", "C", "D"].iter().enumerate() {
        let base: Vec<_> = all.iter().filter(|r| r.bank == *bank).collect();
        let core = base.len();
        for (i, name) in NAMES[b].iter().enumerate() {
            let src = base[i % core];
            let amount = 0.2 + 0.1 * ((i % 3) as f32 / 2.0);
            let mut seed = (b as u64 + 1) * 1000 + i as u64;
            let res = loop {
                let r = mutate(seed, &src.patch, Some(src.archetype), amount, 0);
                if r.passed_on.is_some() {
                    break r;
                }
                seed += 7919;
            };
            let mut p = to_preset(src);
            p.name = name.to_string();
            p.patch = res.patch;
            p.description = format!("Variation of {}", src.name);
            let file = format!(
                "{}{:02}_{}",
                bank,
                core + i + 1,
                name.to_lowercase().replace([' ', '-'], "_")
            );
            std::fs::write(dir.join(format!("{file}.json")), p.to_json()).unwrap();
            println!("{file}");
        }
    }
}
