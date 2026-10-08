//! Filters: nodes that read their input's neighborhood.
//!
//! Distances (radius, strength, warp) are fractions of the image or tuned in
//! pixels at 512², so a graph looks the same at every resolution.

use std::ops::{Add, Mul, Sub};

use bevy::prelude::*;

use super::Ctx;
use crate::graph::buf::{Buf, In, fill_color, fill_gray, par_rows, resample_like, wrap};

/// Pixel `(x + dx, y + dy)` of a gray input, wrapping.
fn at(input: In, n: usize, x: usize, y: usize, dx: i64, dy: i64) -> f32 {
    input.gray(wrap(y as i64 + dy, n) * n + wrap(x as i64 + dx, n))
}

/// The height gradient in image units (`dh/du`, `dh/dv`, v down), from
/// central differences over `r` pixels.
fn gradient(input: In, n: usize, x: usize, y: usize, r: i64) -> Vec2 {
    let scale = n as f32 / (2 * r) as f32;
    Vec2::new(
        at(input, n, x, y, r, 0) - at(input, n, x, y, -r, 0),
        at(input, n, x, y, 0, r) - at(input, n, x, y, 0, -r),
    ) * scale
}

trait Pixel:
    Copy + Default + Send + Sync + Add<Output = Self> + Sub<Output = Self> + Mul<f32, Output = Self>
{
}
impl Pixel for f32 {}
impl Pixel for Vec3 {}

/// One horizontal box blur of radius `r`, wrapping (a running sum, so the
/// cost doesn't grow with the radius).
fn box_rows<T: Pixel>(src: &[T], n: usize, r: usize) -> Vec<T> {
    let mut out = vec![T::default(); src.len()];
    let norm = 1.0 / (2 * r + 1) as f32;
    par_rows(&mut out, n, |y, row| {
        let line = &src[y * n..(y + 1) * n];
        let px = |i: i64| line[wrap(i, n)];
        let mut sum = (-(r as i64)..=r as i64).fold(T::default(), |s, i| s + px(i));
        for (x, o) in row.iter_mut().enumerate() {
            *o = sum * norm;
            sum = sum + px(x as i64 + r as i64 + 1) - px(x as i64 - r as i64);
        }
    });
    out
}

fn transpose<T: Copy + Default>(src: &[T], n: usize) -> Vec<T> {
    let mut out = vec![T::default(); src.len()];
    for y in 0..n {
        for x in 0..n {
            out[x * n + y] = src[y * n + x];
        }
    }
    out
}

/// Three box passes per axis approximate a gaussian of `sigma` pixels.
fn gaussian<T: Pixel>(src: Vec<T>, n: usize, sigma: f32) -> Vec<T> {
    // Box widths for three passes (Kovesi, "Fast almost-Gaussian filtering").
    let ideal = (12.0 * sigma * sigma / 3.0 + 1.0).sqrt();
    let mut lower = ideal.floor() as i32;
    if lower % 2 == 0 {
        lower -= 1;
    }
    let lower = lower.max(1);
    let upper = lower + 2;
    let m = ((12.0 * sigma * sigma - (3 * lower * lower) as f32 - (12 * lower) as f32 - 9.0)
        / (-4 * lower - 4) as f32)
        .round() as i32;
    let radii: Vec<usize> = (0..3)
        .map(|i| ((if i < m { lower } else { upper }) as usize - 1) / 2)
        .collect();
    let mut img = src;
    for _ in 0..2 {
        for &r in &radii {
            if r > 0 {
                img = box_rows(&img, n, r.min(n / 2 - 1));
            }
        }
        img = transpose(&img, n);
    }
    img
}

pub fn blur(ctx: &Ctx) -> Vec<Option<Buf>> {
    let input = ctx.input("image");
    let n = ctx.n;
    let sigma = ctx.fraction("radius") * n as f32 * 0.5;
    let buf = if input.uniform().is_some() || sigma < 0.3 {
        // Nothing to blur: a copy.
        resample_like(n, input, |x, y| Vec2::new(x as f32 + 0.5, y as f32 + 0.5))
    } else if input.is_gray() {
        Buf::Gray(gaussian(
            (0..n * n).map(|i| input.gray(i)).collect(),
            n,
            sigma,
        ))
    } else {
        Buf::Color(gaussian(
            (0..n * n).map(|i| input.color(i)).collect(),
            n,
            sigma,
        ))
    };
    vec![Some(buf)]
}

