//! The patch's wires. React says which sockets are wired (`patch.set`);
//! everything about how a wire looks and moves is Bevy's:
//!
//!   * **Where**: each end is a socket — a named node React renders — read
//!     from its laid-out transform every frame, so a wire follows its node
//!     through drags, the pop-in spring, the drag tilt and the arrange
//!     glide without React computing a single point.
//!   * **How it hangs**: a cubic bezier whose two control points ride
//!     damped springs toward where the ends want them, plus a little sag —
//!     a moved node whips its cables. The simulation runs in graph space,
//!     so panning and zooming move wires rigidly.
//!   * **Lifecycle**: a new wire grows out of its output; a wire dragged
//!     onto a socket keeps the cable the hand was holding and flashes; a
//!     removed or dropped one snaps back into its socket ([`Ghost`]).
//!   * **Drawing**: one shader under the UI ([`backdrop`]) draws them all
//!     with the dotted grid, signals flowing along them while the patch
//!     plays.

mod backdrop;

use std::collections::{HashMap, HashSet};

use bevy::math::Affine2;
use bevy::picking::pointer::{PointerId, PointerLocation};
use bevy::prelude::*;
use bevy::ui::{UiGlobalTransform, UiSystems};
use bevy_react::raster::parse_css_color;
use bevy_react::{ReactAppExt, ReactNodes, Request, react_message, react_request};
use serde::Deserialize;
use ts_rs::TS;

use backdrop::Frame;
pub use backdrop::MainCamera;

use crate::synth::Transport;
use crate::view::View;

/// What a wire carries — it picks how the signal flows along it.
#[derive(Deserialize, TS, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Signal {
    Audio,
    Cv,
    Gate,
}

/// A patched wire, from an output socket to an input socket. Sockets are
/// named nodes (`socketName` in `ui/src/graph/model.ts`).
#[derive(Deserialize, TS, Clone, PartialEq, Debug)]
pub struct Wire {
    pub id: String,
    pub from: String,
    pub to: String,
    pub signal: Signal,
    /// Any CSS color.
    pub color: String,
    /// On a selected node: drawn hotter.
    pub hot: bool,
}

/// The wire being pulled out of a socket.
#[derive(Deserialize, TS, Clone, PartialEq, Debug)]
pub struct Drag {
    /// The socket it comes out of.
    pub from: String,
    /// Whether `from` is an output (the free end is then the input end).
    pub output: bool,
    /// The socket the free end snapped to.
    pub to: Option<String>,
    /// Where the free end rests instead of the pointer (logical px), while
    /// a menu picks what to connect it to.
    pub at: Option<[f32; 2]>,
    pub signal: Signal,
    pub color: String,
}

/// React → Bevy: the whole patch, sent whenever it changes.
#[react_message(name = "patch.set")]
pub struct SetPatch {
    pub wires: Vec<Wire>,
    pub drag: Option<Drag>,
    /// Signals flow along the wires while the transport plays.
    pub playing: bool,
}

/// The pointer whose position a dragged wire follows (the mouse, unless
/// `--shoot` scripts its own).
#[derive(Resource)]
pub struct WirePointer(pub PointerId);

/// React → Bevy (request): where that pointer is (logical px), if over the
/// window — asked once when Shift+A opens the Add menu there, so nothing
/// tracks the pointer every frame.
#[react_request(name = "pointer.get", response = Option<[f32; 2]>)]
pub struct GetPointer;

pub struct WiresPlugin;

impl Plugin for WiresPlugin {
    fn build(&self, app: &mut App) {
        register_bindings(app);
        app.init_resource::<Patch>()
            .insert_resource(WirePointer(PointerId::Mouse))
            .add_plugins(backdrop::BackdropPlugin)
            // After layout (and the layout transitions it hands to
            // `PostLayout`): this frame's socket positions.
            .add_systems(PostUpdate, simulate.after(UiSystems::PostLayout));
    }
}

pub fn register_bindings(app: &mut App) {
    app.add_react_handler(on_set)
        .add_react_request_handler(on_get_pointer);
}

