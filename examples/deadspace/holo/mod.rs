//! The holograms. Every readout in the game is a React `<surface>` (see
//! `ui/src/holos/`) on a quad in the world, drawn by one material
//! (`hologram.wgsl`): premultiplied UI, scanlines, flicker, an edge glow,
//! pushed past 1.0 so it blooms. React owns what is on them and which are
//! open (`holo.open`); this module owns where they hang and how they unfold.
//!
//! Most hang at a fixed spot in the room. The ones worn by the engineer or
//! his gun (`spine`, `stasis`, `air`, `ammo`) hang on a [`HoloAnchor`] the
//! body code places, so they follow every pose.

mod material;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_react::surface::{SurfacePointer, SurfaceSpec, Surfaces};
use bevy_react::{ReactAppExt, react_message};

pub use material::HologramMaterial;

use crate::layout::{self, facing};

/// One hologram: its surface's name and logical size (React lays it out in
/// these px), its size in the world, and where it hangs.
pub struct Holo {
    pub name: &'static str,
    /// Logical px (must match `SURFACES` in `ui/src/holos/surfaces.ts`).
    pub px: Vec2,
    /// Width in meters (the height follows the aspect).
    pub width_m: f32,
    /// Texture px per logical px at 1080p: raised for the ones the camera
    /// films close.
    pub density: f32,
    pub place: Place,
}

pub enum Place {
    /// A fixed spot: center and facing direction.
    World(Vec3, Vec3),
    /// On the [`HoloAnchor`] of the same name.
    Anchor,
}

pub const HOLOS: [Holo; 13] = [
    Holo {
        name: "spine",
        px: Vec2::new(48.0, 360.0),
        width_m: 0.075,
        density: 1.5,
        place: Place::Anchor,
    },
    Holo {
        name: "stasis",
        px: Vec2::new(160.0, 160.0),
        width_m: 0.2,
        density: 1.5,
        place: Place::Anchor,
    },
    Holo {
        name: "air",
        px: Vec2::new(220.0, 110.0),
        width_m: 0.36,
        density: 1.5,
        place: Place::Anchor,
    },
    Holo {
        name: "ammo",
        px: Vec2::new(220.0, 130.0),
        width_m: 0.2,
        density: 1.5,
        place: Place::Anchor,
    },
    // In front of him and to his right, turned to the camera over his
    // shoulder (`tour.rs` beat 3).
    Holo {
        name: "menu",
        px: Vec2::new(1280.0, 720.0),
        width_m: 2.4,
        density: 1.0,
        place: Place::World(Vec3::new(1.05, 1.55, -0.4), Vec3::new(-0.14, 0.0, 1.0)),
    },
    Holo {
        name: "store",
        px: Vec2::new(1280.0, 760.0),
        width_m: 2.4,
        density: 1.0,
        place: Place::World(Vec3::new(-8.95, 1.65, -0.5), Vec3::X),
    },
    Holo {
        name: "bench",
        px: Vec2::new(1280.0, 760.0),
        width_m: 2.4,
        density: 1.0,
        place: Place::World(Vec3::new(8.95, 1.65, -0.5), Vec3::NEG_X),
    },
    Holo {
        name: "lock",
        px: Vec2::new(200.0, 200.0),
        width_m: 0.7,
        density: 1.0,
        place: Place::World(Vec3::new(0.0, 1.65, -4.8), Vec3::Z),
    },
    Holo {
        name: "save",
        px: Vec2::new(480.0, 320.0),
        width_m: 1.1,
        density: 1.0,
        place: Place::World(Vec3::new(layout::SAVE_STATION.x, 1.55, -4.55), Vec3::Z),
    },
    Holo {
        name: "elevator",
        px: Vec2::new(240.0, 360.0),
        width_m: 0.5,
        density: 1.0,
        place: Place::World(Vec3::new(2.1, 1.45, -4.78), Vec3::Z),
    },
    Holo {
        name: "floor",
        px: Vec2::new(240.0, 80.0),
        width_m: 0.9,
        density: 1.0,
        place: Place::World(Vec3::new(3.7, 3.15, -4.86), Vec3::Z),
    },
    Holo {
        name: "hack",
        px: Vec2::new(420.0, 420.0),
        width_m: 0.8,
        density: 1.5,
        place: Place::World(Vec3::new(layout::HACK_PANEL.x, 1.45, -4.45), Vec3::Z),
    },
    Holo {
        name: "title",
        px: Vec2::new(960.0, 540.0),
        width_m: 1.6,
        density: 1.0,
        place: Place::World(Vec3::new(-9.72, 1.6, 3.6), Vec3::X),
    },
];

