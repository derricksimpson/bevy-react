use super::*;
use crate::synth::drums::Drum;
use crate::synth::effects::{Delay, Plate};

fn spec(id: &str, kind: Kind, params: &[f32], inputs: &[Option<(usize, usize)>]) -> Spec {
    let mut slots = [0.0; SLOTS];
    slots[..params.len()].copy_from_slice(params);
    let mut ins = [None; INS];
    ins[..inputs.len()].copy_from_slice(inputs);
    Spec {
        id: id.into(),
        kind,
        params: slots,
        inputs: ins,
    }
}

fn patch(modules: Vec<Spec>) -> Patch {
    Patch {
        modules,
        playing: true,
        bpm: 120.0,
        wiring: 1,
    }
}

fn run(patch: &Patch, secs: f32) -> Vec<f32> {
    let mut engine = Engine::default();
    engine.apply(patch);
    let mut out = vec![0.0; (secs * RATE as f32) as usize * 2];
    engine.render(&mut out);
    out
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

/// Clock → sequencer → oscillator through a filter and a VCA the envelope
/// opens, then delay, reverb and output (Night Drive's spine): it sounds,
/// at a sane level, finite.
#[test]
fn a_sequenced_patch_sounds() {
    let patch = patch(vec![
        spec("clk", Kind::Clock, &[1.0, 0.0], &[]),
        spec(
            "seq",
            Kind::Sequencer,
            &[0.3, 0.55, 0.3, 0.8, 0.45, 0.3, 0.65, 0.95, 255.0],
            &[Some((0, 0))],
        ),
        spec(
            "osc",
            Kind::Oscillator,
            &[1.0, 0.4, 0.8, 0.3],
            &[Some((1, 0))],
        ),
        spec(
            "env",
            Kind::Envelope,
            &[0.05, 0.3, 0.35, 0.5],
            &[Some((1, 1))],
        ),
        spec(
            "flt",
            Kind::Filter,
            &[0.0, 0.42, 0.55, 0.6],
            &[Some((2, 0)), Some((3, 0))],
        ),
        spec("vca", Kind::Vca, &[1.0, 0.0], &[Some((4, 0)), Some((3, 0))]),
        spec(
            "dly",
            Kind::Delay,
            &[0.45, 0.55, 0.35, 1.0],
            &[Some((5, 0))],
        ),
        spec("rev", Kind::Reverb, &[0.6, 0.4, 0.3], &[Some((6, 0))]),
        spec("out", Kind::Output, &[0.7], &[Some((7, 0)), Some((7, 1))]),
    ]);
    let out = run(&patch, 2.0);
    assert!(out.iter().all(|s| s.is_finite() && s.abs() < 1.0));
    let level = rms(&out[RATE as usize..]);
    assert!((0.02..0.6).contains(&level), "rms {level}");
}

/// Each source alone, straight into the output, sits at a sane level — the
/// drums struck on every beat.
#[test]
fn sources_are_not_too_loud() {
    let mut sources = vec![
        (Kind::Oscillator, vec![1.0, 0.5, 1.0, 0.0]),
        (Kind::Oscillator, vec![2.0, 0.5, 1.0, 0.0]),
        (Kind::Oscillator, vec![1.0, 0.5, 1.0, 1.0]),
        (Kind::Noise, vec![0.0, 1.0]),
        (Kind::Noise, vec![1.0, 1.0]),
        (Kind::Noise, vec![2.0, 1.0]),
    ];
    for voice in 0..4 {
        sources.push((Kind::Drum, vec![voice as f32, 0.5, 0.5, 1.0]));
    }
    for (kind, params) in sources {
        let patch = patch(vec![
            spec("clk", Kind::Clock, &[0.0, 0.0], &[]),
            spec("src", kind, &params, &[Some((0, 0))]),
            spec("out", Kind::Output, &[1.0], &[Some((1, 0))]),
        ]);
        let out = run(&patch, 2.0);
        let level = rms(&out[RATE as usize / 4..]);
        let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(
            (0.03..0.6).contains(&level) && peak < 0.99,
            "{kind:?} {params:?}: rms {level}, peak {peak}"
        );
    }
}

/// Paused, the patch fades out and then renders nothing.
#[test]
fn pause_is_silence() {
    let mut patch = patch(vec![
        spec("osc", Kind::Oscillator, &[1.0, 0.5, 1.0], &[]),
        spec("out", Kind::Output, &[1.0], &[Some((0, 0))]),
    ]);
    let mut engine = Engine::default();
    engine.apply(&patch);
    let mut out = vec![0.0; RATE as usize];
    engine.render(&mut out);
    patch.playing = false;
    engine.post(Arc::new(patch));
    engine.render(&mut out);
    assert!(out[out.len() / 2..].iter().all(|s| s.abs() < 1e-3));
}

/// A feedback loop (a delay fed its own output through a mixer) runs
/// and stays finite.
#[test]
fn feedback_loops_stay_finite() {
    let patch = patch(vec![
        spec("nse", Kind::Noise, &[0.0, 1.0], &[]),
        spec(
            "mix",
            Kind::Mixer,
            &[1.0, 1.0, 0.0, 0.0],
            &[Some((0, 0)), Some((2, 0))],
        ),
        spec("dly", Kind::Delay, &[0.0, 1.0, 1.0, 0.0], &[Some((1, 0))]),
        spec("out", Kind::Output, &[1.0], &[Some((2, 0))]),
    ]);
    assert!(run(&patch, 3.0).iter().all(|s| s.is_finite()));
}

/// A filter at full resonance swept by an envelope stays finite and
/// doesn't blow up the level.
#[test]
fn a_ringing_filter_stays_tame() {
    let patch = patch(vec![
        spec("clk", Kind::Clock, &[2.0, 0.0], &[]),
        spec("osc", Kind::Oscillator, &[1.0, 0.3, 1.0, 0.0], &[]),
        spec(
            "env",
            Kind::Envelope,
            &[0.0, 0.2, 0.0, 0.1],
            &[Some((0, 0))],
        ),
        spec(
            "flt",
            Kind::Filter,
            &[0.0, 0.3, 1.0, 1.0],
            &[Some((1, 0)), Some((2, 0))],
        ),
        spec("out", Kind::Output, &[1.0], &[Some((3, 0))]),
    ]);
    let out = run(&patch, 2.0);
    assert!(out.iter().all(|s| s.is_finite()));
    let level = rms(&out[RATE as usize / 2..]);
    assert!(level < 0.7, "rms {level}");
}

#[test]
fn steps_snap_to_the_scale() {
    assert_eq!(quantize(0.0), 0.0);
    assert_eq!(quantize(1.0), 1.0);
    assert_eq!(quantize(0.31), 7.0 / 24.0);
}

/// Pulling the only cable while a sine plays: the output ducks to silence
/// instead of stepping there.
#[test]
fn a_rewire_never_pops() {
    let osc = spec("osc", Kind::Oscillator, &[0.0, 0.5, 1.0], &[]);
    let wired = patch(vec![
        osc.clone(),
        spec("out", Kind::Output, &[1.0], &[Some((0, 0))]),
    ]);
    let cut = Patch {
        wiring: 2,
        ..patch(vec![osc, spec("out", Kind::Output, &[1.0], &[])])
    };
    let mut engine = Engine::default();
    engine.post(Arc::new(wired));
    let mut out = vec![0.0; RATE as usize / 5 * 2];
    engine.render(&mut out);
    let last = out[out.len() - 2];
    engine.post(Arc::new(cut));
    let mut after = vec![0.0; RATE as usize / 20 * 2];
    engine.render(&mut after);
    let left: Vec<f32> = std::iter::once(last)
        .chain(after.iter().step_by(2).copied())
        .collect();
    let jump = left
        .windows(2)
        .fold(0.0f32, |m, w| m.max((w[1] - w[0]).abs()));
    assert!(jump < 0.02, "jump {jump}");
    assert!(left[left.len() - 100..].iter().all(|s| s.abs() < 1e-2));
}

/// Gates out of a module, sample by sample.
fn gates(patch: &Patch, module: usize, frames: usize) -> Vec<[f32; OUTS]> {
    let mut engine = Engine::default();
    engine.apply(patch);
    (0..frames)
        .map(|_| {
            engine.render(&mut [0.0; 2]);
            engine.modules[module].out
        })
        .collect()
}

fn rises(gates: &[[f32; OUTS]], lane: usize) -> Vec<usize> {
    let mut last = 0.0;
    let mut at = Vec::new();
    for (i, g) in gates.iter().enumerate() {
        if g[lane] > 0.5 && last <= 0.5 {
            at.push(i);
        }
        last = g[lane];
    }
    at
}

/// A drum's pitch input transposes it, held from the hit: a kick half a
/// unit up rings an octave higher, and a pitch change mid-ring leaves it be.
#[test]
fn a_drum_takes_its_pitch_at_the_hit() {
    let ring = |at_hit: f32, later: f32| {
        let mut drum = Drum::new(Rng(7));
        // A kick, long enough to count its cycles.
        let p = [0.0, 0.4, 0.6, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let out: Vec<f32> = (0..RATE as usize / 2)
            .map(|i| {
                let (trig, pitch) = if i < 100 { (1.0, at_hit) } else { (0.0, later) };
                drum.step(&p, &p, trig, pitch)
            })
            .collect();
        // Sign changes once the sweep has settled.
        out[RATE as usize * 15 / 100..]
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count()
    };
    let (low, high) = (ring(0.0, 0.0), ring(0.5, 0.5));
    assert!((high as f32 / low as f32 - 2.0).abs() < 0.1, "{low} {high}");
    assert_eq!(ring(0.5, 0.0), high);
}

/// On a sixteenth clock, a lane fires on each of its sixteenths in the bar:
/// only the downbeat, every one, or none.
#[test]
fn the_beat_fires_its_sixteenths() {
    let patch = patch(vec![
        spec("clk", Kind::Clock, &[2.0, 0.0], &[]),
        spec(
            "beat",
            Kind::Beat,
            &[1.0, 65535.0, 0.0, 0.0],
            &[Some((0, 0))],
        ),
    ]);
    // Just short of a bar at 120 BPM.
    let gates = gates(&patch, 1, 95_000);
    assert_eq!(rises(&gates, 0), [0]);
    assert_eq!(rises(&gates, 1).len(), 16);
    assert!(rises(&gates, 2).is_empty());
}

/// On a slower clock the beat still plays all sixteen, splitting the
/// clock's pulses evenly: here eighths at full swing, the late one a third
/// of the way on, so the sixteenths fall at 0, 1⅓, 2⅔ and 3⅓.
#[test]
fn a_slow_clock_still_plays_every_sixteenth() {
    let patch = patch(vec![
        spec("clk", Kind::Clock, &[1.0, 1.0], &[]),
        spec("beat", Kind::Beat, &[65535.0], &[Some((0, 0))]),
    ]);
    let at = rises(&gates(&patch, 1, 95_000), 0);
    assert_eq!(at.len(), 16, "{at:?}");
    // Sixteenths at 120 BPM are 6000 samples.
    for (got, want) in at.iter().zip([0, 8000, 16_000, 20_000, 24_000]) {
        assert!(got.abs_diff(want) <= 2, "{at:?}");
    }
}

/// Full swing puts every other eighth a third of the way to the next: a
/// triplet shuffle.
#[test]
fn swing_delays_every_other_pulse() {
    let patch = patch(vec![spec("clk", Kind::Clock, &[1.0, 1.0], &[])]);
    let gates = gates(&patch, 0, 30_000);
    // Eighths at 120 BPM are 12000 samples; the second lands 4/3 of one in.
    let at = rises(&gates, 0);
    assert_eq!(at[0], 0);
    assert!(at[1].abs_diff(16_000) <= 2, "{at:?}");
}

/// A struck plate rings out in stereo, longer the bigger it is.
#[test]
fn the_plate_rings_out_in_stereo() {
    let tail = |size: f32| {
        let mut plate = Plate::new();
        let params = [size, 0.3, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let out: Vec<[f32; 2]> = (0..RATE as usize * 2)
            .map(|i| plate.step(&params, if i == 0 { 1.0 } else { 0.0 }))
            .collect();
        assert!(out.iter().flatten().all(|s| s.is_finite()));
        let energy = |from: f32, to: f32| {
            out[(from * RATE as f32) as usize..(to * RATE as f32) as usize]
                .iter()
                .map(|[l, r]| l * l + r * r)
                .sum::<f32>()
        };
        let (l, r): (Vec<f32>, Vec<f32>) = out.iter().map(|[l, r]| (*l, *r)).unzip();
        let dot = l.iter().zip(&r).map(|(a, b)| a * b).sum::<f32>();
        let correlation = dot / (rms(&l) * rms(&r) * l.len() as f32);
        assert!(correlation.abs() < 0.5, "correlation {correlation}");
        energy(0.6, 1.0) / energy(0.1, 0.5)
    };
    let (small, big) = (tail(0.2), tail(0.9));
    assert!(small > 0.0 && small < big && big < 1.0, "{small} {big}");
}

/// Synced, a quarter-note echo lands half a second later at 120 BPM.
#[test]
fn a_synced_delay_lands_on_the_beat() {
    let mut delay = Delay::new();
    // A quarter (the fourth note value), no feedback, all wet.
    let params = [0.6, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let target = [0.6, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let out: Vec<f32> = (0..RATE as usize)
        .map(|i| delay.step(&params, &target, if i == 0 { 1.0 } else { 0.0 }, 120.0))
        .collect();
    let peak = (0..out.len())
        .max_by(|&a, &b| out[a].abs().total_cmp(&out[b].abs()))
        .unwrap();
    assert!(peak.abs_diff(RATE as usize / 2) < 8, "peak at {peak}");
}
