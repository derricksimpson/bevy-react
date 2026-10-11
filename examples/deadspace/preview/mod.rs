//! The little worlds inside the holograms, each filmed into a `<portal>`:
//!
//!   * `preview` — the turntable showing whatever React asked for
//!     (`preview.show`), lit cold like a projection;
//!   * `map` — the deck as a hologram miniature you drag to turn and wheel to
//!     zoom (`map.orbit`, `map.zoom`), the objective marked (`map.mark`);
//!   * `icon-<item>` — every item's inventory icon, filmed once.
//!
//! Each world has its own render layer, and they stand far below the deck:
//! Bevy's GPU light clustering ignores render layers, so only distance keeps
//! the station's lights out of them (and theirs out of the station).

mod items;
mod map;

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy_react::{
    PortalCamera, ReactAppExt, RenderMode, RenderTargetSpec, RenderTargets, Resolution,
    react_message,
};

use items::{ITEMS, ItemKit, spawn_item};

const PREVIEW_LAYER: usize = 1;
const ICON_LAYER: usize = 2;
pub(crate) const MAP_LAYER: usize = 3;
const PREVIEW_AT: Vec3 = Vec3::new(0.0, -400.0, 0.0);
const ICONS_AT: Vec3 = Vec3::new(0.0, -800.0, 0.0);
pub(crate) const MAP_AT: Vec3 = Vec3::new(0.0, -1200.0, 0.0);
/// Icons stand this far apart in their row: beyond each other's lights.
const ICON_SPACING: f32 = 20.0;
const ICON_PX: u32 = 128;
/// One turn of the turntable takes this long.
const TURN_SECS: f32 = 9.0;

/// React → Bevy: put `item` on the preview turntable.
#[react_message(name = "preview.show")]
pub struct ShowItem {
    pub item: String,
}

/// React → Bevy: turn the map by a drag (logical px).
#[react_message(name = "map.orbit")]
pub struct OrbitMap {
    pub dx: f32,
    pub dy: f32,
}

/// React → Bevy: zoom the map (wheel lines; + = closer).
#[react_message(name = "map.zoom")]
pub struct ZoomMap {
    pub delta: f32,
}

/// React → Bevy: mark where the objective leads on the map (a
/// `layout::destination` name).
#[react_message(name = "map.mark")]
pub struct MarkMap {
    pub to: String,
}

pub struct PreviewPlugin;

impl Plugin for PreviewPlugin {
    fn build(&self, app: &mut App) {
        register_bindings(app);
        app.add_systems(Startup, (spawn_preview, spawn_icons, map::spawn_map))
            .add_systems(
                Update,
                (turn, retake_icons, map::orbit_camera, map::pulse_marker),
            );
    }
}

pub fn register_bindings(app: &mut App) {
    app.add_react_handler(on_show)
        .add_react_handler(map::on_orbit)
        .add_react_handler(map::on_zoom)
        .add_react_handler(map::on_mark)
        .add_react_handler(map::on_unlock);
}

/// The turntable: the item on it (its root turns) and what it is.
#[derive(Component)]
struct Turntable {
    item: Option<(String, Entity)>,
}

/// A cold, projected look: a cyan-white key, a deep blue rim, little fill.
fn studio_lights(commands: &mut Commands, at: Vec3, layers: &RenderLayers) {
    for (offset, color, intensity) in [
        (
            Vec3::new(1.2, 1.6, 1.6),
            Color::srgb(0.82, 0.95, 1.0),
            24_000.0,
        ),
        (
            Vec3::new(-1.6, 0.8, -1.4),
            Color::srgb(0.2, 0.55, 1.0),
            20_000.0,
        ),
    ] {
        commands.spawn((
            PointLight {
                color,
                intensity,
                range: 8.0,
                ..default()
            },
            Transform::from_translation(at + offset),
            layers.clone(),
        ));
    }
}

/// A camera filming `look` from `eye` into the target `name`, transparent
/// behind (the hologram's glass shows through).
fn portal_camera(
    name: &str,
    eye: Vec3,
    look: Vec3,
    fov: f32,
    layers: &RenderLayers,
    target: bevy::camera::RenderTarget,
) -> impl Bundle {
    (
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::NONE),
            is_active: false,
            ..default()
        },
        AmbientLight {
            color: Color::srgb(0.5, 0.7, 1.0),
            brightness: 260.0,
            ..default()
        },
        Projection::from(PerspectiveProjection {
            fov: fov.to_radians(),
            near: 0.02,
            far: 60.0,
            ..default()
        }),
        Transform::from_translation(eye).looking_at(look, Vec3::Y),
        target,
        layers.clone(),
        PortalCamera(name.into()),
    )
}

