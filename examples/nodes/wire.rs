//! Two elements the app registers itself, through the same extension API the
//! bevy-react feature crates use: `<wire>`, a link's bezier, and `<grid>`,
//! the graph's dotted backdrop. Both are drawn by a UI material on the GPU,
//! so they stay crisp under the graph's zoom transform and cost no CPU
//! raster when a dragged node drags its wires along.
//!
//! React lays each out like any node (a wire is an absolutely placed box
//! around its curve) and passes the geometry in that box's logical pixels.

use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy::ui::{ComputedNode, UiSystems};
use bevy::ui_render::prelude::{MaterialNode, UiMaterial, UiMaterialPlugin};
use bevy_react::ReactAppExt;
use bevy_react::element::{Attribute, Attrs, Common, Element, SpawnCtx};
use bevy_react::ext::ElementFlags;
use bevy_react::style::{Writer, owns};

pub struct WirePlugin;

impl Plugin for WirePlugin {
    fn build(&self, app: &mut App) {
        register_bindings(app);
        app.add_plugins((
            UiMaterialPlugin::<WireMaterial>::default(),
            UiMaterialPlugin::<GridMaterial>::default(),
        ))
        // After layout: the shaders work in the box's logical pixels, so a
        // resized box must reach its material the same frame.
        .add_systems(
            PostUpdate,
            (
                apply::<WireAttrs, WireMaterial>,
                apply::<GridAttrs, GridMaterial>,
            )
                .after(UiSystems::Layout),
        );
    }
}

pub fn register_bindings(app: &mut App) {
    app.add_react_element(&WIRE).add_react_element(&GRID);
}

// --- <wire> ---

/// The cubic bezier, in the element's logical pixels: start, first control,
/// second control, end.
pub static POINTS: Attribute<[f32; 8]> = Attribute::new("points");
/// The color at the start (any CSS color).
pub static COLOR: Attribute<String> = Attribute::new("color");
/// The color at the end (default: `color`).
pub static COLOR_TO: Attribute<String> = Attribute::new("colorTo");
/// Stroke width in logical pixels (default 2).
pub static THICKNESS: Attribute<f32> = Attribute::new("thickness");
/// Dash period in logical pixels; `0` (the default) draws a solid line.
pub static DASH: Attribute<f32> = Attribute::new("dash");

pub static WIRE: Element = Element {
    flags: ElementFlags {
        pick_ignore: true,
        ..ElementFlags::NODE
    },
    attrs: &[&POINTS, &COLOR, &COLOR_TO, &THICKNESS, &DASH],
    // Never picked: no pointer props, no children.
    common: Common::IDENTITY,
    writers: &[&WIRE_WRITER],
    spawn: Some(spawn::<WireAttrs, WireMaterial>),
    ..Element::new("wire")
};

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, Default)]
pub struct WireMaterial {
    #[uniform(0)]
    wire: WireAttrs,
}

impl UiMaterial for WireMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/wire.wesl".into()
    }
}

/// The wire's attributes as the shader reads them.
#[derive(Component, ShaderType, Clone, Copy, Debug, Default, PartialEq)]
pub struct WireAttrs {
    /// The box's logical size (xy), filled in by [`apply`].
    size: Vec4,
    /// Start (xy) and first control point (zw).
    a: Vec4,
    /// Second control point (xy) and end (zw).
    b: Vec4,
    from: LinearRgba,
    to: LinearRgba,
    /// Thickness, dash period.
    stroke: Vec4,
}

fn color(attrs: &Attrs, attr: &'static Attribute<String>) -> Option<LinearRgba> {
    let css = attrs.get(attr)?;
    let color = bevy_react::raster::parse_css_color(css).unwrap_or_else(|| {
        bevy_react::diag::report("color", css, &format!("unrecognized color {css:?}"));
        Srgba::new(1.0, 0.0, 1.0, 1.0)
    });
    Some(color.into())
}

impl From<&Attrs> for WireAttrs {
    fn from(attrs: &Attrs) -> Self {
        let p = attrs.get(&POINTS).copied().unwrap_or_default();
        let from = color(attrs, &COLOR).unwrap_or(LinearRgba::WHITE);
        Self {
            size: Vec4::ZERO,
            a: Vec4::new(p[0], p[1], p[2], p[3]),
            b: Vec4::new(p[4], p[5], p[6], p[7]),
            from,
            to: color(attrs, &COLOR_TO).unwrap_or(from),
            stroke: Vec4::new(
                attrs.get(&THICKNESS).copied().unwrap_or(2.0),
                attrs.get(&DASH).copied().unwrap_or(0.0),
                0.0,
                0.0,
            ),
        }
    }
}

pub static WIRE_WRITER: Writer = Writer {
    reads: &[],
    attrs: &[&POINTS, &COLOR, &COLOR_TO, &THICKNESS, &DASH],
    writes: &[owns::<WireAttrs>],
    apply: |ctx, _style, ec| write_attrs::<WireAttrs>(ctx.fresh, ctx.attrs, ec),
};

// --- <grid> ---