/// Walk each pixel down the slope map's gradient, gathering the image along
/// the way: streaks and erosion that follow the slope.
pub fn slope_blur(ctx: &Ctx) -> Vec<Option<Buf>> {
    let (image, slope) = (ctx.input("image"), ctx.input("slope"));
    let n = ctx.n;
    let samples = ctx.int("samples").max(1) as usize;
    let step = ctx.fraction("intensity") * n as f32 / samples as f32;
    let mode = ctx.choice("mode");
    // The downhill direction per pixel, sampled bilinearly along the walk.
    let field = fill_color(n, |x, y| {
        let g = -gradient(slope, n, x, y, 1);
        let len = g.length();
        if len < 1e-4 {
            Vec3::ZERO
        } else {
            (g / len).extend(0.0)
        }
    });
    let field = In::Buf(&field);
    let walk = |x: usize, y: usize, visit: &mut dyn FnMut(Vec2)| {
        let mut p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
        visit(p);
        for _ in 0..samples {
            p += field.sample_color(n, p.x, p.y).truncate() * step;
            visit(p);
        }
    };
    let weight = 1.0 / (samples + 1) as f32;
    let buf = if image.is_gray() {
        fill_gray(n, |x, y| {
            let mut acc = match mode {
                "min" => f32::MAX,
                "max" => f32::MIN,
                _ => 0.0,
            };
            walk(x, y, &mut |p| {
                let v = image.sample_gray(n, p.x, p.y);
                acc = match mode {
                    "min" => acc.min(v),
                    "max" => acc.max(v),
                    _ => acc + v * weight,
                };
            });
            acc
        })
    } else {
        fill_color(n, |x, y| {
            let mut acc = match mode {
                "min" => Vec3::MAX,
                "max" => Vec3::MIN,
                _ => Vec3::ZERO,
            };
            walk(x, y, &mut |p| {
                let v = image.sample_color(n, p.x, p.y);
                acc = match mode {
                    "min" => acc.min(v),
                    "max" => acc.max(v),
                    _ => acc + v * weight,
                };
            });
            acc
        })
    };
    vec![Some(buf)]
}

pub fn warp(ctx: &Ctx) -> Vec<Option<Buf>> {
    let (image, intensity) = (ctx.input("image"), ctx.input("intensity"));
    let n = ctx.n;
    // In pixels: a fraction of the image per unit of intensity (gradient).
    let strength = ctx.fraction("strength") * n as f32;
    let dir = Vec2::from_angle(ctx.float("angle").to_radians());
    let directional = ctx.choice("mode") == "directional";
    vec![Some(resample_like(n, image, |x, y| {
        let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
        let offset = if directional {
            dir * intensity.gray(y * n + x)
        } else {
            // A gradient of 4 (a typical noise slope) moves by `strength`.
            gradient(intensity, n, x, y, 1) * 0.25
        };
        p + offset * strength
    }))]
}

pub fn transform(ctx: &Ctx) -> Vec<Option<Buf>> {
    let n = ctx.n as f32;
    let offset = Vec2::new(ctx.float("offsetX"), ctx.float("offsetY"));
    let rot = Vec2::from_angle(-ctx.float("rotation").to_radians());
    let scale = ctx.float("scale");
    vec![Some(resample_like(ctx.n, ctx.input("image"), |x, y| {
        let uv = (Vec2::new(x as f32, y as f32) + 0.5) / n;
        let src = rot.rotate(uv - 0.5 - offset) * scale + 0.5;
        src * n
    }))]
}

/// Curvature (the height's Laplacian) or slope steepness, centered on gray.
pub fn edges(ctx: &Ctx) -> Vec<Option<Buf>> {
    let height = ctx.input("height");
    let n = ctx.n;
    let k = n as f32 / 512.0;
    let r = (ctx.float("radius") * k).round().max(1.0) as i64;
    let strength = ctx.float("strength");
    let mode = ctx.choice("mode");
    vec![Some(fill_gray(n, |x, y| {
        let h = at(height, n, x, y, 0, 0);
        if mode == "edges" {
            return (gradient(height, n, x, y, r).length() * strength * 0.004).min(1.0);
        }
        let around = at(height, n, x, y, r, 0)
            + at(height, n, x, y, -r, 0)
            + at(height, n, x, y, 0, r)
            + at(height, n, x, y, 0, -r);
        // Convex (a ridge) is positive. Per pixel at 512², so the radius
        // doesn't change the scale of the result.
        let convex = (4.0 * h - around) / (r * r) as f32 * k * k * strength * 2.0;
        match mode {
            "convex" => convex.clamp(0.0, 1.0),
            "concave" => (-convex).clamp(0.0, 1.0),
            _ => (0.5 + convex).clamp(0.0, 1.0),
        }
    }))]
}

/// A tangent-space normal map (OpenGL convention, +Y up), encoded `0..1`.
pub fn normal(ctx: &Ctx) -> Vec<Option<Buf>> {
    let height = ctx.input("height");
    let n = ctx.n;
    let strength = ctx.float("strength") * 0.01;
    let flip = if ctx.flag("invert") { -1.0 } else { 1.0 };
    vec![Some(fill_color(n, |x, y| {
        let g = gradient(height, n, x, y, 1) * strength;
        // Image y runs down; the texture's +Y runs up.
        let normal = Vec3::new(-g.x, g.y * flip, 1.0).normalize();
        normal * 0.5 + 0.5
    }))]
}
