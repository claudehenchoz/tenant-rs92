# TENANT RS-92

**Rave stab synthesizer · VST3 · CLAP · standalone · Windows, macOS, Linux**

Hardcore, you know the score. 🔊

TENANT RS-92 is a stab machine for the old skool massive. One key drops a whole minor chord,
FM-bent and phase-distorted, pushed through three filters, then crunched down to 12-bit like it
was sampled off a pirate radio tape at 3am. That's the classic '91 warehouse stab, straight out
the box, no effects chain needed. Big up the breakbeat hardcore and early jungle producers who
cooked these up on samplers that could barely hold a second of audio. This one's for you.

- **Choons out of the box:** 64 presets across four banks: Classic Rave, Tenants, Warehouse and
  Hardcore. LANDLORD '91 is the one, rewind selecta.
- **One finger, full chord:** minor, major, min7, min9, sus4, fifths or your own, with sub-octave
  doubling for that bassy weight.
- **Vintage sampler engine:** bakes the chord into a gritty 8/12-bit sample and repitches it
  across the keyboard, just like the originals. Pitch it down for jungle.
- **HYPER STAB:** harder, louder-feeling, proper rush.
- **RANDOM:** a fresh stab every click, and they actually sound like stabs. Lock the sections
  you like and spin the rest.

## Download

Grab the latest release from the **[Releases page](../../releases/latest)**:

| System | File |
| --- | --- |
| Windows 10/11 (64-bit) | `TENANT-RS-92-<version>-windows-x64.zip` |
| macOS 11+ (Apple Silicon and Intel) | `TENANT-RS-92-<version>-macos-universal.zip` |
| Linux (64-bit, X11) | `TENANT-RS-92-<version>-linux-x64.tar.gz` |

Each download contains the **VST3** and **CLAP** plugins plus the **standalone** app.

## Install

Unpack the download, then copy the plugin folders to where your DAW looks for them:

