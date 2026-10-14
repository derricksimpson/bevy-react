//! The graph's pan and zoom. React owns where the view should be
//! (`view.set`); Bevy glides the visible view there.
//!
//! Text stays crisp at every zoom because nothing is scaled at rest: every
//! length inside the graph is in `rem`, so a zoom is a new [`RemSize`] — one
//! relayout, text shaped again at its new size. While the visible view is
//! still gliding toward the target, the difference rides the graph root's
//! `UiTransform` (a pan and a scale, no relayout), so the zoom eases
//! smoothly and the root's transform is the identity once it lands.

use bevy::prelude::*;
use bevy::text::RemSize;
use bevy::ui::Val2;
use bevy_react::{ReactAppExt, ReactApplySet, ReactEvents, ReactNodes, react_event, react_message};

/// `RemSize` at zoom 1, logical px: the graph's lengths are written in rem
/// of this (`REM` in `ui/src/theme.ts`).
pub const REM: f32 = 16.0;

/// The named node every graph node is laid out in (`GRAPH_ROOT` in
/// `ui/src/graph/Canvas.tsx`): an empty box at the window's top-left whose
/// `UiTransform` this module owns.
const ROOT: &str = "graph-root";

/// React → Bevy: where the view should be. A world point `w` (logical px at
/// zoom 1) shows at `(x, y) + w * zoom` in the window.
#[react_message(name = "view.set")]
pub struct SetView {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
    /// How long the glide takes, ms; `0` snaps (a drag pans with the hand).
    pub glide: f32,
}

/// Bevy → React: W A S D held this frame pan the view by `(x, y)` window
/// px. React owns the view, so it moves it; Bevy only reads the keys
/// (`ButtonInput` lets go of them when the window loses focus).
#[react_event(name = "view.pan")]
pub struct PanView {
    pub x: f32,
    pub y: f32,
}

/// The keys' pan speed, window px per second at any zoom.
const KEY_PAN: f32 = 900.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Xf {
    pub pan: Vec2,
    pub zoom: f32,
}

#[derive(Resource)]
pub struct View {
    /// Where React wants the view; `RemSize` follows its zoom.
    pub target: Xf,
    /// What is on screen this frame.
    pub current: Xf,
    /// The glide's time constant, seconds.
    tau: f32,
}

pub struct ViewPlugin;

impl Plugin for ViewPlugin {
    fn build(&self, app: &mut App) {
        register_bindings(app);
        let rest = Xf {
            pan: Vec2::ZERO,
            zoom: 1.0,
        };
        app.insert_resource(RemSize(REM))
            .insert_resource(View {
                target: rest,
                current: rest,
                tau: 0.0,
            })
            // Before layout reads the root's transform, after this frame's
            // ops mounted it.
            .add_systems(Update, (glide.after(ReactApplySet), key_pan));
    }
}

pub fn register_bindings(app: &mut App) {
    app.add_react_handler(on_set).add_react_event::<PanView>();
}

/// The camera moves with the key — W shows what is above — so the graph
/// moves against it. Off under a modifier: Ctrl+A, Ctrl+D and Shift+A are
/// shortcuts.
fn key_pan(time: Res<Time>, keys: Res<ButtonInput<KeyCode>>, events: ReactEvents) {
    use KeyCode::*;
    let modifiers = [
        ShiftLeft,
        ShiftRight,
        ControlLeft,
        ControlRight,
        AltLeft,
        AltRight,
        SuperLeft,
        SuperRight,
    ];
    if keys.any_pressed(modifiers) {
        return;
    }
    let axis = |pos, neg| keys.pressed(pos) as i8 as f32 - keys.pressed(neg) as i8 as f32;
    let dir = Vec2::new(axis(KeyA, KeyD), axis(KeyW, KeyS));
    if dir != Vec2::ZERO {
        let d = dir.normalize() * KEY_PAN * time.delta_secs();
        events.send(&PanView { x: d.x, y: d.y });
    }
}