/// Where a body-worn hologram hangs: the engineer and weapon code spawn one
/// per worn hologram, as a child of the part it rides on, facing the way
/// the hologram should face (+Z of the anchor = the hologram's front).
#[derive(Component)]
pub struct HoloAnchor(pub &'static str);

/// React → Bevy: unfold (`open`) or fold away a hologram.
#[react_message(name = "holo.open")]
pub struct OpenHolo {
    pub surface: String,
    pub open: bool,
}

/// A hologram's quad. `presence` eases toward whether React wants it open
/// ([`Requested`]); at 0 the quad hides (and its surface stops rendering).
#[derive(Component)]
pub struct HoloQuad {
    pub name: &'static str,
    pub presence: f32,
}

/// The holograms React asked to be open. Kept apart from the quads: a worn
/// hologram's quad only exists once its anchor does, after React's first
/// `holo.open`s have landed.
#[derive(Resource, Default)]
struct Requested(Vec<String>);

/// Unfolding and folding take this long.
const OPEN_SECS: f32 = 0.55;
const CLOSE_SECS: f32 = 0.3;

pub struct HoloPlugin;

impl Plugin for HoloPlugin {
    fn build(&self, app: &mut App) {
        register_bindings(app);
        app.add_plugins(MaterialPlugin::<HologramMaterial>::default())
            .init_resource::<Requested>()
            .add_systems(Startup, create_surfaces)
            .add_systems(Update, (hang_worn_holos, unfold).chain());
    }
}

pub fn register_bindings(app: &mut App) {
    app.add_react_handler(on_open);
}

/// The quad mesh and material of each hologram, made at startup.
#[derive(Resource)]
struct HoloAssets(Vec<(Handle<Mesh>, Handle<HologramMaterial>)>);

fn create_surfaces(
    mut commands: Commands,
    mut surfaces: ResMut<Surfaces>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<HologramMaterial>>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    // Texture density follows the screen at startup: one texel per logical
    // px at 1080p.
    let screen = (window.physical_height() as f32 / 1080.0).max(1.0);
    let mut assets = Vec::new();
    for (i, holo) in HOLOS.iter().enumerate() {
        let scale = screen * holo.density;
        let texture = surfaces.create(
            &mut images,
            holo.name,
            SurfaceSpec {
                size: (holo.px * scale).round().as_uvec2(),
                clear_color: Color::NONE,
                scale_factor: scale,
                ..default()
            },
        );
        let size = Vec2::new(holo.width_m, holo.width_m * holo.px.y / holo.px.x);
        let mesh = meshes.add(Rectangle::from_size(size));
        let material = materials.add(HologramMaterial::new(texture, holo.px, i as f32 * 7.31));
        if let Place::World(at, dir) = holo.place {
            commands.spawn((
                quad(holo.name, mesh.clone(), material.clone()),
                Transform::from_translation(at).with_rotation(facing(dir)),
            ));
        }
        assets.push((mesh, material));
    }
    commands.insert_resource(HoloAssets(assets));
}

fn quad(name: &'static str, mesh: Handle<Mesh>, material: Handle<HologramMaterial>) -> impl Bundle {
    (
        HoloQuad {
            name,
            presence: 0.0,
        },
        Mesh3d(mesh),
        MeshMaterial3d(material),
        SurfacePointer::new(name),
        Visibility::Hidden,
        bevy::light::NotShadowCaster,
    )
}

/// Hang each worn hologram on its anchor once the body code places it.
fn hang_worn_holos(
    mut commands: Commands,
    anchors: Query<(Entity, &HoloAnchor), Added<HoloAnchor>>,
    assets: Option<Res<HoloAssets>>,
) {
    let Some(assets) = assets else {
        return;
    };
    for (entity, anchor) in &anchors {
        let Some(i) = HOLOS.iter().position(|h| h.name == anchor.0) else {
            warn!("no hologram named {:?}", anchor.0);
            continue;
        };
        let (mesh, material) = assets.0[i].clone();
        commands
            .entity(entity)
            .with_child((quad(HOLOS[i].name, mesh, material), Transform::IDENTITY));
    }
}

fn on_open(open: On<OpenHolo>, mut requested: ResMut<Requested>) {
    requested.0.retain(|name| *name != open.surface);
    if open.open {
        requested.0.push(open.surface.clone());
    }
}

/// Ease each hologram's presence toward its open state; show it while any
/// of it is there and hand the material the value.
fn unfold(
    time: Res<Time>,
    requested: Res<Requested>,
    mut quads: Query<(
        &mut HoloQuad,
        &mut Visibility,
        &MeshMaterial3d<HologramMaterial>,
    )>,
    mut materials: ResMut<Assets<HologramMaterial>>,
) {
    let dt = time.delta_secs();
    for (mut quad, mut visibility, material) in &mut quads {
        let open = requested.0.iter().any(|name| name == quad.name);
        let target = if open { 1.0 } else { 0.0 };
        if quad.presence == target {
            continue;
        }
        let rate = if open { OPEN_SECS } else { CLOSE_SECS };
        quad.presence =
            (quad.presence + (target - quad.presence).signum() * dt / rate).clamp(0.0, 1.0);
        visibility.set_if_neq(if quad.presence > 0.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        if let Some(mut material) = materials.get_mut(&material.0) {
            material.look.x = quad.presence;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every hologram's name and logical size must match React's table:
    /// the `<surface target>` names and the px each one lays out in.
    #[test]
    fn holos_match_the_react_table() {
        let ts = include_str!("../ui/src/holos/surfaces.ts");
        for holo in &HOLOS {
            let row = format!(
                "{}: {{ width: {}, height: {} }}",
                holo.name, holo.px.x, holo.px.y
            );
            assert!(ts.contains(&row), "surfaces.ts lacks `{row}`");
        }
        assert_eq!(ts.matches(": { width:").count(), HOLOS.len());
    }
}
