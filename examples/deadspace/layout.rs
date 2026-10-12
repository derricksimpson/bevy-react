//! The deck's floor plan: where every fixture stands, where the engineer can
//! walk and how far a shot flies. Shared by the station (which builds the
//! fixtures), the holograms (which hang on them) and the player (who walks
//! among them). Meters; +Y up; yaw 0 faces -Z, toward the bulkhead in the
//! far wall.
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

/// Where the engineer starts (his feet): in front of the controls terminal,
/// reading it.
pub const START: Vec3 = Vec3::new(-8.1, 0.0, 4.5);
/// The yaw that faces -X (the left wall); its negative faces +X.
pub const FACING_LEFT: f32 = std::f32::consts::FRAC_PI_2;

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

/// Left wall (x = -10): the store kiosk mid-wall, the controls terminal's
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

/// Where `--shoot … --do "<secs> go <spot>"` stands him: his feet and his
/// yaw. A spot at a fixture is in front of its panel, facing it: `use`
/// then locks the camera on it and he steps to his post.
pub fn spot(name: &str) -> Option<(Vec3, f32)> {
    Some(match name {
        "start" => (START, FACING_LEFT),
        "center" => (Vec3::new(0.0, 0.0, 1.5), 0.0),
        "store" => (Vec3::new(-7.6, 0.0, 0.15), FACING_LEFT),
        "bench" => (Vec3::new(7.6, 0.0, -1.1), -FACING_LEFT),
        "save" => (Vec3::new(-4.05, 0.0, -3.6), 0.0),
        "hack" => (Vec3::new(-2.5, 0.0, -3.7), 0.0),
        "doors" => (Vec3::new(-0.4, 0.0, -0.6), 0.0),
        "lift" => (Vec3::new(1.5, 0.0, -3.7), 0.0),
        _ => return None,
    })
}

/// How close his feet come to a wall.
const BODY: f32 = 0.65;
/// The floor he can stand on (x, z): the room, short of the far wall's
/// fixtures and their holograms.
const FLOOR: Rect = Rect {
    min: Vec2::new(ROOM_MIN.x + BODY, ROOM_MIN.z + 1.25),
    max: Vec2::new(ROOM_MAX.x - BODY, ROOM_MAX.z - BODY),
};
/// What stands on that floor (x, z), each with room to read its hologram:
/// the kiosk, the bench, the controls terminal.
const OBSTACLES: [Rect; 3] = [
    Rect {
        min: Vec2::new(-10.0, -2.1),
        max: Vec2::new(-7.75, 1.1),
    },
    Rect {
        min: Vec2::new(7.75, -2.1),
        max: Vec2::new(10.0, 1.1),
    },
    Rect {
        min: Vec2::new(-10.0, 2.3),
        max: Vec2::new(-8.6, 4.9),
    },
];
/// The corridor beyond the bulkhead, from inside the room to its end wall.
const HALL: Rect = Rect {
    min: Vec2::new(-0.85, ROOM_MIN.z - 6.4),
    max: Vec2::new(0.85, ROOM_MIN.z + 1.25),
};
/// The corridor's volume (the stub `doors/build.rs` builds).
const HALL_MIN: Vec3 = Vec3::new(-1.5, 0.0, ROOM_MIN.z - 7.0);
const HALL_MAX: Vec3 = Vec3::new(1.5, 3.4, ROOM_MIN.z);

/// The nearest place to `at` he can stand: inside the room and off its
/// fixtures, or in the corridor once the bulkhead stands `open`.
pub fn confine(at: Vec3, open: bool) -> Vec3 {
    let p = at.xz();
    let mut room = p.clamp(FLOOR.min, FLOOR.max);
    for o in OBSTACLES {
        if !o.contains(room) {
            continue;
        }
        // Out through the nearest side.
        let sides = [
            (room.x - o.min.x, Vec2::new(o.min.x, room.y)),
            (o.max.x - room.x, Vec2::new(o.max.x, room.y)),
            (room.y - o.min.y, Vec2::new(room.x, o.min.y)),
            (o.max.y - room.y, Vec2::new(room.x, o.max.y)),
        ];
        room = sides
            .into_iter()
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .unwrap()
            .1;
    }
    let hall = p.clamp(HALL.min, HALL.max);
    let to = if open && hall.distance(p) < room.distance(p) {
        hall
    } else {
        room
    };
    Vec3::new(to.x, at.y, to.y)
}

