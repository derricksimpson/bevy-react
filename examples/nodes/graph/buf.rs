//! Pixel buffers the evaluator passes between nodes, and the accessors node
//! code reads its inputs through.
//!
//! Every image is square (`n × n`) and tiles: sampling wraps at the edges.
//! Color values are display values (what a thumbnail shows), the way texture
//! artists paint albedo; the material decides what is sRGB and what is data.

use bevy::prelude::*;
use bevy::tasks::{ComputeTaskPool, ParallelSliceMut, TaskPool};

/// One node output.
#[derive(Debug, Clone)]
pub enum Buf {
    Gray(Vec<f32>),
    Color(Vec<Vec3>),
    /// The same value everywhere: a `Value`/`Color` node or an unlinked input
    /// never allocates an image.
    Uniform(Vec3),
}

impl Buf {
    pub fn gray(&self, i: usize) -> f32 {
        match self {
            Buf::Gray(v) => v[i],
            Buf::Color(v) => luminance(v[i]),
            Buf::Uniform(c) => luminance(*c),
        }
    }

    pub fn color(&self, i: usize) -> Vec3 {
        match self {
            Buf::Gray(v) => Vec3::splat(v[i]),
            Buf::Color(v) => v[i],
            Buf::Uniform(c) => *c,
        }
    }

    /// Whether this output carries one channel (a node fed by it keeps the
    /// cheaper gray buffer, see [`map_like`]).
    pub fn is_gray(&self) -> bool {
        match self {
            Buf::Gray(_) => true,
            Buf::Color(_) => false,
            Buf::Uniform(c) => c.x == c.y && c.y == c.z,
        }
    }
}

pub fn luminance(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

/// A node input: a linked upstream output, or the inline value an unlinked
/// socket shows.
#[derive(Clone, Copy)]
pub enum In<'a> {
    Buf(&'a Buf),
    Gray(f32),
    Color(Vec3),
}

impl In<'_> {
    pub fn gray(&self, i: usize) -> f32 {
        match self {
            In::Buf(b) => b.gray(i),
            In::Gray(v) => *v,
            In::Color(c) => luminance(*c),
        }
    }

    pub fn color(&self, i: usize) -> Vec3 {
        match self {
            In::Buf(b) => b.color(i),
            In::Gray(v) => Vec3::splat(*v),
            In::Color(c) => *c,
        }
    }

    pub fn is_gray(&self) -> bool {
        match self {
            In::Buf(b) => b.is_gray(),
            In::Gray(_) => true,
            In::Color(_) => false,
        }
    }

    /// The constant value, when this input doesn't vary over the image.
    pub fn uniform(&self) -> Option<Vec3> {
        match self {
            In::Buf(Buf::Uniform(c)) => Some(*c),
            In::Buf(_) => None,
            In::Gray(v) => Some(Vec3::splat(*v)),
            In::Color(c) => Some(*c),
        }
    }

    /// Bilinear sample at pixel coordinates (pixel centers on `.5`), wrapping.
    pub fn sample_gray(&self, n: usize, x: f32, y: f32) -> f32 {
        match self.uniform() {
            Some(c) => luminance(c),
            None => bilinear(n, x, y, |i| self.gray(i)),
        }
    }

    pub fn sample_color(&self, n: usize, x: f32, y: f32) -> Vec3 {
        match self.uniform() {
            Some(c) => c,
            None => bilinear(n, x, y, |i| self.color(i)),
        }
    }
}

/// Wrap an integer coordinate into `0..n`.
pub fn wrap(v: i64, n: usize) -> usize {
    v.rem_euclid(n as i64) as usize
}

fn bilinear<T>(n: usize, x: f32, y: f32, at: impl Fn(usize) -> T) -> T
where
    T: std::ops::Mul<f32, Output = T> + std::ops::Add<Output = T>,
{
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let (x0, y0) = (x0 as i64, y0 as i64);
    let idx = |x: i64, y: i64| wrap(y, n) * n + wrap(x, n);
    let top = at(idx(x0, y0)) * (1.0 - tx) + at(idx(x0 + 1, y0)) * tx;
    let bottom = at(idx(x0, y0 + 1)) * (1.0 - tx) + at(idx(x0 + 1, y0 + 1)) * tx;
    top * (1.0 - ty) + bottom * ty
}

