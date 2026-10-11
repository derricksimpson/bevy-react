//! The locator: a ribbon of light traced along the floor from the
//! engineer's feet to a destination (`layout::destination`), the way the
//! suit's navigation would draw it — out along the deck in straight runs
//! with rounded corners, a bright head racing ahead, chevrons flowing
//! toward the goal, then a hold and a fade. `locator_ribbon.wgsl` draws it.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy_react::{ReactAppExt, react_message};

use crate::layout::{ENGINEER, destination};

/// React → Bevy: trace the line to `to` (`store`, `bench`, `save`,
/// `elevator`, `bulkhead`).
#[react_message(name = "locator.show")]
pub struct ShowLocator {
    pub to: String,
}

/// The ribbon's look. `trace` = (head m, length m, strength 0..1, -).
#[derive(Asset, AsBindGroup, TypePath, Clone)]
pub struct RibbonMaterial {
    #[uniform(0)]
    pub trace: Vec4,
}

impl Material for RibbonMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/locator_ribbon.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}

/// The ribbon's run: which path, how far along.
#[derive(Resource)]
struct Trace {
    entity: Entity,
    material: Handle<RibbonMaterial>,
    paths: HashMap<String, (Handle<Mesh>, f32)>,
    age: Option<f32>,
    length: f32,
}

const TRACE_SECS: f32 = 0.8;
const HOLD_SECS: f32 = 2.2;
const FADE_SECS: f32 = 0.8;
const WIDTH: f32 = 0.14;
const CORNER: f32 = 0.6;

pub struct LocatorPlugin;

impl Plugin for LocatorPlugin {
    fn build(&self, app: &mut App) {
        register_bindings(app);
        app.add_plugins(MaterialPlugin::<RibbonMaterial>::default())
            .add_systems(Startup, spawn)
            .add_systems(Update, run);
    }
}

pub fn register_bindings(app: &mut App) {
    app.add_react_handler(on_show);
}

fn spawn(mut commands: Commands, mut materials: ResMut<Assets<RibbonMaterial>>) {
    let material = materials.add(RibbonMaterial { trace: Vec4::ZERO });
    let entity = commands
        .spawn((
            Mesh3d::default(),
            MeshMaterial3d(material.clone()),
            Transform::default(),
            Visibility::Hidden,
            bevy::light::NotShadowCaster,
            // The mesh swaps per destination; its first bounds would stick.
            bevy::camera::visibility::NoFrustumCulling,
        ))
        .id();
    commands.insert_resource(Trace {
        entity,
        material,
        paths: HashMap::new(),
        age: None,
        length: 0.0,
    });
}

fn on_show(
    show: On<ShowLocator>,
    mut trace: ResMut<Trace>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut ribbons: Query<(&mut Mesh3d, &mut Visibility)>,
) {
    let Some(to) = destination(&show.to) else {
        return;
    };
    let (mesh, length) = trace
        .paths
        .entry(show.to.clone())
        .or_insert_with(|| {
            let points = route(ENGINEER + Vec3::new(0.0, 0.0, -0.3), to);
            let (mesh, length) = ribbon(&points);
            (meshes.add(mesh), length)
        })
        .clone();
    trace.length = length;
    trace.age = Some(0.0);
    if let Ok((mut m, mut v)) = ribbons.get_mut(trace.entity) {
        m.0 = mesh;
        *v = Visibility::Inherited;
    }
}

fn run(
    time: Res<Time>,
    mut trace: ResMut<Trace>,
    mut materials: ResMut<Assets<RibbonMaterial>>,
    mut ribbons: Query<&mut Visibility>,
) {
    let Some(age) = trace.age else {
        return;
    };
    let age = age + time.delta_secs();
    let k = (age / TRACE_SECS).min(1.0);
    let head = trace.length * (1.0 - (1.0 - k) * (1.0 - k)) + 0.3 * (age / TRACE_SECS).min(1.0);
    let strength = 1.0 - ((age - TRACE_SECS - HOLD_SECS) / FADE_SECS).clamp(0.0, 1.0);
    // Only while something moves: the hold writes nothing.
    let next = Vec4::new(head, trace.length, strength, 0.0);
    if materials
        .get(&trace.material)
        .is_some_and(|m| m.trace != next)
        && let Some(mut m) = materials.get_mut(&trace.material)
    {
        m.trace = next;
    }
    if strength <= 0.0 {
        trace.age = None;
        if let Ok(mut v) = ribbons.get_mut(trace.entity) {
            *v = Visibility::Hidden;
        }
    } else {
        trace.age = Some(age);
    }
}

