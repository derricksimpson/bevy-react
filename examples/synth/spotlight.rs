//! `spotlight` — the cards' cursor light (the demos app's, ported): an
//! app-registered style property (`style={{ spotlight: { … } }}`) drawn by a
//! bevy [`UiMaterial`] (`assets/shaders/spotlight.wesl`). Near the pointer a
//! card's border catches the light and its surface takes a soft wash; away
//! from it the card shows nothing. The light follows the pointer every frame
//! without a React render or event: the pointer the wires follow
//! ([`WirePointer`], the mouse or `--shoot`'s scripted one) is read here and
//! handed to the shader as a uniform.
//!
//! The property has no writer: the core auto-stamps it as a
//! [`StyleValue<Spotlight>`], and [`sync_spotlights`] mirrors that, plus the
//! pointer in the node's own space, into one material per node. The material
//! paints over the node's background and under its children, so a card puts
//! it on a bare absolutely-positioned first child (`NodeCard.tsx`). A node
//! the light can't reach writes one canonical "dark" uniform, so pointer
//! motion elsewhere costs it nothing.

use bevy::picking::pointer::{PointerId, PointerLocation};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiSystems};
use bevy_react::ReactAppExt;
use bevy_react::layer::{LayerContentDirt, resolve_layer_repaints};
use bevy_react::raster::parse_css_color;
use bevy_react::style::{Invalidate, Invalidation, StyleProperty, StyleValue};
use serde::Deserialize;
use ts_rs::TS;

use crate::wires::{MainCamera, WirePointer};

/// The default light.
const WHITE: Srgba = Srgba::WHITE;
/// How far the light reaches by default, logical px.
const DEFAULT_REACH: f32 = 260.0;
/// Where an unlit node's pointer sits: far outside any reach.
const DARK: Vec2 = Vec2::splat(-1.0e6);

/// The `spotlight` style value. Lengths are logical px.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Spotlight {
    /// Light color, any CSS color (default white).
    #[serde(default)]
    #[ts(optional)]
    pub color: Option<String>,
    /// How far from the pointer the light reaches (default `260`).
    #[serde(default)]
    #[ts(optional)]
    pub reach: Option<f32>,
    /// Width of the lit edge ring along the node's rounded border
    /// (default `0` = no edge).
    #[serde(default)]
    #[ts(optional)]
    pub edge: Option<f32>,
    /// Strength of the soft light pool on the node's surface, `0..1`
    /// (default `0`).
    #[serde(default)]
    #[ts(optional)]
    pub wash: Option<f32>,
}

/// The property declaration — the static is also its typed key. A change
/// repaints: the core pushes layer dirt for it.
pub static SPOTLIGHT: StyleProperty<Spotlight> = StyleProperty {
    invalidate: Invalidate::Fixed(Invalidation::PAINT),
    ..StyleProperty::new("spotlight")
};

/// Mirrors `struct Spotlight` in `spotlight.wesl`.
#[derive(Debug, Clone, Copy, PartialEq, ShaderType)]
struct SpotlightUniform {
    /// Linear RGBA.
    color: Vec4,
    /// xy: the pointer in the node's space (physical px from its top-left
    /// corner); z: physical px per logical px.
    pointer: Vec4,
    /// reach, edge (logical px), wash, unused.
    shape: Vec4,
}

impl SpotlightUniform {
    fn new(spotlight: &Spotlight, pointer: Vec2, scale: f32) -> Self {
        let srgb = spotlight
            .color
            .as_deref()
            .and_then(parse_css_color)
            .unwrap_or(WHITE);
        let linear = LinearRgba::from(srgb);
        Self {
            color: Vec4::new(linear.red, linear.green, linear.blue, linear.alpha),
            pointer: pointer.extend(scale).extend(0.0),
            shape: Vec4::new(
                reach(spotlight),
                spotlight.edge.unwrap_or(0.0),
                spotlight.wash.unwrap_or(0.0),
                0.0,
            ),
        }
    }
}

fn reach(spotlight: &Spotlight) -> f32 {
    spotlight.reach.unwrap_or(DEFAULT_REACH).max(1.0)
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct SpotlightMaterial {
    #[uniform(0)]
    light: SpotlightUniform,
}

impl UiMaterial for SpotlightMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/spotlight.wesl".into()
    }
}

/// Adds the `spotlight` style property and its material.
pub struct SpotlightPlugin;

impl Plugin for SpotlightPlugin {
    fn build(&self, app: &mut App) {
        register_bindings(app);
        app.add_plugins(UiMaterialPlugin::<SpotlightMaterial>::default())
            .add_systems(
                PostUpdate,
                // This frame's rects (a drag or the zoom glide moves a card
                // under a resting pointer), and before the frame's dirt
                // becomes repaints.
                sync_spotlights
                    .after(UiSystems::PostLayout)
                    .before(resolve_layer_repaints),
            );
    }
}

