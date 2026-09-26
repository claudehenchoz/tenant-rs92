//! Operator phase continuity.

use rs92_dsp::osc::{OpParams, OpState};
use rs92_dsp::Patch;
use wide::f32x8;

#[test]
fn phase_stays_continuous_over_ten_seconds() {
    let fs = 96_000.0f32;
    let patch = Patch {
        op1_fdbk: 60.0,
        ..Patch::default()
    };
    let prm = OpParams::from_patch(&patch);
    let mut st = OpState::default();
    let freqs = [41.2f32, 82.4, 164.8, 196.0, 246.9, 523.3, 1046.5, 4186.0];
    let dt = f32x8::from(freqs.map(|f| f / fs));
    let mut prev1 = st.phase1;
    let mut prev2 = st.phase2;
    for _ in 0..(10.0 * fs) as usize {
        st.tick(&prm, dt, f32x8::ONE);
        for lane in 0..8 {
            let d1 = st.phase1.as_array_ref()[lane] - prev1.as_array_ref()[lane];
            let d1 = d1 - d1.round();
            let e1 = (d1 - dt.as_array_ref()[lane]).abs();
            let d2 = st.phase2.as_array_ref()[lane] - prev2.as_array_ref()[lane];
            let d2 = d2 - d2.round();
            let e2 = (d2 - dt.as_array_ref()[lane] * prm.ratio2).abs();
            assert!(
                e1 < 1e-3 && e2 < 1e-3,
                "phase jump in lane {lane}: {e1} {e2}"
            );
        }
        prev1 = st.phase1;
        prev2 = st.phase2;
    }
}