fn on_get_pointer(
    req: On<Request<GetPointer>>,
    pointers: Query<(&PointerId, &PointerLocation)>,
    which: Res<WirePointer>,
) {
    let at = pointers
        .iter()
        .find(|(id, _)| **id == which.0)
        .and_then(|(_, at)| at.location.as_ref())
        .map(|at| at.position.to_array());
    req.respond(at);
}

/// Spring constants of a wire's control points (per unit mass).
const STIFFNESS: f32 = 150.0;
const DAMPING: f32 = 13.0;
/// How long a dropped or removed wire takes to snap back, seconds.
const RETRACT: f32 = 0.3;
/// How long a new wire takes to grow, and the stagger between several.
const GROW: f32 = 0.55;
const GROW_STAGGER: f32 = 0.06;
/// How long the connect flash takes to run down the wire.
const FLASH: f32 = 0.45;
/// Wires past this many are not drawn (the shader's uniform array).
pub const MAX_WIRES: usize = 64;

/// One simulated cable. Everything is in graph space (logical px at zoom 1).
#[derive(Clone, Debug)]
struct Cable {
    /// The output end and the input end.
    ends: [Vec2; 2],
    ctrl: [Vec2; 2],
    vel: [Vec2; 2],
    /// Whether the cable has seen its ends yet.
    placed: bool,
    /// The drawn stretch, as curve parameters from the output end (0) to
    /// the input end (1).
    span: [f32; 2],
    /// Seconds into the grow animation (negative: still waiting).
    grow: Option<f32>,
    /// Seconds into the connect flash.
    flash: Option<f32>,
    /// Eases toward 1 while the wire is hot.
    hot: f32,
}

impl Cable {
    fn new() -> Self {
        Self {
            ends: [Vec2::ZERO; 2],
            ctrl: [Vec2::ZERO; 2],
            vel: [Vec2::ZERO; 2],
            placed: false,
            span: [0.0, 1.0],
            grow: None,
            flash: None,
            hot: 0.0,
        }
    }

    fn growing(delay: f32) -> Self {
        Self {
            span: [0.0, 0.0],
            grow: Some(-delay),
            ..Self::new()
        }
    }

    /// Move the ends and let the control points follow.
    fn settle(&mut self, ends: [Vec2; 2], dt: f32) {
        let rest = rest(ends);
        self.ends = ends;
        if !self.placed {
            self.ctrl = rest;
            self.placed = true;
            return;
        }
        let steps = (dt * 240.0).ceil().clamp(1.0, 8.0);
        let h = dt / steps;
        for _ in 0..steps as u32 {
            for ((ctrl, vel), rest) in self.ctrl.iter_mut().zip(&mut self.vel).zip(rest) {
                *vel += ((rest - *ctrl) * STIFFNESS - *vel * DAMPING) * h;
                *ctrl += *vel * h;
            }
        }
    }

    /// Advance the grow and flash animations.
    fn animate(&mut self, dt: f32) {
        if let Some(t) = &mut self.grow {
            *t += dt;
            let k = (*t / GROW).clamp(0.0, 1.0);
            self.span[1] = 1.0 - (1.0 - k).powi(3);
            if k >= 1.0 {
                self.grow = None;
            }
        }
        if let Some(t) = &mut self.flash {
            *t += dt;
            if *t > FLASH {
                self.flash = None;
            }
        }
    }
}

/// Where the control points of a wire between `ends` want to be: out of
/// the output to the right, into the input from the left, sagging a little
/// with length like a cable.
fn rest([out, inp]: [Vec2; 2]) -> [Vec2; 2] {
    let d = inp - out;
    let reach = (d.x.abs() * 0.5).clamp(40.0, 240.0) + (-d.x).max(0.0) * 0.2;
    let sag = Vec2::new(0.0, (d.length() * 0.08).min(70.0));
    [out + Vec2::X * reach + sag, inp - Vec2::X * reach + sag]
}

