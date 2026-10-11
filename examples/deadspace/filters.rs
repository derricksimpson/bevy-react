//! The holograms' own WGSL effects over React's pixels, each a params struct
//! and a shader under `assets/shaders/`, typed into `bevy.ts` like every
//! other binding. They run inside the `<surface>`s, on each surface's own
//! camera.
//!
//!   * `holoNoise` — the projector's imperfection over a whole hologram:
//!     fine noise, a slow interference band rolling down, a row now and then
//!     knocked sideways. Time-driven.
//!   * `glitch` — the signal breaking up when the suit is hit or an action
//!     is refused: torn slices and split channels. React arms it for a
//!     moment (`intensity`).
//!   * `holoStatic` — the dip between two fixtures: an empty full-screen
//!     node whose pass writes a screen of hologram static.
//!   * `scanSwap` (morph) — every tab and page change inside a hologram: a
//!     scan line sweeps down, the new page drawn above it.
//!
//! Each shader's header documents its `params[i]` packing.

use bevy::prelude::*;
use bevy_react::filters::FilterColor;
use bevy_react::protocol::units::Length;
use bevy_react::{ReactAppExt, react_filter, react_morph_filter};

/// Packing: `params[0]` = (amount, band, jitter px, seed).
///
/// Jittered rows move up to `jitter` px sideways, so the capture is inflated
/// by `outset` (keep `jitter` within it).
#[react_filter(shader = "shaders/holo_noise.wgsl", time = true, outset = 8.0)]
struct HoloNoise {
    /// How visible the grain is, 0..1.
    #[serde(default = "default_amount")]
    amount: f32,
    /// How bright the rolling interference band is, 0..1.
    #[serde(default = "default_band")]
    band: f32,
    /// How far a knocked row moves, px (≤ 8).
    #[serde(default = "default_jitter")]
    jitter: Length,
    /// Decorrelates holograms side by side.
    #[serde(default)]
    seed: f32,
}

/// Packing: `params[0]` = (intensity, frequency, split, tear),
/// `params[1].x` = seed. (The same shader as the cyberpunk example's.)
#[react_filter(shader = "shaders/glitch.wgsl", time = true, outset = 32.0)]
struct Glitch {
    /// How broken the signal is, 0..1.
    #[serde(default = "one")]
    intensity: f32,
    /// 0 = glitching all the time; above 0, the chance a quarter second
    /// glitches (bursts).
    #[serde(default)]
    frequency: f32,
    /// How far the channels split at full intensity, px.
    #[serde(default = "default_split")]
    split: Length,
    /// How far a torn slice moves at full intensity, px (≤ 32).
    #[serde(default = "default_tear")]
    tear: Length,
    /// Decorrelates glitches that run side by side.
    #[serde(default)]
    seed: f32,
}

/// Packing: `params[0]` = tint (straight linear RGBA; alpha = coverage).
///
/// Sits on an empty full-screen node: the pass writes static over the frame,
/// opaque by default — the camera cuts under it.
#[react_filter(shader = "shaders/holo_static.wgsl", time = true)]
struct HoloStatic {
    #[serde(default = "opaque_cyan")]
    tint: FilterColor,
}

/// Packing: `params[0].x` = softness (0..1 of the height),
/// `params[1]` = tint (straight linear RGBA; alpha = how bright the line).
#[react_morph_filter(shader = "shaders/scan_swap.wgsl")]
struct ScanSwap {
    #[serde(default = "default_softness")]
    softness: f32,
    #[serde(default = "cyan")]
    tint: FilterColor,
}

fn one() -> f32 {
    1.0
}

fn default_amount() -> f32 {
    0.25
}

fn default_band() -> f32 {
    0.35
}

fn default_jitter() -> Length {
    Length::Px(3.0)
}

fn default_split() -> Length {
    Length::Px(4.0)
}

fn default_tear() -> Length {
    Length::Px(18.0)
}

fn default_softness() -> f32 {
    0.06
}

fn cyan() -> FilterColor {
    FilterColor([0.5, 0.89, 1.0, 0.8])
}

fn opaque_cyan() -> FilterColor {
    FilterColor([0.5, 0.89, 1.0, 1.0])
}

/// Register the filters — in the running app and the exporter alike, so the
/// generated typing matches what resolves at runtime.
pub fn register_bindings(app: &mut App) {
    app.add_react_filter::<HoloNoise>()
        .add_react_filter::<Glitch>()
        .add_react_filter::<HoloStatic>()
        .add_react_morph_filter::<ScanSwap>();
}