fn spawn_preview(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut targets: ResMut<RenderTargets>,
) {
    let layers = RenderLayers::layer(PREVIEW_LAYER);
    let target = targets.create(&mut images, "preview", RenderTargetSpec::default());
    commands.spawn(portal_camera(
        "preview",
        PREVIEW_AT + Vec3::new(0.0, 0.32, 1.25),
        PREVIEW_AT + Vec3::Y * 0.02,
        30.0,
        &layers,
        target.camera_target(),
    ));
    studio_lights(&mut commands, PREVIEW_AT, &layers);
    // The projector's pad: a thin glowing ring under the item.
    commands.spawn((
        Mesh3d(meshes.add(Annulus::new(0.3, 0.315))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: LinearRgba::rgb(0.5, 2.2, 3.0),
            unlit: true,
            ..default()
        })),
        Transform::from_translation(PREVIEW_AT - Vec3::Y * 0.2)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        layers.clone(),
    ));
    commands.insert_resource(ItemKit::new(&mut meshes, &mut materials));
    commands.spawn(Turntable { item: None });
}

/// Swap the turntable's item.
fn on_show(
    show: On<ShowItem>,
    mut commands: Commands,
    kit: Res<ItemKit>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut turntable: Single<&mut Turntable>,
) {
    if turntable
        .item
        .as_ref()
        .is_some_and(|(id, _)| *id == show.item)
    {
        return;
    }
    let Some(&(_, radius)) = ITEMS.iter().find(|(id, _)| *id == show.item) else {
        return;
    };
    if let Some((_, old)) = turntable.item.take() {
        commands.entity(old).despawn();
    }
    let layers = RenderLayers::layer(PREVIEW_LAYER);
    let root = spawn_item(
        &mut commands,
        &kit,
        &mut meshes,
        &mut materials,
        &show.item,
        PREVIEW_AT,
        &layers,
    );
    // Fit any item in the same frame: the camera frames a 0.3 m radius.
    commands
        .entity(root)
        .insert(Transform::from_translation(PREVIEW_AT).with_scale(Vec3::splat(0.3 / radius)));
    turntable.item = Some((show.item.clone(), root));
}

fn turn(time: Res<Time>, turntable: Single<&Turntable>, mut transforms: Query<&mut Transform>) {
    if let Some((_, root)) = &turntable.item
        && let Ok(mut transform) = transforms.get_mut(*root)
    {
        transform.rotation =
            Quat::from_rotation_y(time.elapsed_secs() * std::f32::consts::TAU / TURN_SECS);
    }
}

fn spawn_icons(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut targets: ResMut<RenderTargets>,
) {
    let layers = RenderLayers::layer(ICON_LAYER);
    let kit = ItemKit::new(&mut meshes, &mut materials);
    for (i, (id, radius)) in ITEMS.iter().enumerate() {
        let at = ICONS_AT + Vec3::X * ICON_SPACING * i as f32;
        let name = format!("icon-{id}");
        let target = targets.create(
            &mut images,
            &name,
            RenderTargetSpec {
                size: Resolution::Fixed(UVec2::splat(ICON_PX)),
                mode: RenderMode::Snapshot,
                ..default()
            },
        );
        // A three-quarter view from above, the item filling the frame.
        let eye = at + Vec3::new(0.55, 0.62, 1.0).normalize() * radius * 3.4;
        commands.spawn(portal_camera(
            &name,
            eye,
            at,
            34.0,
            &layers,
            target.camera_target(),
        ));
        let root = spawn_item(
            &mut commands,
            &kit,
            &mut meshes,
            &mut materials,
            id,
            at,
            &layers,
        );
        commands
            .entity(root)
            .insert(Transform::from_translation(at).with_rotation(Quat::from_rotation_y(-0.5)));
        studio_lights(&mut commands, at, &layers);
    }
}

/// A snapshot renders once, and its one frame would come out wrong: meshes
/// missing (pipelines still compiling) and unlit (a view gets its lights
/// from its second consecutive frame). So the icons are retaken as the app
/// settles, over a few consecutive frames each (cyberpunk's recipe).
// ponytail: fixed retake times; key them to pipeline readiness if a slow
// GPU still shows partial icons at 45 s.
fn retake_icons(
    time: Res<Time>,
    mut targets: ResMut<RenderTargets>,
    mut taken: Local<usize>,
    mut frames: Local<u32>,
) {
    const AT: [f32; 5] = [1.0, 3.0, 8.0, 20.0, 45.0];
    if AT.get(*taken).is_none_or(|&at| time.elapsed_secs() < at) {
        return;
    }
    for (id, _) in ITEMS {
        targets.invalidate(&format!("icon-{id}"));
    }
    *frames += 1;
    if *frames == 6 {
        *frames = 0;
        *taken += 1;
    }
}
