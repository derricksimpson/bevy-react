//! The camera tour. The showroom is a strip of beats (`ui/src/beats.ts`);
//! React sends the one to show (`tour.beat`) and this module films it: a
//! beat at the same spot as the last one glides there, a beat at another
//! fixture cuts while React covers the screen in hologram static (the dip).
//!
//! The camera also owns the frame's look: HDR, bloom (only the holograms and
//! the lights pass 1.0), the station's fog.

use bevy::camera::{Hdr, ShadowLodOrigin};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::cluster::{ClusterConfig, ClusterZConfig};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::ui::IsDefaultUiCamera;
use bevy_react::{ReactAppExt, react_message};

/// The camera that films the deck and draws the flat UI (`--shoot`
/// re-targets it).
#[derive(Component)]
pub struct MainCamera;

/// One beat's shot: where the camera stands, what it looks at, its vertical
/// field of view, and the spot it belongs to (same spot = glide).
pub struct Shot {
    pub eye: Vec3,
    pub look: Vec3,
    pub fov: f32,
    pub spot: u8,
}

const ENGINEER_SPOT: u8 = 1;

/// Index = beat (`ui/src/beats.ts`): title, suit, weapon, suit menu, store,
/// bench, doors, hack, locator.
pub const SHOTS: [Shot; 9] = [
    // 0 The title terminal in its alcove.
    Shot {
        eye: Vec3::new(-7.35, 1.62, 3.6),
        look: Vec3::new(-9.72, 1.6, 3.6),
        fov: 40.0,
        spot: 0,
    },
    // 1 Over the engineer's right shoulder: the spine, stasis and air.
    Shot {
        eye: Vec3::new(0.62, 1.78, 3.55),
        look: Vec3::new(0.12, 1.32, 0.6),
        fov: 42.0,
        spot: ENGINEER_SPOT,
    },
    // 2 Aiming: tight over the right shoulder, along the gun.
    Shot {
        eye: Vec3::new(0.95, 1.7, 2.75),
        look: Vec3::new(0.25, 1.45, -1.5),
        fov: 42.0,
        spot: ENGINEER_SPOT,
    },
    // 3 The suit menu projected in front of him, his shoulder at the edge.
    Shot {
        eye: Vec3::new(0.62, 1.72, 2.6),
        look: Vec3::new(0.85, 1.5, -0.4),
        fov: 42.0,
        spot: ENGINEER_SPOT,
    },
    // 4 The store kiosk.
    Shot {
        eye: Vec3::new(-6.05, 1.68, -0.5),
        look: Vec3::new(-8.95, 1.65, -0.5),
        fov: 42.0,
        spot: 2,
    },
    // 5 The bench.
    Shot {
        eye: Vec3::new(6.05, 1.68, -0.5),
        look: Vec3::new(8.95, 1.65, -0.5),
        fov: 42.0,
        spot: 3,
    },
    // 6 The far wall: save station, bulkhead lock, elevator.
    Shot {
        eye: Vec3::new(0.25, 1.85, 1.0),
        look: Vec3::new(0.25, 1.95, -5.0),
        fov: 48.0,
        spot: 4,
    },
    // 7 The hack panel, close (layout::HACK_PANEL), from its left so the
    // save station's hologram stays out of frame.
    Shot {
        eye: Vec3::new(-2.25, 1.5, -2.95),
        look: Vec3::new(-1.9, 1.45, -4.45),
        fov: 36.0,
        spot: 4,
    },
    // 8 High behind the engineer, the floor ahead in view.
    Shot {
        eye: Vec3::new(0.0, 3.4, 5.6),
        look: Vec3::new(0.0, 0.4, -1.5),
        fov: 52.0,
        spot: ENGINEER_SPOT,
    },
];

/// A glide between two shots at the same spot takes this long.
const GLIDE_SECS: f32 = 1.1;
/// A cut lands this long after the beat changes: the middle of React's
/// static dip (`ui/src/hud/Dip.tsx`), under full cover.
const CUT_AT_SECS: f32 = 0.24;

/// The fog every shot is filmed through.
pub const FOG: Color = Color::srgb(0.012, 0.02, 0.03);

/// React → Bevy: show beat `beat`.
#[react_message(name = "tour.beat")]
pub struct ShowBeat {
    pub beat: u32,
}

/// The beat on screen (the one React asked for last), for every system that
/// poses or shows things per beat.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub struct Beat(pub usize);

/// The camera's move toward the current beat's shot: where it started, how
/// long ago, whether it cuts, and the pose it is at — eye, look-at, fov,
/// before the handheld sway (a new move starts from here, so the sway never
/// counts twice).
#[derive(Resource)]
struct Move {
    from: (Vec3, Vec3, f32),
    t: f32,
    cut: bool,
    pose: (Vec3, Vec3, f32),
}

