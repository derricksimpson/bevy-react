//! The holograms. Every readout in the game is a React `<surface>` (see
//! `ui/src/holos/`) on a quad in the world, drawn by one material
//! (`hologram.wesl`): premultiplied UI, scanlines, flicker, an edge glow,
//! pushed past 1.0 so it blooms. React owns what is on them and which are
//! open (`holo.open`); this module owns where they hang, how they unfold,
//! and which are live.
//!
//! The fixtures' panels (those with a `reach`) stay projected but dead —
//! grey and dim — until he comes within reach; then they take their color
//! and React hears `holo.active`. Nearer still, facing one, F locks the
//! camera on it (`player/lock.rs`) and the material draws the cursor's
//! reticle where it points.
//!
//! Most hang at a fixed spot in the room. The ones worn by the engineer or
//! his gun (`spine`, `stasis`, `air`, `ammo`, `prompt`, and the `menu` he
//! projects ahead of him) hang on a [`HoloAnchor`] the body code places,
//! so they go where he goes.

mod material;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_react::surface::{SurfacePointer, SurfaceSpec, Surfaces};
use bevy_react::{ReactAppExt, ReactEvents, react_event, react_message};

pub use material::HologramMaterial;

use crate::layout::{self, facing};
use crate::player::{Player, PlayerSet, Sync};

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
    /// A fixture's panel: live only while he stands within this many meters
    /// of it (0 = always live).
    pub reach: f32,
}

impl Holo {
    const fn new(name: &'static str, px: Vec2, width_m: f32, density: f32, place: Place) -> Self {
        Self {
            name,
            px,
            width_m,
            density,
            place,
            reach: 0.0,
        }
    }

    /// A fixture's panel (see [`Holo::reach`]).
    const fn panel(mut self, reach: f32) -> Self {
        self.reach = reach;
        self
    }
}

/// A panel that woke stays live this much farther out (no flutter at the
/// edge of its reach).
const LINGER: f32 = 0.4;

pub enum Place {
    /// A fixed spot: center and facing direction.
    World(Vec3, Vec3),
    /// On the [`HoloAnchor`] of the same name.
    Anchor,
}

