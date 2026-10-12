//! The carryable items as little procedural models, for the turntable and
//! the inventory icons. Every id in `ui/src/game/items.ts` has one (a test
//! checks); the weapons come from `weapons/models.rs`.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

/// Every item id, in `items.ts` order, and the radius its model fits in (m)
/// — the cameras frame by it.
pub const ITEMS: [(&str, f32); 13] = [
    ("medpack-s", 0.16),
    ("medpack-m", 0.2),
    ("medpack-l", 0.25),
    ("stasis-pack", 0.18),
    ("air-canister", 0.22),
    ("cutter-ammo", 0.15),
    ("pulse-ammo", 0.16),
    ("rail-ammo", 0.2),
    ("power-cell", 0.18),
    ("semiconductor", 0.15),
    // The weapons are long and thin: framed by half their length, they fill
    // the frame as they turn.
    ("cutter", 0.24),
    ("pulse", 0.27),
    ("rail", 0.28),
];

/// The shared meshes and materials the item models are built from.
#[derive(Resource)]
pub struct ItemKit {
    cube: Handle<Mesh>,
    cylinder: Handle<Mesh>,
    hex: Handle<Mesh>,
    steel: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
    white: Handle<StandardMaterial>,
    red: Handle<StandardMaterial>,
    blue: Handle<StandardMaterial>,
    cyan: Handle<StandardMaterial>,
    amber: Handle<StandardMaterial>,
    green: Handle<StandardMaterial>,
}

impl ItemKit {
    pub fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        let paint = |materials: &mut Assets<StandardMaterial>, c: Color, metal: f32, rough: f32| {
            materials.add(StandardMaterial {
                base_color: c,
                metallic: metal,
                perceptual_roughness: rough,
                ..default()
            })
        };
        let glow = |materials: &mut Assets<StandardMaterial>, c: LinearRgba| {
            materials.add(StandardMaterial {
                base_color: Color::BLACK,
                emissive: c,
                ..default()
            })
        };
        Self {
            cube: meshes.add(Cuboid::from_length(1.0)),
            cylinder: meshes.add(Cylinder::new(0.5, 1.0).mesh().resolution(24)),
            hex: meshes.add(Cylinder::new(0.5, 1.0).mesh().resolution(6)),
            steel: paint(materials, Color::srgb(0.42, 0.44, 0.46), 0.85, 0.38),
            dark: paint(materials, Color::srgb(0.08, 0.09, 0.1), 0.6, 0.55),
            white: paint(materials, Color::srgb(0.78, 0.8, 0.8), 0.1, 0.5),
            red: glow(materials, LinearRgba::rgb(5.0, 0.35, 0.25)),
            blue: glow(materials, LinearRgba::rgb(0.6, 1.6, 6.0)),
            cyan: glow(materials, LinearRgba::rgb(0.8, 3.6, 4.6)),
            amber: glow(materials, LinearRgba::rgb(4.8, 2.2, 0.4)),
            green: glow(materials, LinearRgba::rgb(0.6, 4.2, 1.2)),
        }
    }
}

/// One box/cylinder part: mesh, material, center, size.
type Part = (Handle<Mesh>, Handle<StandardMaterial>, Vec3, Vec3);

/// Spawn item `id`'s model at `at`, every part on `layers`. Weapons are
/// built by `weapons/models.rs`. Returns the root (rotate it to turn the model).
pub fn spawn_item(
    commands: &mut Commands,
    kit: &ItemKit,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    id: &str,
    at: Vec3,
    layers: &RenderLayers,
) -> Entity {
    if matches!(id, "cutter" | "pulse" | "rail") {
        let root =
            crate::weapons::spawn_weapon_model(commands, meshes, materials, id, layers.clone());
        commands
            .entity(root)
            .insert(Transform::from_translation(at));
        return root;
    }
    let parts = parts(kit, id);
    let root = commands
        .spawn((
            Transform::from_translation(at),
            Visibility::default(),
            layers.clone(),
        ))
        .id();
    for (mesh, material, center, size) in parts {
        let part = commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_translation(center).with_scale(size),
                layers.clone(),
            ))
            .id();
        commands.entity(root).add_child(part);
    }
    root
}

