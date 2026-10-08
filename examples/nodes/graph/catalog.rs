//! The node catalog: every node type's sockets and parameters, defined once
//! here and served to React (`bevy.catalog()`), which renders nodes, fields
//! and the Add menu from it — adding a node is one entry here plus its
//! evaluation in `nodes/`.

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Serialize, TS, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    Input,
    Texture,
    Color,
    Converter,
    Filter,
    Vector,
    Layout,
    Output,
}

/// What flows through a socket (its dot color). `Any` follows its link
/// (the reroute).
#[derive(Serialize, TS, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SocketType {
    Gray,
    Color,
    Vector,
    Any,
}

/// A color-ramp stop.
#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq)]
pub struct Stop {
    /// Position along the ramp, `0..1`.
    pub pos: f32,
    pub color: [f32; 3],
}

/// The field a parameter (or an unlinked input) shows, with its default.
#[derive(Serialize, TS, Clone, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(tag = "kind", rename_all = "camelCase")]
pub enum Field {
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Float {
        default: f32,
        min: f32,
        max: f32,
        /// Change per dragged pixel.
        step: f32,
        /// A `0..1` amount drawn as a filled slider.
        factor: bool,
        /// Shown after the number (`"°"`, `"%"`), empty for none.
        unit: &'static str,
    },
    Int {
        default: i32,
        min: i32,
        max: i32,
    },
    Enum {
        default: &'static str,
        options: Vec<Choice>,
    },
    Color {
        default: [f32; 3],
    },
    Bool {
        default: bool,
    },
    Ramp {
        default: Vec<Stop>,
    },
}

#[derive(Serialize, TS, Clone, Debug)]
pub struct Choice {
    pub id: &'static str,
    pub label: &'static str,
}

#[derive(Serialize, TS, Clone, Debug)]
pub struct Socket {
    pub id: &'static str,
    pub label: &'static str,
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub ty: SocketType,
    /// The value an unlinked input shows and uses; `None` = no field (an
    /// image input that means nothing as a constant).
    pub field: Option<Field>,
}

#[derive(Serialize, TS, Clone, Debug)]
pub struct Param {
    pub id: &'static str,
    pub label: &'static str,
    pub field: Field,
}

#[derive(Serialize, TS, Clone, Debug)]
pub struct NodeDef {
    pub kind: &'static str,
    pub label: &'static str,
    pub category: Category,
    pub inputs: Vec<Socket>,
    pub outputs: Vec<Socket>,
    pub params: Vec<Param>,
    /// Shows a live thumbnail.
    pub preview: bool,
}

impl NodeDef {
    pub fn input(&self, id: &str) -> Option<&Socket> {
        self.inputs.iter().find(|s| s.id == id)
    }

    /// The default for a parameter or an input's inline field.
    pub fn field(&self, id: &str) -> Option<&Field> {
        self.params
            .iter()
            .find(|p| p.id == id)
            .map(|p| &p.field)
            .or_else(|| self.input(id).and_then(|s| s.field.as_ref()))
    }
}

pub fn get(kind: &str) -> Option<&'static NodeDef> {
    CATALOG.iter().find(|d| d.kind == kind)
}

// --- builders, so the table below reads like the node faces ---

fn float(default: f32, min: f32, max: f32, step: f32) -> Field {
    Field::Float {
        default,
        min,
        max,
        step,
        factor: false,
        unit: "",
    }
}

fn factor(default: f32) -> Field {
    Field::Float {
        default,
        min: 0.0,
        max: 1.0,
        step: 0.005,
        factor: true,
        unit: "",
    }
}

fn percent(default: f32, max: f32) -> Field {
    Field::Float {
        default,
        min: 0.0,
        max,
        step: 0.05,
        factor: false,
        unit: "%",
    }
}

fn degrees(default: f32) -> Field {
    Field::Float {
        default,
        min: -180.0,
        max: 180.0,
        step: 1.0,
        factor: false,
        unit: "°",
    }
}

fn int(default: i32, min: i32, max: i32) -> Field {
    Field::Int { default, min, max }
}

fn choice(default: &'static str, options: &[(&'static str, &'static str)]) -> Field {
    Field::Enum {
        default,
        options: options
            .iter()
            .map(|&(id, label)| Choice { id, label })
            .collect(),
    }
}

fn param(id: &'static str, label: &'static str, field: Field) -> Param {
    Param { id, label, field }
}

fn seed() -> Param {
    param("seed", "Seed", int(0, 0, 9999))
}