/// A wire on its way out: it retracts into the socket it hangs from.
#[derive(Debug)]
struct Ghost {
    cable: Cable,
    /// The sockets it was between, followed while they are still mounted.
    sockets: [Option<String>; 2],
    /// Which end it retracts into: `0` the output, `1` the input.
    into: usize,
    signal: Signal,
    color: LinearRgba,
    t: f32,
}

#[derive(Resource, Default)]
struct Patch {
    wires: Vec<Wire>,
    drag: Option<Drag>,
    playing: bool,
    cables: HashMap<String, Cable>,
    dragged: Option<Cable>,
    /// Where the dragged wire's free end is (graph space).
    tip: Option<Vec2>,
    ghosts: Vec<Ghost>,
    /// Seconds of signal flow (runs while playing, eases to a stop).
    clock: f32,
    /// 1 while playing, easing to 0 when paused.
    play: f32,
}

impl Drag {
    /// The sockets at the output and the input end, as far as known.
    fn sockets(&self) -> [Option<&String>; 2] {
        if self.output {
            [Some(&self.from), self.to.as_ref()]
        } else {
            [self.to.as_ref(), Some(&self.from)]
        }
    }
}

impl Patch {
    /// Take in a new patch: grow the new wires, retract the removed ones,
    /// and hand the dragged cable over when a drag lands or picks a wire
    /// up.
    fn apply(&mut self, next: &SetPatch) {
        let old_drag = self.drag.take();
        let mut dragged = self.dragged.take();
        let kept: HashSet<&str> = next.wires.iter().map(|w| w.id.as_str()).collect();
        for wire in std::mem::take(&mut self.wires) {
            if kept.contains(wire.id.as_str()) {
                continue;
            }
            let Some(cable) = self.cables.remove(&wire.id) else {
                continue;
            };
            // Its input end was grabbed: the new drag is this very cable.
            let picked_up = old_drag.is_none()
                && dragged.is_none()
                && next
                    .drag
                    .as_ref()
                    .is_some_and(|d| d.output && d.from == wire.from);
            if picked_up {
                // Whole in the hand, even if it was still growing.
                dragged = Some(Cable {
                    span: [0.0, 1.0],
                    grow: None,
                    ..cable
                });
                continue;
            }
            self.ghosts.push(Ghost {
                cable,
                into: 0,
                signal: wire.signal,
                color: color(&wire.color),
                sockets: [Some(wire.from), Some(wire.to)],
                t: 0.0,
            });
        }

        let mut stagger = 0.0;
        for wire in &next.wires {
            if self.cables.contains_key(&wire.id) {
                continue;
            }
            // A drag that ended on this wire's socket: keep the cable.
            let landed = next.drag.is_none()
                && old_drag.as_ref().is_some_and(|d| {
                    let [out, inp] = d.sockets();
                    if d.output {
                        out == Some(&wire.from)
                    } else {
                        inp == Some(&wire.to)
                    }
                });
            let cable = match dragged.take() {
                Some(mut cable) if landed => {
                    cable.flash = Some(0.0);
                    cable
                }
                other => {
                    dragged = other;
                    stagger += GROW_STAGGER;
                    Cable::growing(stagger - GROW_STAGGER)
                }
            };
            self.cables.insert(wire.id.clone(), cable);
        }

        match (&old_drag, &next.drag) {
            // Dropped on nothing: back into the socket it came from.
            (Some(drag), None) => {
                if let Some(cable) = dragged.take() {
                    self.ghosts.push(Ghost {
                        cable,
                        into: if drag.output { 0 } else { 1 },
                        signal: drag.signal,
                        color: color(&drag.color),
                        sockets: drag.sockets().map(|s| s.cloned()),
                        t: 0.0,
                    });
                }
                self.tip = None;
            }
            (_, Some(_)) => {
                dragged.get_or_insert_with(Cable::new);
            }
            (None, None) => {}
        }
        self.dragged = dragged;
        self.wires = next.wires.clone();
        self.drag = next.drag.clone();
        self.playing = next.playing;
    }
}

