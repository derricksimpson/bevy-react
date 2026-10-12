//! The camera. Free, it rides over his right shoulder: it orbits a point at
//! his neck with the mouse, comes in tighter while he aims, and shortens
//! its boom rather than leave the room. Held by a screen (`lock.rs`), it
//! stands off that screen and looks at it square; it glides between the
//! two, and from one screen to the next.
//!
//! The camera owns the frame's look too: HDR, bloom (only the holograms and
//! the lights pass 1.0), the station's fog.

use bevy::camera::{Hdr, ShadowLodOrigin};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::cluster::{ClusterConfig, ClusterZConfig};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::ui::IsDefaultUiCamera;

use super::Player;
use crate::doors::Bulkhead;
use crate::layout;

/// The camera that films the deck (`--shoot` re-targets it).
#[derive(Component)]
pub struct MainCamera;

/// Where the camera rides: right of, above and behind the point at his
/// neck it orbits, and its vertical field of view (degrees).
struct Rig {
    offset: Vec3,
    fov: f32,
}

const WALKING: Rig = Rig {
    offset: Vec3::new(0.62, 0.25, 2.1),
    fov: 46.0,
};
const AIMING: Rig = Rig {
    offset: Vec3::new(0.8, 0.17, 1.3),
    fov: 40.0,
};
/// The widest the view gets (radians): a locked camera fits its screen
/// into this.
pub const WIDE: f32 = WALKING.fov * std::f32::consts::PI / 180.0;
/// The height of the point the camera orbits.
const NECK: f32 = 1.55;
/// The camera keeps this far off the walls.
const MARGIN: f32 = 0.22;
/// Taking or leaving a screen, the camera glides this long.
const GLIDE_SECS: f32 = 0.6;

/// The fog everything is filmed through.
const FOG: Color = Color::srgb(0.012, 0.02, 0.03);

pub(super) fn spawn_camera(mut commands: Commands) {
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
            fov: WALKING.fov.to_radians(),
            near: 0.03,
            far: 80.0,
            ..default()
        }),
        DistanceFog {
            color: FOG,
            falloff: FogFalloff::Exponential { density: 0.075 },
            ..default()
        },
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

/// Where the camera is, the way it looks, its vertical field of view
/// (radians).
#[derive(Clone, Copy, Default)]
struct Pose {
    eye: Vec3,
    look: Quat,
    fov: f32,
}

impl Pose {
    fn lerp(&self, to: &Pose, k: f32) -> Pose {
        Pose {
            eye: self.eye.lerp(to.eye, k),
            look: self.look.slerp(to.look, k),
            fov: self.fov + (to.fov - self.fov) * k,
        }
    }
}

/// How far out its boom the free camera is (1 = all the way), and the
/// glide in flight: from which pose, how far along (1 = arrived), toward
/// which screen's hold (`None` = free).
#[derive(Default)]
pub(super) struct Lens {
    boom: f32,
    from: Pose,
    glide: f32,
    held: Option<&'static str>,
    shown: Option<Pose>,
}

/// Put the camera behind him or before the screen that holds it, and
/// breathe a little.
pub(super) fn film(
    time: Res<Time>,
    bulkhead: Option<Res<Bulkhead>>,
    mut player: ResMut<Player>,
    mut lens: Local<Lens>,
    mut camera: Single<(&mut Transform, &mut Projection), With<MainCamera>>,
) {
    let dt = time.delta_secs();
    let open = bulkhead.is_some_and(|b| b.is_open());
    let (transform, projection) = &mut *camera;
    let Projection::Perspective(lens_p) = &mut **projection else {
        return;
    };
    let aspect = lens_p.aspect_ratio;

    // Free: over his shoulder. In along the boom until the camera is inside
    // the room; out again slowly, so it doesn't pump while he walks along a
    // wall.
    let aim = player.aim;
    let view = player.view();
    let neck = player.at + Vec3::Y * NECK;
    let boom = view * WALKING.offset.lerp(AIMING.offset, aim);
    let mut reach = 1.0;
    while reach > 0.08 && !layout::inside(neck + boom * reach, MARGIN, open) {
        reach -= 0.04;
    }
    lens.boom = if reach < lens.boom {
        reach
    } else {
        lens.boom + (reach - lens.boom) * (1.0 - (-6.0 * dt).exp())
    };
    let mut goal = Pose {
        eye: neck + boom * lens.boom,
        look: view,
        fov: (WALKING.fov + (AIMING.fov - WALKING.fov) * aim).to_radians(),
    };
    // Held: off the screen, square to it, nearer if a wall is in the way.
    if let Some((_, shot)) = player.shot(aspect) {
        let mut distance = shot.distance;
        while distance > 0.6 && !layout::inside(shot.eye(distance), MARGIN, open) {
            distance -= 0.05;
        }
        goal = Pose {
            eye: shot.eye(distance),
            look: Transform::default()
                .looking_to(-shot.normal, Vec3::Y)
                .rotation,
            fov: shot.fov(distance, aspect),
        };
    }

    // Glide from where it was whenever the hold changes.
    if lens.held != player.held {
        lens.held = player.held;
        lens.from = lens.shown.unwrap_or(goal);
        lens.glide = 0.0;
    }
    lens.glide = (lens.glide + dt / GLIDE_SECS).min(1.0);
    let k = lens.glide;
    let shown = lens.from.lerp(&goal, k * k * (3.0 - 2.0 * k));
    lens.shown = Some(shown);

    // A handheld breath: a couple of centimeters, never still.
    let t = time.elapsed_secs();
    let sway = shown.look
        * Vec3::new(
            (t * 0.37).sin() * 0.012 + (t * 0.91).sin() * 0.004,
            (t * 0.53).sin() * 0.008,
            0.0,
        );
    **transform = Transform::from_translation(shown.eye + sway).with_rotation(shown.look);
    if (lens_p.fov - shown.fov).abs() > 1e-5 {
        lens_p.fov = shown.fov;
    }
    player.zoom = shown.fov / WIDE;
}
