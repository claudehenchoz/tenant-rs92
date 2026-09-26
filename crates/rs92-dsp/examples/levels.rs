//! Prints the level at each stage of the LIVE chain for the default patch.
use rs92_dsp::engine::{Finish, VoiceBank};
use rs92_dsp::*;

fn peak(x: &[f32]) -> f32 {
    20.0 * x.iter().fold(0.0f32, |m, v| m.max(v.abs())).log10()
}

fn main() {
    let sr = 48_000.0;
    let p = Patch {
        sampler_on: false,
        ..Patch::default()
    };
    let mut bank = VoiceBank::new(sr);
    bank.set(&p, 0.0);
    bank.note_on(64, 100.0 / 127.0, 100.0 / 127.0, &p, 1);
    let n = 24_000;
    let mut l = vec![0.0; n];
    let mut r = vec![0.0; n];
    for (a, b) in l.chunks_mut(32).zip(r.chunks_mut(32)) {
        bank.render(a, b);
    }
    println!("voices   {:.1} dBFS", peak(&l));
    let mut f = Finish::default();
    f.set(&p, sr, None);
    let (mut l1, mut r1) = (l.clone(), r.clone());
    for (a, b) in l1.chunks_mut(32).zip(r1.chunks_mut(32)) {
        f.crusher.process(a, b);
    }
    println!("crush    {:.1} dBFS", peak(&l1));
    for (a, b) in l1.chunks_mut(32).zip(r1.chunks_mut(32)) {
        f.comp.process(a, b);
    }
    println!("comp     {:.1} dBFS", peak(&l1));
    for (a, b) in l1.chunks_mut(32).zip(r1.chunks_mut(32)) {
        f.tape.process(a, b);
    }
    println!("tape     {:.1} dBFS", peak(&l1));
}
