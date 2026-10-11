//! Opt-in schedule shuffling (the `schedule_shuffle` cargo feature).
//!
//! Two systems with no ordering edge between them run in whatever order the
//! schedule's topological sort happens to produce — in practice the order
//! they were added, so a missing edge stays invisible. Bevy can randomize
//! that order (`ScheduleBuildSettings::shuffle_seed`, compiled only with
//! bevy's `debug` feature); a test that fails under some seed either found
//! a missing edge or relied on the accidental order itself.
//!
//! ```sh
//! BEVY_REACT_SHUFFLE_SEED=7 cargo test --workspace --lib \
//!     --features bevy_react_core/schedule_shuffle
//! ```
//!
//! The test harnesses build their own schedules, restating the edges they
//! depend on; the schedule `ReactUiPlugin` itself builds is only run by an
//! app. With the feature on the plugin honours the same variable, so an app
//! runs shuffled too (the demos: `cargo run --bin demos --features
//! bevy_react_core/schedule_shuffle -- --shoot …`).
//!
//! The feature rebuilds all of Bevy (another feature configuration), hence
//! opt-in. Known blind spot: the automatic sync points keep a system at the
//! shallowest "sync depth" its edges allow, so not every legal order is
//! reachable.

use bevy::prelude::*;

/// Shuffle the unordered systems of `app`'s frame schedules by
/// `BEVY_REACT_SHUFFLE_SEED`, on the single-threaded executor so a seed
/// replays the same order. Every test harness calls it on the app it builds,
/// and so does `ReactUiPlugin`; a no-op without the `schedule_shuffle`
/// feature or the variable. The settings stay with the schedules, so
/// systems added afterwards are shuffled too.
pub fn shuffle_schedules(app: &mut App) {
    #[cfg(feature = "schedule_shuffle")]
    if let Ok(seed) = std::env::var("BEVY_REACT_SHUFFLE_SEED") {
        let seed = seed.parse().expect("BEVY_REACT_SHUFFLE_SEED is a u64");
        // Captured by the test runner, shown with a failure.
        eprintln!("schedules shuffled with BEVY_REACT_SHUFFLE_SEED={seed}");
        shuffle(app, seed);
    }
    #[cfg(not(feature = "schedule_shuffle"))]
    let _ = app;
}

#[cfg(feature = "schedule_shuffle")]
fn shuffle(app: &mut App, seed: u64) {
    use bevy::ecs::schedule::{ScheduleLabel, SingleThreadedExecutor};
    let labels = [
        First.intern(),
        PreUpdate.intern(),
        Update.intern(),
        PostUpdate.intern(),
        Last.intern(),
    ];
    for label in labels {
        app.edit_schedule(label, |schedule| {
            let mut settings = schedule.get_build_settings();
            settings.shuffle_seed = Some(seed);
            schedule
                .set_build_settings(settings)
                .set_executor(SingleThreadedExecutor::new());
        });
    }
}

#[cfg(all(test, feature = "schedule_shuffle"))]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Ran(Vec<u8>);

    /// The run order of four `Update` systems, the last two ordered.
    fn order(seed: u64) -> Vec<u8> {
        let mut app = App::new();
        shuffle(&mut app, seed);
        // Added after the call, like a test's own systems.
        app.init_resource::<Ran>().add_systems(
            Update,
            (
                |mut ran: ResMut<Ran>| ran.0.push(0),
                |mut ran: ResMut<Ran>| ran.0.push(1),
                (
                    |mut ran: ResMut<Ran>| ran.0.push(2),
                    |mut ran: ResMut<Ran>| ran.0.push(3),
                )
                    .chain(),
            ),
        );
        app.update();
        std::mem::take(&mut app.world_mut().resource_mut::<Ran>().0)
    }

    /// Seeds reorder what no edge orders, replay exactly, and keep the edges.
    #[test]
    fn seeds_reorder_unordered_systems_only() {
        let orders: Vec<Vec<u8>> = (0..16).map(order).collect();
        assert!(orders.iter().any(|o| *o != orders[0]), "{orders:?}");
        assert_eq!(order(5), orders[5]);
        for o in &orders {
            let at = |n| o.iter().position(|&ran| ran == n).unwrap();
            assert!(at(2) < at(3), "{o:?}");
        }
    }
}
