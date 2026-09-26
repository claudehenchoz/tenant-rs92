//! TENANT RS-92 DSP engine.
//!
//! Pure DSP with no plugin-framework dependency, so the randomizer's quality gate and
//! the golden tests can render audio offline.

pub mod bake;
pub mod chord;
pub mod comp;
pub mod crush;
pub mod engine;
pub mod env;
pub mod filter;
pub mod math;
pub mod osc;
pub mod oversample;
pub mod patch;
pub mod pd;
pub mod sampler;
pub mod tape;
pub mod voice;

pub use engine::{Engine, MAX_SUB_BLOCK, MAX_VOICES};
pub use patch::*;

/// Builds the shared lookup tables. Called when an engine is constructed so the audio
/// thread never runs a lazy initialiser.
pub fn init_tables() {
    std::sync::LazyLock::force(&math::SINE_TABLE);
    std::sync::LazyLock::force(&oversample::HALFBAND_COEFS);
}