/// Whether `p` is in the air of the room (or of the corridor, once the
/// bulkhead stands `open`), `margin` off every wall: where the camera may
/// be.
pub fn inside(p: Vec3, margin: f32, open: bool) -> bool {
    let within = |lo: Vec3, hi: Vec3| p.cmpgt(lo + margin).all() && p.cmplt(hi - margin).all();
    // The corridor's test box runs on into the room, so the doorway itself
    // is inside.
    within(ROOM_MIN, ROOM_MAX) || (open && within(HALL_MIN, HALL_MAX.with_z(ROOM_MIN.z + 1.0)))
}

/// How far a shot from `from` along `dir` (unit) flies before it strikes a
/// wall, the floor or the ceiling — on through the bulkhead's opening when
/// it stands `open`, and back out of the corridor into the room.
pub fn reach(from: Vec3, dir: Vec3, open: bool) -> f32 {
    // Distance from a point inside a box to where the ray leaves it.
    let leave = |from: Vec3, lo: Vec3, hi: Vec3| {
        let mut t = f32::MAX;
        for i in 0..3 {
            if dir[i] > 1e-6 {
                t = t.min((hi[i] - from[i]) / dir[i]);
            } else if dir[i] < -1e-6 {
                t = t.min((lo[i] - from[i]) / dir[i]);
            }
        }
        t.max(0.0)
    };
    let in_doorway = |p: Vec3| {
        (p.x - BULKHEAD.x).abs() < BULKHEAD_SIZE.x * 0.5 - 0.1 && p.y < BULKHEAD_SIZE.y - 0.1
    };
    let (first, lo, hi) = if from.z < ROOM_MIN.z {
        (leave(from, HALL_MIN, HALL_MAX), ROOM_MIN, ROOM_MAX)
    } else {
        (leave(from, ROOM_MIN, ROOM_MAX), HALL_MIN, HALL_MAX)
    };
    let hit = from + dir * first;
    // Through the doorway (the plane both volumes share): fly on.
    if open && (hit.z - ROOM_MIN.z).abs() < 1e-3 && in_doorway(hit) {
        first + leave(hit, lo, hi)
    } else {
        first
    }
}

/// A rotation that turns a plane facing +Z (a `Rectangle`'s front) to face
/// `dir` (horizontal).
pub fn facing(dir: Vec3) -> Quat {
    Quat::from_rotation_arc(Vec3::Z, dir.normalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn he_stays_on_the_floor_and_off_the_fixtures() {
        // Into the left wall beside the kiosk: stopped short of it.
        assert_eq!(confine(Vec3::new(-12.0, 0.0, 2.0), false).x, FLOOR.min.x);
        // Into the kiosk from the room: pushed back out its front.
        assert_eq!(confine(Vec3::new(-7.9, 0.0, -0.5), false).x, -7.75);
        // The bulkhead holds him until it opens.
        let through = Vec3::new(0.0, 0.0, ROOM_MIN.z - 2.0);
        assert_eq!(confine(through, false).z, FLOOR.min.y);
        assert_eq!(confine(through, true), through);
        // Every scripted spot is somewhere he can stand.
        for name in [
            "start", "center", "store", "bench", "save", "hack", "doors", "lift",
        ] {
            let (at, _) = spot(name).unwrap();
            assert_eq!(confine(at, false), at, "{name}");
        }
    }

    #[test]
    fn shots_stop_at_the_walls_or_fly_through_the_open_bulkhead() {
        let from = Vec3::new(0.0, 1.4, 1.0);
        assert_eq!(reach(from, Vec3::X, false), ROOM_MAX.x);
        assert_eq!(reach(from, Vec3::NEG_Y, false), 1.4);
        assert_eq!(reach(from, Vec3::NEG_Z, false), 1.0 - ROOM_MIN.z);
        assert_eq!(reach(from, Vec3::NEG_Z, true), 1.0 - HALL_MIN.z);
        // Beside the opening the wall still stops it.
        let beside = Vec3::new(3.0, 1.4, 1.0);
        assert_eq!(reach(beside, Vec3::NEG_Z, true), 1.0 - ROOM_MIN.z);
        // From the corridor back into the room, to the near wall.
        let hall = Vec3::new(0.0, 1.4, ROOM_MIN.z - 3.0);
        assert_eq!(reach(hall, Vec3::Z, true), ROOM_MAX.z - hall.z);
    }
}
