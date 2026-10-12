//! The strategy camera: looks down on the map from the south, pans with
//! WASD/arrows or a left-drag, zooms with the wheel, and flies wherever
//! React asks. A second, top-down camera films a flat copy of the map into
//! the minimap's `<portal>`, with your view outlined on it.

use bevy::camera::ScalingMode;
use bevy::camera::visibility::RenderLayers;
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use bevy::ui::IsDefaultUiCamera;
use bevy::window::PrimaryWindow;
use bevy_react::anchor::Anchored;
use bevy_react::{
    PointerCapture, PointerCaptureSet, PortalCamera, ReactAppExt, ReactInteraction,
    RenderTargetSpec, RenderTargets, react_message,
};

use crate::map::hex;
use crate::map::mesh::LAND;
use crate::map::pick::TilePos;
use crate::map::world::{COLS, ROWS, World};

pub const MINIMAP: &str = "minimap";
/// What the minimap camera sees, and no other does: the flat map
/// (`mesh::minimap`) and your view's outline.
pub const MINIMAP_LAYER: usize = 1;
const MIN_HEIGHT: f32 = 9.0;
const MAX_HEIGHT: f32 = 42.0;
/// How fast a flight settles, per second: most of the way in a fifth of a
/// second.
const FLIGHT_EASE: f32 = 14.0;
/// The zoom settles twice as fast.
const ZOOM_EASE: f32 = 28.0;

#[derive(Component)]
pub struct MainCamera;

/// Where the camera looks (ground x, z) and how far out it is (0..1). The
/// view eases toward the targets: the zoom always, the focus when the camera
/// is sent somewhere — a pan moves both at once.
#[derive(Resource)]
pub struct Rig {
    focus: Vec2,
    target: Vec2,
    zoom: f32,
    target_zoom: f32,
    /// The point of the ground a left-drag holds under the cursor.
    grabbed: Option<Vec2>,
}

impl Rig {
    pub fn fly_to(&mut self, tile: IVec2, zoom: Option<f32>) {
        self.target = hex::center(tile);
        if let Some(z) = zoom {
            self.target_zoom = z.clamp(0.0, 1.0);
        }
    }

    /// Move the view at once, taking a flight in progress along.
    fn pan(&mut self, delta: Vec2) {
        self.focus += delta;
        self.target += delta;
    }

    fn height(&self) -> f32 {
        MIN_HEIGHT + (MAX_HEIGHT - MIN_HEIGHT) * self.zoom.powf(1.3)
    }

    /// Where the camera stands for this focus and zoom. A change of focus
    /// moves it without turning it.
    fn view(&self) -> Transform {
        // Further out, the view tips toward straight down.
        let pitch = (50.0 + 14.0 * self.zoom).to_radians();
        let height = self.height();
        let ground = Vec3::new(self.focus.x, 0.0, self.focus.y);
        Transform::from_translation(ground + Vec3::new(0.0, height, height / pitch.tan()))
            .looking_at(ground, Vec3::Y)
    }
}

/// The point of the land (x, z) that `cursor` is over, seen from `eye`.
pub fn ground_at(camera: &Camera, eye: &GlobalTransform, cursor: Vec2) -> Option<Vec2> {
    let ray = camera.viewport_to_world(eye, cursor).ok()?;
    let d = ray.intersect_plane(Vec3::Y * LAND, InfinitePlane3d::new(Vec3::Y))?;
    Some(ray.get_point(d).xz())
}

/// React → Bevy: fly to a tile (and optionally a zoom, 0 close … 1 far).
#[react_message(name = "map.focus")]
pub struct Focus {
    pub tile: TilePos,
    pub zoom: Option<f32>,
}

/// React → Bevy: look at a point of the minimap (`0..1` across and down).
#[react_message(name = "map.jump")]
pub struct Jump {
    pub u: f32,
    pub v: f32,
}

#[derive(Default, Reflect, GizmoConfigGroup)]
struct MinimapGizmos;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        register_bindings(app);
        app.insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.82, 0.88, 1.0),
            brightness: 420.0,
            ..default()
        })
        .init_gizmo_group::<MinimapGizmos>()
        .add_systems(Startup, spawn_cameras)
        .add_systems(
            Update,
            (drive.after(PointerCaptureSet), outline_view).chain(),
        );
    }
}

pub fn register_bindings(app: &mut App) {
    app.add_react_handler(on_focus).add_react_handler(on_jump);
}

fn bounds() -> Rect {
    let far = hex::center(IVec2::new(COLS - 1, ROWS - 1));
    Rect::new(0.0, 0.0, far.x + hex::WIDTH * 0.5, far.y)
}