| | VST3 (`TENANT RS-92.vst3`) | CLAP (`TENANT RS-92.clap`) |
| --- | --- | --- |
| **Windows** | `C:\Program Files\Common Files\VST3\` | `C:\Program Files\Common Files\CLAP\` |
| **macOS** | `~/Library/Audio/Plug-Ins/VST3/` | `~/Library/Audio/Plug-Ins/CLAP/` |
| **Linux** | `~/.vst3/` | `~/.clap/` |

Rescan plugins in your DAW and look for **TENANT RS-92** under instruments.

**Standalone:** no DAW needed. Run `tenant-rs92.exe` on Windows, `TENANT-RS-92.command` on macOS
or `tenant-rs92.sh` on Linux. It plays through your default audio device, and the on-screen
keyboard works without a MIDI controller. To use a controller, run
`tenant-rs92 --midi-input ""` to list MIDI inputs, then `tenant-rs92 --midi-input "Your Keyboard"`.

**macOS says the app is from an unidentified developer?** If the release isn't signed, clear the
quarantine flag once in Terminal:

```sh
xattr -dr com.apple.quarantine ~/Library/Audio/Plug-Ins/VST3/"TENANT RS-92.vst3" ~/Library/Audio/Plug-Ins/CLAP/"TENANT RS-92.clap"
```

## Quick guide

- **Play:** hit a key and you get the whole chord. The display shows which notes are sounding.
- **Knobs and faders:** drag up and down. Hold **Shift** for fine moves, use the mouse wheel for
  small steps, and **double-click** to reset. **Right-click** to reset, type in a value, or find
  extra settings under **More**.
- **Presets:** **PREV / NEXT** or drag the big **PROGRAM / DATA** dial. Click the preset name to
  browse the banks. **STORE** saves your own sound. **COMPARE** flips between your edit and
  the sound before it.
- **RANDOM:** click for a brand-new stab. **Shift-click** mutates the current sound a little,
  **Alt-click** just a touch, and **Ctrl/Cmd-Z** goes back. Click a section number (**01–06**)
  to lock that section so RANDOM leaves it alone.
- **Chords:** pick a chord with the buttons. To teach your own, hold the notes on your
  keyboard and press and hold a chord button for about half a second.
- **SAMPLER PITCH** (on by default) gives the resampled vintage sound. Turn it off for the
  clean live synth. **TRANSPOSE** and **TAIL LP / TAIL REL** shape the sampled stab.
- **HYPER STAB:** press it. You'll know.
- **Look:** right-click the **TENANT** logo to switch between Silver and Black, pick the display
  colour, or change the window size.

---

## For developers

Written in Rust on [nih-plug](https://github.com/robbert-vdh/nih-plug). The full spec is in
[`TENANT RS-92 — Design Document.md`](TENANT%20RS-92%20—%20Design%20Document.md). The visual
reference is `docs/mockup/Main.dc.html`.

### Build

Rust toolchain from `rust-toolchain.toml`. On Linux, install the ALSA, JACK, X11/XCB and GL
development packages (see `.github/workflows/ci.yml`).

```sh
cargo xtask bundle rs92 --release            # Windows / Linux: .vst3 + .clap in target/bundled/
cargo xtask bundle-universal rs92 --release  # macOS arm64 + x86_64
cargo build --release --bin tenant-rs92      # standalone app
```

Workspace layout:
- `crates/rs92-dsp`: pure DSP, with no nih-plug dependency.
- `crates/rs92-presets`: preset format, factory bank, randomizer and quality gate.
- `crates/rs92-plugin`: package `rs92`, with the plugin, parameters, egui editor and standalone.

### Tests and tools

```sh
cargo test --workspace --release
cargo test -p rs92 --release --test ui                               # headless UI tests (egui_kittest)
cargo test -p rs92-dsp --test realtime                               # assert_no_alloc (debug only)
cargo test -p rs92-presets --release --test randomizer -- --ignored  # 1,000-seed gate test
cargo test -p rs92-presets --release --test golden -- --ignored regen_golden  # on purpose only
cargo run -p rs92-dsp --release --example budgets                    # CPU budgets
cargo bench -p rs92-dsp                                              # criterion benches
node scripts/gen_assets.mjs                                          # re-render bitmaps
cargo run --release -p rs92-presets --example write_core_presets     # rewrite factory JSON
```

`regen_golden` also writes WAVs of every core preset's riff to `target/golden-wav/`. Listen to
them before committing new snapshots.

### Standalone flags

nih-plug's standalone wrapper has no settings window, so audio and MIDI are set with flags.
Without flags it opens the default output device at the rate that device already runs at.

| Flag | Meaning |
| --- | --- |
| `--backend auto\|jack\|alsa\|core-audio\|wasapi\|dummy` | Audio/MIDI backend |
| `--sample-rate 44100` | Defaults to the output device's current rate; ignored by JACK |
| `--period-size 4096` | Largest callback accepted (default 4096); ignored by JACK |
| `--output-device "<name>"` | Pass `""` to list devices |
| `--midi-input "<name>"` | Pass `""` to list MIDI inputs |
| `--dpi-scale 1.5` | Editor DPI on Windows/Linux |

The launchers pass `--midi-input "$RS92_MIDI"` when that environment variable is set.

### Releasing

Pushing a version tag builds, tests and packages every platform and publishes a GitHub Release
(`.github/workflows/release.yml`):

```sh
# bump [workspace.package] version in Cargo.toml first; the tag must match it
git tag v0.1.0
git push origin v0.1.0
```

Tags with a hyphen (`v0.2.0-beta.1`) become pre-releases. Builds are signed only when the
`APPLE_*` / `WINDOWS_CERT_*` repository secrets exist. The workflow can also be run by hand
from the Actions tab to produce the archives without publishing.

### Interpretations of the spec

Where the design doc was ambiguous, these are the choices made:

- **Tape level** is the input level that reaches the clipper at 0 dBFS (drive = −level dB, no
  make-up gain).
- **Comp auto make-up** is half of the gain reduction a 0 dBFS signal receives.
- **Bake buffers:** the dry buffer is the LIVE chain through Tape with crush mix 0. The crushed
  buffer is the same chain with crush mix 100 %, resampled at `crush_rate` and requantised.
  `crush_mix` only crossfades at playback, so it's left out of the bake key.
- The bake runs at amp velocity 1 and tone velocity 0.5; velocity is applied at playback.
- **SIMD sine:** the operators use an odd polynomial on `f32x8` because `wide` cannot gather
  from a table; the 4096-point table is used for scalar paths.
- **Editor:** egui (immediate mode), redrawing every frame from live parameter values (about
  1.3 ms per frame).
- **Filter cutoff ranges** use a skew that puts the geometric mean at the knob centre.
- TRANSPOSE defaults to 0 (the mockup shows −12).

### Known issues

- **LIVE-mode CPU:** 6.4 % of one core at 2× oversampling on a Ryzen 3900XT with the portable
  SSE2 build, against a < 5 % budget. An AVX2 build (`RUSTFLAGS="-C target-cpu=x86-64-v3"`)
  measures about 4 %. SAMPLER mode, idle and bake times are within budget.
- **clap-validator 0.4.1:** the state-reproducibility tests fail and `state-invalid-random`
  crashes. nih-plug's own `gain` example does the same at the pinned commit, so these come from
  nih-plug's CLAP wrapper. pluginval (VST3, strictness 8) passes.
- Still to do by hand: code-signing certificates, DAW smoke tests (Live, Bitwig, Reaper, FL),
  and tuning the 46 generated bank variations by ear.

### Licence

GPL-3.0-or-later. nih-plug's VST3 bindings (`vst3-sys`) are GPLv3, which makes the VST3 build
GPLv3. Archivo and Doto are under the SIL Open Font License (`assets/fonts/*-OFL.txt`, shipped
with the downloads). Preset names like `LANDLORD '91` and `M1 ORGAN` are working names; consider
neutral names before a commercial release.