fn pool() -> &'static TaskPool {
    ComputeTaskPool::get_or_init(TaskPool::default)
}

/// Fill an `n × n` image row-parallel over the compute pool (inline on wasm).
fn fill<T: Copy + Default + Send + Sync>(n: usize, f: impl Fn(usize, usize) -> T + Sync) -> Vec<T> {
    let mut out = vec![T::default(); n * n];
    // ~8 rows per task: enough tasks to balance, few enough to stay cheap.
    let rows = 8.min(n).max(1);
    out.par_chunk_map_mut(pool(), rows * n, |chunk, pixels| {
        for (k, px) in pixels.iter_mut().enumerate() {
            let i = chunk * rows * n + k;
            *px = f(i % n, i / n);
        }
    });
    out
}

/// Run `f(row, pixels)` over each row of an `n × n` image, row-parallel.
pub fn par_rows<T: Send>(mut image: &mut [T], n: usize, f: impl Fn(usize, &mut [T]) + Sync) {
    image.par_chunk_map_mut(pool(), 8 * n, |chunk, rows| {
        for (k, row) in rows.chunks_mut(n).enumerate() {
            f(chunk * 8 + k, row);
        }
    });
}

pub fn fill_gray(n: usize, f: impl Fn(usize, usize) -> f32 + Sync) -> Buf {
    Buf::Gray(fill(n, f))
}

pub fn fill_color(n: usize, f: impl Fn(usize, usize) -> Vec3 + Sync) -> Buf {
    Buf::Color(fill(n, f))
}

/// A per-pixel filter over an input that keeps the input's channel count: a
/// gray input stays a gray buffer (a third of the memory), and a uniform one
/// stays uniform.
pub fn map_like(n: usize, input: In, f: impl Fn(Vec3, usize) -> Vec3 + Sync) -> Buf {
    if let Some(c) = input.uniform() {
        return Buf::Uniform(f(c, 0));
    }
    if input.is_gray() {
        fill_gray(n, |x, y| {
            let i = y * n + x;
            f(Vec3::splat(input.gray(i)), i).x
        })
    } else {
        fill_color(n, |x, y| {
            let i = y * n + x;
            f(input.color(i), i)
        })
    }
}

/// Resample through a per-pixel source position (pixel coordinates), keeping
/// the input's channel count: what warp, transform and slope blur share.
pub fn resample_like(n: usize, input: In, at: impl Fn(usize, usize) -> Vec2 + Sync) -> Buf {
    if let Some(c) = input.uniform() {
        return Buf::Uniform(c);
    }
    if input.is_gray() {
        fill_gray(n, |x, y| {
            let p = at(x, y);
            input.sample_gray(n, p.x, p.y)
        })
    } else {
        fill_color(n, |x, y| {
            let p = at(x, y);
            input.sample_color(n, p.x, p.y)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bilinear_wraps_and_hits_pixel_centers() {
        let buf = Buf::Gray(vec![0.0, 1.0, 1.0, 0.0]);
        let input = In::Buf(&buf);
        assert_eq!(
            input.sample_gray(2, 1.5, 0.5),
            1.0,
            "a pixel center is exact"
        );
        assert_eq!(input.sample_gray(2, 2.5, 0.5), 0.0, "x wraps to column 0");
        assert!((input.sample_gray(2, 1.0, 0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn map_like_keeps_gray_gray_and_uniform_uniform() {
        let gray = Buf::Gray(vec![0.25; 4]);
        assert!(matches!(map_like(2, In::Buf(&gray), |c, _| c * 2.0), Buf::Gray(v) if v[3] == 0.5));
        assert!(
            matches!(map_like(2, In::Gray(0.5), |c, _| c * 2.0), Buf::Uniform(c) if c.x == 1.0)
        );
    }
}
