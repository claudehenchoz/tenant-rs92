use cpal::traits::{DeviceTrait, HostTrait};
use nih_plug::prelude::*;
use rs92::Rs92;

/// Largest callback size accepted when none is given on the command line. WASAPI and
/// CoreAudio in shared mode pick their own callback size (often ~10 ms, e.g. 970 frames)
/// and ignore the requested one; nih-plug aborts if a callback exceeds this, while smaller
/// callbacks are fine. So this is only an upper bound and adds no latency.
const DEFAULT_PERIOD: u32 = 4096;

/// Sample rate the default output device currently runs at. In shared mode WASAPI and
/// CoreAudio only accept that rate, and nih-plug's standalone otherwise asks for 48 kHz.
fn device_sample_rate() -> Option<u32> {
    let device = cpal::default_host().default_output_device()?;
    Some(device.default_output_config().ok()?.sample_rate().0)
}

fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    let has = |a: &[String], long: &str, short: &str| {
        a.iter()
            .any(|x| x == long || x == short || x.starts_with(&format!("{long}=")))
    };
    let dummy = args
        .windows(2)
        .any(|w| (w[0] == "--backend" || w[0] == "-b") && w[1] == "dummy");
    if !has(&args, "--sample-rate", "-r") && !dummy {
        if let Some(sr) = device_sample_rate() {
            args.extend(["--sample-rate".into(), sr.to_string()]);
        }
    }
    if !has(&args, "--period-size", "-p") {
        args.extend(["--period-size".into(), DEFAULT_PERIOD.to_string()]);
    }
    nih_export_standalone_with_args::<Rs92, _>(args);
}
