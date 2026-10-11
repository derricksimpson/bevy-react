//! The far wall's machinery: the bulkhead (locked until the hack opens it),
//! the elevator (called to a floor from its panel) and the hack panel's
//! sparks when a lock attempt fails. Beyond the bulkhead runs a dark
//! corridor stub, behind the elevator doors a lit car — opening either
//! shows something.

use bevy::prelude::*;
use bevy_react::{ReactAppExt, ReactEvents, react_event, react_message};

use crate::layout::{BULKHEAD, BULKHEAD_SIZE, ELEVATOR, ELEVATOR_SIZE, HACK_PANEL, ROOM_MIN};
use crate::station::kit::{Build, Kit, PaintMaterial, glow};
use crate::station::{Burst, BurstKind, Flash, Flicker, Noise};

mod build;

/// React → Bevy: the bulkhead's lock was hacked open; open the door.
#[react_message(name = "door.unlock")]
pub struct UnlockDoor;

/// Bevy → React: the bulkhead finished opening.
#[react_event(name = "door.opened")]
pub struct DoorOpened;

/// React → Bevy: call the elevator to `floor`.
#[react_message(name = "elevator.call")]
pub struct CallElevator {
    pub floor: u32,
}

/// Bevy → React: the elevator reached `floor` and opened.
#[react_event(name = "elevator.arrived")]
pub struct ElevatorArrived {
    pub floor: u32,
}

/// React → Bevy: a lock attempt failed; the panel shocks.
#[react_message(name = "hack.shock")]
pub struct HackShock;

/// The bulkhead's halves part this far (each), sliding into the wall.
const BULKHEAD_TRAVEL: f32 = 1.32;
/// The lock clunks, then the halves slide.
const BULKHEAD_CLUNK: f32 = 0.45;
const BULKHEAD_SLIDE: f32 = 1.6;
const ELEVATOR_TRAVEL: f32 = 1.0;
const ELEVATOR_DOORS: f32 = 1.0;
const ELEVATOR_RIDE: f32 = 2.5;
/// The door slabs sit inside the wall's thickness, so an open half
/// vanishes into it.
const DOOR_Z: f32 = ROOM_MIN.z - 0.12;

/// A sliding door half: its closed x and which way it opens.
#[derive(Component)]
struct Half {
    closed: f32,
    dir: f32,
    elevator: bool,
}

/// The bulkhead's state (shots and stasis fly through it once it's open).
#[derive(Resource)]
pub struct Bulkhead {
    /// Seconds since the unlock (`None` = still locked).
    since: Option<f32>,
    opened: bool,
    lock: Handle<StandardMaterial>,
}

impl Bulkhead {
    /// Whether the halves have parted enough to shoot through.
    pub fn is_open(&self) -> bool {
        self.since
            .is_some_and(|s| s > BULKHEAD_CLUNK + BULKHEAD_SLIDE * 0.6)
    }
}

#[derive(Resource)]
struct Elevator {
    phase: Phase,
    /// 0 = closed, 1 = open.
    doors: f32,
    floor: u32,
    car_light: Handle<StandardMaterial>,
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    Closing(u32),
    Riding(u32, f32),
    Opening(u32),
}

const CAR_LIGHT: LinearRgba = LinearRgba::rgb(2.4, 2.6, 2.6);

pub struct DoorsPlugin;

impl Plugin for DoorsPlugin {
    fn build(&self, app: &mut App) {
        register_bindings(app);
        app.add_systems(Startup, build::build)
            .add_systems(Update, (open_bulkhead, run_elevator, slide_halves).chain());
    }
}

pub fn register_bindings(app: &mut App) {
    app.add_react_handler(on_unlock)
        .add_react_handler(on_call)
        .add_react_handler(on_shock)
        .add_react_event::<DoorOpened>()
        .add_react_event::<ElevatorArrived>();
}

fn on_unlock(
    _: On<UnlockDoor>,
    mut bulkhead: ResMut<Bulkhead>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if bulkhead.since.is_some() {
        return;
    }
    bulkhead.since = Some(0.0);
    if let Some(mut m) = materials.get_mut(&bulkhead.lock) {
        m.emissive = LinearRgba::rgb(0.3, 5.0, 1.2);
    }
}

fn on_call(
    call: On<CallElevator>,
    mut elevator: ResMut<Elevator>,
    mut noise: MessageWriter<Noise>,
    events: ReactEvents,
) {
    if elevator.phase != Phase::Idle {
        return;
    }
    // Already there (React's floor may lag ours after a full reload):
    // answer at once, or React waits on its call forever.
    if call.floor == elevator.floor {
        events.send(&ElevatorArrived { floor: call.floor });
        return;
    }
    elevator.phase = Phase::Closing(call.floor);
    noise.write(Noise("elevatorDoors", ELEVATOR + Vec3::Y * 1.4));
}