fn spawn_cameras(
    mut commands: Commands,
    world: Res<World>,
    mut targets: ResMut<RenderTargets>,
    mut images: ResMut<Assets<Image>>,
    mut gizmos: ResMut<GizmoConfigStore>,
) {
    let home = hex::center(world.cities[0].tile);
    commands.insert_resource(Rig {
        focus: home,
        target: home,
        zoom: 0.35,
        target_zoom: 0.35,
        grabbed: None,
    });
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: 40f32.to_radians(),
            ..default()
        }),
        Transform::default(),
        IsDefaultUiCamera,
        MainCamera,
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 9_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-5.0, 10.0, 7.0).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 14.0,
            maximum_distance: 70.0,
            ..default()
        }
        .build(),
    ));

    let area = bounds();
    let minimap = targets.create(&mut images, MINIMAP, RenderTargetSpec::default());
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: area.width() + 1.5,
                min_height: area.height() + 1.5,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(area.center().x, 40.0, area.center().y)
            .looking_at(area.center().extend(0.0).xzy(), Vec3::NEG_Z),
        minimap.camera_target(),
        PortalCamera(MINIMAP.into()),
        RenderLayers::layer(MINIMAP_LAYER),
    ));
    let (config, _) = gizmos.config_mut::<MinimapGizmos>();
    config.render_layers = RenderLayers::layer(MINIMAP_LAYER);
    config.line.width = 2.5;
}

/// Whether the pointer is over the HUD: an interactive element that is not
/// pinned to the map. The banners and flags are `<anchor>`s, and the wheel
/// zooms the map through them.
fn over_hud(
    hovered: &Query<(Entity, &ReactInteraction)>,
    parents: &Query<&ChildOf>,
    anchored: &Query<(), With<Anchored>>,
) -> bool {
    hovered.iter().any(|(entity, interaction)| {
        *interaction != ReactInteraction::None
            && !std::iter::once(entity)
                .chain(parents.iter_ancestors(entity))
                .any(|e| anchored.contains(e))
    })
}

#[allow(clippy::too_many_arguments)]
fn drive(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    capture: Res<PointerCapture>,
    hovered: Query<(Entity, &ReactInteraction)>,
    parents: Query<&ChildOf>,
    anchored: Query<(), With<Anchored>>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut rig: ResMut<Rig>,
    camera: Single<(&Camera, &mut Transform), With<MainCamera>>,
) {
    let (camera, mut transform) = camera.into_inner();
    let dt = time.delta_secs();
    // The land under the cursor, were the camera where the rig puts it.
    let under_cursor = |rig: &Rig| {
        let eye = GlobalTransform::from(rig.view());
        ground_at(camera, &eye, window.cursor_position()?)
    };

    let mut pan = Vec2::ZERO;
    for (key, dir) in [
        (KeyCode::KeyW, Vec2::NEG_Y),
        (KeyCode::ArrowUp, Vec2::NEG_Y),
        (KeyCode::KeyS, Vec2::Y),
        (KeyCode::ArrowDown, Vec2::Y),
        (KeyCode::KeyA, Vec2::NEG_X),
        (KeyCode::ArrowLeft, Vec2::NEG_X),
        (KeyCode::KeyD, Vec2::X),
        (KeyCode::ArrowRight, Vec2::X),
    ] {
        if keys.pressed(key) {
            pan += dir;
        }
    }
    let step = pan.normalize_or_zero() * rig.height() * 1.1 * dt;
    rig.pan(step);

    // A left-drag that starts on the map grabs the land under the cursor
    // (and stops a flight).
    if buttons.just_pressed(MouseButton::Left) {
        rig.grabbed = under_cursor(&rig).filter(|_| !capture.is_captured());
        if rig.grabbed.is_some() {
            rig.target = rig.focus;
        }
    }
    if !buttons.pressed(MouseButton::Left) {
        rig.grabbed = None;
    }

    if !(capture.dragging || capture.wheel_captured || over_hud(&hovered, &parents, &anchored)) {
        let notches = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / 100.0,
        };
        rig.target_zoom = (rig.target_zoom - notches * 0.07).clamp(0.0, 1.0);
    }

    let (target, target_zoom) = (rig.target, rig.target_zoom);
    rig.focus = rig.focus.lerp(target, 1.0 - (-FLIGHT_EASE * dt).exp());
    rig.zoom += (target_zoom - rig.zoom) * (1.0 - (-ZOOM_EASE * dt).exp());

    // The grabbed land stays under the cursor, whatever the zoom does: moving
    // the focus slides the view without turning it, so the land the cursor
    // is over moves by exactly as much.
    if let Some(grabbed) = rig.grabbed
        && let Some(at) = under_cursor(&rig)
    {
        rig.pan(grabbed - at);
    }

    let area = bounds();
    rig.focus = rig.focus.clamp(area.min, area.max);
    rig.target = rig.target.clamp(area.min, area.max);
    *transform = rig.view();
}