fn sock(id: &'static str, label: &'static str, ty: SocketType, field: Option<Field>) -> Socket {
    Socket {
        id,
        label,
        ty,
        field,
    }
}

fn gray_in(id: &'static str, label: &'static str, default: f32) -> Socket {
    sock(id, label, SocketType::Gray, Some(factor(default)))
}

fn color_in(id: &'static str, label: &'static str, default: [f32; 3]) -> Socket {
    sock(id, label, SocketType::Color, Some(Field::Color { default }))
}

/// An image input with no constant field.
fn image_in(id: &'static str, label: &'static str, ty: SocketType) -> Socket {
    sock(id, label, ty, None)
}

fn out(id: &'static str, label: &'static str, ty: SocketType) -> Socket {
    sock(id, label, ty, None)
}

fn node(
    kind: &'static str,
    label: &'static str,
    category: Category,
    inputs: Vec<Socket>,
    outputs: Vec<Socket>,
    params: Vec<Param>,
) -> NodeDef {
    NodeDef {
        kind,
        label,
        category,
        inputs,
        outputs,
        params,
        preview: !matches!(category, Category::Output | Category::Layout),
    }
}

use Category as C;
use SocketType as S;

const BLEND_MODES: &[(&str, &str)] = &[
    ("mix", "Mix"),
    ("add", "Add"),
    ("multiply", "Multiply"),
    ("screen", "Screen"),
    ("overlay", "Overlay"),
    ("softLight", "Soft Light"),
    ("subtract", "Subtract"),
    ("difference", "Difference"),
    ("darken", "Darken"),
    ("lighten", "Lighten"),
];

