//! The deck's floor plan: where every fixture stands. Shared by the station
//! (which builds the fixtures), the holograms (which hang on them) and the
//! tour (which films them). Meters; +Y up; the engineer faces -Z, toward
//! the bulkhead in the far wall.
//!
//! ```text
//!          z = -5   save  hack  [ BULKHEAD ]  call [ELEVATOR]
//!   x = -10 ┌─────────────────────────────────────────────────┐ x = +10
//!           │                                                 │
//!   KIOSK ▐ │                    ENGINEER                     │ ▌ BENCH
//!           │                                                 │
//!  TERMINAL▐│                                                 │
//!           └─────────────────────────────────────────────────┘ z = +6.5
//! ```

use bevy::prelude::*;

/// The room's interior bounds.
pub const ROOM_MIN: Vec3 = Vec3::new(-10.0, 0.0, -5.0);
pub const ROOM_MAX: Vec3 = Vec3::new(10.0, 4.4, 6.5);

/// Where the engineer stands (his feet), facing -Z.
pub const ENGINEER: Vec3 = Vec3::new(0.0, 0.0, 1.5);

/// Far wall (z = -5), left to right. Each is the fixture's floor point on
/// the wall plane; the fixture builds out into the room from there.
pub const SAVE_STATION: Vec3 = Vec3::new(-3.45, 0.0, -5.0);
pub const HACK_PANEL: Vec3 = Vec3::new(-1.9, 0.0, -5.0);
pub const BULKHEAD: Vec3 = Vec3::new(0.0, 0.0, -5.0);
/// The bulkhead's opening (width, height).
pub const BULKHEAD_SIZE: Vec2 = Vec2::new(2.6, 3.2);
pub const ELEVATOR_PANEL: Vec3 = Vec3::new(2.1, 0.0, -5.0);
pub const ELEVATOR: Vec3 = Vec3::new(3.7, 0.0, -5.0);
/// The elevator's door opening (width, height).
pub const ELEVATOR_SIZE: Vec2 = Vec2::new(2.0, 2.8);

/// Left wall (x = -10): the store kiosk mid-wall, the title terminal's
/// alcove near the entrance. Both face +X.
pub const KIOSK: Vec3 = Vec3::new(-10.0, 0.0, -0.5);
pub const TERMINAL: Vec3 = Vec3::new(-10.0, 0.0, 3.6);

/// Right wall (x = +10): the upgrade bench, facing -X.
pub const BENCH: Vec3 = Vec3::new(10.0, 0.0, -0.5);

/// Where the locator line leads (`locator.show { to }`): a point on the
/// floor in front of each destination.
pub fn destination(to: &str) -> Option<Vec3> {
    Some(match to {
        "store" => KIOSK + Vec3::new(1.6, 0.0, 0.0),
        "bench" => BENCH - Vec3::new(1.6, 0.0, 0.0),
        "save" => SAVE_STATION + Vec3::new(0.0, 0.0, 1.4),
        "elevator" => ELEVATOR + Vec3::new(0.0, 0.0, 1.4),
        "bulkhead" => BULKHEAD + Vec3::new(0.0, 0.0, 1.4),
        _ => return None,
    })
}

/// A rotation that turns a plane facing +Z (a `Rectangle`'s front) to face
/// `dir` (horizontal).
pub fn facing(dir: Vec3) -> Quat {
    Quat::from_rotation_arc(Vec3::Z, dir.normalize())
}