pub const HOLOS: [Holo; 14] = [
    Holo::new("spine", Vec2::new(48.0, 360.0), 0.075, 1.5, Place::Anchor),
    Holo::new("stasis", Vec2::new(160.0, 160.0), 0.2, 1.5, Place::Anchor),
    Holo::new("air", Vec2::new(220.0, 110.0), 0.36, 1.5, Place::Anchor),
    Holo::new("ammo", Vec2::new(220.0, 130.0), 0.2, 1.5, Place::Anchor),
    Holo::new("prompt", Vec2::new(460.0, 48.0), 0.72, 1.5, Place::Anchor),
    // Projected ahead of him and to his right; the camera locks on it.
    Holo::new("menu", Vec2::new(1280.0, 720.0), 2.4, 1.0, Place::Anchor),
    Holo::new(
        "store",
        Vec2::new(1280.0, 760.0),
        2.4,
        1.0,
        Place::World(Vec3::new(-8.95, 1.65, -0.5), Vec3::X),
    )
    .panel(3.6),
    Holo::new(
        "bench",
        Vec2::new(1280.0, 760.0),
        2.4,
        1.0,
        Place::World(Vec3::new(8.95, 1.65, -0.5), Vec3::NEG_X),
    )
    .panel(3.6),
    Holo::new(
        "lock",
        Vec2::new(200.0, 200.0),
        0.7,
        1.0,
        Place::World(Vec3::new(0.0, 1.65, -4.8), Vec3::Z),
    ),
    Holo::new(
        "save",
        Vec2::new(480.0, 320.0),
        1.1,
        1.0,
        Place::World(Vec3::new(layout::SAVE_STATION.x, 1.55, -4.55), Vec3::Z),
    )
    .panel(2.8),
    Holo::new(
        "elevator",
        Vec2::new(240.0, 360.0),
        0.5,
        1.0,
        Place::World(Vec3::new(2.1, 1.45, -4.78), Vec3::Z),
    )
    .panel(2.8),
    Holo::new(
        "floor",
        Vec2::new(240.0, 80.0),
        0.9,
        1.0,
        Place::World(Vec3::new(3.7, 3.15, -4.86), Vec3::Z),
    ),
    Holo::new(
        "hack",
        Vec2::new(420.0, 420.0),
        0.8,
        1.5,
        Place::World(Vec3::new(layout::HACK_PANEL.x, 1.45, -4.45), Vec3::Z),
    )
    .panel(2.8),
    // The terminal in the alcove: the controls, where he starts.
    Holo::new(
        "controls",
        Vec2::new(960.0, 540.0),
        1.6,
        1.0,
        Place::World(Vec3::new(-9.72, 1.6, 3.6), Vec3::X),
    )
    .panel(3.6),
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

/// Bevy → React: he came within reach of a fixture's panel, or left it.
#[react_event(name = "holo.active")]
pub struct HoloActive {
    pub surface: String,
    pub active: bool,
}

/// A hologram's quad. `presence` eases toward whether React wants it open
/// ([`Requested`]); at 0 the quad hides (and its surface stops rendering).
/// `power` eases toward whether it is live ([`Live`]): 0 is the dead grey.
#[derive(Component)]
pub struct HoloQuad {
    holo: &'static Holo,
    presence: f32,
    power: f32,
}

/// The holograms React asked to be open. Kept apart from the quads: a worn
/// hologram's quad only exists once its anchor does, after React's first
/// `holo.open`s have landed.
#[derive(Resource, Default)]
pub struct Requested(Vec<String>);

impl Requested {
    pub fn is_open(&self, name: &str) -> bool {
        self.0.iter().any(|open| open == name)
    }
}

/// The panels he stands within reach of, and the ones React was told of.
#[derive(Resource, Default)]
struct Live {
    on: Vec<&'static str>,
    told: Vec<&'static str>,
}

/// Unfolding and folding take this long; so does waking.
const OPEN_SECS: f32 = 0.55;
const CLOSE_SECS: f32 = 0.3;
const WAKE_SECS: f32 = 0.45;

pub struct HoloPlugin;

impl Plugin for HoloPlugin {
    fn build(&self, app: &mut App) {
        register_bindings(app);
        app.add_plugins(MaterialPlugin::<HologramMaterial>::default())
            .init_resource::<Requested>()
            .init_resource::<Live>()
            .add_systems(Startup, create_surfaces)
            .add_systems(
                Update,
                (hang_worn_holos, wake, unfold, point)
                    .chain()
                    .after(PlayerSet),
            );
    }
}

pub fn register_bindings(app: &mut App) {
    app.add_react_handler(on_open)
        .add_react_handler(on_sync)
        .add_react_event::<HoloActive>();
}

/// React starts over: it will say again what is open (a menu left open by
/// the tree it replaced would hold him still for good), and wants to hear
/// again what is live.
fn on_sync(_: On<Sync>, mut requested: ResMut<Requested>, mut live: ResMut<Live>) {
    requested.0.clear();
    live.told.clear();
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
                quad(holo, mesh.clone(), material.clone()),
                Transform::from_translation(at).with_rotation(facing(dir)),
            ));
        }
        assets.push((mesh, material));
    }
    commands.insert_resource(HoloAssets(assets));
}

fn quad(
    holo: &'static Holo,
    mesh: Handle<Mesh>,
    material: Handle<HologramMaterial>,
) -> impl Bundle {
    (
        HoloQuad {
            holo,
            presence: 0.0,
            power: 0.0,
        },
        Mesh3d(mesh),
        MeshMaterial3d(material),
        SurfacePointer::new(holo.name),
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
            .with_child((quad(&HOLOS[i], mesh, material), Transform::IDENTITY));
    }
}

fn on_open(open: On<OpenHolo>, mut requested: ResMut<Requested>) {
    requested.0.retain(|name| *name != open.surface);
    if open.open {
        requested.0.push(open.surface.clone());
    }
}