pub static CATALOG: LazyLock<Vec<NodeDef>> = LazyLock::new(|| {
    vec![
        // --- input ---
        node(
            "value",
            "Value",
            C::Input,
            vec![],
            vec![out("value", "Value", S::Gray)],
            vec![param("value", "Value", float(0.5, -10.0, 10.0, 0.01))],
        ),
        node(
            "color",
            "Color",
            C::Input,
            vec![],
            vec![out("color", "Color", S::Color)],
            vec![param(
                "color",
                "Color",
                Field::Color {
                    default: [0.8, 0.35, 0.2],
                },
            )],
        ),
        // --- texture ---
        node(
            "noise",
            "Noise",
            C::Texture,
            vec![],
            vec![out("fac", "Fac", S::Gray), out("color", "Color", S::Color)],
            vec![
                param("scale", "Scale", int(6, 1, 64)),
                param("detail", "Detail", int(6, 1, 10)),
                param("roughness", "Roughness", factor(0.5)),
                param("distortion", "Distortion", float(0.0, 0.0, 4.0, 0.01)),
                seed(),
            ],
        ),
        node(
            "voronoi",
            "Voronoi",
            C::Texture,
            vec![],
            vec![
                out("distance", "Distance", S::Gray),
                out("color", "Color", S::Color),
                out("random", "Random", S::Gray),
            ],
            vec![
                param(
                    "feature",
                    "Feature",
                    choice(
                        "f1",
                        &[("f1", "F1"), ("f2", "F2"), ("edge", "Distance to Edge")],
                    ),
                ),
                param(
                    "metric",
                    "Metric",
                    choice(
                        "euclidean",
                        &[
                            ("euclidean", "Euclidean"),
                            ("manhattan", "Manhattan"),
                            ("chebyshev", "Chebychev"),
                        ],
                    ),
                ),
                param("scale", "Scale", int(8, 1, 64)),
                param("randomness", "Randomness", factor(1.0)),
                seed(),
            ],
        ),
        node(
            "bricks",
            "Bricks",
            C::Texture,
            vec![],
            vec![
                out("mask", "Mask", S::Gray),
                out("random", "Random", S::Gray),
            ],
            vec![
                param("rows", "Rows", int(8, 1, 64)),
                param("columns", "Columns", int(4, 1, 32)),
                param("offset", "Offset", factor(0.5)),
                param("mortar", "Mortar", percent(4.0, 25.0)),
                param("bevel", "Bevel", percent(10.0, 50.0)),
                seed(),
            ],
        ),
        node(
            "gradient",
            "Gradient",
            C::Texture,
            vec![],
            vec![out("fac", "Fac", S::Gray)],
            vec![
                param(
                    "type",
                    "Type",
                    choice(
                        "linear",
                        &[
                            ("linear", "Linear"),
                            ("quadratic", "Quadratic"),
                            ("diagonal", "Diagonal"),
                            ("radial", "Radial"),
                            ("spherical", "Spherical"),
                        ],
                    ),
                ),
                param("rotation", "Rotation", degrees(0.0)),
            ],
        ),
        node(
            "shape",
            "Shape",
            C::Texture,
            vec![],
            vec![out("mask", "Mask", S::Gray)],
            vec![
                param(
                    "shape",
                    "Shape",
                    choice(
                        "disc",
                        &[
                            ("disc", "Disc"),
                            ("square", "Square"),
                            ("polygon", "Polygon"),
                            ("star", "Star"),
                        ],
                    ),
                ),
                param("sides", "Sides", int(6, 3, 12)),
                param("size", "Size", factor(0.7)),
                param("softness", "Softness", factor(0.05)),
                param("tiles", "Tiles", int(1, 1, 32)),
                param("rotation", "Rotation", degrees(0.0)),
            ],
        ),
        // --- color ---
        node(
            "mix",
            "Mix",
            C::Color,
            vec![
                gray_in("fac", "Fac", 0.5),
                color_in("a", "A", [0.5, 0.5, 0.5]),
                color_in("b", "B", [0.2, 0.2, 0.2]),
            ],
            vec![out("result", "Result", S::Color)],
            vec![
                param("mode", "Blend", choice("mix", BLEND_MODES)),
                param("clamp", "Clamp", Field::Bool { default: true }),
            ],
        ),
        node(
            "invert",
            "Invert",
            C::Color,
            vec![
                gray_in("fac", "Fac", 1.0),
                color_in("color", "Color", [0.0, 0.0, 0.0]),
            ],
            vec![out("color", "Color", S::Color)],
            vec![],
        ),
        node(
            "levels",
            "Levels",
            C::Color,
            vec![color_in("color", "Color", [0.5, 0.5, 0.5])],
            vec![out("color", "Color", S::Color)],
            vec![
                param("inLow", "In Low", factor(0.0)),
                param("inHigh", "In High", factor(1.0)),
                param("gamma", "Gamma", float(1.0, 0.1, 10.0, 0.01)),
                param("outLow", "Out Low", factor(0.0)),
                param("outHigh", "Out High", factor(1.0)),
            ],
        ),
        node(
            "ramp",
            "Color Ramp",
            C::Color,
            vec![gray_in("fac", "Fac", 0.5)],
            vec![out("color", "Color", S::Color)],
            vec![
                param(
                    "interpolation",
                    "Interpolation",
                    choice(
                        "linear",
                        &[
                            ("linear", "Linear"),
                            ("ease", "Ease"),
                            ("constant", "Constant"),
                        ],
                    ),
                ),
                param(
                    "ramp",
                    "Ramp",
                    Field::Ramp {
                        default: vec![
                            Stop {
                                pos: 0.0,
                                color: [0.0, 0.0, 0.0],
                            },
                            Stop {
                                pos: 1.0,
                                color: [1.0, 1.0, 1.0],
                            },
                        ],
                    },
                ),
            ],
        ),
        // --- converter ---
        node(
            "math",
            "Math",
            C::Converter,
            vec![gray_in("a", "A", 0.5), gray_in("b", "B", 0.5)],
            vec![out("value", "Value", S::Gray)],
            vec![
                param(
                    "operation",
                    "Operation",
                    choice(
                        "add",
                        &[
                            ("add", "Add"),
                            ("subtract", "Subtract"),
                            ("multiply", "Multiply"),
                            ("divide", "Divide"),
                            ("power", "Power"),
                            ("minimum", "Minimum"),
                            ("maximum", "Maximum"),
                            ("difference", "Difference"),
                        ],
                    ),
                ),
                param("clamp", "Clamp", Field::Bool { default: true }),
            ],
        ),
        node(
            "mapRange",
            "Map Range",
            C::Converter,
            vec![gray_in("value", "Value", 0.5)],
            vec![out("result", "Result", S::Gray)],
            vec![
                param(
                    "interpolation",
                    "Interpolation",
                    choice(
                        "linear",
                        &[
                            ("linear", "Linear"),
                            ("stepped", "Stepped"),
                            ("smoothstep", "Smooth Step"),
                            ("smootherstep", "Smoother Step"),
                        ],
                    ),
                ),
                param("fromMin", "From Min", float(0.0, -10.0, 10.0, 0.005)),
                param("fromMax", "From Max", float(1.0, -10.0, 10.0, 0.005)),
                param("toMin", "To Min", float(0.0, -10.0, 10.0, 0.005)),
                param("toMax", "To Max", float(1.0, -10.0, 10.0, 0.005)),
                param("steps", "Steps", int(4, 2, 32)),
                param("clamp", "Clamp", Field::Bool { default: true }),
            ],
        ),
        // --- filter ---
        node(
            "blur",
            "Blur",
            C::Filter,
            vec![image_in("image", "Image", S::Color)],
            vec![out("image", "Image", S::Color)],
            vec![param("radius", "Radius", percent(1.0, 15.0))],
        ),
        node(
            "slopeBlur",
            "Slope Blur",
            C::Filter,
            vec![
                image_in("image", "Image", S::Color),
                image_in("slope", "Slope", S::Gray),
            ],
            vec![out("image", "Image", S::Color)],
            vec![
                param(
                    "mode",
                    "Mode",
                    choice("blur", &[("blur", "Blur"), ("min", "Min"), ("max", "Max")]),
                ),
                param("intensity", "Intensity", percent(2.0, 20.0)),
                param("samples", "Samples", int(16, 2, 64)),
            ],
        ),
        node(
            "warp",
            "Warp",
            C::Filter,
            vec![
                image_in("image", "Image", S::Color),
                image_in("intensity", "Intensity", S::Gray),
            ],
            vec![out("image", "Image", S::Color)],
            vec![
                param(
                    "mode",
                    "Mode",
                    choice(
                        "gradient",
                        &[("gradient", "Gradient"), ("directional", "Directional")],
                    ),
                ),
                param("strength", "Strength", percent(3.0, 25.0)),
                param("angle", "Angle", degrees(0.0)),
            ],
        ),
        node(
            "transform",
            "Transform",
            C::Filter,
            vec![image_in("image", "Image", S::Color)],
            vec![out("image", "Image", S::Color)],
            vec![
                param("offsetX", "Offset X", float(0.0, -1.0, 1.0, 0.002)),
                param("offsetY", "Offset Y", float(0.0, -1.0, 1.0, 0.002)),
                param("rotation", "Rotation", degrees(0.0)),
                param("scale", "Scale", float(1.0, 0.1, 16.0, 0.01)),
            ],
        ),
        node(
            "edges",
            "Edges",
            C::Filter,
            vec![image_in("height", "Height", S::Gray)],
            vec![out("fac", "Fac", S::Gray)],
            vec![
                param(
                    "mode",
                    "Mode",
                    choice(
                        "curvature",
                        &[
                            ("curvature", "Curvature"),
                            ("convex", "Convex"),
                            ("concave", "Concave"),
                            ("edges", "Edges"),
                        ],
                    ),
                ),
                param("radius", "Radius", float(1.5, 0.5, 8.0, 0.01)),
                param("strength", "Strength", float(4.0, 0.0, 32.0, 0.05)),
            ],
        ),
        // --- vector ---
        node(
            "normal",
            "Normal Map",
            C::Vector,
            vec![image_in("height", "Height", S::Gray)],
            vec![out("normal", "Normal", S::Vector)],
            vec![
                param("strength", "Strength", float(4.0, 0.0, 32.0, 0.05)),
                param("invert", "Invert", Field::Bool { default: false }),
            ],
        ),
        // --- layout ---
        node(
            "reroute",
            "Reroute",
            C::Layout,
            vec![image_in("in", "", S::Any)],
            vec![out("out", "", S::Any)],
            vec![],
        ),
        // --- output ---
        node(
            "output",
            "Material Output",
            C::Output,
            vec![
                color_in("baseColor", "Base Color", [0.8, 0.8, 0.8]),
                gray_in("metallic", "Metallic", 0.0),
                gray_in("roughness", "Roughness", 0.5),
                image_in("normal", "Normal", S::Vector),
                image_in("height", "Height", S::Gray),
                color_in("emission", "Emission", [0.0, 0.0, 0.0]),
            ],
            vec![],
            vec![
                param(
                    "emissionStrength",
                    "Emission Strength",
                    float(1.0, 0.0, 50.0, 0.05),
                ),
                param("heightScale", "Height Scale", percent(4.0, 20.0)),
            ],
        ),
    ]
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_are_unique_and_fields_resolve() {
        let mut kinds: Vec<_> = CATALOG.iter().map(|d| d.kind).collect();
        kinds.sort();
        kinds.dedup();
        assert_eq!(kinds.len(), CATALOG.len());
        let noise = get("noise").unwrap();
        assert!(matches!(
            noise.field("scale"),
            Some(Field::Int { default: 6, .. })
        ));
        assert!(
            get("mix").unwrap().field("fac").is_some(),
            "an input's inline field"
        );
    }
}
