//! Per-pixel color and value nodes.

use bevy::prelude::*;

use super::{Ctx, smoothstep};
use crate::graph::buf::{Buf, fill_color, fill_gray, map_like};
use crate::graph::catalog::Stop;

fn blend(mode: &str, a: Vec3, b: Vec3) -> Vec3 {
    let per = |f: fn(f32, f32) -> f32| Vec3::new(f(a.x, b.x), f(a.y, b.y), f(a.z, b.z));
    match mode {
        "add" => a + b,
        "multiply" => a * b,
        "screen" => 1.0 - (1.0 - a) * (1.0 - b),
        "overlay" => per(|a, b| {
            if a < 0.5 {
                2.0 * a * b
            } else {
                1.0 - 2.0 * (1.0 - a) * (1.0 - b)
            }
        }),
        "softLight" => per(|a, b| (1.0 - 2.0 * b) * a * a + 2.0 * b * a),
        "subtract" => a - b,
        "difference" => (a - b).abs(),
        "darken" => a.min(b),
        "lighten" => a.max(b),
        _ => b,
    }
}

pub fn mix(ctx: &Ctx) -> Vec<Option<Buf>> {
    let (fac, a, b) = (ctx.input("fac"), ctx.input("a"), ctx.input("b"));
    let mode = ctx.choice("mode");
    let clamp = ctx.flag("clamp");
    let at = |i: usize| {
        let (a, f) = (a.color(i), fac.gray(i));
        let c = a.lerp(blend(mode, a, b.color(i)), f);
        if clamp {
            c.clamp(Vec3::ZERO, Vec3::ONE)
        } else {
            c
        }
    };
    let n = ctx.n;
    // Both sides gray: stay gray.
    let buf = if a.is_gray() && b.is_gray() {
        fill_gray(n, |x, y| at(y * n + x).x)
    } else {
        fill_color(n, |x, y| at(y * n + x))
    };
    vec![Some(buf)]
}

pub fn invert(ctx: &Ctx) -> Vec<Option<Buf>> {
    let fac = ctx.input("fac");
    vec![Some(map_like(ctx.n, ctx.input("color"), |c, i| {
        c.lerp(1.0 - c, fac.gray(i))
    }))]
}

pub fn levels(ctx: &Ctx) -> Vec<Option<Buf>> {
    let (in_lo, in_hi) = (ctx.float("inLow"), ctx.float("inHigh"));
    let inv_gamma = 1.0 / ctx.float("gamma");
    let (out_lo, out_hi) = (ctx.float("outLow"), ctx.float("outHigh"));
    let span = in_hi - in_lo;
    let span = if span.abs() < 1e-4 {
        1e-4f32.copysign(span)
    } else {
        span
    };
    vec![Some(map_like(ctx.n, ctx.input("color"), |c, _| {
        let t = ((c - in_lo) / span).clamp(Vec3::ZERO, Vec3::ONE);
        let t = t.powf(inv_gamma);
        Vec3::splat(out_lo) + t * (out_hi - out_lo)
    }))]
}

/// The ramp's color at `t`.
pub fn sample_ramp(stops: &[Stop], interpolation: &str, t: f32) -> Vec3 {
    let first = &stops[0];
    if t <= first.pos {
        return first.color.into();
    }
    for pair in stops.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if t <= b.pos {
            let f = ((t - a.pos) / (b.pos - a.pos).max(1e-6)).clamp(0.0, 1.0);
            let f = match interpolation {
                "constant" => 0.0,
                "ease" => smoothstep(0.0, 1.0, f),
                _ => f,
            };
            return Vec3::from(a.color).lerp(b.color.into(), f);
        }
    }
    stops[stops.len() - 1].color.into()
}

pub fn ramp(ctx: &Ctx) -> Vec<Option<Buf>> {
    let stops = ctx.ramp("ramp");
    let interpolation = ctx.choice("interpolation");
    let fac = ctx.input("fac");
    if let Some(c) = fac.uniform() {
        return vec![Some(Buf::Uniform(sample_ramp(&stops, interpolation, c.x)))];
    }
    let n = ctx.n;
    vec![Some(fill_color(n, |x, y| {
        sample_ramp(&stops, interpolation, fac.gray(y * n + x))
    }))]
}

pub fn math(ctx: &Ctx) -> Vec<Option<Buf>> {
    let (a, b) = (ctx.input("a"), ctx.input("b"));
    let op = ctx.choice("operation");
    let clamp = ctx.flag("clamp");
    let n = ctx.n;
    vec![Some(fill_gray(n, |x, y| {
        let i = y * n + x;
        let (a, b) = (a.gray(i), b.gray(i));
        let v = match op {
            "subtract" => a - b,
            "multiply" => a * b,
            "divide" => {
                if b.abs() < 1e-6 {
                    0.0
                } else {
                    a / b
                }
            }
            "power" => a.max(0.0).powf(b),
            "minimum" => a.min(b),
            "maximum" => a.max(b),
            "difference" => (a - b).abs(),
            _ => a + b,
        };
        if clamp { v.clamp(0.0, 1.0) } else { v }
    }))]
}

pub fn map_range(ctx: &Ctx) -> Vec<Option<Buf>> {
    let (from_lo, from_hi) = (ctx.float("fromMin"), ctx.float("fromMax"));
    let (to_lo, to_hi) = (ctx.float("toMin"), ctx.float("toMax"));
    let interpolation = ctx.choice("interpolation");
    let steps = ctx.int("steps") as f32;
    let clamp = ctx.flag("clamp");
    let span = if (from_hi - from_lo).abs() < 1e-6 {
        1e-6
    } else {
        from_hi - from_lo
    };
    vec![Some(map_like(ctx.n, ctx.input("value"), |c, _| {
        let t = (c.x - from_lo) / span;
        let t = match interpolation {
            "stepped" => (t * steps).floor() / (steps - 1.0).max(1.0),
            "smoothstep" => smoothstep(0.0, 1.0, t),
            "smootherstep" => {
                let t = t.clamp(0.0, 1.0);
                t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
            }
            _ => t,
        };
        let v = to_lo + t * (to_hi - to_lo);
        let v = if clamp {
            v.clamp(to_lo.min(to_hi), to_lo.max(to_hi))
        } else {
            v
        };
        Vec3::splat(v)
    }))]
}