/// Which panels he stands within reach of; React hears each change.
fn wake(player: Res<Player>, mut live: ResMut<Live>, events: ReactEvents) {
    let on: Vec<&'static str> = HOLOS
        .iter()
        .filter(|holo| {
            let Place::World(at, _) = holo.place else {
                return false;
            };
            let linger = if live.on.contains(&holo.name) {
                LINGER
            } else {
                0.0
            };
            holo.reach > 0.0 && at.xz().distance(player.at.xz()) < holo.reach + linger
        })
        .map(|holo| holo.name)
        .collect();
    for holo in &HOLOS {
        let (now, was) = (on.contains(&holo.name), live.told.contains(&holo.name));
        if now != was {
            events.send(&HoloActive {
                surface: holo.name.into(),
                active: now,
            });
        }
    }
    live.told.clone_from(&on);
    live.on = on;
}

/// Ease each hologram's presence toward its open state and its power
/// toward whether it is live; show it while any of it is there and hand
/// the material both.
fn unfold(
    time: Res<Time>,
    requested: Res<Requested>,
    live: Res<Live>,
    mut quads: Query<(
        &mut HoloQuad,
        &mut Visibility,
        &MeshMaterial3d<HologramMaterial>,
    )>,
    mut materials: ResMut<Assets<HologramMaterial>>,
) {
    let dt = time.delta_secs();
    let step = |now: f32, target: f32, secs: f32| {
        (now + (target - now).signum() * dt / secs).clamp(0.0, 1.0)
    };
    for (mut quad, mut visibility, material) in &mut quads {
        let holo = quad.holo;
        let open = requested.is_open(holo.name);
        let presence = if open { 1.0 } else { 0.0 };
        let power = if holo.reach == 0.0 || live.on.contains(&holo.name) {
            1.0
        } else {
            0.0
        };
        if quad.presence == presence && quad.power == power {
            continue;
        }
        let rate = if open { OPEN_SECS } else { CLOSE_SECS };
        quad.presence = step(quad.presence, presence, rate);
        quad.power = step(quad.power, power, WAKE_SECS);
        visibility.set_if_neq(if quad.presence > 0.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        if let Some(mut material) = materials.get_mut(&material.0) {
            material.look.x = quad.presence;
            material.shape.w = quad.power;
        }
    }
}

/// The screen that holds the camera draws the reticle where its cursor
/// is. Straight from the cursor, not from where the picking pointer got
/// to: that is this frame's mouse.
fn point(
    player: Res<Player>,
    quads: Query<(&HoloQuad, &MeshMaterial3d<HologramMaterial>)>,
    mut materials: ResMut<Assets<HologramMaterial>>,
) {
    for (quad, material) in &quads {
        let Some(look) = materials.get(&material.0) else {
            continue;
        };
        let holo = quad.holo;
        let cursor = if player.mouse && player.held == Some(holo.name) {
            // Meters from the center → 0..1 from the top left.
            let size = Vec2::new(holo.width_m, holo.width_m * holo.px.y / holo.px.x);
            let uv = Vec2::new(0.5, 0.5) + player.cursor * Vec2::new(1.0, -1.0) / size;
            uv.extend(1.0).extend(0.0)
        } else {
            Vec4::ZERO
        };
        if look.cursor != cursor
            && let Some(mut look) = materials.get_mut(&material.0)
        {
            look.cursor = cursor;
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

    /// React greys the same panels out (`PANELS` in `ui/src/holos/open.ts`).
    #[test]
    fn panels_match_react() {
        let ts = include_str!("../ui/src/holos/open.ts");
        let list = ts.split("PANELS: SurfaceName[] = [").nth(1).unwrap();
        let react: Vec<&str> = list
            .split(']')
            .next()
            .unwrap()
            .split(',')
            .map(|name| name.trim().trim_matches('"'))
            .filter(|name| !name.is_empty())
            .collect();
        let panels: Vec<&str> = HOLOS
            .iter()
            .filter(|holo| holo.reach > 0.0)
            .map(|holo| holo.name)
            .collect();
        assert_eq!(react, panels);
    }
}
