//! The Animations tab's playback controls: pause and speed drive Bevy's
//! `Time<Virtual>` — the whole app's clock, not only its animations — and
//! the tab hands the clock back as it found it when it unmounts.

use bevy::prelude::*;

use crate::react_message;
use crate::reconcile::OpApplyStats;

use super::super::{DevtoolsConfig, DevtoolsState};

/// JS → Bevy: the tab's pause button / speed chips.
#[react_message(name = "devtools.animationsClock")]
pub(in crate::devtools) struct DevtoolsAnimationsClockMessage {
    pub(in crate::devtools) paused: bool,
    pub(in crate::devtools) speed: f32,
}

/// The clock as it was before the tab first touched it — `None` while the
/// tab has not.
#[derive(Resource, Default)]
pub(in crate::devtools) struct TakenClock(Option<(bool, f32)>);

impl TakenClock {
    /// Remember the clock as the app had it, the first time the tab moves it.
    fn take(&mut self, time: &Time<Virtual>) {
        self.0.get_or_insert((time.is_paused(), time.relative_speed()));
    }
}

pub(in crate::devtools) fn on_clock_message(
    msg: On<DevtoolsAnimationsClockMessage>,
    mut taken: ResMut<TakenClock>,
    mut time: ResMut<Time<Virtual>>,
) {
    taken.take(&time);
    let msg = msg.event();
    // `set_relative_speed` panics on a negative or non-finite speed.
    if msg.speed.is_finite() && msg.speed > 0.0 {
        time.set_relative_speed(msg.speed);
    }
    if msg.paused {
        time.pause();
    } else {
        time.unpause();
    }
}

/// [`DevtoolsConfig::pause_key`]: the pause chip, while the tab is shown.
pub(in crate::devtools) fn pause_on_key(
    keys: Res<ButtonInput<KeyCode>>,
    cfg: Res<DevtoolsConfig>,
    state: Res<DevtoolsState>,
    mut taken: ResMut<TakenClock>,
    mut time: ResMut<Time<Virtual>>,
) {
    if !(state.open && state.animations_tab_open && keys.just_pressed(cfg.pause_key)) {
        return;
    }
    taken.take(&time);
    if time.is_paused() {
        time.unpause();
    } else {
        time.pause();
    }
}

/// Hand the clock back once the tab is gone — however it went: unmounted
/// (`animationsOpen: false`), the panel closed, or a JS runtime reset that
/// dropped the panel without running its cleanup.
pub(in crate::devtools) fn restore_clock(
    state: Res<DevtoolsState>,
    stats: Res<OpApplyStats>,
    mut last_reset: Local<u64>,
    mut taken: ResMut<TakenClock>,
    mut time: ResMut<Time<Virtual>>,
) {
    let reset = std::mem::replace(&mut *last_reset, stats.reset_count) != stats.reset_count;
    if taken.0.is_none() || (state.open && state.animations_tab_open && !reset) {
        return;
    }
    let Some((paused, speed)) = taken.0.take() else {
        return;
    };
    time.set_relative_speed(speed);
    if paused {
        time.pause();
    } else {
        time.unpause();
    }
}
