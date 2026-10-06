//! Bake worker: renders one chord hit through the LIVE chain into a sample buffer.

use crate::crush::{quantize, OnePoleLp};
use crate::engine::{Finish, VoiceBank, MAX_SUB_BLOCK};
use crate::patch::Patch;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

/// Stop once the level stays under this for [`SILENCE_HOLD_MS`].
const SILENCE_DB: f32 = -90.0;
const SILENCE_HOLD_MS: f32 = 50.0;
/// Hard cap on the buffer length.
pub const MAX_BAKE_S: f32 = 4.0;
/// Number of bakes kept in the LRU cache.
pub const CACHE_SIZE: usize = 4;

/// A baked chord hit.
#[derive(Debug)]
pub struct BakedSample {
    pub key: u64,
    /// Host rate the dry buffer was rendered at.
    pub sr: f32,
    /// Rate of the crushed buffer.
    pub crush_rate: f32,
    /// MIDI note the chord was baked on.
    pub root: u8,
    /// Dry buffer at `sr`, stereo.
    pub dry: [Vec<f32>; 2],
    /// Crushed buffer at `crush_rate`, already quantized, stereo.
    pub crushed: [Vec<f32>; 2],
}

impl BakedSample {
    pub fn len(&self) -> usize {
        self.dry[0].len()
    }

    pub fn is_empty(&self) -> bool {
        self.dry[0].is_empty()
    }
}

/// Renders the bake for `patch` at host rate `sr`. Deterministic: the same inputs
/// always give bit-identical buffers.
pub fn render_bake(patch: &Patch, sr: f32) -> BakedSample {
    let mut p = *patch;
    p.sampler_on = false;
    let key = patch.bake_key(sr);
    let max_len = (MAX_BAKE_S * sr) as usize;
    let gate_len = (p.smp_gate * 0.001 * sr) as usize;
    let hold_len = (SILENCE_HOLD_MS * 0.001 * sr) as usize;
    let silence = 10f32.powf(SILENCE_DB / 20.0);

    // 1. Voice output (pre-finish) at the host rate.
    let mut bank = VoiceBank::new(sr);
    bank.set(&p, 0.0);
    // Amplitude velocity 1 (no scaling), tone velocity 0.5 (VEL>TONE neutral):
    // velocity is applied at playback.
    bank.note_on(p.smp_root, 1.0, 0.5, &p, 1);
    let mut vl = Vec::with_capacity(max_len);
    let mut vr = Vec::with_capacity(max_len);
    let mut bl = [0.0f32; MAX_SUB_BLOCK];
    let mut br = [0.0f32; MAX_SUB_BLOCK];
    let mut released = false;
    let mut quiet = 0usize;
    while vl.len() < max_len {
        let n = MAX_SUB_BLOCK.min(max_len - vl.len());
        if !released && vl.len() + n > gate_len {
            // Cut the sub-block exactly at the gate.
            let n = (gate_len - vl.len()).max(1).min(n);
            bank.render(&mut bl[..n], &mut br[..n]);
            vl.extend_from_slice(&bl[..n]);
            vr.extend_from_slice(&br[..n]);
            bank.release_all(2);
            released = true;
            continue;
        }
        let active = bank.render(&mut bl[..n], &mut br[..n]);
        vl.extend_from_slice(&bl[..n]);
        vr.extend_from_slice(&br[..n]);
        if released {
            let peak = bl[..n]
                .iter()
                .chain(&br[..n])
                .fold(0.0f32, |m, x| m.max(x.abs()));
            if peak < silence {
                quiet += n;
            } else {
                quiet = 0;
            }
            if !active || quiet >= hold_len {
                break;
            }
        }
    }
    // Room for the finish chain's own tail.
    let tail = (hold_len).min(max_len.saturating_sub(vl.len()));
    vl.resize(vl.len() + tail, 0.0);
    vr.resize(vr.len() + tail, 0.0);

    // 2. Dry and crushed passes through Crush → Comp → Tape.
    let run = |mix: f32| {
        let mut f = Finish::default();
        f.set(&p, sr, Some(mix));
        let mut l = vl.clone();
        let mut r = vr.clone();
        for (cl, cr) in l.chunks_mut(MAX_SUB_BLOCK).zip(r.chunks_mut(MAX_SUB_BLOCK)) {
            f.process(cl, cr);
        }
        (l, r)
    };
    let (dl, dr) = run(0.0);
    let (wl, wr) = run(1.0);

    // 3. Store the crushed pass at the crush rate, as a vintage sampler would.
    let bits = p.crush_bits.bits();
    let rate = p.crush_rate.min(sr);
    let decimate = |x: &[f32]| {
        let mut aa = OnePoleLp::default();
        aa.set(0.45 * rate, sr);
        let inc = rate / sr;
        let mut phase = 1.0f32;
        let mut out = Vec::with_capacity((x.len() as f32 * inc) as usize + 2);
        for &s in x {
            let y = aa.tick(s);
            if phase >= 1.0 {
                phase -= 1.0;
                out.push(match bits {
                    Some(b) => quantize(y, b),
                    None => y,
                });
            }
            phase += inc;
        }
        out
    };
    let crushed = [decimate(&wl), decimate(&wr)];

    BakedSample {
        key,
        sr,
        crush_rate: rate,
        root: p.smp_root,
        dry: [dl, dr],
        crushed,
    }
}