fn parts(k: &ItemKit, id: &str) -> Vec<Part> {
    let cube = || k.cube.clone();
    let cyl = || k.cylinder.clone();
    match id {
        // A white case, a dark lid, a red cross glowing on top; three sizes.
        "medpack-s" | "medpack-m" | "medpack-l" => {
            let s = match id {
                "medpack-s" => 0.62,
                "medpack-m" => 0.8,
                _ => 1.0,
            };
            let w = Vec3::new(0.34, 0.14, 0.24) * s;
            vec![
                (cube(), k.white.clone(), Vec3::ZERO, w),
                (
                    cube(),
                    k.dark.clone(),
                    Vec3::Y * w.y * 0.55,
                    w * Vec3::new(0.92, 0.12, 0.9),
                ),
                (
                    cube(),
                    k.red.clone(),
                    Vec3::Y * w.y * 0.63,
                    Vec3::new(w.x * 0.42, 0.01, w.z * 0.14),
                ),
                (
                    cube(),
                    k.red.clone(),
                    Vec3::Y * w.y * 0.63,
                    Vec3::new(w.x * 0.12, 0.01, w.z * 0.5),
                ),
                (
                    cube(),
                    k.steel.clone(),
                    Vec3::new(0.0, 0.0, w.z * 0.52),
                    Vec3::new(w.x * 0.3, w.y * 0.3, 0.02),
                ),
            ]
        }
        // A canister with a blue core showing through three windows.
        "stasis-pack" => vec![
            (
                cyl(),
                k.steel.clone(),
                Vec3::ZERO,
                Vec3::new(0.14, 0.26, 0.14),
            ),
            (
                cyl(),
                k.blue.clone(),
                Vec3::ZERO,
                Vec3::new(0.145, 0.16, 0.145),
            ),
            (
                cyl(),
                k.dark.clone(),
                Vec3::Y * 0.15,
                Vec3::new(0.16, 0.05, 0.16),
            ),
            (
                cyl(),
                k.dark.clone(),
                Vec3::Y * -0.15,
                Vec3::new(0.16, 0.05, 0.16),
            ),
        ],
        // A tall tank, a green band, a valve on top.
        "air-canister" => vec![
            (
                cyl(),
                k.steel.clone(),
                Vec3::ZERO,
                Vec3::new(0.15, 0.36, 0.15),
            ),
            (
                cyl(),
                k.green.clone(),
                Vec3::Y * 0.06,
                Vec3::new(0.152, 0.03, 0.152),
            ),
            (
                cyl(),
                k.dark.clone(),
                Vec3::Y * 0.21,
                Vec3::new(0.07, 0.07, 0.07),
            ),
            (
                cube(),
                k.dark.clone(),
                Vec3::Y * 0.26,
                Vec3::new(0.12, 0.02, 0.03),
            ),
        ],
        // Ammo crates: a dark box with its weapon's color in stripes.
        "cutter-ammo" | "pulse-ammo" | "rail-ammo" => {
            let (glow, size) = match id {
                "cutter-ammo" => (k.cyan.clone(), Vec3::new(0.2, 0.12, 0.14)),
                "pulse-ammo" => (k.amber.clone(), Vec3::new(0.24, 0.13, 0.16)),
                _ => (k.red.clone(), Vec3::new(0.34, 0.1, 0.12)),
            };
            vec![
                (cube(), k.dark.clone(), Vec3::ZERO, size),
                (
                    cube(),
                    k.steel.clone(),
                    Vec3::Y * size.y * 0.5,
                    size * Vec3::new(1.02, 0.1, 1.02),
                ),
                (
                    cube(),
                    glow.clone(),
                    Vec3::new(0.0, 0.0, size.z * 0.505),
                    Vec3::new(size.x * 0.8, size.y * 0.12, 0.005),
                ),
                (
                    cube(),
                    glow,
                    Vec3::new(0.0, -size.y * 0.25, size.z * 0.505),
                    Vec3::new(size.x * 0.8, size.y * 0.06, 0.005),
                ),
            ]
        }
        // A hexagonal cell, cyan light at both ends.
        "power-cell" => vec![
            (
                k.hex.clone(),
                k.dark.clone(),
                Vec3::ZERO,
                Vec3::new(0.16, 0.24, 0.16),
            ),
            (
                k.hex.clone(),
                k.cyan.clone(),
                Vec3::Y * 0.13,
                Vec3::new(0.12, 0.03, 0.12),
            ),
            (
                k.hex.clone(),
                k.cyan.clone(),
                Vec3::Y * -0.13,
                Vec3::new(0.12, 0.03, 0.12),
            ),
            (
                k.hex.clone(),
                k.steel.clone(),
                Vec3::ZERO,
                Vec3::new(0.17, 0.04, 0.17),
            ),
        ],
        // A flat chip with pins and glowing traces.
        "semiconductor" => {
            let mut parts = vec![
                (
                    cube(),
                    k.dark.clone(),
                    Vec3::ZERO,
                    Vec3::new(0.18, 0.025, 0.18),
                ),
                (
                    cube(),
                    k.amber.clone(),
                    Vec3::Y * 0.014,
                    Vec3::new(0.08, 0.004, 0.08),
                ),
            ];
            for i in 0..6 {
                let x = -0.075 + i as f32 * 0.03;
                parts.push((
                    cube(),
                    k.steel.clone(),
                    Vec3::new(x, 0.0, 0.1),
                    Vec3::new(0.012, 0.006, 0.03),
                ));
                parts.push((
                    cube(),
                    k.steel.clone(),
                    Vec3::new(x, 0.0, -0.1),
                    Vec3::new(0.012, 0.006, 0.03),
                ));
            }
            parts
        }
        _ => {
            warn!("no item model for {id:?}");
            vec![(cube(), k.steel.clone(), Vec3::ZERO, Vec3::splat(0.15))]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ITEMS;

    /// React names the icon targets and asks the turntable by the ids in
    /// `items.ts`: every one needs a model, in the same order.
    #[test]
    fn every_item_has_a_model() {
        let ts = include_str!("../ui/src/game/items.ts");
        let catalog = ts
            .split("export const ITEMS = {")
            .nth(1)
            .and_then(|rest| rest.split("} as const;").next())
            .expect("ITEMS in items.ts");
        let ids: Vec<&str> = catalog
            .lines()
            .filter(|l| {
                l.starts_with("  ") && !l.starts_with("   ") && l.trim_end().ends_with(": {")
            })
            .map(|l| l.trim().trim_end_matches(": {").trim_matches('"'))
            .collect();
        let models: Vec<&str> = ITEMS.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids, models);
    }
}
