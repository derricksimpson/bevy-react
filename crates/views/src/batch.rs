//! Batching: a flush window, so a burst of writes costs one pass.
//!
//! Writes never touch the output on their own; they mark work. With a window
//! set, [`Runtime::poll`] holds that work until the window has run since the
//! **first** pending write, then flushes everything at once. Writes inside the
//! window coalesce: a slot written ten times is evaluated once, a list staged
//! ten times is diffed once (see [`crate::list`]). The window is anchored to
//! the first write, so latency is bounded by it however long the burst lasts.
//!
//! The clock is injectable, so the window is tested without sleeping.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::command::Command;
use crate::runtime::Runtime;

/// Monotonic time since an arbitrary origin.
pub type Clock = Arc<dyn Fn() -> Duration + Send + Sync>;

/// The default clock: wall time since the runtime was created.
pub(crate) fn instant_clock() -> Clock {
    let origin = Instant::now();
    Arc::new(move || origin.elapsed())
}

impl Runtime {
    /// Hold pending work for up to `window` after the first write (zero, the
    /// default, makes [`poll`](Self::poll) flush at once).
    pub fn set_flush_window(&mut self, window: Duration) {
        self.window = window;
    }

    pub fn flush_window(&self) -> Duration {
        self.window
    }

    /// Replace the clock (tests drive a manual one).
    pub fn set_clock(&mut self, clock: Clock) {
        self.clock = clock;
    }

    /// Anything waiting for a flush?
    pub fn has_pending(&self) -> bool {
        self.pending_since.is_some()
    }

    /// When the pending work becomes due, on the runtime's clock; `None` when
    /// nothing is pending. A host sleeps until then.
    pub fn next_deadline(&self) -> Option<Duration> {
        self.pending_since.map(|since| since + self.window)
    }

    /// Flush if the window has run, else `None`. [`flush`](Self::flush) is the
    /// forced version (frame ends, shutdown).
    pub fn poll(&mut self) -> Option<Vec<Command>> {
        let due = self.next_deadline()?;
        ((self.clock)() >= due).then(|| self.flush())
    }

    /// Record that work is pending (starts the window if it isn't running).
    pub(crate) fn note_pending(&mut self) {
        if self.pending_since.is_none() {
            self.pending_since = Some((self.clock)());
        }
    }
}