/// A request from the audio thread.
#[derive(Clone, Copy, Debug)]
pub struct BakeRequest {
    pub patch: Patch,
    pub sr: f32,
    pub key: u64,
}

#[derive(Debug, Default)]
pub struct BakeStats {
    pub renders: AtomicU64,
    pub cache_hits: AtomicU64,
    /// Duration of the last render in microseconds.
    pub last_render_us: AtomicU64,
}

/// LRU cache of recent bakes.
#[derive(Default)]
pub struct BakeCache {
    entries: Vec<Arc<BakedSample>>,
}

impl BakeCache {
    pub fn get(&mut self, key: u64) -> Option<Arc<BakedSample>> {
        let i = self.entries.iter().position(|e| e.key == key)?;
        let e = self.entries.remove(i);
        self.entries.insert(0, e.clone());
        Some(e)
    }

    pub fn insert(&mut self, e: Arc<BakedSample>) {
        self.entries.insert(0, e);
        self.entries.truncate(CACHE_SIZE);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Audio-thread side of the worker.
pub struct BakeClient {
    pub requests: rtrb::Producer<BakeRequest>,
    pub results: rtrb::Consumer<Arc<BakedSample>>,
    /// Finished buffers go back to the worker, which deallocates them.
    pub retire: rtrb::Producer<Arc<BakedSample>>,
    /// The worker thread, unparked when a request is queued.
    pub worker: std::thread::Thread,
}

/// Owns the worker thread; joins it on drop.
pub struct BakeWorker {
    handle: Option<JoinHandle<()>>,
    shutdown: Arc<AtomicBool>,
    pub stats: Arc<BakeStats>,
}

impl Drop for BakeWorker {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            h.thread().unpark();
            let _ = h.join();
        }
    }
}

pub fn spawn_worker() -> (BakeWorker, BakeClient) {
    let (req_tx, mut req_rx) = rtrb::RingBuffer::<BakeRequest>::new(16);
    let (res_tx, res_rx) = rtrb::RingBuffer::<Arc<BakedSample>>::new(16);
    let (ret_tx, mut ret_rx) = rtrb::RingBuffer::<Arc<BakedSample>>::new(128);
    let shutdown = Arc::new(AtomicBool::new(false));
    let stats = Arc::new(BakeStats::default());
    let (sd, st) = (shutdown.clone(), stats.clone());
    let mut res_tx = res_tx;
    let handle = std::thread::Builder::new()
        .name("rs92-bake".into())
        .spawn(move || {
            let mut cache = BakeCache::default();
            while !sd.load(Ordering::Relaxed) {
                // Deallocate retired buffers here, off the audio thread.
                while let Ok(b) = ret_rx.pop() {
                    drop(b);
                }
                // Only the newest request matters.
                let mut latest = None;
                while let Ok(r) = req_rx.pop() {
                    latest = Some(r);
                }
                if let Some(req) = latest {
                    let buf = match cache.get(req.key) {
                        Some(b) => {
                            st.cache_hits.fetch_add(1, Ordering::Relaxed);
                            b
                        }
                        None => {
                            let t = std::time::Instant::now();
                            let b = Arc::new(render_bake(&req.patch, req.sr));
                            st.last_render_us
                                .store(t.elapsed().as_micros() as u64, Ordering::Relaxed);
                            st.renders.fetch_add(1, Ordering::Relaxed);
                            cache.insert(b.clone());
                            b
                        }
                    };
                    let _ = res_tx.push(buf);
                    continue;
                }
                std::thread::park_timeout(Duration::from_millis(2));
            }
        })
        .expect("spawn bake worker");
    let worker = handle.thread().clone();
    (
        BakeWorker {
            handle: Some(handle),
            shutdown,
            stats,
        },
        BakeClient {
            requests: req_tx,
            results: res_rx,
            retire: ret_tx,
            worker,
        },
    )
}
