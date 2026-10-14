//! [`Run`]: a [`Runner`] that remembers what it was built from, so its
//! timeline can be read and moved — the devtools Animations tab's scrubber.
//!
//! Positions are *structural*: where the runner is inside its driver (a
//! repeat's iteration times its cycle, plus the child's position), never a
//! sum of frame deltas — the live runner drops each boundary's leftover time
//! (a repeat or sequence starts its next child fresh), so a frame-delta sum
//! drifts from the driver's own clock as a loop runs.

use super::protocol::Driver;
use super::runner::{Runner, build_runner, build_runner_with_target, terminal_value};

/// The step a spring is replayed at (to measure it, to seek into it): its
/// integration depends on the frame rate, so a replay matches a 60 fps run.
const REPLAY_STEP: f32 = 1.0 / 60.0;
/// A spring replay stops here — clamped hostile params settle very slowly.
const REPLAY_CAP: f32 = 60.0;

/// How long a driver runs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RunLength {
    /// It finishes after this many seconds.
    Finite(f32),
    /// After `lead` seconds it loops forever, `period` seconds a cycle.
    Looping { lead: f32, period: f32 },
}

/// How long `driver` runs from the reading `from`. A spring's length is its
/// settle time, measured by replaying it; a reversed cycle is taken to last
/// as long as a forward one (true of timings, and of springs, which travel
/// the same distance back).
pub fn run_length(driver: &Driver, from: f32) -> RunLength {
    use RunLength::*;
    match driver {
        Driver::Timing { duration, .. } => Finite(duration.max(0.0)),
        Driver::Spring { .. } => {
            let mut runner = build_runner(driver, from);
            let mut t = 0.0;
            while t < REPLAY_CAP {
                t += REPLAY_STEP;
                if runner.step(REPLAY_STEP).1 {
                    break;
                }
            }
            Finite(t)
        }
        Driver::Delay { delay, animation } => match run_length(animation, from) {
            Finite(l) => Finite(delay.max(0.0) + l),
            Looping { lead, period } => Looping {
                lead: delay.max(0.0) + lead,
                period,
            },
        },
        Driver::Sequence { steps } => {
            let (mut start, mut at) = (0.0, from);
            for step in steps {
                match run_length(step, at) {
                    Finite(l) => start += l,
                    Looping { lead, period } => {
                        return Looping {
                            lead: start + lead,
                            period,
                        };
                    }
                }
                at = terminal_value(step, at);
            }
            Finite(start)
        }
        Driver::Repeat {
            animation, count, ..
        } => match (run_length(animation, from), *count) {
            (_, 0) => Finite(0.0),
            (Finite(p), c) if c < 0 => Looping {
                lead: 0.0,
                period: p,
            },
            (Finite(p), c) => Finite(p * c as f32),
            // The child never ends, so the repeat never cycles.
            (looping, _) => looping,
        },
    }
}

impl Runner {
    /// Seconds into its driver the runner is (see the module doc).
    pub fn position(&self) -> f32 {
        match self {
            Runner::Done(_) => 0.0,
            Runner::Timing {
                duration, elapsed, ..
            } => elapsed.min(*duration),
            Runner::Spring { elapsed, .. } => *elapsed,
            Runner::Delay {
                delay,
                remaining,
                child,
                ..
            } => match child {
                Some(child) => delay + child.position(),
                None => (delay - remaining).max(0.0),
            },
            Runner::Sequence {
                steps,
                index,
                child,
                from,
            } => {
                let (mut start, mut at) = (0.0, *from);
                for step in &steps[..*index] {
                    if let RunLength::Finite(l) = run_length(step, at) {
                        start += l;
                    }
                    at = terminal_value(step, at);
                }
                start + child.position()
            }
            Runner::Repeat {
                template,
                iteration,
                a,
                child,
                ..
            } => match run_length(template, *a) {
                RunLength::Finite(p) => *iteration as f32 * p + child.position(),
                RunLength::Looping { .. } => child.position(),
            },
        }
    }
}

