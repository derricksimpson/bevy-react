# Plan — retire `bevy::ui::Interaction`; one picking-driven pointer path; a drag owns its click

Audit sections: §1.3 (deprecations), §2.5 (pointer capture), §3.8 (retire
`Interaction`), plus the known bug "drag release clicks the ancestor".

Written against `main` at `4ad1b6d` **plus an uncommitted working tree** that
another agent is landing while this was read (it deletes `pick_clip.rs` and
already moved `cursor.rs` onto the `HoverMap` — audit §3.9 is done there).
Line numbers for our files are "at the time of writing"; symbols are the
anchor. Nothing here was compiled or run: every Bevy claim is from reading the
0.20 source, every claim about our behavior from reading ours.

**The design in one paragraph.** A bevy-react-owned component
`ReactInteraction { None, Hovered, Pressed }` replaces `Interaction`. One small
system derives it each frame for _every_ pointer (mouse, touches, the
transform3d pointer, `<surface>` pointers, a mesh backend's hits) from two
stock resources — `HoverMap` (who is under each pointer) and
`PointerState.pressing` (who each pointer's primary button is holding) — in
O(hover stack), not O(nodes). The hover and drag collectors become
pointer-driven too (picking messages + `PointerLocation`), which deletes
`virtual_events.rs`, pick3d's `correct_transformed_interactions`, svg's and the
cube's `Interaction` synthesis, the `RelativeCursorPosition` dependency and the
touch-attribution hack. The click bug is fixed by one rule — _while an element
is being dragged, that pointer's click can only go to it or its descendants_ —
and **not** by `PointerCaptureMap`, which turns out to be the wrong tool for it
(§1.3, §2.8).

---

## 0. Today's picture

### Who produces hover/press state

| Producer                                            | Where                                 | Covers                                                                                                                    | Cost                                                                                            |
| --------------------------------------------------- | ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| bevy `ui_focus_system`                              | `PreUpdate`, `UiSystems::Focus`       | `Interaction` + `RelativeCursorPosition` (RCP) on window-camera nodes, from ONE cursor position (mouse, else first touch) | walks the whole `UiStack` every frame                                                           |
| `layer::pick3d::correct_transformed_interactions`   | `Update`, before `InteractionSyncSet` | overrides both for members of visually-transformed layers, from `HoverMap[transform3d pointer]`                           | iterates every `Interaction` entity (× a `ChildOf` climb) whenever one transformed layer exists |
| `bevy_react_svg::interact::sync_shape_interactions` | `InteractionSyncSet`                  | `Interaction` + RCP + `EventLocalPos` on handler-bearing shapes                                                           | per shape                                                                                       |
| `examples/demos/cube::sync_cube_interactions`       | `InteractionSyncSet`                  | `Interaction` on cubes, from the `HoverMap`                                                                               | per cube                                                                                        |
| _(nothing)_                                         | —                                     | `<surface>` UI: no state at all; everything is message-driven in `reconcile/virtual_events.rs`                            | —                                                                                               |

`Interaction` is inserted by `reconcile/stamps.rs` (`apply_style_variants` for
`hoverStyle`/`pressStyle`; `apply_pointer_handlers` for `onClick`/`onPointer*`;
removed only in `node_less_pointer_slot`), by `elements/node.rs` (`BUTTON`'s
spawn) and `elements/editable.rs` (`spawn_editable`). RCP is inserted with
`PointerHandlers`.

### Who consumes it

| Consumer                                                   | Reads                                                                                                                                            | For                                                                                                                      |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------ |
| `reconcile/interaction.rs::apply_interaction_styles`       | `Changed<Interaction>`                                                                                                                           | hover/press restyle (window UI)                                                                                          |
| `reconcile/hover.rs::collect_hover_events`                 | `Changed<Interaction>` + `HoverState`; RCP; window cursor                                                                                        | `pointerEnter/Leave/Hover` (window UI)                                                                                   |
| `reconcile/pointer.rs::collect_pointer_events`             | `Interaction::Pressed` (left / touch), `!= None && rel.cursor_over()` (other buttons), RCP, `ButtonInput<MouseButton>`, `Touches`, window cursor | drag begin/move/end; `PointerCapture::over_ui` via `interactions.iter().any(..)` over **every** `Interaction` each frame |
| `reconcile/virtual_events.rs` (4 systems, 11 reader loops) | `PointerClick/Press/Release/Drag/Enter/Leave/Move` filtered to `VirtualPointers`                                                                 | the same four jobs for `<surface>` UI (and the demos' `--shoot` scripted pointer)                                        |
| `reconcile/events.rs::collect_ui_events`                   | `PointerClick` + `ClickOwners` (never `Interaction`)                                                                                             | clicks                                                                                                                   |
| `PointerCapture` (resource)                                | —                                                                                                                                                | `surface::drive_surface_pointer` (`over_ui`), the example cameras, `civilization/map/pick.rs`                            |
| `cursor.rs`                                                | `HoverMap` (after the in-flight change)                                                                                                          | cursor icon                                                                                                              |
| `touch_scroll.rs`                                          | `ActiveDrag::touch_id()`, `TouchScrollState::stole_drag`                                                                                         | contested touch claims — no `Interaction`                                                                                |
| `examples/demos/scenes/named_pins.rs::place_pins`          | `Option<&Interaction>`                                                                                                                           | pin glow (the documented "read a React node's hover from Bevy" pattern, `docs/guide/communication/named-nodes.md`)       |
| `examples/demos/cube/mod.rs::sync_cube_pickable`           | `With<Interaction>` as "has pointer handlers"                                                                                                    | adds/removes `Pickable`                                                                                                  |

Counts now: 126 non-comment lines mention `Interaction` in 20 files; production
code in **10** files (the audit's 9 + `elements/node.rs`, added by `7b8721c`);
the rest are tests (`reconcile/{pointer,hover,interaction,stamps,events}.rs`,
`reconcile/update/tests.rs`, `style/{ext_tests,invalidation_tests,writers/tests}.rs`,
`transition/gradient_tests.rs`, `filters/transition.rs`, `layer/pick3d.rs`,
`svg/{interact,op_tests,op_pointer_tests}.rs`, `demos/cube/tests.rs`). No
`#[allow(deprecated)]` exists, so each use warns today.

---

## 1. Verified facts

Registry root: `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`.

### 1.1 What `ui_focus_system` did for us (bevy_ui-0.20.0 `src/focus.rs`)

- `Interaction` is a deprecated alias of `DeprecatedInteraction` (`:50`, `:61-65`,
  "TODO this should be removed in 0.20"); `ui_focus_system` carries the same
  removal note (`:163-166`). Still registered: `lib.rs:183-186`, alone in
  `UiSystems::Focus` (other crates only order `.after` it).
- It walks the **whole** `UiStack` every frame, `node_query.get_mut` +
  `contains_point` + `normalize_point` per node (`:238-319`), and on a release
  iterates every node again (`:191-201`). It runs whether or not any node has
  an `Interaction` — we cannot remove it from `DefaultPlugins`.
- One cursor per camera: the window's, else the first pressed touch
  (`:206-230`); only cameras rendering to a window (`:209-214`).
- Hovered = cursor inside the node and its inherited `CalculatedClip`
  (`:275-280`), visible (`:261-273`), top-down until the first
  `FocusPolicy::Block`, an **absent** policy counting as Block (`:321-348`).
- Pressed on left-press / any touch-press of a hovered node (`:203-204`,
  `:327-336`); **stays Pressed after the pointer leaves** until left-release /
  any touch-release (`:191-201`, `:310-316`, `:355`). (Exception: cursor
  outside the window → `normalized == None` → reset even if Pressed, `:311-314`.)
- `RelativeCursorPosition` is written **only** here (`:294-305`), for every
  node that has one, each frame the cursor moves. Nothing else in Bevy 0.20
  writes it — it dies with this system.
- `FocusPolicy` itself is not deprecated and is a required component of `Node`
  (`ui_node.rs:472`).

### 1.2 What bevy_picking gives us instead

`bevy_ui-0.20.0/src/picking_backend.rs` (`ui_picking`, `PickingSystems::Backend`):

- Same geometry as the focus system: `contains_point` + inherited clip
  (`:205-208`), hidden nodes skipped (`:188-194`), topmost first, stops at the
  first node whose `Pickable.should_block_lower` is set — an **absent**
  `Pickable` blocks (`:249-266`). Depth 0 for the topmost, `+0.00001` per node
  beneath (`:247`, `:265`). Every pointer whose target matches a UI camera is
  tested (`:112-146`): mouse, each touch, custom pointers on windows or images.
- **Text hits target the section entity** — the `TextSpan` under the pointer
  — not the text node (`:209-226`, `pick_ui_text_section` `:278-297`; same in
  0.19). The _node's_ `Pickable` still decides whether the backend goes on
  (`:234`, `:255-263`).

`bevy_picking-0.20.0/src/hover.rs`:

- `HoverMap: HashMap<PointerId, EntityHashMap<HitData>>` (`:64`) — documented
  as the authoritative state to build on (`:52-58`). Rebuilt each frame;
  `build_hover_map` re-applies blocking per **hit entity**, and an entity
  without `Pickable` is inserted and **stops the walk** (`:265-291`). So a
  text span (ours have no `Pickable`) hides everything beneath a glyph run —
  the `<text>` block itself, a `<button>` around it, a slider track under its
  label. This is why the click collectors climb from the hit, and why
  hover-by-membership needs §2.2.
- `PickingInteraction` (`:293-312`) is **not** a replacement: it is `Pressed`
  whenever the hovering pointer has _any_ button down (`:374-377`) — not
  sticky, lights everything a held button is dragged across, and merges all
  pointers. `Hovered`/`DirectlyHovered` are mouse-only (`:470`, `:500`). `Pressed`
  (bevy_ui) is inserted only by the widget `Button` observers
  (`bevy_ui_widgets-0.20.0/src/button.rs:78-98`). The audit's verdict stands.
- Systems, chained in `PickingSystems::Hover` (`PreUpdate`):
  `generate_hovermap → update_interactions → (update_is_hovered, …) →
pointer_events` (`lib.rs:446-455`).

`bevy_picking-0.20.0/src/events.rs`:

- `PointerState` is a public resource (`:697-704`):
  `pointer_buttons: HashMap<(PointerId, PointerButton), PointerButtonState>`,
  with `pressing: EntityHashMap<(Location, Instant, HitData)>` (`:622-631`).
  `pressing` gains every entity hovered at the press (`:1053-1084`) and is
  cleared on that button's release (`:1163-1164`) or the pointer's cancel
  (`:1309-1310`). **That is exactly "pressed until release even when the
  pointer leaves"**, per pointer and button, maintained for us.
- `PointerClick` and `PointerRelease` go to the entities of the **previous**
  hover map (`:805-809`, `:1092-1095`); a Click needs the entity in `pressing`
  (`:1098`).
- `PointerOver`/`PointerOut` are exact `HoverMap` membership edges per
  (pointer, entity) (`:847-874`, `:977-1032`); `PointerEnter`/`PointerLeave`
  add ancestors (CSS `:hover` semantics, `:876-913`, `:983-1022`).
- `PointerDrag` follows the entities pressed, wherever the pointer is
  (`:1167-1235`); `PointerMove` goes to the hover map (`:1257-1271`).
- `trait PointerEvent: EntityEvent { fn pointer(&self) -> &Pointer; }`
  (`:127-130`) — the pointer only. `button`, `hit` and the per-event payloads
  are not on it.

`bevy_picking-0.20.0/src/{input,pointer}.rs`:

- Touch pointers are spawned on `TouchPhase::Started` in `First`
  (`input.rs:227-237`) and despawned in `Last` after Ended/Canceled
  (`:280-300`); a touch press is `Press(Primary)`.
- `PointerLocation` changes only on a `Move` (`pointer.rs:358-364`).
- **No `CursorLeft` handling** (`input.rs:130-199` handles `CursorMoved`,
  `MouseButtonInput`, `MouseWheel` only): the mouse pointer keeps its last
  location when the cursor leaves the window, so `ui_picking` keeps hitting
  and the `HoverMap` keeps the last node hovered. `Interaction` did not have
  this (`Window::physical_cursor_position` is `None` outside,
  `bevy_window-0.20.0/src/window.rs:631-646`). Needs a runtime check, but the
  design guards it either way (§2.3).

### 1.3 `PointerCaptureMap` (bevy_picking `hover.rs`) — and why capture alone does not fix the click bug

- `PointerCaptureMap(HashMap<PointerId, (Entity, HitData)>)` (`:84-109`):
  `capture(pointer, entity, hit)`, `release`, `get`, `is_captured`. Any
  `PointerId` works, custom (virtual) ones included.
- Effect: after the real hover map is built, a captured pointer's entry is
  **replaced by that single entity** with the stored hit (`:186-194`, called
  at `:167`). Everything else that pointer was over drops out: `PointerOut`
  for them, `PointerLeave` for those that are not ancestors of the captured
  entity, no `PointerMove`/hover there, and mesh/window hits are gone too.
- Who releases it: bevy, at the **start** of `generate_hovermap` on the frame
  that pointer reports `Release(_)` of **any** button or `Cancel`
  (`:161-164`, `:172-184`), and when the pointer entity disappears
  (`:216-222`). So on the release frame the current hover map is physical
  again, while `PreviousHoverMap` is still the captured one.
- The one stock caller is the widget slider, which captures **itself** on
  `PointerDragStart` and releases on drag end
  (`bevy_ui_widgets-0.20.0/src/slider.rs:381-399`; `slider_on_drag_end` `:446`,
  release at `:491`).

**Audit §2.5 is wrong about the fix** ("call `capture` when a drag begins …
hover and click stay on the dragged node. This fixes the bug"). Combine the
facts: Click = `pressing` ∩ _previous_ hover map, and a captured pointer's
previous hover map is the captured entity alone. Therefore, after one captured
frame:

1. **A click-owning descendant of the captured node can never be clicked.**
   Every demo `Button` is `Pinchable`'s press surface (`onPointerDown/Move/Up/
Leave`, the drag owner) around `<button onClick>` (pass-through), around a
   `<text>`. Capture the surface and only `Click(surface)` is ever emitted —
   the button's `onClick` is dead for any press held ≥ 2 frames.
2. **The captured node clicks wherever the button is released** — "press, drag
   off, release elsewhere does not click" (`docs/guide/events/mouse.md`) breaks
   for a node that has both `onClick` and `onPointer*`.
3. **Nothing else hovers during the drag.** `examples/nodes` drags a wire from
   a socket (`NodeCard.tsx`: a handler node around a dot with a `hoverStyle`);
   today the _target_ sockets' dots light up under the cursor during the drag.
   Capture would stop that, and any other drop-target feedback built on
   `hoverStyle`/`onPointerEnter`.

On the web the mouse has **no** implicit capture; only touch does, and it
captures the innermost `pointerdown` target (with click bubbling up from it).
What "implicit capture" means for bevy-react's model — `onPointerMove`/`Up`
follow the pointer anywhere, and the gesture's click belongs to the dragged
element — needs no `PointerCaptureMap` (§2.8).

### 1.4 Other audit corrections

- §3.8 "fold the 11 reader loops over `PointerEvent`": the trait exposes only
  the pointer, so six of the eleven (press/release/drag/click need `button`,
  clicks need `hit.depth`) could not be folded without a local trait. Moot:
  the redesign deletes all eleven (§2.6).
- §3.8 "it changes the extension contract (`InteractionSyncSet`)": the set
  keeps its name and slot; what changes is that features no longer need it
  for hover/press at all (§2.9).
- New finding (not in the audit): `collect_pointer_events`'s query requires
  `&ComputedNode` and `&UiGlobalTransform` (added with touch support,
  `00b7c4f`). SVG shapes are node-less, so by reading, a shape's
  `onPointerDown/Move/Up` cannot fire today although `docs/guide/elements/
svg.md` documents them. The new collector makes geometry optional (§2.5).
  Unverified by running.
- New finding: `virtual_events.rs::surface_relative` passes the pointer's
  **logical** position to `ComputedNode::normalize_point` (physical). Wrong
  `x`/`y` on a `<surface>` with `scale_factor ≠ 1` and for the `--shoot
--scale` scripted pointer (CLAUDE.md already warns "positions assume scale
  1"). Fixed by §2.4's shared helper.

---

## 2. Design

### 2.1 `ReactInteraction` (new, public)

`crates/core/src/bridge.rs`, beside `FocusState`/`HoverState`:

```rust
/// Whether a pointer is over (or pressing) this React entity. Present on an
/// element with `hoverStyle`/`pressStyle`, `onClick` or an `onPointer*`
/// handler, and on every `<button>` and `<editableText>`. Derived from
/// bevy_picking for every pointer (mouse, touches, transformed layers,
/// `<surface>`): read it, never write it.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ReactInteraction {
    /// The primary button (or a touch) went down on it and is still held —
    /// until release, wherever the pointer is.
    Pressed,
    Hovered,
    #[default]
    None,
}
```

Exported as `bevy_react_core::ReactInteraction` (`lib.rs`, next to
`pub use bridge::ReactNode`) and in the facade prelude. Same three variants as
`Interaction` on purpose: third-party code migrates by renaming the type, and
~60 of our test lines do too.

Inserted exactly where `Interaction` is today (four sites) and removed where it
is (`node_less_pointer_slot`).

### 2.2 Text runs fold onto their block — `fold_span_hits`

React text spans (`TextSpan` + `ReactNode`: bare-string runs and nested
`<text>`) carry no interaction of their own (`TEXT_SPAN` is
`Common::IDENTITY`) and no `Pickable`, so in the hover map a glyph run is a
blocking leaf (§1.2). Undo bevy's per-section targeting for them before the
hover map is built:

```rust
// crates/core/src/reconcile/picking.rs (new file)
pub fn fold_span_hits(
    mut hits: MessageMutator<PointerHits>,
    spans: Query<(), (With<TextSpan>, With<ReactNode>)>,
    blocks: Query<(), With<Text>>,
    child_of: Query<&ChildOf>,
) {
    for hits in hits.read() {
        for (entity, _) in &mut hits.picks {
            if spans.contains(*entity)
                && let Some(block) = climb(*entity, &child_of, |e| blocks.contains(e))
            {
                *entity = block;
            }
        }
    }
}
```

`PreUpdate`, `.in_set(crate::ext::PickRefineSet)` (after the backends and
pick3d's suppression, before `PickingSystems::Hover`). `MessageMutator<
PointerHits>` is the pattern `suppress_transformed_layer_hits` already uses;
`climb` is `reconcile/events.rs::climb`. The block's own `Pickable` (the
`focusPolicy` mirror, `style/writers/interaction.rs::focus_pickable`) then
governs the hover map, as the backend already assumed. Non-React text is
untouched.

Rejected: giving spans a pass-through `Pickable`. It would have to mirror the
block's policy and follow its changes (a pass-through span over a blocking
`<text>` would leak the pointer to meshes and the window entity behind it,
since the hover map walks on into lower pick layers), and every consumer would
still need a span→block attribution.

Cost of the fold: devtools "pick" mode selects the `<text>` block, no longer an
individual span (spans stay reachable from the tree). See D3.

### 2.3 `sync_interactions` — the only writer

```rust
// crates/core/src/reconcile/picking.rs
pub fn sync_interactions(
    hover_map: Option<Res<HoverMap>>,
    presses: Option<Res<bevy::picking::events::PointerState>>,
    virtual_pointers: Option<Res<VirtualPointers>>,
    transform3d: Option<Res<Transform3dPointer>>,
    pointers: Query<(&PointerId, &PointerLocation)>,
    windows: Query<&Window>,
    mut nodes: Query<&mut ReactInteraction>,
    capture: Option<ResMut<PointerCapture>>,      // from commit 5 on
    mut live: Local<EntityHashSet>,               // what we last set non-None
    mut next: Local<EntityHashMap<ReactInteraction>>,
) {
    next.clear();
    let mut over_ui = false;
    let is_virtual = |p| virtual_pointers.as_ref().is_some_and(|v| v.contains(p));
    // The mouse drives two picking pointers (itself and the transform3d
    // remap); bevy_picking keeps their last location when the cursor leaves
    // the window, so their hover is only real while the window has a cursor.
    // (The window the mouse pointer's own location names — not `PrimaryWindow`;
    // no window or no location yet = not gone, so headless tests need neither.)
    let cursor_gone = mouse_window(&pointers, &windows)
        .is_some_and(|w| w.cursor_position().is_none());
    if let Some(map) = &hover_map {
        for (&pointer, hits) in map.iter() {
            if cursor_gone && device(pointer, transform3d.as_deref()) == PointerId::Mouse {
                continue;
            }
            for &entity in hits.keys() {
                if nodes.contains(entity) {
                    next.entry(entity).or_insert(ReactInteraction::Hovered);
                    over_ui |= !is_virtual(pointer);
                }
            }
        }
    }
    if let Some(presses) = &presses {
        for (&(pointer, button), state) in &presses.pointer_buttons {
            if button != PointerButton::Primary { continue; }
            for &entity in state.pressing.keys() {
                if nodes.contains(entity) {
                    next.insert(entity, ReactInteraction::Pressed);
                    over_ui |= !is_virtual(pointer);
                }
            }
        }
    }
    for (&entity, &state) in next.iter() {
        if let Ok(mut i) = nodes.get_mut(entity) { i.set_if_neq(state); }
    }
    for entity in live.drain() {
        if !next.contains_key(&entity) && let Ok(mut i) = nodes.get_mut(entity) {
            i.set_if_neq(ReactInteraction::None);
        }
    }
    live.extend(next.keys());
    if let Some(mut capture) = capture { capture.over_ui = over_ui; }
}

/// The mouse and the transform3d pointer are one physical device.
pub(super) fn device(pointer: PointerId, t3d: Option<&Transform3dPointer>) -> PointerId {
    if t3d.is_some_and(|t| t.id == pointer) { PointerId::Mouse } else { pointer }
}

/// The window the mouse pointer is on: its `PointerLocation` target, when
/// that is `NormalizedRenderTarget::Window(w)` → `windows.get(w.entity())`.
fn mouse_window<'a>(
    pointers: &Query<(&PointerId, &PointerLocation)>,
    windows: &'a Query<&Window>,
) -> Option<&'a Window>;
```

Properties worth keeping:

- **Semantics = hover-map membership** — the stack rule
  `docs/guide/styling/focus-policy.md` already documents ("nodes are visited
  from the topmost down until the first blocking node … a `<button>` inside a
  hover-styled card un-hovers the card"). Not CSS `:hover`-with-descendants:
  see D2.
- **Sticky press, primary only**, straight from `pressing` — same rule as
  `ui_focus_system`, now per pointer.
- It writes only entities the two maps name and resets only what it set, so a
  feature that writes `ReactInteraction` itself (an element picking can't see)
  is never fought, and a node that gains the component while hovered gets its
  state next frame (it reads the component, not a private diff).
- `over_ui` counts **window** pointers only. This is load-bearing:
  `surface::drive_surface_pointer` parks the surface pointer while
  `over_ui` is set, so letting surface hover set it would oscillate.
- `PickingInteraction`, `Hovered`, `Pressed`, and the `Pointer*` hover
  messages are not read anywhere.

Schedule: `PreUpdate`, `.after(bevy::picking::PickingSystems::Hover)`. Like
`Interaction` today, the state is then current for every `Update` system
without any ordering (`named_pins.rs` needs none). All resources are `Option`
so `op_app` harnesses that add the system need no picking plugins.

### 2.4 Shared pointer helpers (same file)

```rust
/// Where a pointer is: its location, and the client position its events
/// report. The transform3d pointer sits at the remapped (layout-space)
/// position; `clientX/Y` stay the real cursor's, like the DOM under a CSS
/// transform.
pub(super) struct PointerSample { pub location: Location, pub client: Vec2 }

pub(super) fn sample_pointer(
    id: PointerId,
    pointers: &Query<(&PointerId, &PointerLocation)>,
    t3d: Option<&Transform3dPointer>,
) -> Option<PointerSample>;

/// Clamped 0..1 position of `location` inside a node (top-left origin).
pub(super) fn node_relative(
    node: &ComputedNode,
    transform: &UiGlobalTransform,
    camera: Option<&Camera>,
    location: &Location,
) -> Option<Vec2> {
    let physical = match camera {
        Some(camera) => crate::raster::pointer_physical_position(location, camera),
        None => location.position * crate::raster::node_scale_factor(node),
    };
    node.normalize_point(*transform, physical)
        .map(|n| (n + 0.5).clamp(Vec2::ZERO, Vec2::ONE))
}

/// The pointer hovering `entity` this frame (any one, if several).
pub(super) fn pointer_over(entity: Entity, map: &HoverMap) -> Option<PointerId>;
```

`node_relative` replaces both `events.rs::normalized_01` (RCP) and
`events.rs::surface_relative` (and fixes its logical-px bug). The camera comes
from the node's `ComputedUiTargetCamera`; `raster::pointer_physical_position`
and `raster::node_scale_factor` exist. `pointer.rs::event_pos` becomes
`event_pos(user: Option<&EventLocalPos>, geometry, location)` — user-space
first (svg shapes), else `node_relative`, else `None` (a node-less entity
without an `EventLocalPos` value).

### 2.5 The collectors, pointer-driven

**Restyle — `apply_interaction_styles`.** Type swap only (`Ref<Interaction>`,
`Changed<Interaction>`, the three match arms). It now serves `<surface>` nodes
too, because they get state from §2.3; `apply_virtual_interaction_styles` is
deleted. Side fix: the virtual restyle hard-coded `promoted: false`, wrong
since layers work inside surfaces.

**Hover — `collect_hover_events`**, one system for every pointer:

```rust
type Geometry = Option<(&'static ComputedNode, &'static UiGlobalTransform, &'static ComputedUiTargetCamera)>;

// boundary: the same signal as the restyle
for (entity, interaction, mut hover, handlers, rnode, geo, user) in &mut nodes /* Changed<ReactInteraction> */ {
    let inside = *interaction != ReactInteraction::None;
    if inside == hover.0.is_some() { continue; }      // Hovered <-> Pressed
    let pointer = if inside {
        hover_map.and_then(|m| pointer_over(entity, m)).unwrap_or(PointerId::Mouse)
    } else {
        hover.0.unwrap()
    };
    hover.0 = inside.then_some(pointer);
    // emit pointerEnter / pointerLeave at sample_pointer(pointer) (ZERO if the pointer is gone)
}
// pointerHover: per pointer that moved, the handler nodes in ITS hover map
for (&pointer, hits) in hover_map.iter() {
    let Some(s) = sample_pointer(pointer, ..) else { continue };
    if last.insert(pointer, s.client) == Some(s.client) { continue; }
    for &entity in hits.keys() { /* hovering.get(entity), handlers.hover → emit */ }
}
last.retain(|p, _| hover_map.contains_key(p));
```

`HoverState(pub bool)` becomes `HoverState(pub Option<PointerId>)` (who is
inside — the leave needs that pointer's position). `last` is a
`Local<HashMap<PointerId, Vec2>>` replacing `Local<Option<Vec2>>`. The
`pointerHover` pass is now O(hover stack) instead of iterating every handler
node on each cursor move. `collect_virtual_hover_events` is deleted.

**Drag — `collect_pointer_events`**, one system for every pointer, driven by
picking instead of `ButtonInput`/`Touches`/RCP:

```rust
#[derive(Resource, Default)]
pub(crate) struct ActiveDrag { drags: Vec<Drag> }      // at most one per device

pub(crate) struct Drag {
    pub entity: Entity,
    pub pointer: PointerId,       // whose location and inputs it follows
    pub button: PointerButton,
    last_pos: Vec2,
    last_abs: Vec2,
}
impl ActiveDrag {
    pub(crate) fn begin(&mut self, entity: Entity, pointer: PointerId, button: PointerButton, pos: Vec2, abs: Vec2);
    pub(crate) fn of_device(&self, device: PointerId) -> Option<&Drag>;
    pub(crate) fn owns_touch(&self, id: u64) -> bool;        // replaces touch_id()
}
```

```text
1. BEGIN — for each device without a drag, among this frame's PointerPress
   messages whose entity has PointerHandlers: the transform3d pointer's press
   beats the raw mouse's (same device; the quad is what the user sees), then
   the smallest hit.depth. Any button. → drag.begin(..); pointerDown at the
   press's own location (ev.pointer).
2. FOLLOW — each drag: sample_pointer(drag.pointer); pos = event_pos(..)
   .unwrap_or(last_pos); if client != last_abs and handlers.moved → pointerMove.
3. END — a PointerInput for drag.pointer that is Release(drag.button) or Cancel
   → pointerUp at THAT message's location (falling back to last_*); a touch
   TouchScrollState::stole_drag → pointerLeave (unchanged contract); a pointer
   that no longer exists → pointerUp at last_*.
capture.dragging = any drag whose pointer is not in VirtualPointers
capture.wheel_captured = false   // unchanged
```

Query: `(&ReactNode, &PointerHandlers, Option<&EventLocalPos>, Geometry)` —
geometry optional, so shapes and custom node-less elements drag too. "Topmost
by `hit.depth`" is the rule `collect_ui_events` already uses, replacing
`ComputedStackIndex`. No climb: a press only begins a drag on a handler node
that is itself under the pointer (what the window path does today, and what
the focus-policy doc says; the virtual path's climb-to-ancestor goes away).

What disappears from `pointer.rs`: `POINTER_BUTTONS`, `DragSource`, the
`Interaction` begin tests, the whole "a touch only binds to a `Pressed` node it
is geometrically inside" block (picking attributes each touch to its own
pointer), the `interactions.iter().any(..)` scan, the window-cursor fallbacks.
`collect_virtual_pointer_events` is deleted.

Two small supporting changes:

- `layer/pick3d.rs::drive_transform3d_pointer`: while a forwarded press is
  held (`state.buttons.any_pressed()`), keep remapping into `state.over_layer`
  even when the inverted cursor is outside its border box (skip the bounds test
  and the topmost search for that layer). Otherwise the virtual pointer parks
  and releases at the quad's edge and a drag bound to it would end there —
  today the window path keeps such a drag alive (with frozen `x`/`y`).
  ~8 lines; `x`/`y` now keep tracking.
- `ext::VirtualButtons::leave(id, release_at, park_at, input)`: the owed
  `Release`s are written at the last in-bounds location, the parking `Move` at
  the off-bounds one (today both use the off-bounds location, which would
  become the `pointerUp` position). Two callers: pick3d and
  `surface/src/pointer.rs`. Extension-contract change.

**Clicks — `collect_ui_events`** absorbs virtual pointers: drop the
`VirtualPointers` skip (it already keeps a per-pointer topmost and climbs to
`ClickOwners` exactly like `collect_virtual_clicks`). `collect_virtual_clicks`
is deleted.

### 2.6 What is deleted

- `crates/core/src/reconcile/virtual_events.rs` — the whole file (429 lines,
  all 11 reader loops). No `PointerEvent` helper is written.
- `layer::pick3d::correct_transformed_interactions` and its test (the pointer
  remap + hit suppression already put members in the right hover map).
- The `Interaction`/RCP half of `svg/src/interact.rs::sync_shape_interactions`
  (rename to `sync_shape_event_pos`; it keeps writing `EventLocalPos` only) and
  `user_to_root_normalized`.
- `examples/demos/cube/mod.rs::sync_cube_interactions`.
- `events.rs::{normalized_01, surface_relative}`, `pointer.rs::{DragSource,
POINTER_BUTTONS}`, every `RelativeCursorPosition` insert/remove in
  `stamps.rs::apply_pointer_handlers`.
- All `Interaction` inserts. We keep writing `FocusPolicy`
  (`FOCUS_POLICY_WRITER`): `ui_focus_system` still runs for the app's own
  bevy_ui nodes, and the policy is what stops their `Interaction` beneath a
  React `focusPolicy: "block"` node. (Its doc comment about an absent policy
  is stale: `Node` requires `FocusPolicy`, `ui_node.rs:463-477`.)

### 2.7 System ordering (sets, not names)

`PreUpdate`:

- `fold_span_hits.in_set(PickRefineSet)`.
- `sync_interactions.after(PickingSystems::Hover)`.

`Update`:

- `InteractionSyncSet` loses `.after(correct_transformed_interactions)`; it
  stays `.before` the restyle and the hover/drag collectors. A feature fills
  `EventLocalPos` there (svg), or overrides `ReactInteraction`.
- `collect_ui_events.before(collect_pointer_events)` — new edge, needed by
  §2.8 (the drag binding is dropped on the release frame).
- Everything else as is: `collect_pointer_events.in_set(PointerCaptureSet)`,
  the wheel/touch/scrollbar systems `.after` it, `apply_interaction_styles
.after(apply_js_ops)`.

`PointerCapture::over_ui` is now assigned in `PreUpdate`; the claims made later
in `PointerCaptureSet` (scrollbar hover) still land after it, and
`drive_surface_pointer` still reads last frame's value — unchanged.

### 2.8 Pointer capture: a drag owns its click

The fix for "drag release clicks the ancestor", in `collect_ui_events`:

```rust
// While an element is being dragged, the gesture's click is its own: only it
// or a descendant can be clicked. (A slider released over its card must not
// click the card; a <button> inside a press surface still clicks.)
if let Some(drag) = active_drag.of_device(device(ev.pointer.id, t3d))
    && target != drag.entity
    && !child_of.iter_ancestors(target).any(|a| a == drag.entity)
{
    continue;
}
```

applied after the climb to the click owner and before the topmost-per-pointer
selection. `ActiveDrag` is read as `Option<Res<..>>` (headless tests). A press
and release inside one frame has no drag bound yet when this runs; the stock
click then stands, which is right — the pointer cannot have left the element.

Why this is the whole fix: the stray click reaches the ancestor because a
pass-through stack presses every ancestor and Click = pressed ∩ hovered; the
rule drops exactly the owners outside the dragged subtree and keeps stock click
semantics for everything else (release off the element still does not click;
a descendant `<button>` still does). It covers the window, transformed layers
(one device), touches and `<surface>` pointers alike, because drags are now
per pointer. `Slider.tsx` drops `focusPolicy: "block"`.

`PointerCaptureMap` is **not** called (D8). If hover-lock is wanted later, the
only safe target is a _pure drag target_ — the drag owner is itself the
gesture's resolved click owner (no click-owning descendant in the press stack)
and declares no `onClick` (`bridge.props_cache[id].on_click`) — captured on the
drag's first displacement like the stock slider, released by bevy on
release/cancel and by us on a touch steal or when the dragged entity stops
resolving. It must be opt-in per element (a `pointerCapture` prop), because of
drop-target hover (§1.3 item 3).

Interplay, for the record:

- **Our `PointerCapture` resource** is unrelated to bevy's map (it answers "does
  the UI own the mouse" for world systems). Add one sentence to its rustdoc
  saying so; no rename.
- **Touch scroll**: unchanged contract. `apply_touch_scroll` asks
  `ActiveDrag::owns_touch(id)` for the contested claim; a steal still ends the
  drag with `pointerLeave` a frame later and suppresses the tap
  (`TouchScrollState::is_suppressed`). Claims are hit-tested geometrically, so
  nothing here changes which container scrolls.
- **An explicit JS `setPointerCapture`**: no. Pointer ids are not on
  `PointerEventData` and there is no act-now channel for it; nothing needs it.

### 2.9 Extension contract and public API

- New: `ReactInteraction`.
- `InteractionSyncSet`: same name and slot. Doc: "fill `EventLocalPos`, or
  override `ReactInteraction` for an entity no picking backend reports". A
  node-less element that a picking backend hits gets hover, enter/leave and the
  drag handlers with no system of its own.
- `EventLocalPos`: unchanged.
- `VirtualPointers`: unchanged API. Meaning narrows to "pointers that are not
  the window's": they do not count for `PointerCapture`, and `cursor.rs` asks
  them first. Their events ride the same collectors as every other pointer.
- `VirtualButtons::leave` gains `release_at`.
- Gone from React entities: `Interaction`, `RelativeCursorPosition`.
- No wire-format change, no TS change, nothing to regenerate.

### 2.10 How each existing mechanism keeps working

- **`focusPolicy`** — still mirrored into `Pickable` by `FOCUS_POLICY_WRITER`;
  the hover map stops at the first blocking node, so hover, press, drag begin
  and clicks all stop there, in the window and in a `<surface>`. An
  `<editableText>` blocks whatever its policy (`focus_pickable`), now for hover
  too (D9). `<root>` stays `Pickable::IGNORE`.
- **`transform3d` picking** — unchanged pipeline: `drive_transform3d_pointer`
  remaps, `suppress_transformed_layer_hits` keeps the mouse off members and the
  virtual pointer on them. Members are then simply in that pointer's hover map
  and `pressing` set; no correction pass. Positions come from the remapped
  `PointerLocation`.
- **SVG shapes** — `refine_svg_pointer_hits` (`PickRefineSet`) still emits the
  shape hit; the shape (pass-through `Pickable`) and its svg root are both in
  the hover map. The crate only fills `EventLocalPos`.
- **Text spans** — never in the hover map after §2.2; a `<text>` with
  `hoverStyle` or handlers is hit as a block.
- **`<surface>` pointers** (and the `--shoot` scripted pointer) — the same
  state and collectors as the mouse; `VirtualPointers` only keeps them out of
  `PointerCapture`.
- **Touch** — each finger is its own picking pointer: its press marks its own
  node, starts its own drag, and `touch_scroll.rs` arbitrates per touch id as
  before.
- **Custom node-less elements** (the demo `<cube>`) — hit by their backend
  (`MeshPickingPlugin`), so hovered, entered and draggable with no sync system.

---

## 3. Edge cases and tests

Harnesses: bare `World` + `run_system_once` (the `pick3d`/`svg::interact`
style) for §2.2–2.3; `reconcile::test_util::op_app` for the restyle;
`reconcile::clip_pick_tests::clip_scene` (real layout, clipping, `ui_picking`,
`PickingPlugin` + `InteractionPlugin`, a mouse pointer on a headless window)
for anything that must be pinned against stock picking; the `click_app()` /
`hover_app()` helpers already in `pointer.rs`/`hover.rs` for the collectors.
Fabricating state: insert a `HoverMap`; `PointerState::default()` then
`get_mut(pointer, PointerButton::Primary).pressing.insert(entity, (location,
Instant::now(), hit))`; write `PointerPress` / `PointerInput` messages by hand
(as `virtual_restyle_keeps_the_focus_overlay` does today).

`picking.rs`:

1. `span_hit_folds_onto_its_text_block` — a `PointerHits` naming a React span →
   the pick's entity is the block; a non-React `TextSpan` is left alone.
2. `hover_then_press_then_none` — in the map → `Hovered`; in `pressing` →
   `Pressed`; out of both → `None`, written once (assert `is_changed` false on
   an idle second run).
3. `press_is_sticky_until_release` — in `pressing`, not in the hover map →
   still `Pressed`; a Secondary-button `pressing` entry does not press.
4. `every_pointer_counts` — a `Touch(7)` entry and a custom pointer's entry
   each hover their node; a registered virtual pointer hovers its node but
   leaves `PointerCapture::over_ui` false; the mouse sets it.
5. `a_left_window_drops_mouse_hover` — the mouse pointer's window has no
   cursor position:
   the mouse's and the transform3d pointer's entries are ignored, a touch's is
   not, a `pressing` entry still presses.
6. `a_component_gained_while_hovered_catches_up` — entity already in the map
   gets `ReactInteraction` inserted → `Hovered` on the next run.
7. `a_blocking_child_unhovers_what_is_beneath` and
   `a_passing_child_keeps_its_parent_hovered` — real `ui_picking`, on the app
   and mouse-pointer setup of `clip_scene` (factor that half out of it):
   parent with `ReactInteraction`, child with
   `Pickable { should_block_lower: true/false }`. Pins the focus-policy doc
   against stock picking.

`interaction.rs`: the existing three tests with the inserted type renamed, plus
`a_virtual_pointer_restyles_like_the_mouse` (hover map for a registered virtual
pointer → `sync_interactions` → hover background applied) and the ported
`press_on_a_focused_node_keeps_its_focus_style` (was the `virtual_events.rs`
test).

`hover.rs` (three tests rewritten on `ReactInteraction` + a pointer entity):
boundary only / no re-enter on press; `pointerHover` only on motion and only
for nodes in the moving pointer's map; `x`/`y` use the physical conversion
(scale factor 2 window: logical 50 → 0.5 of a 200 px node) and `EventLocalPos`
wins; a virtual pointer reports its own target's px as `clientX/Y`.

`pointer.rs` (eight tests rewritten to write messages): down/move/up; move only
on displacement; overlapping handler nodes → smallest depth gets the drag; a
second button during a drag is ignored; `Touch(7)` drag reports button 0 and
the touch's position; a `Cancel` → `pointerUp` at the last position; a stolen
touch → `pointerLeave`, no `pointerUp` (`TouchScrollState::steal_for_test`);
dragged entity despawned mid-drag → no panic, binding gone on release; two
pointers drag two nodes; transform3d press beats a mouse press in the same
frame; `EventLocalPos` overrides `x`/`y`; `wheel_captured` resets each frame.

`events.rs`: `nested_onclick_owners_click_topmost_only` once for a mouse and
once for a registered virtual pointer (replacing the `surface_…` twin and
`surface_pointer_clicks_are_not_main_clicks`); and for §2.8:
`drag_release_over_an_ancestor_does_not_click_it`,
`a_button_inside_the_dragged_surface_still_clicks`,
`another_pointers_click_is_not_scoped`,
`a_mouse_click_is_scoped_by_a_transform3d_drag`.

`layer/pick3d.rs`: `a_held_press_keeps_its_layer_past_the_edge` (buttons
pressed, cursor inverted outside the box → a `Move` to the inverted position,
no `Release`). `touch_scroll.rs`: the drag seeds switch to the new `begin`
signature; assertions unchanged.

`bevy_react_svg`: `interact.rs` tests keep the `EventLocalPos` assertions and
drop the `Interaction`/RCP ones; `op_pointer_tests.rs` asserts
`ReactInteraction`/`EventLocalPos` presence; new op-path test
`shape_pointer_down_reports_user_space` (press message on a shape → one
`pointerDown` with the user-space `x`/`y`) — the regression test for the §1.4
finding. `examples/demos/cube/tests.rs`: the hover map drives
`ReactInteraction` with no cube system.

Not testable headless, to try by hand or `--shoot`: the cursor-left-the-window
hover (fact §1.2), a touch device, in-world `<surface>` hover.

---

## 4. Commit sequence

Each commit builds, passes `cargo test` (workspace), `cargo clippy --workspace
--all-targets` and `cargo hack check -p bevy-react --each-feature`. Bevy
features: everything used is `bevy_picking`/`bevy_ui` API already reachable
through core's `ui_picking` feature; the facade prelude gains one name.

1. **`fix(pick) fold text-run hits onto their text block`** — §2.2 + test 1.
   Visible effect on its own: a pass-through `<text>` no longer hides what is
   beneath its glyphs from clicks and the cursor.
2. **`feat(input) ReactInteraction: hover and press from picking`** — the
   component, exports, `sync_interactions` (without the `PointerCapture`
   write), inserted **beside** `Interaction` at the four sites; tests 2–7.
   Nothing reads it yet.
3. **`ref(input) hover and press styles follow ReactInteraction`** —
   `apply_interaction_styles` swaps types; delete
   `apply_virtual_interaction_styles`; rename the inserted type in
   `interaction.rs`, `style/{ext_tests,invalidation_tests,writers/tests}.rs`,
   `transition/gradient_tests.rs`, `filters/transition.rs`.
4. **`ref(input) one hover collector for every pointer`** — §2.4 helpers,
   `collect_hover_events`, `HoverState(Option<PointerId>)`; delete
   `collect_virtual_hover_events`.
5. **`ref(input) one drag collector for every pointer`** — `ActiveDrag`,
   `collect_pointer_events`, clicks absorb virtual pointers, `over_ui` moves
   to `sync_interactions`, the pick3d hold, `VirtualButtons::leave`; delete
   `virtual_events.rs`; `touch_scroll.rs` call sites.
6. **`ref(input) drop Interaction and RelativeCursorPosition`** — remove the
   inserts and RCP stamps; delete `correct_transformed_interactions`, the svg
   and cube synthesis; `named_pins.rs` and `sync_cube_pickable` read
   `ReactInteraction`; remaining presence asserts (`stamps.rs`,
   `update/tests.rs`, svg op tests, cube test). After it,
   `grep -rnw Interaction crates examples` finds no code. Docs, CHANGELOG,
   CLAUDE.md for commits 2–6 land here.
7. **`fix(input) a drag owns its click`** — §2.8, the ordering edge, the four
   tests, `Slider.tsx`, the Mouse demo card, `mouse.md`.

Commits 1–6 are the `Interaction` retirement (needed before Bevy 0.21); 7 is
independent of how 2–6 are split but needs 5's per-pointer `ActiveDrag`.
Between 2 and 6 interactive nodes carry both components — a transient cost,
never released.

---

## 5. Demos and docs

**Demos**

- `examples/demos/ui/src/components/Slider.tsx`: remove `focusPolicy: "block"`
  from `track` (commit 7).
- `examples/demos/ui/src/demos/events/MouseDemo.tsx` (nav label "Mouse"): one
  more example — a card with `onClick` counting clicks, a `Slider` inside it;
  the copy says a drag released outside the slider does not click the card.
  Snippet lines ≤ 40 chars.
- `examples/demos/scenes/named_pins.rs`, `examples/demos/cube/mod.rs` as in
  commit 6; the "Custom elements" demo copy if it mentions syncing
  `Interaction`.
- No new nav label; no `bevy.ts` regeneration.

**Guide**

- `docs/guide/events/mouse.md`: intro ("through `bevy_ui`'s interaction state
  and Bevy picking" → Bevy picking); _Clicks_: add the drag rule; _Drags_: "one
  drag per pointer — the mouse runs one at a time, each finger its own" (twice:
  the bullet and _Touch_); note that `x`/`y` keep tracking past a 3D-transformed
  layer's edge; `PointerCapture` table wording for `over_ui` unchanged in
  meaning. Drop the stale "No modifier keys / no hover-move events" limits
  while there.
- `docs/guide/styling/focus-policy.md`: the `<editableText>` paragraph — it now
  also keeps **hover and press** from what lies beneath it; "In a `<surface>`,
  the policy applies the same way" is finally true for hover.
- `docs/guide/communication/named-nodes.md`: the example (`Query<&Interaction>`
  → `Query<&ReactInteraction>`), the sentence before it, and the bridge-owned
  list.
- `docs/guide/extending/custom-elements.md`: _Pointer events_ (nothing to sync;
  a picking backend that hits the entity is enough; the drag handlers now reach
  node-less elements, `x`/`y` being 0 unless the element fills
  `EventLocalPos`), the sets table row, the two limits bullets.
- `docs/guide/elements/button.md`: the comparison table rows naming
  `Interaction`.
- `docs/guide/elements/svg.md`: nothing, if the shape drag test passes.
- `docs/guide/performance.md`: one line — an app with no bevy_ui `Interaction`
  or `RelativeCursorPosition` of its own can switch the legacy pass off with
  `app.configure_sets(PreUpdate, UiSystems::Focus.run_if(|| false))` (D5).

**CHANGELOG (`## [Unreleased]`)**

- _Changed_: **Hover and press come from Bevy picking** (breaking for Rust code
  that queries React entities). They no longer carry `bevy::ui::Interaction`
  (deprecated in Bevy 0.20) or `RelativeCursorPosition`; read
  `bevy_react::ReactInteraction`. Edit the existing `<button>` entry ("with
  `Interaction` added explicitly") to match.
- _Changed_: hover in `<surface>` UI follows `focusPolicy` like the window (a
  blocking child un-hovers its ancestors); an `<editableText>` stops hover and
  press of what is beneath it; drags run per pointer;
  `VirtualButtons::leave` takes the release location; a custom node-less
  element needs no `Interaction` sync.
- _Fixed_: releasing a drag over an ancestor with `onClick` clicked it; a touch
  did not press its element while the mouse rested elsewhere; pointer `x`/`y`
  in a `<surface>` with a `scale_factor` other than 1; a drag inside a
  3D-transformed layer froze `x`/`y` at the layer's edge; drag handlers on
  `<svg>` shapes (only if the new test shows they were dead).

**Other text**: CLAUDE.md (the `transform3d` paragraph's "`Interaction`/
`RelativeCursorPosition`/cursor-icon are corrected", the SVG paragraph's
`interact.rs` sentence, the extension-contract bullets for `VirtualPointers`
and `InteractionSyncSet`, the named-nodes list); `bridge.rs`'s `ReactNode`
rustdoc list; `ext/mod.rs` set docs; `PointerCapture` rustdoc; `CONTEXT.md`
gains "Interaction state" and "Drag owner" if the implementer finds them used
in prose. `screenshots/screenshots.json`: retune the Mouse shot's crop if the
page grows.

---

## 6. Decisions

- **D1 — Name and shape.** `ReactInteraction`, an enum with `Interaction`'s
  three variants. Reason: 1:1 migration for users and tests; a distinct name
  avoids a glob clash with `bevy::prelude::Interaction` while that still
  exists. (`PointerState` is taken by bevy_picking.)
- **D2 — Hover semantics.** Hover-map membership (today's documented stack
  rule), for every pointer. Not CSS `:hover`-with-descendants, although that is
  the more web-faithful one: it would silently change what `focusPolicy:
"block"` means for ancestors, which the guide documents with a workaround.
  Changing it is a separate product decision.
- **D3 — Text runs.** Fold to the block (§2.2). Cost: devtools pick mode
  selects the block, not a span. Alternative if span picking matters: keep the
  fold and record the folded span per pointer for devtools only.
- **D4 — Stop inserting `Interaction` now** (and RCP). Reasons: deprecated;
  it would disagree with ours exactly where we fix things (touch, transformed
  layers, shapes, surfaces); it costs a component and `ui_focus_system` writes
  per node; keeping it needs `#[allow(deprecated)]`. Breaking for Rust code
  querying it — changelog. No compatibility flag.
- **D5 — `ui_focus_system`.** Leave it running; it cannot be removed and the
  app's own bevy_ui nodes may use it. It finds nothing to write on React nodes.
  No plugin knob; document the one-line app-side opt-out. The `stress` example
  may use it if the benchmark shows the walk.
- **D6 — Drags per pointer.** One drag per device (mouse + transform3d pointer
  = one device; each touch; each virtual pointer). Web-faithful, and simpler
  than keeping one global drag across unified pointers. Doc line changes.
- **D7 — `PointerCapture::{dragging, over_ui}` count window pointers only**, as
  today. A `<surface>` drag does not gate the app's camera; changing that is a
  separate decision (atrium's look-around and the demos camera read
  `is_captured()`).
- **D8 — No `PointerCaptureMap`, no JS capture API.** The click rule is the
  fix. Implicit hover-lock regresses drop-target hover and needs two extra
  rules to be click-safe (§1.3). Upgrade path, if asked for: an opt-in
  `pointerCapture` prop with the pure-drag-target rule.
- **D9 — Inputs block hover beneath them.** Accept and document. The input
  must block picks (focus), so ancestors lose hover while the pointer is on it.
  A fix would be an input-only ancestor walk; not worth it unless reported.
- **D10 — Press while the cursor is outside the window** stays `Pressed` until
  release (was reset to `None`). Follows "pressed until release".
- **D11 — `clientX/Y` inside a 3D-transformed layer** stay the real window
  cursor (as today, as in the DOM); only `x`/`y` use the remapped position.
- **D12 — Sync in `PreUpdate`** after `PickingSystems::Hover`, so `Update`
  readers need no ordering (what `Interaction` gave them).
- **D13 — `VirtualButtons::leave(release_at, park_at)`**: change the contract
  rather than special-case off-bounds positions in the collector.
- **D14 — `FocusPolicy` keeps being written** until Bevy removes
  `ui_focus_system`; then delete the mirror.

---

## 7. Risks and perf

**Per-frame cost at 10k interactive nodes** (table-ops rows are `<button
onClick>`: one `Interaction`, no RCP, no variants):

|                                 | Today                                                                                       | After                                                                                       |
| ------------------------------- | ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| State derivation (ours)         | none for window UI; pick3d's fix iterates all `Interaction` when a transformed layer exists | `sync_interactions`: O(hover stack + pressed set) — tens of entities; writes only on change |
| `over_ui`                       | `interactions.iter().any(..)` — a full 10k scan on every frame nothing is hovered           | a bool from the same loop                                                                   |
| `pointerHover`                  | every handler node, each frame the cursor moves                                             | the moving pointer's hover stack                                                            |
| Enter/leave, restyle            | `Changed<Interaction>` archetype scans                                                      | same scans on `ReactInteraction`                                                            |
| Drag begin                      | every handler node on a press frame                                                         | that frame's `PointerPress` messages                                                        |
| RCP                             | `ui_focus_system` rewrites it on every node that has one, each cursor move                  | gone                                                                                        |
| Components per interactive node | `Interaction` (+ RCP with `onPointer*`)                                                     | `ReactInteraction`                                                                          |
| Stock passes                    | `ui_picking` + `ui_focus_system`, both O(nodes)                                             | unchanged (the second now writes nothing for React nodes; D5 to drop it)                    |

So nothing new scales with node count, two O(n) scans go away, and op-path cost
is one component either way. Create/remove benchmarks should not move;
hover-heavy frames should get cheaper. If table-ops regresses, look at (in
order): commits 2–5 measured mid-sequence (two components per row — measure at
6); a `sync_interactions` that lost its write guard (`set_if_neq`) and
re-triggers the restyle scan's work every frame; `fold_span_hits` allocating.
Run the benchmark twice (the ~17.6 ms frame-floor trap) on a prod bundle.

**Behavior drift to watch** (each is intended; each is a changelog line):

- Surface hover follows blocking; inputs block hover beneath; `<root>`
  (`Pickable::IGNORE`) can no longer be hovered itself; a mesh drawn by a
  higher-order camera now blocks UI beneath it in hover, as it already did for
  clicks.
- In a `<surface>`, a press on a blocking child no longer presses or drags a
  handler ancestor (the virtual path climbed; the window never did), and
  overlapping handler nodes give the drag to the topmost only.
- A released touch may read `Hovered` for one frame before its pointer is
  despawned (it used to go straight to `None`). Visible only as a one-frame
  retarget of a hover transition; skip pointers that released this frame if it
  shows.
- An app that disables bevy_picking's hover (`PickingSettings`) or sets
  `UiPickingSettings::require_markers` without marking the UI camera loses
  hover and drags, not only clicks.

**Dependence on stock internals**: `HoverMap` and `PointerState.pressing` are
public and documented as the layer to build on; the previous-hover Click rule
is only relied on the way `collect_ui_events` already relies on it. Test 7
(and the existing `clip_pick_tests`) fail loudly if 0.21 changes blocking.

**Unverified by running**: the sticky mouse location after `CursorLeft`
(guarded regardless); that shape drags are dead today; that the touch hover
map is populated on the release frame (only matters for the one-frame
`Hovered`); behavior of bevy's widget `Button` observers on our `<button>`
(untouched by this plan).

**Layers/caching**: none. Hover restyles reach layers through the same
`apply_style_masked` invalidation as before; no new dirt source. The restyle
now also runs for promoted nodes inside a `<surface>` with the right
`promoted` flag.

**`--shoot` checks** (scripted pointer = a virtual pointer, so it exercises the
unified path end to end):

- `--shoot "Mouse" out --record 3 --input "0.5 move X Y" --input "1 press"
--input "1.2 move X2 Y2" --input "2 release"` across the new card: the
  slider follows, the card's counter stays 0; a plain click on the card counts.
- `--shoot "<button>"` and `"Focus policy"` with the pointer parked on a
  control: hover style shows; on a blocking child, the parent's does not.
- `--shoot "<svg>"` over a shape: its enter handler fires (the readout
  changes).
- `--scale 1.5 --record` pressing a `Pinchable`: the pinch lands under the
  arrow (it was off at scale ≠ 1).

By hand, once: the real mouse leaving the window from over a hover-styled
button; "3D transforms" — hover and a slider drag on a tilted card, past its
edge (the remap follows the real mouse, not the scripted pointer); a wire drag
in `examples/nodes` (target sockets still light up); the `<surface>` demo
(hover, press, drag off the mesh); a phone or touch screen for touch drags
inside a scrolling list.