fn on_shock(
    _: On<HackShock>,
    mut bursts: MessageWriter<Burst>,
    mut flashes: MessageWriter<Flash>,
    mut flicker: MessageWriter<Flicker>,
) {
    let at = HACK_PANEL + Vec3::new(0.05, 1.32, 0.18);
    bursts.write(Burst {
        at,
        dir: Vec3::new(0.2, 0.4, 1.0),
        kind: BurstKind::Sparks,
    });
    flashes.write(Flash {
        at: at + Vec3::Z * 0.3,
        color: LinearRgba::rgb(1.0, 0.7, 0.4),
        intensity: 400_000.0,
        secs: 0.25,
    });
    flicker.write(Flicker {
        near: at,
        secs: 0.6,
    });
}

/// The bulkhead's run after the unlock: the clunk and a breath of steam
/// from the seam, the slide, then `door.opened`.
fn open_bulkhead(
    time: Res<Time>,
    mut bulkhead: ResMut<Bulkhead>,
    mut bursts: MessageWriter<Burst>,
    mut noise: MessageWriter<Noise>,
    events: ReactEvents,
) {
    let Some(since) = bulkhead.since else {
        return;
    };
    let now = since + time.delta_secs();
    bulkhead.since = Some(now);
    if since < BULKHEAD_CLUNK * 0.6 && now >= BULKHEAD_CLUNK * 0.6 {
        for y in [0.4, 3.0] {
            bursts.write(Burst {
                at: BULKHEAD + Vec3::new(0.0, y, -0.05),
                dir: Vec3::Z,
                kind: BurstKind::Steam,
            });
        }
        noise.write(Noise("bulkheadOpen", BULKHEAD + Vec3::Y * 1.6));
    }
    if !bulkhead.opened && now >= BULKHEAD_CLUNK + BULKHEAD_SLIDE {
        bulkhead.opened = true;
        events.send(&DoorOpened);
    }
}

/// Close the doors, ride, open, report — one call at a time.
fn run_elevator(
    time: Res<Time>,
    mut elevator: ResMut<Elevator>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut noise: MessageWriter<Noise>,
    events: ReactEvents,
) {
    let dt = time.delta_secs();
    let step = dt / ELEVATOR_DOORS;
    let mut light = None;
    match elevator.phase {
        Phase::Idle => {}
        Phase::Closing(floor) => {
            elevator.doors = (elevator.doors - step).max(0.0);
            if elevator.doors == 0.0 {
                elevator.phase = Phase::Riding(floor, 0.0);
                noise.write(Noise("elevatorMove", ELEVATOR + Vec3::Y * 1.4));
            }
        }
        Phase::Riding(floor, t) => {
            let t = t + dt;
            // The car's light dims and stutters while it moves.
            let k = 0.35 + 0.25 * (t * 23.0).sin().abs();
            light = Some(CAR_LIGHT * k);
            if t >= ELEVATOR_RIDE {
                elevator.floor = floor;
                elevator.phase = Phase::Opening(floor);
                light = Some(CAR_LIGHT);
                noise.write(Noise("elevatorArrive", ELEVATOR + Vec3::Y * 1.4));
            } else {
                elevator.phase = Phase::Riding(floor, t);
            }
        }
        Phase::Opening(floor) => {
            elevator.doors = (elevator.doors + step).min(1.0);
            if elevator.doors == 1.0 {
                elevator.phase = Phase::Idle;
                events.send(&ElevatorArrived { floor });
            }
        }
    }
    if let Some(light) = light
        && let Some(mut m) = materials.get_mut(&elevator.car_light)
    {
        m.emissive = light;
    }
}

/// Place every door half from its door's state.
fn slide_halves(
    bulkhead: Res<Bulkhead>,
    elevator: Res<Elevator>,
    mut halves: Query<(&Half, &mut Transform)>,
) {
    let ease = |k: f32| k * k * (3.0 - 2.0 * k);
    let bulk = bulkhead.since.map_or(0.0, |s| {
        ((s - BULKHEAD_CLUNK) / BULKHEAD_SLIDE).clamp(0.0, 1.0)
    });
    for (half, mut t) in &mut halves {
        let (k, travel) = if half.elevator {
            (elevator.doors, ELEVATOR_TRAVEL)
        } else {
            (bulk, BULKHEAD_TRAVEL)
        };
        let x = half.closed + half.dir * travel * ease(k);
        if t.translation.x != x {
            t.translation.x = x;
        }
    }
}