/// A runner for `driver` from `from`, placed `t` seconds in, and its reading
/// there — what [`build_runner_with_target`] and `t` seconds of steps would
/// make, structurally (a loop seeks straight to its cycle). `target` is that
/// builder's ping-pong override.
fn seek_runner(driver: &Driver, from: f32, target: Option<f32>, t: f32) -> (Runner, f32) {
    let t = t.max(0.0);
    match driver {
        Driver::Timing { .. } | Driver::Spring { .. } => {
            let mut runner = build_runner_with_target(driver, from, target);
            let (mut value, mut left) = (from, t);
            while left > 0.0 {
                let h = left.min(REPLAY_STEP);
                let (v, done) = runner.step(h);
                value = v;
                left -= h;
                if done {
                    break;
                }
            }
            (runner, value)
        }
        Driver::Delay { delay, animation } => {
            let delay = delay.max(0.0);
            let (remaining, child, value) = if t < delay {
                (delay - t, None, from)
            } else {
                let (child, v) = seek_runner(animation, from, None, t - delay);
                (0.0, Some(Box::new(child)), v)
            };
            let runner = Runner::Delay {
                delay,
                remaining,
                animation: animation.clone(),
                from,
                child,
            };
            (runner, value)
        }
        Driver::Sequence { steps } => {
            if steps.is_empty() {
                return (Runner::Done(from), from);
            }
            let (mut start, mut at, mut index) = (0.0, from, 0);
            while index + 1 < steps.len() {
                match run_length(&steps[index], at) {
                    RunLength::Finite(l) if t >= start + l => {
                        start += l;
                        at = terminal_value(&steps[index], at);
                        index += 1;
                    }
                    _ => break,
                }
            }
            let (child, value) = seek_runner(&steps[index], at, None, t - start);
            let runner = Runner::Sequence {
                steps: steps.clone(),
                index,
                child: Box::new(child),
                from,
            };
            (runner, value)
        }
        Driver::Repeat {
            animation,
            count,
            reverse,
        } => {
            if *count == 0 {
                return (Runner::Done(from), from);
            }
            let (a, b) = (from, terminal_value(animation, from));
            let (mut iteration, mut t) = (0, t);
            if let RunLength::Finite(p) = run_length(animation, a)
                && p > 0.0
            {
                let mut k = (t / p).floor();
                if *count > 0 {
                    k = k.min((*count - 1) as f32);
                }
                iteration = k as u32;
                t -= k * p;
            }
            let (start, end) = if *reverse && iteration % 2 == 1 {
                (b, a)
            } else {
                (a, b)
            };
            // Iteration 0 is built without the override, but for a scalar
            // child `b` is its own target and a composite one ignores it.
            let (child, value) = seek_runner(animation, start, Some(end), t);
            let runner = Runner::Repeat {
                template: animation.clone(),
                remaining: if *count > 0 {
                    *count - iteration as i32
                } else {
                    *count
                },
                reverse: *reverse,
                iteration,
                a,
                b,
                child: Box::new(child),
            };
            (runner, value)
        }
    }
}

/// A [`Runner`] plus the driver and the reading it started from — what a
/// rewind rebuilds it from. The engines that run drivers (the shared values,
/// the transition channels) hold these, so the devtools can scrub any of
/// them.
pub struct Run {
    runner: Runner,
    driver: Driver,
    from: f32,
    /// A seek's reading, held until time moves again: a paused frame steps
    /// by zero, and a run scrubbed to its very end must not finish there.
    held: Option<f32>,
}

impl Run {
    pub fn new(driver: Driver, from: f32) -> Self {
        Self {
            runner: build_runner(&driver, from),
            driver,
            from,
            held: None,
        }
    }

    /// [`Runner::step`] — except that a run a seek left holds its reading
    /// through zero steps.
    pub fn step(&mut self, dt: f32) -> (f32, bool) {
        if let Some(value) = self.held {
            if dt <= 0.0 {
                return (value, false);
            }
            self.held = None;
        }
        self.runner.step(dt)
    }

    pub fn runner(&self) -> &Runner {
        &self.runner
    }

    pub fn driver(&self) -> &Driver {
        &self.driver
    }

    /// The reading the run started from.
    pub fn from(&self) -> f32 {
        self.from
    }

    /// Where the run is and the stretch of its timeline a scrubber spans, as
    /// `(at, start, end)` seconds since it started: all of a finite run; the
    /// current cycle of a looping one (the lead-in before the first cycle
    /// comes with it).
    pub fn timeline(&self) -> (f32, f32, f32) {
        let at = self.runner.position();
        match run_length(&self.driver, self.from) {
            RunLength::Finite(l) => (at.min(l), 0.0, l),
            RunLength::Looping { lead, period } => {
                if period <= 0.0 || at < lead + period {
                    (at, 0.0, lead + period)
                } else {
                    let start = lead + ((at - lead) / period).floor() * period;
                    (at, start, start + period)
                }
            }
        }
    }

