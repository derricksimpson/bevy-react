//! Tileable noise: every lattice wraps at an integer period, so the images
//! repeat seamlessly on the preview mesh.

use std::f32::consts::TAU;

use bevy::math::{Vec2, Vec3};

/// A 32-bit integer hash (lowbias32).
pub fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

pub fn hash3(x: u32, y: u32, seed: u32) -> u32 {
    hash(x ^ hash(y ^ hash(seed.wrapping_add(0x9e37_79b9))))
}

/// A float in `0..1` from a lattice cell.
pub fn rand01(x: u32, y: u32, seed: u32) -> f32 {
    hash3(x, y, seed) as f32 / u32::MAX as f32
}

fn wrap(i: i32, period: u32) -> u32 {
    i.rem_euclid(period as i32) as u32
}

fn gradient(ix: i32, iy: i32, period: u32, seed: u32) -> Vec2 {
    let a = rand01(wrap(ix, period), wrap(iy, period), seed) * TAU;
    Vec2::new(a.cos(), a.sin())
}

fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Gradient (Perlin) noise at lattice coordinates `p`, repeating every
/// `period` cells. Roughly `-0.7..0.7`.
pub fn perlin(p: Vec2, period: u32, seed: u32) -> f32 {
    let i = p.floor();
    let f = p - i;
    let (ix, iy) = (i.x as i32, i.y as i32);
    let corner = |dx: i32, dy: i32| {
        gradient(ix + dx, iy + dy, period, seed).dot(f - Vec2::new(dx as f32, dy as f32))
    };
    let (u, v) = (fade(f.x), fade(f.y));
    let top = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * u;
    let bottom = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * u;
    top + (bottom - top) * v
}

/// Fractal noise in `0..1` (mostly) at image coordinates `uv` (`0..1`):
/// `octaves` layers, each double the frequency, `roughness` the gain.
pub fn fbm(uv: Vec2, scale: u32, octaves: u32, roughness: f32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut norm) = (0.0, 1.0, 0.0);
    let mut period = scale.max(1);
    for octave in 0..octaves {
        sum += amp * perlin(uv * period as f32, period, seed.wrapping_add(octave * 101));
        norm += amp;
        amp *= roughness;
        period *= 2;
    }
    (0.5 + 0.75 * sum / norm).clamp(0.0, 1.0)
}

/// Fractal noise with a domain warp of `distortion` (in cells), three
/// decorrelated channels when `color` is set.
pub struct Fbm {
    pub scale: u32,
    pub octaves: u32,
    pub roughness: f32,
    pub distortion: f32,
    pub seed: u32,
}

impl Fbm {
    fn warp(&self, uv: Vec2) -> Vec2 {
        if self.distortion <= 0.0 {
            return uv;
        }
        let at = |s: u32| fbm(uv, self.scale, self.octaves.min(4), 0.5, self.seed ^ s) - 0.5;
        uv + Vec2::new(at(0x51ed), at(0x2c1b)) * self.distortion / self.scale as f32
    }

    pub fn gray(&self, uv: Vec2) -> f32 {
        fbm(
            self.warp(uv),
            self.scale,
            self.octaves,
            self.roughness,
            self.seed,
        )
    }

    pub fn color(&self, uv: Vec2) -> Vec3 {
        let uv = self.warp(uv);
        let at = |s: u32| fbm(uv, self.scale, self.octaves, self.roughness, self.seed ^ s);
        Vec3::new(at(0x1111), at(0x2222), at(0x3333))
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Metric {
    Euclidean,
    Manhattan,
    Chebyshev,
}

impl Metric {
    fn dist(self, d: Vec2) -> f32 {
        match self {
            Metric::Euclidean => d.length(),
            Metric::Manhattan => d.x.abs() + d.y.abs(),
            Metric::Chebyshev => d.x.abs().max(d.y.abs()),
        }
    }
}

/// One Voronoi lookup.
pub struct Cell {
    pub f1: f32,
    pub f2: f32,
    /// Distance to the nearest cell border (euclidean only; `f2 - f1`
    /// otherwise).
    pub edge: f32,
    /// The nearest cell's lattice coordinates (wrapped), for its random
    /// value and color.
    pub id: (u32, u32),
}

/// Voronoi at image coordinates `uv`, `period` cells per side.
pub fn voronoi(uv: Vec2, period: u32, randomness: f32, metric: Metric, seed: u32) -> Cell {
    let p = uv * period as f32;
    let base = p.floor();
    let point = |dx: i32, dy: i32| {
        let c = Vec2::new(base.x + dx as f32, base.y + dy as f32);
        let (wx, wy) = (wrap(c.x as i32, period), wrap(c.y as i32, period));
        let jitter = Vec2::new(rand01(wx, wy, seed), rand01(wx, wy, seed ^ 0xa5a5)) - 0.5;
        (c + 0.5 + jitter * randomness, (wx, wy))
    };
    let (mut f1, mut f2) = (f32::MAX, f32::MAX);
    let (mut nearest, mut id) = (Vec2::ZERO, (0, 0));
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (q, cell) = point(dx, dy);
            let d = metric.dist(q - p);
            if d < f1 {
                f2 = f1;
                f1 = d;
                nearest = q;
                id = cell;
            } else if d < f2 {
                f2 = d;
            }
        }
    }
    let edge = if metric == Metric::Euclidean {
        // The distance to the bisector with each neighbor of the nearest
        // point (Quilez): exact for euclidean cells.
        let mut edge = f32::MAX;
        for dy in -2..=2 {
            for dx in -2..=2 {
                let (q, _) = point(dx, dy);
                let to = q - nearest;
                if to.length_squared() > 1e-6 {
                    let mid = (nearest + q) * 0.5;
                    edge = edge.min((mid - p).dot(to.normalize()));
                }
            }
        }
        edge
    } else {
        f2 - f1
    };
    Cell { f1, f2, edge, id }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_tiles() {
        for (u, v) in [(0.0, 0.3), (0.2, 0.0), (0.0, 0.0)] {
            let a = fbm(Vec2::new(u, v), 4, 5, 0.5, 7);
            let b = fbm(Vec2::new(u + 1.0, v + 1.0), 4, 5, 0.5, 7);
            assert!((a - b).abs() < 1e-4, "fbm wraps at the image edge");
            let a = voronoi(Vec2::new(u, v), 5, 1.0, Metric::Euclidean, 3);
            let b = voronoi(Vec2::new(u + 1.0, v), 5, 1.0, Metric::Euclidean, 3);
            assert!((a.f1 - b.f1).abs() < 1e-4 && a.id == b.id, "voronoi wraps");
        }
    }
}