/// The property alone — what the TypeScript exporter needs to type
/// `BevyStyle.spotlight` (the generated `bevy.ts` augmentation).
pub fn register_bindings(app: &mut App) {
    app.add_react_style(&SPOTLIGHT);
}

/// The pointer in a node's own space — physical px from its top-left corner,
/// through its global transform — or [`DARK`] when the light can't reach the
/// node at all.
fn pointer_in_node(
    cursor: Option<Vec2>,
    global: &UiGlobalTransform,
    size: Vec2,
    reach_px: f32,
) -> Vec2 {
    let Some(local) = cursor.and_then(|c| Some(global.try_inverse()?.transform_point2(c))) else {
        return DARK;
    };
    // Distance from the (centred) box: zero inside, growing outside.
    let outside = (local.abs() - size * 0.5).max(Vec2::ZERO).length();
    if outside > reach_px {
        DARK
    } else {
        local + size * 0.5
    }
}

/// Mirror each node's stamped `spotlight` and the pointer into its material.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn sync_spotlights(
    mut commands: Commands,
    mut materials: ResMut<Assets<SpotlightMaterial>>,
    pointers: Query<(&PointerId, &PointerLocation)>,
    which: Res<WirePointer>,
    camera: Single<&Camera, With<MainCamera>>,
    nodes: Query<(
        Entity,
        &StyleValue<Spotlight>,
        &ComputedNode,
        &UiGlobalTransform,
        Option<&MaterialNode<SpotlightMaterial>>,
    )>,
    mut unset: RemovedComponents<StyleValue<Spotlight>>,
    drawn: Query<(), With<MaterialNode<SpotlightMaterial>>>,
    mut dirt: ResMut<LayerContentDirt>,
) {
    // Physical px of the UI's target, as the nodes' transforms are.
    let target_scale = camera.target_scaling_factor().unwrap_or(1.0);
    let cursor = pointers
        .iter()
        .find(|(id, _)| **id == which.0)
        .and_then(|(_, at)| at.location.as_ref())
        .map(|at| at.position * target_scale);
    for (entity, spotlight, computed, global, material) in &nodes {
        let scale = computed.inverse_scale_factor().recip();
        let pointer = pointer_in_node(cursor, global, computed.size, reach(&spotlight.0) * scale);
        let light = SpotlightUniform::new(&spotlight.0, pointer, scale);
        match material {
            // Compare before write: `get_mut` alone marks the asset changed.
            Some(MaterialNode(handle)) => {
                if materials.get(handle).is_some_and(|m| m.light != light)
                    && let Some(mut m) = materials.get_mut(handle)
                {
                    m.light = light;
                    dirt.nodes.push(entity);
                }
            }
            None => {
                let handle = materials.add(SpotlightMaterial { light });
                commands.entity(entity).insert(MaterialNode(handle));
            }
        }
    }
    // `RemovedComponents` also yields despawned entities; `drawn` keeps
    // `commands.entity` off them.
    for entity in unset.read() {
        if drawn.contains(entity) {
            commands
                .entity(entity)
                .remove::<MaterialNode<SpotlightMaterial>>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wire value lands in the uniform slots `spotlight.wesl` reads.
    #[test]
    fn wire_value_packs_the_shader_uniform() {
        let spotlight: Spotlight = serde_json::from_value(serde_json::json!({
            "color": "#ffffff",
            "edge": 1,
            "wash": 0.5,
        }))
        .unwrap();
        let light = SpotlightUniform::new(&spotlight, Vec2::new(3.0, 4.0), 2.0);
        assert_eq!(light.color, Vec4::ONE);
        assert_eq!(light.pointer, Vec4::new(3.0, 4.0, 2.0, 0.0));
        assert_eq!(light.shape, Vec4::new(DEFAULT_REACH, 1.0, 0.5, 0.0));
    }

    /// Inside the reach the pointer is measured from the node's top-left;
    /// beyond it every position collapses to one dark value, so pointer
    /// motion far away never rewrites the uniform.
    #[test]
    fn pointer_is_node_local_and_dark_out_of_reach() {
        let global = UiGlobalTransform::from_xy(100.0, 50.0);
        let size = Vec2::new(40.0, 20.0);
        let at = |c| pointer_in_node(Some(c), &global, size, 10.0);
        assert_eq!(at(Vec2::new(100.0, 50.0)), Vec2::new(20.0, 10.0));
        assert_eq!(at(Vec2::new(125.0, 50.0)), Vec2::new(45.0, 10.0));
        assert_eq!(at(Vec2::new(200.0, 50.0)), DARK);
        assert_eq!(at(Vec2::new(300.0, 90.0)), DARK);
        assert_eq!(pointer_in_node(None, &global, size, 10.0), DARK);
    }
}