pub struct TourPlugin;

impl Plugin for TourPlugin {
    fn build(&self, app: &mut App) {
        register_bindings(app);
        app.init_resource::<Beat>()
            .insert_resource(Move {
                from: shot_pose(&SHOTS[0]),
                t: 0.0,
                cut: true,
                pose: shot_pose(&SHOTS[0]),
            })
            .add_systems(Startup, spawn_camera)
            .add_systems(Update, film);
    }
}

pub fn register_bindings(app: &mut App) {
    app.add_react_handler(on_beat);
}

fn shot_pose(shot: &Shot) -> (Vec3, Vec3, f32) {
    (shot.eye, shot.look, shot.fov)
}

fn spawn_camera(mut commands: Commands) {
    let (eye, look, fov) = shot_pose(&SHOTS[0]);
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(FOG),
            ..default()
        },
        Hdr,
        Tonemapping::TonyMcMapface,
        Bloom {
            intensity: 0.22,
            ..Bloom::NATURAL
        },
        Projection::from(PerspectiveProjection {
            fov: fov.to_radians(),
            near: 0.03,
            far: 80.0,
            ..default()
        }),
        DistanceFog {
            color: FOG,
            falloff: FogFalloff::Exponential { density: 0.075 },
            ..default()
        },
        Transform::from_translation(eye).looking_at(look, Vec3::Y),
        IsDefaultUiCamera,
        // Shadows are detailed around this camera, also when `--shoot`
        // points it at an image (no camera renders to the window then).
        ShadowLodOrigin,
        // A quarter of the default clusters: the deck's eight lights each
        // reach most of the room, and WebGL2's 16k light-index list would
        // overflow on the first frame at the default count.
        ClusterConfig::FixedZ {
            total: 1024,
            z_slices: 16,
            z_config: ClusterZConfig::default(),
            dynamic_resizing: true,
        },
        MainCamera,
    ));
}

fn on_beat(show: On<ShowBeat>, mut beat: ResMut<Beat>, mut motion: ResMut<Move>) {
    let next = (show.beat as usize).min(SHOTS.len() - 1);
    if next == beat.0 {
        return;
    }
    // From wherever the camera is now, mid-move included.
    *motion = Move {
        from: motion.pose,
        t: 0.0,
        cut: SHOTS[next].spot != SHOTS[beat.0].spot,
        pose: motion.pose,
    };
    beat.0 = next;
}

/// Move the camera along the current move, breathe a little at rest.
fn film(
    time: Res<Time>,
    beat: Res<Beat>,
    mut motion: ResMut<Move>,
    mut camera: Query<(&mut Transform, &mut Projection), With<MainCamera>>,
) {
    let Ok((mut transform, mut projection)) = camera.single_mut() else {
        return;
    };
    motion.t += time.delta_secs();
    let target = shot_pose(&SHOTS[beat.0]);
    let (eye, look, fov) = if motion.cut {
        if motion.t < CUT_AT_SECS {
            motion.from
        } else {
            target
        }
    } else {
        let k = (motion.t / GLIDE_SECS).clamp(0.0, 1.0);
        let k = k * k * k * (k * (k * 6.0 - 15.0) + 10.0);
        (
            motion.from.0.lerp(target.0, k),
            motion.from.1.lerp(target.1, k),
            motion.from.2 + (target.2 - motion.from.2) * k,
        )
    };
    motion.pose = (eye, look, fov);

    // A handheld breath: a couple of centimeters, never still.
    let t = time.elapsed_secs();
    let sway = Vec3::new(
        (t * 0.37).sin() * 0.012 + (t * 0.91).sin() * 0.004,
        (t * 0.53).sin() * 0.008,
        0.0,
    );
    *transform = Transform::from_translation(eye + sway).looking_at(look + sway * 0.5, Vec3::Y);
    if let Projection::Perspective(p) = &mut *projection {
        let fov = fov.to_radians();
        if (p.fov - fov).abs() > 1e-5 {
            p.fov = fov;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SHOTS;

    /// React decides glide vs. dip from the same spots (`SPOTS` in
    /// `ui/src/beats.ts`); they must agree with the shots.
    #[test]
    fn spots_match_react() {
        let ts = include_str!("ui/src/beats.ts");
        let spots: Vec<String> = SHOTS.iter().map(|s| s.spot.to_string()).collect();
        let row = format!("export const SPOTS = [{}];", spots.join(", "));
        assert!(ts.contains(&row), "beats.ts lacks `{row}`");
    }
}