    /// Move the run to `t` seconds since it started and hold it there until
    /// time moves on (see [`Self::step`]). Returns the reading at `t`.
    pub fn seek(&mut self, t: f32) -> f32 {
        let (runner, value) = seek_runner(&self.driver, self.from, None, t);
        self.runner = runner;
        self.held = Some(value);
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::Easing;

    fn timing(to: f32, duration: f32) -> Driver {
        Driver::Timing {
            to,
            duration,
            easing: Easing::Linear,
        }
    }

    fn spring(to: f32) -> Driver {
        Driver::Spring {
            to,
            stiffness: 120.0,
            damping: 14.0,
            mass: 1.0,
        }
    }

    /// Step a fresh run `n` frames of `dt` and seek a second one to the
    /// first's position: both must read the same.
    fn seek_matches_live(driver: &Driver, from: f32, frames: usize) {
        let mut live = Run::new(driver.clone(), from);
        let mut value = from;
        for _ in 0..frames {
            value = live.step(REPLAY_STEP).0;
        }
        let (at, ..) = live.timeline();
        let mut seeked = Run::new(driver.clone(), from);
        let v = seeked.seek(at);
        assert!(
            (v - value).abs() < 0.05,
            "{driver:?} after {frames} frames: live {value}, seek({at}) {v}"
        );
        assert!(
            (seeked.timeline().0 - at).abs() < 1e-3,
            "a seek lands where it was asked"
        );
    }

    #[test]
    fn lengths() {
        assert_eq!(run_length(&timing(1.0, 0.5), 0.0), RunLength::Finite(0.5));
        let delayed = Driver::Delay {
            delay: 0.25,
            animation: Box::new(timing(1.0, 0.5)),
        };
        let seq = Driver::Sequence {
            steps: vec![delayed.clone(), timing(0.0, 1.0)],
        };
        assert_eq!(run_length(&seq, 0.0), RunLength::Finite(1.75));
        let twice = Driver::Repeat {
            animation: Box::new(seq.clone()),
            count: 2,
            reverse: true,
        };
        assert_eq!(run_length(&twice, 0.0), RunLength::Finite(3.5));
        let lead_then_loop = Driver::Sequence {
            steps: vec![
                timing(1.0, 0.5),
                Driver::Repeat {
                    animation: Box::new(delayed),
                    count: -1,
                    reverse: false,
                },
                timing(9.0, 9.0),
            ],
        };
        assert_eq!(
            run_length(&lead_then_loop, 0.0),
            RunLength::Looping {
                lead: 0.5,
                period: 0.75
            },
            "steps after a loop are never reached"
        );
        let RunLength::Finite(settle) = run_length(&spring(100.0), 0.0) else {
            panic!("a spring finishes");
        };
        assert!(settle > 0.5 && settle < REPLAY_CAP, "settles in {settle}s");
    }

    #[test]
    fn seeks_land_where_live_runs_are() {
        seek_matches_live(&timing(100.0, 1.0), 0.0, 20);
        seek_matches_live(&spring(100.0), 0.0, 25);
        let bounce = Driver::Repeat {
            animation: Box::new(Driver::Sequence {
                steps: vec![
                    Driver::Delay {
                        delay: 0.2,
                        animation: Box::new(timing(100.0, 0.5)),
                    },
                    spring(-100.0),
                ],
            }),
            count: -1,
            reverse: true,
        };
        for frames in [5, 20, 47, 130, 400] {
            seek_matches_live(&bounce, -100.0, frames);
        }
    }

    #[test]
    fn a_loop_spans_its_current_cycle() {
        let pingpong = Driver::Repeat {
            animation: Box::new(timing(10.0, 1.0)),
            count: -1,
            reverse: true,
        };
        let mut run = Run::new(pingpong, 0.0);
        assert_eq!(run.timeline(), (0.0, 0.0, 1.0));
        // The fourth cycle (index 3) runs back from 10 to 0.
        let v = run.seek(3.25);
        assert!((v - 7.5).abs() < 1e-3, "reversed cycle, got {v}");
        let (at, start, end) = run.timeline();
        assert!((at - 3.25).abs() < 1e-3);
        assert_eq!((start, end), (3.0, 4.0));
    }

    /// A seek holds its reading through zero steps — a paused frame — even
    /// at the very end, and time moving on finishes the run.
    #[test]
    fn a_seek_holds_until_time_moves() {
        let mut run = Run::new(timing(100.0, 1.0), 0.0);
        assert_eq!(run.seek(1.0), 100.0);
        assert_eq!(run.step(0.0), (100.0, false), "held at the end");
        assert_eq!(run.step(0.0), (100.0, false));
        assert_eq!(run.step(0.016), (100.0, true), "resumed: it finishes");

        let mut run = Run::new(timing(100.0, 1.0), 0.0);
        run.step(0.75);
        assert_eq!(run.seek(0.0), 0.0, "rewound to the start");
        let (v, done) = run.step(0.5);
        assert!(!done && (v - 50.0).abs() < 1e-3, "plays on from there");
    }
}
