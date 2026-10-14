//! Display strings for the Animations tab: opaque lines the panel shows
//! verbatim (no JS-side table to keep in sync), ASCII only — the default
//! font may lack arrow glyphs.

use crate::animations::{Driver, Easing, Run, Runner};

use super::super::layers::round3;

/// One display line per driver stage, outermost first, with its live phase.
pub(super) fn describe(runner: &Runner, out: &mut Vec<String>) {
    match runner {
        Runner::Done(v) => out.push(format!("done at {}", num(*v))),
        Runner::Timing {
            from,
            to,
            duration,
            easing,
            elapsed,
        } => {
            let progress = if *duration > 0.0 {
                (elapsed / duration).clamp(0.0, 1.0)
            } else {
                1.0
            };
            out.push(format!(
                "timing {} -> {}, {}ms {}, {}%",
                num(*from),
                num(*to),
                ms(*duration),
                easing_name(*easing),
                // Floored: a run a hair from its end must not read 100%.
                (progress * 100.0).floor()
            ));
        }
        Runner::Spring {
            to,
            stiffness,
            damping,
            mass,
            vel,
            ..
        } => out.push(format!(
            "spring -> {}, stiffness {} damping {} mass {}, velocity {}",
            num(*to),
            num(*stiffness),
            num(*damping),
            num(*mass),
            num(*vel)
        )),
        Runner::Repeat {
            remaining,
            reverse,
            iteration,
            child,
            ..
        } => {
            // `remaining` counts runs left including this one; negative = forever.
            let runs = if *remaining < 0 {
                "inf".to_string()
            } else {
                (iteration + *remaining as u32).to_string()
            };
            let reverse = if *reverse { ", reverse" } else { "" };
            out.push(format!("repeat {} of {runs}{reverse}", iteration + 1));
            describe(child, out);
        }
        Runner::Sequence {
            steps,
            index,
            child,
            ..
        } => {
            out.push(format!("sequence {} of {}", index + 1, steps.len()));
            describe(child, out);
        }
        Runner::Delay {
            remaining, child, ..
        } => match child {
            Some(child) => describe(child, out),
            None => out.push(format!("delay, {}ms left", ms(*remaining))),
        },
    }
}

/// A transition run's timing, as its spec reads (`300ms easeOut`,
/// `delay 100ms, 300ms easeOut`, `spring stiffness 170 damping 26 mass 1`).
pub(super) fn spec(driver: &Driver) -> String {
    match driver {
        Driver::Timing {
            duration, easing, ..
        } => format!("{}ms {}", ms(*duration), easing_name(*easing)),
        Driver::Spring {
            stiffness,
            damping,
            mass,
            ..
        } => format!(
            "spring stiffness {} damping {} mass {}",
            num(*stiffness),
            num(*damping),
            num(*mass)
        ),
        Driver::Delay { delay, animation } => {
            format!("delay {}ms, {}", ms(*delay), spec(animation))
        }
        // A `transition` spec arms only the shapes above.
        Driver::Repeat { .. } | Driver::Sequence { .. } => String::new(),
    }
}

/// A run's scrubber stretch, `[at, start, end]` seconds rounded for the wire
/// (see [`Run::timeline`]).
pub(super) fn timeline(run: &Run) -> [f64; 3] {
    let (at, start, end) = run.timeline();
    [round3(at), round3(start), round3(end)]
}

/// The meter's extent: the reading the driver started from plus every value
/// it aims for. `None` for a single point — nothing to sweep.
pub(super) fn span(runner: &Runner, from: f32) -> Option<(f64, f64)> {
    let (mut lo, mut hi) = (from, from);
    runner_targets(runner, &mut |v| {
        lo = lo.min(v);
        hi = hi.max(v);
    });
    let (lo, hi) = (round3(lo), round3(hi));
    (hi > lo).then_some((lo, hi))
}

/// Every value a runner aims for. Composite runners keep their full driver
/// templates (a sequence holds every step), so this is stable while it runs.
fn runner_targets(runner: &Runner, f: &mut impl FnMut(f32)) {
    match runner {
        Runner::Done(v) => f(*v),
        Runner::Timing { to, .. } | Runner::Spring { to, .. } => f(*to),
        Runner::Repeat { template, a, b, .. } => {
            f(*a);
            f(*b);
            driver_targets(template, f);
        }
        Runner::Sequence { steps, .. } => {
            for step in steps {
                driver_targets(step, f);
            }
        }
        Runner::Delay { animation, .. } => driver_targets(animation, f),
    }
}

fn driver_targets(driver: &Driver, f: &mut impl FnMut(f32)) {
    match driver {
        Driver::Timing { to, .. } | Driver::Spring { to, .. } => f(*to),
        Driver::Repeat { animation, .. } | Driver::Delay { animation, .. } => {
            driver_targets(animation, f)
        }
        Driver::Sequence { steps } => {
            for step in steps {
                driver_targets(step, f);
            }
        }
    }
}

/// A display number: 3 decimals, never `-0`.
fn num(v: f32) -> String {
    (round3(v) + 0.0).to_string()
}

fn ms(secs: f32) -> String {
    ((secs * 1000.0).round() + 0.0).to_string()
}

/// The wire name (`withTiming`'s `easing`).
fn easing_name(easing: Easing) -> &'static str {
    match easing {
        Easing::Linear => "linear",
        Easing::EaseIn => "easeIn",
        Easing::EaseOut => "easeOut",
        Easing::EaseInOut => "easeInOut",
    }
}
