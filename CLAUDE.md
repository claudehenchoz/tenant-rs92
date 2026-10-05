# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

TENANT RS-92 is a rave-stab synthesizer (VST3, CLAP, standalone) written in Rust on nih-plug
with an egui editor. README.md covers user-facing behaviour, standalone flags, releasing and
the documented interpretations of the design spec; read its "For developers" section before
changing DSP behaviour. The design document it links to is not checked into this repo; the
visual reference is `docs/mockup/Main.dc.html`.

## Commands

Toolchain is pinned in `rust-toolchain.toml` (1.96.1). `cargo xtask` is an alias in
`.cargo/config.toml` for nih-plug's bundler.

```sh
cargo clippy --workspace --all-targets -- -D warnings   # CI lint gate (warnings are errors)
cargo test --workspace --release                         # main test run (CI uses --release)
cargo test -p rs92-dsp --test realtime                   # assert_no_alloc; must be a DEBUG build
cargo test -p rs92-presets --release --test randomizer -- --ignored   # 1,000-seed gate, run in CI
cargo test -p rs92 --release --test ui                   # headless egui_kittest UI tests
cargo test -p rs92-dsp --release --test filters <name>   # single test: --test <file> <filter>

cargo xtask bundle rs92 --release                        # .vst3 + .clap -> target/bundled/
cargo xtask bundle-universal rs92 --release              # macOS universal
cargo build --release --bin tenant-rs92                  # standalone app
cargo run -p rs92-dsp --release --example budgets        # CPU budget check
cargo bench -p rs92-dsp                                  # criterion benches
```

Note the plugin crate's package name is `rs92` (directory `crates/rs92-plugin`), so use `-p rs92`.

Regeneration tools (run only deliberately):
- `cargo test -p rs92-presets --release --test golden -- --ignored regen_golden` rewrites the
  golden spectrogram snapshots in `crates/rs92-presets/tests/golden/` and writes WAVs to
  `target/golden-wav/`. Audio changes that break golden tests need a by-ear review before
  regenerating.
- `cargo run --release -p rs92-presets --example write_core_presets` rewrites
  `assets/presets/factory/*.json` from the recipes; this overwrites hand-tuned JSON.
- `node scripts/gen_assets.mjs` re-renders `assets/images/*.png` from the mockup's CSS via
  headless Chromium/Edge.
- `scripts/*-window.ps1` (Windows) screenshot/click/drag the running standalone window, useful
  for visually checking the editor.

## Architecture

Three crates with a strict dependency direction: `rs92-dsp` ← `rs92-presets` ← `rs92` (plugin).

**`rs92-dsp`** has no nih-plug dependency so presets/tests can render audio offline. A plain
`Patch` struct (`patch.rs`) is the full sound definition. `Engine` (`engine.rs`) has two
paths:
- **LIVE**: `VoiceBank` of `ChordVoice`s (each key plays a whole chord: two phase-distortion/FM
  operators → three filters), rendered oversampled (2×/4×, SIMD via `wide::f32x8`) then
  decimated, followed by the host-rate `Finish` chain: Crush → Comp → Tape.
- **SAMPLER** (`sampler_on`, default): the LIVE chain is pre-rendered ("baked") into a sample
  by a background `BakeWorker` thread (`bake.rs`) and repitched across the keyboard.
  `Patch::bake_key` hashes only bake-relevant params to cache/debounce re-bakes; playback-only
  params (e.g. `crush_mix`, velocity) must stay out of the key.

Real-time safety is enforced: the audio path must not allocate or lock (checked by
`tests/realtime.rs` and the `assert_process_allocs` feature). Lookup tables are forced in
`init_tables()` at engine construction; bake work goes through the worker, never the audio
thread.

**`rs92-presets`**: `ids.rs` is the single table mapping string parameter IDs ↔ `Patch`
fields, used by preset JSON, the plugin's parameter setter and the randomizer; adding a
parameter means updating it there as well as in `Patch` and `params.rs`. Factory presets are
embedded via `include_dir!` from `assets/presets/factory/` (the JSON is the source of truth;
`recipes.rs` holds the 18 core recipes it was generated from). `random.rs` is the RANDOM
button/mutation logic; `gate.rs` renders and analyses a hit to reject bad random results.

**`rs92` (plugin)**: `params.rs` defines nih-plug params (`Rs92Params`) and `to_patch()`,
which is called each sub-block (≤ `MAX_SUB_BLOCK`) in `process()` and pushed to
`Engine::set_patch`. Bump `STATE_VERSION` when the saved state format changes. Audio→GUI data
flows through `UiBridge` (atomics plus `rtrb` ring buffers for scope samples and on-screen
keyboard notes). The editor (`editor/`) is immediate-mode egui at a fixed 1280×800 logical
size, repainting from live params every frame. All parameter access goes through the
`ParamHost` trait (`PluginHost` in the plugin, `MockHost` in `tests/ui.rs`), so the whole panel
runs headless in tests. `main.rs` is the standalone entry (queries the device sample rate via
cpal before handing off to nih-plug's standalone wrapper).

## Conventions and gotchas

- nih-plug is pinned to a git rev in the workspace `Cargo.toml`; bump deliberately and re-run
  pluginval.
- `rs92-dsp` builds at `opt-level = 2` even in dev so debug tests stay fast.
- Release builds target portable SSE2; LIVE mode is slightly over its CPU budget there (see
  README Known issues).
- On Linux, building needs ALSA, JACK, X11/XCB and GL dev packages (list in
  `.github/workflows/ci.yml`).
- Version lives in `[workspace.package]`; release tags `vX.Y.Z` must match it.
