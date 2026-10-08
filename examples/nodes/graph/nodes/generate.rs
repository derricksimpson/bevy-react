//! Generators: patterns made from nothing but their parameters.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use super::{Ctx, smoothstep};
use crate::graph::buf::{Buf, fill_color, fill_gray};
use crate::graph::noise::{self, Fbm, Metric, rand01};

fn seed(ctx: &Ctx) -> u32 {
    ctx.int("seed") as u32
}

pub fn noise(ctx: &Ctx) -> Vec<Option<Buf>> {
    let fbm = Fbm {
        scale: ctx.int("scale") as u32,
        octaves: ctx.int("detail") as u32,
        roughness: ctx.float("roughness"),
        distortion: ctx.float("distortion"),
        seed: seed(ctx),
    };
    vec![
        Some(fill_gray(ctx.n, |x, y| fbm.gray(ctx.uv(x, y)))),
        ctx.wants(1)
            .then(|| fill_color(ctx.n, |x, y| fbm.color(ctx.uv(x, y)))),
    ]
}

pub fn voronoi(ctx: &Ctx) -> Vec<Option<Buf>> {
    let scale = ctx.int("scale") as u32;
    let randomness = ctx.float("randomness");
    let seed = seed(ctx);
    let metric = match ctx.choice("metric") {
        "manhattan" => Metric::Manhattan,
        "chebyshev" => Metric::Chebyshev,
        _ => Metric::Euclidean,
    };
    let feature = ctx.choice("feature");
    let cell = |x, y| noise::voronoi(ctx.uv(x, y), scale, randomness, metric, seed);
    vec![
        Some(fill_gray(ctx.n, |x, y| {
            let c = cell(x, y);
            match feature {
                "f2" => c.f2,
                "edge" => c.edge,
                _ => c.f1,
            }
        })),
        ctx.wants(1).then(|| {
            fill_color(ctx.n, |x, y| {
                let (cx, cy) = cell(x, y).id;
                Vec3::new(
                    rand01(cx, cy, seed ^ 0x100),
                    rand01(cx, cy, seed ^ 0x200),
                    rand01(cx, cy, seed ^ 0x300),
                )
            })
        }),
        ctx.wants(2).then(|| {
            fill_gray(ctx.n, |x, y| {
                let (cx, cy) = cell(x, y).id;
                rand01(cx, cy, seed ^ 0x400)
            })
        }),
    ]
}

/// A running bond: `rows` courses of `columns` bricks, odd courses shifted
/// by `offset`. Tiles seamlessly with an even row count.
pub fn bricks(ctx: &Ctx) -> Vec<Option<Buf>> {
    let rows = ctx.int("rows") as f32;
    let cols = ctx.int("columns") as f32;
    let offset = ctx.float("offset");
    // Mortar and bevel are fractions of a brick's height.
    let mortar = ctx.fraction("mortar") / rows;
    let bevel = ctx.fraction("bevel") / rows;
    let seed = seed(ctx);
    let brick = |uv: Vec2| {
        let row = (uv.y * rows).floor();
        let shift = if row as i32 % 2 == 1 { offset } else { 0.0 };
        let u = uv.x * cols + shift;
        let col = u.floor();
        let (fx, fy) = (u - col, uv.y * rows - row);
        let edge = (fx.min(1.0 - fx) / cols).min(fy.min(1.0 - fy) / rows);
        let col = (col as i32).rem_euclid(cols as i32) as u32;
        (edge, col, row as u32)
    };
    vec![
        Some(fill_gray(ctx.n, |x, y| {
            let (edge, ..) = brick(ctx.uv(x, y));
            smoothstep(mortar * 0.5, mortar * 0.5 + bevel.max(1e-5), edge)
        })),
        ctx.wants(1).then(|| {
            fill_gray(ctx.n, |x, y| {
                let (edge, col, row) = brick(ctx.uv(x, y));
                if edge < mortar * 0.5 {
                    0.0
                } else {
                    rand01(col, row, seed)
                }
            })
        }),
    ]
}

pub fn gradient(ctx: &Ctx) -> Vec<Option<Buf>> {
    let kind = ctx.choice("type");
    let rot = Vec2::from_angle(-ctx.float("rotation").to_radians());
    vec![Some(fill_gray(ctx.n, |x, y| {
        let p = rot.rotate(ctx.uv(x, y) - 0.5) + 0.5;
        match kind {
            "quadratic" => p.x.clamp(0.0, 1.0).powi(2),
            "diagonal" => ((p.x + p.y) * 0.5).clamp(0.0, 1.0),
            "radial" => (p - 0.5).to_angle().rem_euclid(TAU) / TAU,
            "spherical" => (1.0 - (p - 0.5).length() * 2.0).max(0.0),
            _ => p.x.clamp(0.0, 1.0),
        }
    }))]
}

/// One shape per tile, centered, `size` of the tile wide.
pub fn shape(ctx: &Ctx) -> Vec<Option<Buf>> {
    let kind = ctx.choice("shape");
    let sides = ctx.int("sides") as f32;
    let radius = ctx.float("size") * 0.5;
    let tiles = ctx.int("tiles") as f32;
    let rot = Vec2::from_angle(-ctx.float("rotation").to_radians());
    // At least a pixel of antialiasing, in tile units.
    let aa = tiles / ctx.n as f32;
    let soft = (ctx.float("softness") * radius).max(aa);
    vec![Some(fill_gray(ctx.n, |x, y| {
        let p = rot.rotate((ctx.uv(x, y) * tiles).fract_gl() - 0.5);
        // Each shape's distance from its outline, positive outside.
        let d = match kind {
            "square" => p.abs().max_element() - radius,
            "polygon" => {
                let sector = TAU / sides;
                // Angle from the nearest edge midpoint (one sits up top).
                let a = p.to_angle() + PI * 0.5;
                let along = (a - (a / sector).round() * sector).cos() * p.length();
                along - radius * (sector * 0.5).cos()
            }
            "star" => {
                let sector = TAU / sides;
                // Fold into half a point: 0 = the tip (one up top), sector/2
                // = the notch; the outline runs straight between them.
                let a = (p.to_angle() + PI * 0.5).rem_euclid(sector);
                let a = a.min(sector - a);
                let q = Vec2::from_angle(a) * p.length();
                let tip = Vec2::new(radius, 0.0);
                let notch = Vec2::from_angle(sector * 0.5) * radius * 0.45;
                let edge = notch - tip;
                -edge.perp_dot(q - tip) / edge.length()
            }
            _ => p.length() - radius,
        };
        1.0 - smoothstep(-soft * 0.5, soft * 0.5, d)
    }))]
}
