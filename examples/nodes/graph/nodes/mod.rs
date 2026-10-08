//! What each node computes. A node reads its parameters and inputs through
//! [`Ctx`] (missing values fall back to the catalog defaults) and returns one
//! buffer per output, skipping outputs nobody wants.

mod adjust;
mod filter;
mod generate;

use std::collections::BTreeMap;
use std::sync::Arc;

use bevy::prelude::*;

use super::Value;
use super::buf::{Buf, In};
use super::catalog::{Field, NodeDef, Stop};

pub struct Ctx<'a> {
    /// Image size, pixels per side.
    pub n: usize,
    pub def: &'static NodeDef,
    pub values: &'a BTreeMap<String, Value>,
    /// Per input socket (catalog order): the linked upstream buffer.
    pub inputs: &'a [Option<Arc<Buf>>],
    pub wanted: u32,
}

impl Ctx<'_> {
    fn field(&self, id: &str) -> Option<&'static Field> {
        let field = self.def.field(id);
        debug_assert!(field.is_some(), "{} has no field {id}", self.def.kind);
        field
    }

    pub fn float(&self, id: &str) -> f32 {
        match (self.values.get(id), self.field(id)) {
            (Some(Value::Num(v)), Some(Field::Float { min, max, .. })) => v.clamp(*min, *max),
            (Some(Value::Num(v)), _) => *v,
            (_, Some(Field::Float { default, .. })) => *default,
            _ => 0.0,
        }
    }

    /// A `%` parameter as a fraction.
    pub fn fraction(&self, id: &str) -> f32 {
        self.float(id) / 100.0
    }

    pub fn int(&self, id: &str) -> i32 {
        match (self.values.get(id), self.field(id)) {
            (Some(Value::Num(v)), Some(Field::Int { min, max, .. })) => {
                (v.round() as i32).clamp(*min, *max)
            }
            (_, Some(Field::Int { default, .. })) => *default,
            _ => 0,
        }
    }

    pub fn choice(&self, id: &str) -> &str {
        match (self.values.get(id), self.field(id)) {
            (Some(Value::Text(v)), _) => v,
            (_, Some(Field::Enum { default, .. })) => default,
            _ => "",
        }
    }

    pub fn flag(&self, id: &str) -> bool {
        match (self.values.get(id), self.field(id)) {
            (Some(Value::Bool(v)), _) => *v,
            (_, Some(Field::Bool { default })) => *default,
            _ => false,
        }
    }

    pub fn color(&self, id: &str) -> Vec3 {
        match (self.values.get(id), self.field(id)) {
            (Some(Value::Color(c)), _) => Vec3::from(*c),
            (_, Some(Field::Color { default })) => Vec3::from(*default),
            _ => Vec3::ZERO,
        }
    }

    /// The ramp's stops, sorted by position (at least one).
    pub fn ramp(&self, id: &str) -> Vec<Stop> {
        let mut stops = match (self.values.get(id), self.field(id)) {
            (Some(Value::Ramp(v)), _) if !v.is_empty() => v.clone(),
            (_, Some(Field::Ramp { default })) => default.clone(),
            _ => vec![Stop {
                pos: 0.0,
                color: [0.0; 3],
            }],
        };
        stops.sort_by(|a, b| a.pos.total_cmp(&b.pos));
        stops
    }

    /// An input socket: the linked buffer, else its inline value.
    pub fn input(&self, id: &str) -> In<'_> {
        let socket = self.def.inputs.iter().position(|s| s.id == id);
        debug_assert!(socket.is_some(), "{} has no input {id}", self.def.kind);
        if let Some(buf) = socket.and_then(|i| self.inputs[i].as_deref()) {
            return In::Buf(buf);
        }
        // An image input has no field: unlinked, it reads black.
        match self.def.field(id) {
            Some(Field::Float { .. }) => In::Gray(self.float(id)),
            Some(Field::Color { .. }) => In::Color(self.color(id)),
            _ => In::Gray(0.0),
        }
    }

    /// Whether output `i` is linked or viewed (output 0, the thumbnail, always is).
    pub fn wants(&self, i: usize) -> bool {
        self.wanted & (1 << i) != 0
    }

    /// Image coordinates (`0..1`, pixel centers) of pixel `(x, y)`.
    pub fn uv(&self, x: usize, y: usize) -> Vec2 {
        (Vec2::new(x as f32, y as f32) + 0.5) / self.n as f32
    }
}

pub fn eval(ctx: &Ctx) -> Vec<Option<Buf>> {
    match ctx.def.kind {
        "value" => vec![Some(Buf::Uniform(Vec3::splat(ctx.float("value"))))],
        "color" => vec![Some(Buf::Uniform(ctx.color("color")))],
        "noise" => generate::noise(ctx),
        "voronoi" => generate::voronoi(ctx),
        "bricks" => generate::bricks(ctx),
        "gradient" => generate::gradient(ctx),
        "shape" => generate::shape(ctx),
        "mix" => adjust::mix(ctx),
        "invert" => adjust::invert(ctx),
        "levels" => adjust::levels(ctx),
        "ramp" => adjust::ramp(ctx),
        "math" => adjust::math(ctx),
        "mapRange" => adjust::map_range(ctx),
        "blur" => filter::blur(ctx),
        "slopeBlur" => filter::slope_blur(ctx),
        "warp" => filter::warp(ctx),
        "transform" => filter::transform(ctx),
        "edges" => filter::edges(ctx),
        "normal" => filter::normal(ctx),
        // The output node has no outputs; the material reads its inputs.
        _ => ctx.def.outputs.iter().map(|_| None).collect(),
    }
}

/// Smooth Hermite step.
pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::catalog::{CATALOG, get};

    fn run(kind: &str, values: &[(&str, Value)], inputs: &[Option<Arc<Buf>>]) -> Vec<Option<Buf>> {
        let def = get(kind).unwrap();
        let values = values
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        let mut linked = vec![None; def.inputs.len()];
        linked[..inputs.len()].clone_from_slice(inputs);
        eval(&Ctx {
            n: 16,
            def,
            values: &values,
            inputs: &linked,
            wanted: u32::MAX,
        })
    }

    /// Every node runs on its defaults and fills every output it has.
    #[test]
    fn every_node_runs_on_defaults() {
        for def in CATALOG.iter().filter(|d| d.kind != "reroute") {
            let out = run(def.kind, &[], &[]);
            assert_eq!(out.len(), def.outputs.len(), "{}", def.kind);
            for buf in out.iter() {
                let buf = buf
                    .as_ref()
                    .unwrap_or_else(|| panic!("{} skipped an output", def.kind));
                for i in [0, 255] {
                    let c = buf.color(i);
                    assert!(c.is_finite(), "{} made {c}", def.kind);
                }
            }
        }
    }

    #[test]
    fn invert_and_levels_keep_gray_gray() {
        let gray = Arc::new(Buf::Gray(vec![0.25; 256]));
        let out = run("invert", &[], &[None, Some(gray.clone())]);
        assert!(matches!(&out[0], Some(Buf::Gray(v)) if (v[0] - 0.75).abs() < 1e-6));
        let out = run("levels", &[("gamma", Value::Num(1.0))], &[Some(gray)]);
        assert!(matches!(&out[0], Some(Buf::Gray(_))));
    }

    #[test]
    fn ramp_interpolates_between_stops() {
        let fac = Arc::new(Buf::Gray(vec![0.5; 256]));
        let out = run("ramp", &[], &[Some(fac)]);
        let c = out[0].as_ref().unwrap().color(0);
        assert!((c - Vec3::splat(0.5)).length() < 1e-4, "{c}");
    }
}
