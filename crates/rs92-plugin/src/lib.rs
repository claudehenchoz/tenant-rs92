//! TENANT RS-92: nih-plug wrapper around the `rs92-dsp` engine.

use atomic_float::AtomicF32;
use nih_plug::prelude::*;
use parking_lot::Mutex;
use rs92_dsp::{Engine, MAX_SUB_BLOCK};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

pub mod editor;
pub mod params;
pub mod prefs;

pub use params::Rs92Params;

/// Scope ring: every 8th output sample (mono), read by the GUI at 30 Hz.
pub const SCOPE_DECIMATION: usize = 8;
pub const SCOPE_LEN: usize = 512;

/// A note sent from the on-screen keyboard (or standalone MIDI) to the audio thread.
#[derive(Clone, Copy, Debug)]
pub enum GuiNote {
    On { note: u8, velocity: f32 },
    Off { note: u8 },
}

/// State shared between the audio thread and the editor. The audio thread only writes
/// atomics and pushes to lock-free queues; the mutexes are GUI-side only.
pub struct UiBridge {
    pub peak: [AtomicF32; 2],
    /// Held partials, bit n = MIDI note n.
    pub lit_notes: [AtomicU64; 2],
    pub gain_reduction: AtomicF32,
    pub midi_activity: AtomicU32,
    pub clip: AtomicBool,
    pub bake_pending: AtomicBool,
    pub active_voices: AtomicU32,
    /// Keys physically held (MIDI and on-screen), bit n = MIDI note n.
    pub held_keys: [AtomicU64; 2],
    pub sample_rate: AtomicF32,
    /// Share of the buffer period spent in `process`, smoothed.
    pub cpu: AtomicF32,
    pub scope_rx: Mutex<rtrb::Consumer<f32>>,
    pub notes_tx: Mutex<rtrb::Producer<GuiNote>>,
}

impl UiBridge {
    /// Creates the bridge plus the audio-thread ends of its queues: the scope producer
    /// and the GUI-note consumer.
    pub fn new() -> (Arc<Self>, rtrb::Producer<f32>, rtrb::Consumer<GuiNote>) {
        let (scope_tx, scope_rx) = rtrb::RingBuffer::new(SCOPE_LEN * 4);
        let (notes_tx, notes_rx) = rtrb::RingBuffer::new(256);
        let bridge = Arc::new(UiBridge {
            peak: [AtomicF32::new(0.0), AtomicF32::new(0.0)],
            lit_notes: [AtomicU64::new(0), AtomicU64::new(0)],
            gain_reduction: AtomicF32::new(0.0),
            midi_activity: AtomicU32::new(0),
            clip: AtomicBool::new(false),
            bake_pending: AtomicBool::new(false),
            active_voices: AtomicU32::new(0),
            held_keys: [AtomicU64::new(0), AtomicU64::new(0)],
            sample_rate: AtomicF32::new(48_000.0),
            cpu: AtomicF32::new(0.0),
            scope_rx: Mutex::new(scope_rx),
            notes_tx: Mutex::new(notes_tx),
        });
        (bridge, scope_tx, notes_rx)
    }

    pub fn send_note(&self, n: GuiNote) {
        let _ = self.notes_tx.lock().push(n);
    }

    pub fn is_held(&self, note: u8) -> bool {
        let w = &self.held_keys[(note / 64) as usize & 1];
        w.load(Ordering::Relaxed) & (1u64 << (note % 64)) != 0
    }

    fn set_held(&self, note: u8, on: bool) {
        let w = &self.held_keys[(note / 64) as usize & 1];
        let bit = 1u64 << (note % 64);
        if on {
            w.fetch_or(bit, Ordering::Relaxed);
        } else {
            w.fetch_and(!bit, Ordering::Relaxed);
        }
    }

    pub fn is_lit(&self, note: u8) -> bool {
        let w = &self.lit_notes[(note / 64) as usize & 1];
        w.load(Ordering::Relaxed) & (1u64 << (note % 64)) != 0
    }
}

