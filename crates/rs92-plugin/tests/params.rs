//! Parameter IDs are part of the saved-project contract.

use nih_plug::prelude::Params;
use rs92::Rs92Params;

const SPEC_IDS: &[&str] = &[
    "op1_wave",
    "op1_pd",
    "op1_level",
    "op1_fdbk",
    "op2_wave",
    "op2_tune",
    "op2_pd",
    "op2_pm",
    "chord_type",
    "chord_sub1",
    "chord_sub2",
    "chord_spread",
    "chord_strum",
    "chord_detune",
    "vel_tone",
    "f1_type",
    "f1_cutoff",
    "f1_reso",
    "f1_env",
    "f1_key",
    "f2_type",
    "f2_cutoff",
    "f2_reso",
    "f2_env",
    "f2_key",
    "f3_type",
    "f3_cutoff",
    "f3_reso",
    "f3_env",
    "f3_key",
    "amp_a",
    "amp_d",
    "amp_s",
    "amp_r",
    "flt_a",
    "flt_d",
    "flt_s",
    "flt_r",
    "crush_bits",
    "crush_rate",
    "crush_mix",
    "sampler_on",
    "tail_lp",
    "tail_rel",
    "smp_transpose",
    "comp_thresh",
    "tape_level",
    "hyper",
    "width",
    "output",
    "voices",
    "bend_range",
    "vel_amp",
    "os_factor",
    "smp_root",
    "smp_gate",
    "smp_interp",
];

#[test]
fn every_spec_id_exists_and_nothing_else() {
    let params = Rs92Params::default();
    let mut ids: Vec<String> = params
        .param_map()
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    ids.sort();
    let mut spec: Vec<String> = SPEC_IDS.iter().map(|s| s.to_string()).collect();
    spec.sort();
    assert_eq!(ids, spec);
}

#[test]
fn defaults_are_the_landlord_patch() {
    let params = Rs92Params::default();
    let p = params.to_patch(&rs92_dsp::Patch::default());
    assert_eq!(p, rs92_dsp::Patch::default());
}

#[test]
fn preset_id_table_matches_plugin_params() {
    let params = Rs92Params::default();
    let mut plugin: Vec<String> = params
        .param_map()
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    plugin.sort();
    let mut table: Vec<String> = rs92_presets::ids::all()
        .iter()
        .map(|d| d.id.to_string())
        .collect();
    table.sort();
    assert_eq!(plugin, table);
}

#[test]
fn preset_plain_values_round_trip_through_params() {
    // Every plain value the preset table produces maps to a normalized value and back.
    let params = Rs92Params::default();
    let map: std::collections::HashMap<String, _> = params
        .param_map()
        .into_iter()
        .map(|(id, p, _)| (id, p))
        .collect();
    for preset in rs92_presets::preset::factory() {
        for (id, plain) in rs92_presets::ids::plain_values(&preset.patch) {
            let ptr = map[id];
            let n = unsafe { ptr.preview_normalized(plain) };
            let back = unsafe { ptr.preview_plain(n) };
            assert!(
                (back - plain).abs() <= plain.abs() * 1e-3 + 0.051,
                "{} {id}: {plain} -> {back}",
                preset.name
            );
        }
    }
}