/// The pan offset in logical pixels: the dot lattice moves with it.
pub static OFFSET: Attribute<[f32; 2]> = Attribute::new("offset");
/// Distance between dots in logical pixels (the zoomed grid step).
pub static SPACING: Attribute<f32> = Attribute::new("spacing");
/// The dot color.
pub static DOT_COLOR: Attribute<String> = Attribute::new("color");

pub static GRID: Element = Element {
    flags: ElementFlags {
        pick_ignore: true,
        ..ElementFlags::NODE
    },
    attrs: &[&OFFSET, &SPACING, &DOT_COLOR],
    common: Common::IDENTITY,
    writers: &[&GRID_WRITER],
    spawn: Some(spawn::<GridAttrs, GridMaterial>),
    ..Element::new("grid")
};

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, Default)]
pub struct GridMaterial {
    #[uniform(0)]
    grid: GridAttrs,
}

impl UiMaterial for GridMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/grid.wesl".into()
    }
}

#[derive(Component, ShaderType, Clone, Copy, Debug, Default, PartialEq)]
pub struct GridAttrs {
    /// The box's logical size (xy), filled in by [`apply`].
    size: Vec4,
    /// Offset (xy), spacing (z).
    params: Vec4,
    color: LinearRgba,
}

impl From<&Attrs> for GridAttrs {
    fn from(attrs: &Attrs) -> Self {
        let offset = attrs.get(&OFFSET).copied().unwrap_or_default();
        Self {
            size: Vec4::ZERO,
            params: Vec4::new(
                offset[0],
                offset[1],
                attrs.get(&SPACING).copied().unwrap_or(20.0),
                0.0,
            ),
            color: color(attrs, &DOT_COLOR).unwrap_or(LinearRgba::gray(0.05)),
        }
    }
}

pub static GRID_WRITER: Writer = Writer {
    reads: &[],
    attrs: &[&OFFSET, &SPACING, &DOT_COLOR],
    writes: &[owns::<GridAttrs>],
    apply: |ctx, _style, ec| write_attrs::<GridAttrs>(ctx.fresh, ctx.attrs, ec),
};

// --- shared plumbing ---

/// A material whose uniform is one attributes component plus the box size.
trait AttrMaterial: UiMaterial + Default {
    type Attrs: Component + Copy + PartialEq + for<'a> From<&'a Attrs>;
    fn set(&mut self, attrs: Self::Attrs, size: Vec2);
}

impl AttrMaterial for WireMaterial {
    type Attrs = WireAttrs;
    fn set(&mut self, attrs: WireAttrs, size: Vec2) {
        self.wire = WireAttrs {
            size: size.extend(0.0).extend(0.0),
            ..attrs
        };
    }
}

impl AttrMaterial for GridMaterial {
    type Attrs = GridAttrs;
    fn set(&mut self, attrs: GridAttrs, size: Vec2) {
        self.grid = GridAttrs {
            size: size.extend(0.0).extend(0.0),
            ..attrs
        };
    }
}

/// Spawn the node with its attributes, then give it a material of its own
/// (freed with the entity) — the spawn context holds no material assets, so
/// that's a queued entity command.
fn spawn<A, M>(ctx: &mut SpawnCtx) -> Entity
where
    M: AttrMaterial<Attrs = A>,
    A: Component + Copy + PartialEq + for<'a> From<&'a Attrs>,
{
    let attrs = A::from(&ctx.props.attrs);
    let entity = ctx.spawn(attrs);
    ctx.commands
        .entity(entity)
        .queue(move |mut entity: EntityWorldMut| {
            let mut material = M::default();
            material.set(attrs, Vec2::ZERO);
            let handle =
                entity.world_scope(|world| world.resource_mut::<Assets<M>>().add(material));
            entity.insert(MaterialNode(handle));
        });
    entity
}

/// The writers' body: fold the merged attributes into the component,
/// compare-before-write (the spawn already did it for a create).
fn write_attrs<A>(fresh: bool, attrs: &Attrs, ec: &mut EntityCommands)
where
    A: Component + Copy + PartialEq + for<'a> From<&'a Attrs>,
{
    if fresh {
        return;
    }
    let next = A::from(attrs);
    ec.queue(move |mut entity: EntityWorldMut| {
        if entity.get::<A>() != Some(&next) {
            entity.insert(next);
        }
    });
}

/// Push changed attributes and box sizes into each element's material.
#[allow(clippy::type_complexity)]
fn apply<A, M>(
    changed: Query<
        (&A, &ComputedNode, &MaterialNode<M>),
        Or<(Changed<A>, Changed<ComputedNode>, Added<MaterialNode<M>>)>,
    >,
    mut materials: ResMut<Assets<M>>,
) where
    M: AttrMaterial<Attrs = A>,
    A: Component + Copy + PartialEq + for<'a> From<&'a Attrs>,
{
    for (attrs, node, material) in &changed {
        if let Some(mut material) = materials.get_mut(&material.0) {
            material.set(*attrs, node.size * node.inverse_scale_factor);
        }
    }
}