pub struct Rs92 {
    params: Arc<Rs92Params>,
    bridge: Arc<UiBridge>,
    scope_tx: rtrb::Producer<f32>,
    notes_rx: rtrb::Consumer<GuiNote>,
    scope_phase: usize,
    engine: Option<Engine>,
    patch: rs92_dsp::Patch,
    peak_decay: f32,
    peak: [f32; 2],
}

impl Default for Rs92 {
    fn default() -> Self {
        let (bridge, scope_tx, notes_rx) = UiBridge::new();
        Rs92 {
            params: Arc::new(Rs92Params::default()),
            bridge,
            scope_tx,
            notes_rx,
            scope_phase: 0,
            engine: None,
            patch: rs92_dsp::Patch::default(),
            peak_decay: 0.999,
            peak: [0.0; 2],
        }
    }
}

impl Rs92 {
    fn handle_event(engine: &mut Engine, bridge: &UiBridge, ev: NoteEvent<()>) {
        match ev {
            NoteEvent::NoteOn { note, velocity, .. } => {
                bridge.midi_activity.fetch_add(1, Ordering::Relaxed);
                bridge.set_held(note, true);
                engine.note_on(note, velocity);
            }
            NoteEvent::NoteOff { note, .. } => {
                bridge.set_held(note, false);
                engine.note_off(note);
            }
            NoteEvent::Choke { note, .. } => engine.note_off(note),
            NoteEvent::MidiPitchBend { value, .. } => engine.pitch_bend((value - 0.5) * 2.0),
            NoteEvent::MidiCC { cc, value, .. } => match cc {
                64 => engine.sustain(value >= 0.5),
                120 | 123 => {
                    bridge.held_keys[0].store(0, Ordering::Relaxed);
                    bridge.held_keys[1].store(0, Ordering::Relaxed);
                    engine.all_notes_off();
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn publish_ui(&mut self, engine: &Engine) {
        let mut lit = [0u64; 2];
        engine.for_each_lit_note(|n| {
            if (0..128).contains(&n) {
                lit[(n / 64) as usize] |= 1u64 << (n % 64);
            }
        });
        self.bridge.lit_notes[0].store(lit[0], Ordering::Relaxed);
        self.bridge.lit_notes[1].store(lit[1], Ordering::Relaxed);
        self.bridge.peak[0].store(self.peak[0], Ordering::Relaxed);
        self.bridge.peak[1].store(self.peak[1], Ordering::Relaxed);
        self.bridge
            .gain_reduction
            .store(engine.finish.comp.gain_reduction(), Ordering::Relaxed);
        self.bridge
            .bake_pending
            .store(engine.bake_pending(), Ordering::Relaxed);
    }
}

impl Plugin for Rs92 {
    const NAME: &'static str = "TENANT RS-92";
    const VENDOR: &'static str = "Claude Henchoz";
    const URL: &'static str = "https://github.com/chenchoz/tenant-rs92";
    const EMAIL: &'static str = "claude.henchoz@gmail.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(self.params.clone(), self.bridge.clone())
    }

    fn filter_state(state: &mut PluginState) {
        // Version 0 (no field) had no custom chords; nothing to migrate yet beyond
        // stamping the current version.
        let version = state
            .fields
            .get("state-version")
            .and_then(|v| serde_json::from_str::<u32>(v).ok())
            .unwrap_or(0);
        if version < params::STATE_VERSION {
            state
                .fields
                .insert("state-version".into(), params::STATE_VERSION.to_string());
        }
    }

    fn initialize(
        &mut self,
        _layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        let sr = buffer_config.sample_rate;
        let rebuild = self
            .engine
            .as_ref()
            .map(|e| e.sample_rate() != sr)
            .unwrap_or(true);
        if rebuild {
            // Spawns the bake worker; allocation is fine here.
            let mut e = Engine::new(sr);
            self.patch = self.params.to_patch(&self.patch);
            e.set_patch(&self.patch);
            self.engine = Some(e);
        }
        // Peak meter falls 12 dB in 150 ms.
        self.peak_decay = 0.25f32.powf(1.0 / (0.15 * sr));
        true
    }

    fn reset(&mut self) {
        if let Some(e) = &mut self.engine {
            e.all_notes_off();
        }
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let Some(mut engine) = self.engine.take() else {
            for ch in buffer.as_slice() {
                ch.fill(0.0);
            }
            return ProcessStatus::Normal;
        };
        let n = buffer.samples();
        let out = buffer.as_slice();
        let (left, rest) = out.split_at_mut(1);
        let (l, r) = (&mut *left[0], &mut *rest[0]);

        let started = std::time::Instant::now();
        while let Ok(n) = self.notes_rx.pop() {
            match n {
                GuiNote::On { note, velocity } => {
                    self.bridge.set_held(note, true);
                    engine.note_on(note, velocity);
                }
                GuiNote::Off { note } => {
                    self.bridge.set_held(note, false);
                    engine.note_off(note);
                }
            }
        }

        let mut next = context.next_event();
        let mut start = 0usize;
        while start < n {
            // Events due at or before this point.
            while let Some(ev) = next {
                if ev.timing() as usize > start {
                    break;
                }
                Self::handle_event(&mut engine, &self.bridge, ev);
                next = context.next_event();
            }
            let mut end = (start + MAX_SUB_BLOCK).min(n);
            if let Some(ev) = &next {
                end = end.min((ev.timing() as usize).max(start + 1));
            }
            // Control-rate parameter snapshot, once per sub-block.
            self.patch = self.params.to_patch(&self.patch);
            engine.set_patch(&self.patch);
            engine.render(&mut l[start..end], &mut r[start..end]);
            start = end;
        }
        // Any events past the end of the buffer (should not happen).
        while let Some(ev) = next {
            Self::handle_event(&mut engine, &self.bridge, ev);
            next = context.next_event();
        }

        // Meters and scope.
        let mut clip = false;
        for i in 0..n {
            for (ch, s) in [l[i], r[i]].into_iter().enumerate() {
                let a = s.abs();
                clip |= a > 1.0;
                self.peak[ch] = if a > self.peak[ch] {
                    a
                } else {
                    self.peak[ch] * self.peak_decay
                };
            }
            self.scope_phase += 1;
            if self.scope_phase >= SCOPE_DECIMATION {
                self.scope_phase = 0;
                let _ = self.scope_tx.push(0.5 * (l[i] + r[i]));
            }
        }
        if clip {
            self.bridge.clip.store(true, Ordering::Relaxed);
        }
        self.publish_ui(&engine);
        let sr = engine.sample_rate();
        let load = started.elapsed().as_secs_f32() / (n as f32 / sr).max(1e-6);
        let cpu = self.bridge.cpu.load(Ordering::Relaxed);
        self.bridge
            .cpu
            .store(cpu + (load - cpu) * 0.05, Ordering::Relaxed);
        self.bridge.sample_rate.store(sr, Ordering::Relaxed);
        self.bridge
            .active_voices
            .store(engine.active_voices() as u32, Ordering::Relaxed);
        self.engine = Some(engine);
        ProcessStatus::Normal
    }
}

impl ClapPlugin for Rs92 {
    const CLAP_ID: &'static str = "ch.henchoz.rs92";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Polyphonic rave-stab synthesizer");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for Rs92 {
    // Generated once on 2026-09-25. Never change: hosts identify the plugin by it.
    const VST3_CLASS_ID: [u8; 16] = [
        0xe7, 0x6f, 0xa8, 0x02, 0x41, 0x22, 0xd3, 0x7c, 0xf0, 0xeb, 0xeb, 0xea, 0x06, 0x9c, 0x7a,
        0x55,
    ];
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nih_export_clap!(Rs92);
nih_export_vst3!(Rs92);