fn on_set(set: On<SetView>, mut view: ResMut<View>, mut rem: ResMut<RemSize>) {
    let e = set.event();
    let target = Xf {
        pan: Vec2::new(e.x, e.y),
        zoom: e.zoom.clamp(0.05, 20.0),
    };
    view.target = target;
    // Most of the way there in `glide` ms (three time constants).
    view.tau = e.glide.max(0.0) / 3000.0;
    if view.tau == 0.0 {
        view.current = target;
    }
    rem.set_if_neq(RemSize(REM * target.zoom));
}

fn glide(
    time: Res<Time>,
    mut view: ResMut<View>,
    nodes: ReactNodes,
    mut transforms: Query<&mut UiTransform>,
) {
    if view.current != view.target {
        let k = 1.0 - (-time.delta_secs() / view.tau.max(1e-4)).exp();
        view.current = approach(view.current, view.target, k);
    }
    let Some(mut transform) = nodes.get(ROOT).and_then(|e| transforms.get_mut(e).ok()) else {
        return;
    };
    let View {
        current, target, ..
    } = *view;
    transform.set_if_neq(UiTransform {
        translation: Val2::px(current.pan.x, current.pan.y),
        scale: Vec2::splat(current.zoom / target.zoom),
        rotation: Rot2::IDENTITY,
    });
}

/// A fraction `k` of the way from `a` to `b`, zooming about the one screen
/// point both views agree on — what a wheel zoom anchors on the cursor, a
/// "frame all" on wherever the two framings meet — so no point drifts
/// sideways while the zoom moves. Snaps once the rest is invisible.
fn approach(a: Xf, b: Xf, k: f32) -> Xf {
    let (za, zb) = (a.zoom as f64, b.zoom as f64);
    let (pa, pb) = (a.pan.as_dvec2(), b.pan.as_dvec2());
    if (zb / za - 1.0).abs() < 1e-4 && pa.distance(pb) < 0.05 {
        return b;
    }
    if (zb / za).ln().abs() < 1e-3 {
        let pan = pa.lerp(pb, k as f64);
        return Xf {
            pan: pan.as_vec2(),
            zoom: (za + (zb - za) * k as f64) as f32,
        };
    }
    // pa + w·za = pb + w·zb: the world point `w` sits at the same screen
    // point in both views.
    let w = (pa - pb) / (zb - za);
    let fixed = pa + w * za;
    let zoom = (za.ln() + (zb / za).ln() * k as f64).exp();
    Xf {
        pan: (fixed - w * zoom).as_vec2(),
        zoom: zoom as f32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(v: Xf, w: Vec2) -> Vec2 {
        v.pan + w * v.zoom
    }

    /// A wheel zoom about the cursor keeps the point under it still at every
    /// step of the glide, and the glide lands exactly.
    #[test]
    fn zoom_glide_keeps_the_anchor_still() {
        let cursor = Vec2::new(300.0, 200.0);
        let a = Xf {
            pan: Vec2::new(40.0, 10.0),
            zoom: 1.0,
        };
        let world = (cursor - a.pan) / a.zoom;
        let b = Xf {
            zoom: 1.5,
            pan: cursor - world * 1.5,
        };
        let mut v = a;
        for _ in 0..40 {
            v = approach(v, b, 0.2);
            assert!(screen(v, world).distance(cursor) < 1e-2);
        }
        assert_eq!(v, b);
    }

    /// A pure pan glides in a straight line.
    #[test]
    fn pan_glide_is_straight() {
        let a = Xf {
            pan: Vec2::ZERO,
            zoom: 1.0,
        };
        let b = Xf {
            pan: Vec2::new(100.0, 50.0),
            zoom: 1.0,
        };
        let half = approach(a, b, 0.5);
        assert!(half.pan.distance(Vec2::new(50.0, 25.0)) < 1e-3);
        assert_eq!(half.zoom, 1.0);
    }
}