fn on_set(set: On<SetPatch>, mut patch: ResMut<Patch>) {
    patch.apply(set.event());
}

fn color(css: &str) -> LinearRgba {
    parse_css_color(css).unwrap_or(Srgba::WHITE).into()
}

#[allow(clippy::too_many_arguments)]
fn simulate(
    time: Res<Time>,
    mut patch: ResMut<Patch>,
    view: Res<View>,
    nodes: ReactNodes,
    globals: Query<&UiGlobalTransform>,
    pointers: Query<(&PointerId, &PointerLocation)>,
    which: Res<WirePointer>,
    camera: Single<&Camera, With<MainCamera>>,
    transport: Res<Transport>,
    mut backdrop: backdrop::Writer,
) {
    let dt = time.delta_secs().min(1.0 / 20.0);
    let scale = camera.target_scaling_factor().unwrap_or(1.0);
    let View { current, .. } = *view;
    // Graph space → physical px, and back.
    let to_screen = Affine2::from_scale_angle_translation(
        Vec2::splat(current.zoom * scale),
        0.0,
        current.pan * scale,
    );
    let to_graph = to_screen.inverse();
    // Where a socket is this frame, in graph space.
    let socket = |name: &str| {
        let global = globals.get(nodes.get(name)?).ok()?;
        Some(to_graph.transform_point2(global.translation))
    };
    let pointer = pointers
        .iter()
        .find(|(id, _)| **id == which.0)
        .and_then(|(_, at)| at.location.as_ref())
        .map(|at| at.position * scale);

    let patch = &mut *patch;
    let target = if patch.playing { 1.0 } else { 0.0 };
    patch.play += (target - patch.play) * (1.0 - (-dt / 0.25).exp());
    patch.clock += dt * patch.play;

    let mut out = Vec::with_capacity(patch.wires.len() + 2);
    // Cool wires first, hot ones over them.
    for hot in [false, true] {
        for wire in patch.wires.iter().filter(|w| w.hot == hot) {
            let (Some(a), Some(b)) = (socket(&wire.from), socket(&wire.to)) else {
                continue;
            };
            let Some(cable) = patch.cables.get_mut(&wire.id) else {
                continue;
            };
            cable.settle([a, b], dt);
            cable.animate(dt);
            cable.hot += ((hot as u8 as f32) - cable.hot) * (1.0 - (-dt / 0.12).exp());
            out.push(backdrop::wire(
                cable,
                wire.signal,
                color(&wire.color),
                1.0,
                &to_screen,
            ));
        }
    }

    patch.ghosts.retain_mut(|ghost| {
        ghost.t += dt;
        let k = (ghost.t / RETRACT).min(1.0);
        let ends = [0, 1].map(|i| {
            ghost.sockets[i]
                .as_deref()
                .and_then(socket)
                .unwrap_or(ghost.cable.ends[i])
        });
        let cable = &mut ghost.cable;
        cable.settle(ends, dt);
        // Ease-in: it lets go, then snaps home.
        let left = 1.0 - k * k;
        cable.span = if ghost.into == 0 {
            [0.0, cable.span[1].min(left)]
        } else {
            [cable.span[0].max(1.0 - left), 1.0]
        };
        out.push(backdrop::wire(
            cable,
            ghost.signal,
            ghost.color,
            1.0 - k * 0.5,
            &to_screen,
        ));
        k < 1.0
    });

    if let (Some(drag), Some(cable)) = (&patch.drag, &mut patch.dragged) {
        let free = drag
            .to
            .as_deref()
            .and_then(socket)
            .or_else(|| {
                drag.at
                    .map(|[x, y]| to_graph.transform_point2(Vec2::new(x, y) * scale))
            })
            .or_else(|| pointer.map(|p| to_graph.transform_point2(p)));
        if let (Some(origin), Some(free)) = (socket(&drag.from), free) {
            // The free end follows the hand exactly, and glides onto a
            // socket it snaps to.
            let tip = match (patch.tip, &drag.to) {
                (Some(tip), Some(_)) => tip.lerp(free, 1.0 - (-dt / 0.035).exp()),
                _ => free,
            };
            patch.tip = Some(tip);
            let ends = if drag.output {
                [origin, tip]
            } else {
                [tip, origin]
            };
            cable.settle(ends, dt);
            cable.animate(dt);
            cable.hot = if drag.to.is_some() { 1.0 } else { 0.4 };
            out.push(backdrop::wire(
                cable,
                drag.signal,
                color(&drag.color),
                1.0,
                &to_screen,
            ));
        }
    }

    if out.len() > MAX_WIRES {
        warn_once!("more than {MAX_WIRES} wires: the rest are not drawn");
        out.truncate(MAX_WIRES);
    }
    let pointer = pointer.unwrap_or(Vec2::splat(-1.0e6));
    backdrop.write(
        Frame {
            view: (current.pan * scale)
                .extend(current.zoom * scale)
                .extend(scale),
            pointer: pointer.extend(out.len() as f32).extend(patch.play),
            time: Vec4::new(patch.clock, transport.steps, 0.0, 0.0),
        },
        &out,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire(id: &str, from: &str, to: &str) -> Wire {
        Wire {
            id: id.into(),
            from: from.into(),
            to: to.into(),
            signal: Signal::Audio,
            color: "#fff".into(),
            hot: false,
        }
    }

    fn drag(from: &str, output: bool, to: Option<&str>) -> Drag {
        Drag {
            from: from.into(),
            output,
            to: to.map(Into::into),
            at: None,
            signal: Signal::Audio,
            color: "#fff".into(),
        }
    }

    fn set(wires: Vec<Wire>, drag: Option<Drag>) -> SetPatch {
        SetPatch {
            wires,
            drag,
            playing: false,
        }
    }

    /// Pulling a wire off its input hands the cable to the drag; dropping
    /// it on another input hands it back, flashing; the patch never shows a
    /// wire growing or retracting on the way.
    #[test]
    fn a_rerouted_wire_keeps_its_cable() {
        let mut patch = Patch::default();
        patch.apply(&set(vec![wire("w", "a.o0", "b.i0")], None));
        patch.cables.get_mut("w").unwrap().ctrl = [Vec2::splat(7.0); 2];

        patch.apply(&set(vec![], Some(drag("a.o0", true, None))));
        assert!(patch.ghosts.is_empty());
        assert_eq!(patch.dragged.as_ref().unwrap().ctrl, [Vec2::splat(7.0); 2]);

        patch.apply(&set(vec![], Some(drag("a.o0", true, Some("c.i0")))));
        patch.apply(&set(vec![wire("w2", "a.o0", "c.i0")], None));
        let landed = &patch.cables["w2"];
        assert_eq!(landed.ctrl, [Vec2::splat(7.0); 2]);
        assert!(landed.flash.is_some() && landed.grow.is_none());
        assert!(patch.dragged.is_none() && patch.ghosts.is_empty());
    }

    /// A drag dropped on nothing retracts into its socket; a removed wire
    /// retracts into its output; wires that appear on their own grow, one
    /// after another.
    #[test]
    fn wires_come_and_go_animated() {
        let mut patch = Patch::default();
        patch.apply(&set(vec![], Some(drag("b.i0", false, None))));
        patch.apply(&set(vec![], None));
        assert_eq!(patch.ghosts.len(), 1);
        assert_eq!(patch.ghosts[0].into, 1);

        patch.apply(&set(
            vec![wire("x", "a.o0", "b.i0"), wire("y", "a.o0", "c.i0")],
            None,
        ));
        assert_eq!(patch.cables["x"].grow, Some(0.0));
        assert_eq!(patch.cables["y"].grow, Some(-GROW_STAGGER));

        patch.apply(&set(vec![wire("y", "a.o0", "c.i0")], None));
        assert_eq!(patch.ghosts.len(), 2);
        assert_eq!(patch.ghosts[1].into, 0);
    }
}