/// Outline the main camera's view of the ground on the minimap.
fn outline_view(
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut gizmos: Gizmos<MinimapGizmos>,
) {
    let (camera, eye) = *camera;
    let Some(size) = camera.logical_viewport_size() else {
        return;
    };
    let corners = [
        Vec2::ZERO,
        Vec2::new(size.x, 0.0),
        size,
        Vec2::new(0.0, size.y),
    ];
    let ground: Vec<Vec3> = corners
        .iter()
        .filter_map(|&p| {
            let ray = camera.viewport_to_world(eye, p).ok()?;
            let d = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))?;
            Some(ray.get_point(d).with_y(3.0))
        })
        .collect();
    if ground.len() == 4 {
        gizmos.linestrip(ground.iter().chain(&ground[..1]).copied(), Color::WHITE);
    }
}

fn on_focus(focus: On<Focus>, mut rig: ResMut<Rig>) {
    let f = focus.event();
    rig.fly_to(IVec2::new(f.tile.col, f.tile.row), f.zoom);
}

fn on_jump(jump: On<Jump>, mut rig: ResMut<Rig>, minimap: Single<&Projection, With<PortalCamera>>) {
    // The minimap camera looks straight down, north up, at the map's
    // center; its projection knows how much ground the portal spans. A
    // pan: the view goes there at once.
    if let Projection::Orthographic(view) = *minimap {
        rig.target = bounds().center() + (Vec2::new(jump.u, jump.v) - 0.5) * view.area.size();
        rig.focus = rig.target;
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;

    /// Dragging pins the land to the cursor: after the pan that a cursor
    /// move asks for, the cursor is over the point it grabbed.
    #[test]
    fn a_drag_keeps_the_grabbed_land_under_the_cursor() {
        use bevy::camera::{ComputedCameraValues, RenderTargetInfo};

        let camera = Camera {
            computed: ComputedCameraValues {
                clip_from_view: bevy::math::proj::perspective_infinite_reverse(
                    40f32.to_radians(),
                    1440.0 / 900.0,
                    0.1,
                ),
                target_info: Some(RenderTargetInfo {
                    physical_size: UVec2::new(1440, 900),
                    scale_factor: 1.0,
                }),
                ..default()
            },
            ..default()
        };
        let mut rig = Rig {
            focus: Vec2::new(30.0, 20.0),
            target: Vec2::new(30.0, 20.0),
            zoom: 0.35,
            target_zoom: 0.35,
            grabbed: None,
        };
        let under = |rig: &Rig, cursor: Vec2| {
            ground_at(&camera, &GlobalTransform::from(rig.view()), cursor).unwrap()
        };

        let grabbed = under(&rig, Vec2::new(200.0, 700.0));
        // The cursor crosses the screen, and the zoom changes under it.
        let cursor = Vec2::new(1300.0, 120.0);
        rig.zoom = 0.8;
        let at = under(&rig, cursor);
        rig.pan(grabbed - at);

        assert!(under(&rig, cursor).distance(grabbed) < 1e-3);
    }

    #[test]
    fn the_wheel_passes_through_anchors_only() {
        let mut world = bevy::ecs::world::World::new();
        let probe = |world: &mut bevy::ecs::world::World| {
            world
                .run_system_once(
                    |hovered: Query<(Entity, &ReactInteraction)>,
                     parents: Query<&ChildOf>,
                     anchored: Query<(), With<Anchored>>| {
                        over_hud(&hovered, &parents, &anchored)
                    },
                )
                .unwrap()
        };
        let banner = world
            .spawn(Anchored {
                target: Entity::PLACEHOLDER,
                offset: Vec3::ZERO,
                scale: None,
            })
            .id();
        let button = world
            .spawn((ReactInteraction::Hovered, ChildOf(banner)))
            .id();
        let panel = world.spawn(ReactInteraction::None).id();
        assert!(!probe(&mut world), "a hovered banner is not HUD");

        world.entity_mut(panel).insert(ReactInteraction::Hovered);
        assert!(probe(&mut world), "a hovered panel is");

        world.entity_mut(button).insert(ReactInteraction::None);
        world.entity_mut(panel).insert(ReactInteraction::None);
        assert!(!probe(&mut world), "nothing hovered");
    }
}