/// The line from `from` to `to` on the floor: forward, across, in — runs
/// along the deck's axes so it reads as a route, not a beeline.
fn route(from: Vec3, to: Vec3) -> Vec<Vec2> {
    let (a, b) = (from.xz(), to.xz());
    // Side-wall goals turn at their own depth; far-wall goals cross the
    // room a meter out from the wall's fixtures first.
    let lane = if b.x.abs() > 6.0 {
        b.y
    } else {
        (-1.0f32).max(b.y)
    };
    let mut points = vec![a, Vec2::new(a.x, lane), Vec2::new(b.x, lane), b];
    points.dedup_by(|p, q| p.distance(*q) < 0.05);
    round(&points)
}

/// Round each corner with a quadratic curve of radius `CORNER`, sampled
/// every ~8 cm along straight runs too.
fn round(points: &[Vec2]) -> Vec<Vec2> {
    let mut out = vec![points[0]];
    for i in 1..points.len() {
        let (prev, here) = (points[i - 1], points[i]);
        let next = points.get(i + 1).copied();
        let into = (here - prev).length();
        let cut = next.map_or(0.0, |n| {
            CORNER.min(into * 0.45).min((n - here).length() * 0.45)
        });
        let end = here - (here - prev).normalize_or_zero() * cut;
        let start = *out.last().unwrap();
        let steps = ((end - start).length() / 0.08).ceil().max(1.0) as usize;
        out.extend((1..=steps).map(|s| start.lerp(end, s as f32 / steps as f32)));
        if let Some(n) = next {
            let exit = here + (n - here).normalize_or_zero() * cut;
            out.extend((1..=8).map(|s| {
                let t = s as f32 / 8.0;
                end.lerp(here, t).lerp(here.lerp(exit, t), t)
            }));
        }
    }
    out
}

/// A flat strip along `points`, `WIDTH` wide, just above the floor; UV x is
/// meters along it, y runs across. Returns the mesh and its length.
fn ribbon(points: &[Vec2]) -> (Mesh, f32) {
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    let mut along = 0.0;
    for (i, p) in points.iter().enumerate() {
        if i > 0 {
            along += p.distance(points[i - 1]);
        }
        let a = points[i.saturating_sub(1)];
        let b = points[(i + 1).min(points.len() - 1)];
        let t = (b - a).normalize_or(Vec2::Y);
        let side = Vec2::new(-t.y, t.x) * WIDTH * 0.5;
        for (s, v) in [(1.0, 0.0), (-1.0, 1.0)] {
            let q = *p + side * s;
            positions.push([q.x, 0.014, q.y]);
            uvs.push([along, v]);
        }
        if i > 0 {
            let k = (i as u32 - 1) * 2;
            // Counter-clockwise seen from above: the strip faces up.
            indices.extend_from_slice(&[k, k + 2, k + 1, k + 1, k + 2, k + 3]);
        }
    }
    let normals = vec![[0.0f32, 1.0, 0.0]; positions.len()];
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices));
    (mesh, along)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every destination's route starts at the feet, ends on the goal and
    /// never jumps (samples stay close together).
    #[test]
    fn routes_are_continuous() {
        for to in ["store", "bench", "save", "elevator", "bulkhead"] {
            let goal = destination(to).unwrap();
            let from = ENGINEER + Vec3::new(0.0, 0.0, -0.3);
            let points = route(from, goal);
            assert!(points[0].distance(from.xz()) < 1e-4, "{to}");
            assert!(points.last().unwrap().distance(goal.xz()) < 1e-3, "{to}");
            assert!(points.windows(2).all(|w| w[0].distance(w[1]) < 0.2), "{to}");
        }
    }
}
