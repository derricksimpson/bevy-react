# Plan — `positionType: "fixed"` on Bevy 0.20's `FixedNode`

Audit section: `.scratch/bevy-0.20/audit.md` §2.3. Written against `main` at
`4ad1b6d` plus another agent's uncommitted edits (`layer.rs`, `layer/clip.rs`,
`layer/pick3d.rs`, `transition/layout.rs`, `plugin.rs` are dirty — cite by
symbol, the line numbers below for our files are approximate).

Bevy paths are relative to
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`.

Nothing here was compiled or run (planner brief). Every Bevy claim is from
reading the 0.20.0 source; the ones a test must pin are marked **(pin)**.

---

## 1. Verified facts

### 1.1 Bevy 0.20 — what `FixedNode` is

| #   | Fact                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | Source                                                                             |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------- |
| F1  | `FixedNode` is a unit marker component, `#[require(Node)]`. Rustdoc: "treated as a root node, even when it has a UI node parent … positioned relative to the target camera's viewport … don't inherit their parent's layout, clipping or transform context."                                                                                                                                                                                                                                                                                                                                           | `bevy_ui-0.20.0/src/ui_node.rs:3532-3539`                                          |
| F2  | **Layout: its own taffy root.** `ui_layout_system` iterates `UiRootNodes` chained with `Query<Entity, (With<FixedNode>, With<ChildOf>)>`; each gets `compute_layout(root, computed_target.physical_size)`.                                                                                                                                                                                                                                                                                                                                                                                             | `bevy_ui-0.20.0/src/layout/mod.rs:107, 215, 267-276`                               |
| F3  | **Excluded from the parent's layout children.** The parent's taffy children are `iter_ui_children(..).filter(!fixed)`, and the recursion `continue`s at a fixed child. So it takes no flex/grid space and is not in the parent's `scrollable_overflow_rect` → not in `ComputedNode.content_size` → the scroll range ignores it. Bevy's own test: `fixed_child_is_a_root_node` (`child_count(parent) == 0`).                                                                                                                                                                                            | `layout/mod.rs:235-246, 370-373, 1547`                                             |
| F4  | **Containing block = the camera viewport.** Every root (fixed included) is the single child of an implicit taffy node: `display: Grid`, `size: 100% × 100%`, `align_items: START`, `justify_items: START`, laid out with `AvailableSpace::Definite(physical target size)`. The size comes from the fixed node's **own** `ComputedUiRenderTargetInfo`.                                                                                                                                                                                                                                                  | `layout/ui_surface.rs:196-226, 229-238`; `layout/mod.rs:267-276`                   |
| F5  | **Which camera.** `propagate_ui_target_cameras` puts `Propagate<ComputedUiTargetCamera>` / `Propagate<ComputedUiRenderTargetInfo>` only on parentless roots; a fixed _child_ inherits both through the real `ChildOf` hierarchy. `physical_size` is `camera.physical_viewport_size()`. So the viewport is the viewport of the camera of the node's hierarchy root: the window for the app tree, the texture for a `<surface>`, that root's camera for a `<root>`.                                                                                                                                      | `bevy_ui-0.20.0/src/update.rs:19-73`                                               |
| F6  | **It needs `PositionType::Absolute` for CSS inset semantics.** With `Relative` it is an in-flow item of the 1×1 viewport grid pinned at START/START: `left`/`top` are relative nudges, `right`/`bottom` do not pin to the far edge. With `Absolute` taffy positions it against the grid area, which for auto placement is the grid container's box (`unwrap_or(border.left)` … `container_border_box.width - border.right`), and resolves insets, sizes, min/max, padding, margins against `grid_area_size`. So `left/right/top/bottom` and `%` resolve against the viewport, opposite insets stretch. | `taffy-0.14.0/src/compute/grid/mod.rs:671-767`; `compute/grid/alignment.rs:87-206` |
| F7  | With **no inset** on an axis an absolute grid item is placed by the container's alignment → viewport top/left; its own `alignSelf`/`justifySelf` override that (`align_item_within_area`). **(pin, T3)** — read from taffy, the weakest reading here.                                                                                                                                                                                                                                                                                                                                                  | `taffy-0.14.0/src/compute/grid/alignment.rs:424-490`                               |
| F8  | **Ancestor transforms and scroll do not reach it.** The geometry walk starts every root (fixed included) with `inherited_transform = Affine2::IDENTITY`, `parent_size = ZERO`, `parent_scroll_position = ZERO`, `inherited_use_rounding = true`; the parent's walk returns early at a fixed child (`if is_fixed_node && root != entity { return; }`). Its own `UiTransform` applies. Bevy test: `fixed_node_doesnt_propagate_parents_uitransform`. Consequence: **`LayoutConfig` (our `layoutRounding`) does not inherit into a fixed node** either.                                                   | `layout/mod.rs:279-292, 334-340, 357-358, 2019`                                    |
| F9  | **Clip is reset.** `update_clipping`: `if has_override_clip \|\| has_fixed_node { maybe_inherited_clip = None; }`, _before_ the node's own `Display::None → FullyClipped` check. The fixed node and its subtree carry no ancestor clip (the component is removed). Bevy test: `fixed_node_opens_new_clipping_context`.                                                                                                                                                                                                                                                                                 | `layout/clipping.rs:63-71`; `update.rs:580`                                        |
| F10 | **Stack order: the parent's.** `stack.rs` never mentions `FixedNode`. `ui_stack_system` builds stack roots from parentless nodes and `GlobalZIndex` nodes and walks `UiChildren` (sorted by `ZIndex`). A fixed node sits where its tree position puts it — above its parent and earlier siblings, below later siblings of any ancestor — unless it has `GlobalZIndex`, which makes it a stack root.                                                                                                                                                                                                    | `bevy_ui-0.20.0/src/stack.rs:54-171`                                               |
| F11 | **Picking / hover are geometric over `UiStack`.** Both `ui_picking` and `ui_focus_system` test `ComputedNode::contains_point(UiGlobalTransform)` + the node's own `CalculatedClip`, skip `InheritedVisibility == false` and zero-size nodes, and take the camera from the partition's first node (same camera, F5). A fixed node is hit at its viewport rect, unclipped.                                                                                                                                                                                                                               | `picking_backend.rs:156-211`; `focus.rs:238-281`                                   |
| F12 | **Rendering.** `bevy_ui_render` has no `FixedNode` reference: extraction gates on `InheritedVisibility`, `ComputedNode::is_empty()` and the clip (`FullyClipped` → empty polygon); z comes from `ComputedStackIndex`.                                                                                                                                                                                                                                                                                                                                                                                  | `bevy_ui_render-0.20.0/src/lib.rs:713-756, 1088`; `clipping.rs:16, 137`            |
| F13 | **`Visibility::Hidden` on an ancestor hides it** (visibility propagates over `ChildOf`; F11/F12 gate on `InheritedVisibility`).                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | same lines as F11/F12                                                              |
| F14 | **`display: none` on an ancestor does NOT hide it.** It is laid out as its own root (F2 — real size), and F9 discards the ancestor's `FullyClipped`. Nothing else gates it. **(pin, T5a)**                                                                                                                                                                                                                                                                                                                                                                                                             | F2 + F9 + F12                                                                      |
| F15 | Toggling is supported: `Added<FixedNode>` / `RemovedComponents<FixedNode>` re-root the taffy node and refresh the parent's children the same frame. Bevy tests `fixed_node_reparenting`, `swap_fixed_nodes`, `fixed_node_children`.                                                                                                                                                                                                                                                                                                                                                                    | `layout/mod.rs:117, 135, 210-213, 1579-1780`                                       |
| F16 | System order: `UiSystems::Prepare → Propagate → Content → Layout → PostLayout` in `PostUpdate`; clipping is `PostLayout`, the stack is `UiSystems::Stack`.                                                                                                                                                                                                                                                                                                                                                                                                                                             | `bevy_ui-0.20.0/src/lib.rs:157-205`                                                |

### 1.2 Our side, as it is now

- The style property is **`positionType`** (not `position`): `POSITION_TYPE: StyleProperty<PositionType>` in `crates/core/src/style/props/layout.rs`, keyword table `de_position_type("positionType") … as POSITION_TYPE_KEYWORDS` in `crates/core/src/protocol/keywords.rs` (`"absolute" | "relative"`), generated TS `js/src/generated/style.ts:15`.
- `Node.position_type` is written by `node_from` (`crates/core/src/ui_map.rs`, ≈L133), used by `LAYOUT_WRITER` (`style/writers/layout.rs`, `writes: &[owns::<Node>]`) **and** by `fresh_style_bundle` — `LAYOUT_WRITER` is in `FRESH_BUNDLED`, so on a fresh element its `apply` does not run.
- Node-less elements get only the span writers (`element/registry.rs`, `global_mask`), so a new global writer never runs on a span/shape (matters: `FixedNode` requires `Node`).
- Layer membership (`LayerMembership.node_to_layer`) is a DFS over Bevy `Children` from each promoted root (`SubtreeWalk::mark_subtree`, `layer.rs`); the render side steals phase items by that map (`layer/render.rs`, `extracted.membership.get(&item.main_entity())`) and leaves every other item in the camera phase. Detached nodes are simply not Bevy children — there is no "stop here" notion in the walk today.
- `layer/clip.rs::cascade` already mirrors `Has<FixedNode>` as an inherited-clip reset (no test covers it).
- `drive_layout_transitions` (`transition/layout.rs`) measures taffy's local rect and treats the **hierarchy parent's** `UiGlobalTransform` as the node's layout frame; the composition walk pushes every `Children` entry.
- `layer/pick3d.rs` resolves a hit to its layer with `climb(entity, child_of, |e| node_to_layer.contains_key(&e))`.
- `ReactNode`'s rustdoc (`bridge.rs`) lists the bridge-owned components.

### 1.3 Audit check

- Confirmed: the marker's location, "FixedNode plus Absolute", parent's stacking partition, ignored ancestor transforms, excluded from layout children + content size (the audit's `layout/mod.rs:239, 334` are right), clip reset at `clipping.rs:64`, `display: none` escape (by reading — still not run).
- Imprecise: it says "`position: "fixed"`" — our key is `positionType`.
- I recommend against "the layout writer owns `FixedNode`": `LAYOUT_WRITER` rides the spawn bundle, and the `display: none` fix needs a single owner of `FixedNode` that is not a writer (§2.3).
- Missed: the transformed-layer picking remap (`pick3d` climbs `ChildOf` past the fixed node), the `layoutRounding` inheritance restart (F8), and that FLIP's parent frame is wrong only under a scaled/rotated ancestor (a translated one happens to cancel).

---

## 2. Design

Three sentences: `positionType: "fixed"` decodes into our own three-variant
enum; the node gets `PositionType::Absolute` plus a `PositionFixed` request
marker, and one system turns that into Bevy's `FixedNode` unless an ancestor is
`display: none`. Everywhere our engines assume "a child lives in its parent's
frame" — layer membership, the interior-clip cascade, the transformed-layer
picking remap, the layout-transition walk — a `FixedNode` becomes a boundary.
Nothing changes on the JS runtime or the wire beyond one new keyword.

### 2.1 Wire and TS

No wire shape change. `positionType?: "absolute" | "relative" | "fixed"` in the
generated `js/src/generated/style.ts` (`npm run style:generate`). Devtools
keyword kinds come from the registry at install — nothing to edit in
`js/src/devtools/warnings.ts` (the kind stays `positionType`).

### 2.2 The property (`crates/core/src/style/props/layout.rs`, `protocol/keywords.rs`, `ui_map.rs`)

```rust
/// `positionType`. `Fixed` is ours: bevy's `PositionType` has no such
/// variant — the node is `Absolute` plus bevy's `FixedNode` (`crate::fixed`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PositionMode { #[default] Relative, Absolute, Fixed }

pub static POSITION_TYPE: StyleProperty<PositionMode> = /* otherwise unchanged, LAYOUT_PAINT */;
```

- `keywords.rs`: `fn de_position_type("positionType") -> PositionMode as POSITION_TYPE_KEYWORDS { "absolute" => Absolute, "relative" => Relative, "fixed" => Fixed }` — the `LayerCache` precedent for a core-owned keyword enum (import it from `crate::style::props`, drop `PositionType` from the bevy import). The unknown-keyword fallback stays `Relative` (the macro's `default()`).
- `node_from`: `Relative → PositionType::Relative`, `Absolute | Fixed → PositionType::Absolute` (F6).
- Invalidation stays `LAYOUT_PAINT`. No `PROMOTION`: a fixed flip changes layer _membership_, which the geometry gate handles (§2.4).
- Put the rustdoc on the static (docs live there): what the viewport is, that it leaves layout/clip/scroll/transforms/layers, that order is the tree's.

### 2.3 `FixedNode` ownership — new `crates/core/src/fixed.rs` (~80 lines with tests)

```rust
/// The node's merged style asks for `positionType: "fixed"`.
#[derive(Component, Debug, Clone, Copy)]
pub struct PositionFixed;

/// bevy's `FixedNode` on a React node: present iff the node asks for it and
/// no ancestor is `display: none`. Bevy lays a fixed node out as its own root
/// and drops its inherited clip, so a hidden ancestor would not hide it; as a
/// plain absolute child it gets the zero size every hidden descendant gets.
pub fn sync_fixed_nodes(
    mut commands: Commands,
    nodes: Query<
        (Entity, Has<PositionFixed>, Has<FixedNode>),
        (With<ReactNode>, Or<(With<PositionFixed>, With<FixedNode>)>),
    >,
    parents: Query<&ChildOf>,
    displays: Query<&Node>,
) {
    for (entity, wanted, fixed) in &nodes {
        let want = wanted
            && !parents.iter_ancestors(entity).any(|a| {
                displays.get(a).is_ok_and(|n| n.display == Display::None)
            });
        if want && !fixed {
            commands.entity(entity).insert(FixedNode);
        } else if !want && fixed {
            commands.entity(entity).remove::<FixedNode>();
        }
    }
}
```

The writer, in `style/writers/layout.rs`, listed in `CORE_WRITERS` after
`LAYOUT_ROUNDING_WRITER` (not in `FRESH_BUNDLED`, so the fresh apply runs it
whenever the style sets `positionType`):

```rust
pub static POSITION_FIXED_WRITER: Writer = Writer {
    reads: &[&POSITION_TYPE],
    attrs: &[],
    writes: &[owns::<PositionFixed>],
    apply: |ctx, s, ec| {
        // A detached element (`<root>`, `<surface>`, `<anchor>`) is already
        // out of its parent's layout and owns its placement: `fixed` is
        // plain `absolute` there.
        if !ctx.flags.detached && s.get(&POSITION_TYPE) == Some(&PositionMode::Fixed) {
            ec.insert(PositionFixed);
        } else {
            remove_unless_fresh::<PositionFixed>(ec, ctx.fresh);
        }
    },
};
```

Schedule (`plugin.rs`): `app.add_systems(PostUpdate, crate::fixed::sync_fixed_nodes.before(bevy::ui::UiSystems::Prepare))`.
After every `Update` writer of `Node.display` (op drain, interaction restyle),
before layout; the `before` edge gives the command flush (auto sync point —
the same mechanism `apply_js_ops → evaluate_layer_promotions` relies on).
**(pin, T5b)**: one `app.update()` after the ops must show the fixed layout.

Why a marker + system instead of the writer inserting `FixedNode`:
the writer only re-runs when _this node's_ style changes, but the `display:
none` condition lives on an ancestor. One stateless system owning `FixedNode`
also self-heals (un-request, ancestor shows again) with no extra state.

Add `FixedNode` to the bridge-owned list in `ReactNode`'s rustdoc.

### 2.4 Composited layers — a fixed node leaves its ancestors' layers

Decision D5. A fixed member would be captured into its ancestor's layer
texture: cropped to the capture window (it is positioned against the viewport,
not the root), and its root-relative geometry would change on every scroll of
the layer, defeating the cache. So membership stops at it, exactly like a
detached node is outside by construction.

`crates/core/src/layer.rs`:

1. `LayerGeometryGate`: add `added_fixed: Query<(), Added<FixedNode>>` and
   `removed_fixed: RemovedComponents<FixedNode>`; both are **structural** in
   `take_work` (a flip changes membership, and a geometry-only refresh would
   keep the stale map). Clear the reader next to the other two.
2. `sync_layer_geometry`: add `fixed: Query<(), With<FixedNode>>` (12 params),
   passed through `RootSync` → `SubtreeWalk`, plus `any_fixed = !fixed.is_empty()`
   so the hot loop pays nothing when no fixed node exists.
3. `SubtreeWalk::mark_subtree`, first statement (before the hash fold, both
   `Claim` and `Hash` modes):
   ```rust
   // A fixed node is its own layout root — viewport-positioned, unclipped,
   // untouched by its ancestors' transforms: not this layer's content. It and
   // its subtree draw to the screen at their own stack position; promoted
   // itself, its own DFS claims it.
   if self.any_fixed && node != self.dfs_root && self.fixed.contains(node) {
       return;
   }
   ```
4. `RootSync::sync_root`, the `enclosing` walk: `if self.fixed.contains(cursor) { break; }`
   at the top of the `while let Ok(parent) = self.parents.get(cursor)` body —
   nothing above a fixed node encloses it or the roots under it (a promoted
   fixed node is top-level: its quad composites to the screen, depth 1).

`crates/core/src/layer/clip.rs::cascade` — replace the reset with a prune,
**before** the nested-root branch (otherwise an outer cascade would write a
`quads` entry for a promoted fixed node, racing its own top-level entry by
root iteration order):

```rust
if has_fixed && node != dfs_root {
    return; // Not a member (`SubtreeWalk::mark_subtree`): no interior entry.
}
if has_override {
    inherited = None;
}
```

The disappeared `interior` entries are already re-extracted by the existing
rebuild diff (`changed.extend(prev.keys().filter(..))`). Update the module doc
line that says the cascade mirrors the `FixedNode` reset.

`crates/core/src/layer/pick3d.rs` — `transformed_root_of` and
`member_of_layer` climb `ChildOf` to the first entity in the map; from inside
an escaped fixed subtree that lands on the fixed node's _parent_, a member of
the transformed layer, so the real pointer's hits on the (untransformed,
on-screen) fixed node would be suppressed. Give both a
`fixed: &Query<(), With<FixedNode>>` and stop the climb there:

```rust
let member = climb(entity, child_of, |e| {
    membership.node_to_layer.contains_key(&e) || fixed.contains(e)
})?;
let mut root = *membership.node_to_layer.get(&member)?; // None at an escaped fixed node
```

(`member_of_layer`'s existing `None => return false` already covers it.) The
two systems `suppress_transformed_layer_hits` and
`correct_transformed_interactions` gain the query param (6 and 9 params).

Untouched and correct by construction: `resolve_layer_repaints` (purely
map-driven — a non-member pushes no dirt), `cursor.rs`'s
`visually_transformed_members`, the render-side steal, devtools' Layers tab.

Resulting semantics (document them): a fixed node gets none of an ancestor's
`opacity` group alpha, `filter`, `backdropFilter`, `transform3d`, `morphFilter`
or `cache`; put the effect on the fixed node itself (it promotes like any
node). It paints at its own stack index, i.e. above the enclosing layer's
quad — including above members that come _after_ it in tree order.

### 2.5 `transition: { layout }` and shared elements (`crates/core/src/transition/layout.rs`)

A fixed node's taffy `location` is already viewport-space (its taffy parent is
the implicit viewport node, F4), so "the rect in its parent's layout space"
is well defined — the parent frame is the **identity**, not the hierarchy
parent's global. Changes in `drive_layout_transitions` (8 → 11 params:
`fixed: Query<(), With<FixedNode>>`, `fixed_added: Query<Entity, Added<FixedNode>>`,
`fixed_removed: RemovedComponents<FixedNode>`):

```rust
/// `entity`, then its ancestors up to and including the nearest `FixedNode`:
/// the chain bevy's layout walk inherits through (a fixed node is laid out
/// from the viewport — nothing above it reaches it).
fn layout_chain<'a>(
    entity: Entity,
    parents: &'a Query<&ChildOf>,
    fixed: &'a Query<(), With<FixedNode>>,
) -> impl Iterator<Item = Entity> + 'a {
    let mut done = false;
    std::iter::once(entity)
        .chain(parents.iter_ancestors(entity))
        .take_while(move |&e| !std::mem::replace(&mut done, fixed.contains(e)))
}
```

1. **Parent frame** — both sites that read `parents.get(entity) → globals.get(parent)`
   (the measure loop, ≈L638, and the walk-roots loop, ≈L778): `Affine2::IDENTITY`
   when `fixed.contains(entity)`. Today's code is only right while every
   ancestor is a pure translation (`P·T(Δ)·P⁻¹ = T(Δ)`); a scaled/rotated
   ancestor scales the delta of a node it does not transform. This also fixes
   shared-flight seeds (`seed_in_parent_space`) for fixed nodes.
2. **The composition walk** must not carry an ancestor's delta into a fixed
   descendant: skip `fixed` children when pushing `kids`, and compute
   `has_animating_ancestor` over `layout_chain(e, ..).skip(1)` so an animating
   fixed node under an animating ancestor is its own walk root.
3. **Rounding** — `use_rounding`: `layout_chain(entity, ..).find_map(|e| layout_configs.get(e).ok())`
   (F8: bevy restarts at `true` at a fixed node).
4. Same helper for the two remaining ancestor walks (`hold`, the `SharedReflow`
   regressors) — consistency, no visible behavior; no test needed.
5. **Flip = snap.** Entering/leaving `FixedNode` changes the space the stored
   rect was measured in (parent ↔ viewport); easing between them starts from a
   meaningless point. Before the main loop:
   ```rust
   for e in fixed_added.iter().chain(fixed_removed.read()) {
       if let Ok((_, _, mut state, ..)) = states.get_mut(e)
           && state.layout.seeded
       {
           state.layout.reset(); // adopt the new rect silently (mount rule)
       }
   }
   ```

Shared elements need nothing else: both ends are root-space rects
(`shared_tags.rs` reads the outgoing `ComputedNode` + `UiGlobalTransform`),
pairing is by tag/kind/UI root, and a fixed node keeps its UI root.

### 2.6 Verdicts per engine

| Engine                                                                 | Verdict                                                                                                                                                                     |
| ---------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Composited layers (membership, enclosing, hash, cache)                 | **needs code** — §2.4                                                                                                                                                       |
| Interior-clip cascade                                                  | **needs code** — 3 lines, §2.4                                                                                                                                              |
| `transform3d` picking                                                  | **needs code** — climb predicate, §2.4. The fixed node itself is not transformed: **documented limit**                                                                      |
| `transition: { layout }`                                               | **needs code** — §2.5. Flipping fixed on/off snaps: **documented limit**                                                                                                    |
| Shared elements                                                        | **works** once §2.5.1 lands (a fixed node can be either end)                                                                                                                |
| Scroll containers                                                      | **works** (F3, F8): no scroll offset, no content-size contribution, not clipped. Wheel/touch targeting stays geometric (the container under the cursor scrolls) — unchanged |
| `display: none` ancestors                                              | **needs code** — `sync_fixed_nodes`, §2.3                                                                                                                                   |
| `Visibility::Hidden` ancestors (apps, a hidden `<anchor>`/`<surface>`) | **works** (F13)                                                                                                                                                             |
| `<anchor>` / `<root>` / `<surface>` as the fixed element               | **works**: `fixed` = `absolute` there (the writer's `detached` rule); avoids a second writer of the anchor's layout/visibility                                              |
| Fixed nodes _inside_ a `<surface>` / `<root>`                          | **works**: viewport = that root's camera target (F5)                                                                                                                        |
| `globalZIndex`                                                         | **works** (F10): the fixed node becomes a stack root; documented as the way to float it above everything                                                                    |
| `layoutRounding`                                                       | **works**, restarts at a fixed node (F8): **documented**                                                                                                                    |
| `{ animated }` insets/size, `transition: { size }`                     | **works**, and cheaper than on an absolute node: only the fixed node's own taffy tree is dirtied                                                                            |
| Hover/press variants carrying `positionType`                           | **works**: the restyle runs `POSITION_FIXED_WRITER`; `sync_fixed_nodes` applies it the same frame                                                                           |
| Events                                                                 | **works**: click/pointer bubbling is `ChildOf`-based (`reconcile::climb`), like the DOM                                                                                     |

Deleted: nothing in code. In docs, the "There is no `"fixed"`" passage.

---

## 3. Edge cases and tests

Real layout = `crate::transition::layout_tests::layout_app()` (target 1000×100,
scale 1; `pub(crate)`, already reused by `reconcile/clip_pick_tests.rs`). Add
`bevy::ui::update::update_clipping_system.after(ui_layout_system)` where clips
matter (that file's precedent).

**`crates/core/src/fixed.rs` tests**

- T1 `fixed_node_ignores_parent_offset_scroll_and_transform` — root row → 200×100 `overflow: scroll_y` port with `ScrollPosition(0, 40)`, margin and a `UiTransform` scale 2 → child `{Absolute, right: 10, bottom: 10, 20×20} + FixedNode`. Its global translation is `(980, 80)`; unchanged after changing the port's scroll, margin and transform.
- T2 `fixed_child_leaves_the_parents_content_size` — the port with a 50×300 in-flow child: `crate::scroll::scroll_range(port)` is identical with and without an extra 50×5000 fixed child.
- T3 `fixed_node_resolves_against_the_viewport` — `width: 50%`, `left: 10%` → 500 wide at x 100; `left: 0, right: 0` with no width → 1000 wide; no insets → top-left; `justify_self: Center` centers. Pins F6/F7.
- T4 `fixed_node_escapes_ancestor_clip` — fixed child (and a grandchild) under an `overflow: clip` parent carry no `CalculatedClip`. (Bevy tests this too; ours pins the upgrade path.)
- T5a `bevy_fixed_node_survives_a_display_none_parent` — plain `FixedNode` under a `display: none` parent: non-zero `ComputedNode.size`, no `CalculatedClip`. Documents why T5b's system exists; if it ever fails, delete `sync_fixed_nodes`.
- T5b `display_none_ancestor_unfixes_and_restores` — `(ReactNode, PositionFixed)` child, `sync_fixed_nodes.before(propagate_ui_target_cameras)` added to the harness: parent `display: none` → no `FixedNode`, size zero, in **one** update; back to `flex` → `FixedNode` and the viewport rect return. A grandparent `none` works the same.
- T5c `dropping_the_request_removes_fixed_node` — remove `PositionFixed` → `FixedNode` gone; a `FixedNode` on a non-`ReactNode` entity is left alone.

**Op path (`style/writers/tests.rs`, `op_app`)**

- T6 `position_fixed_writer_stamps_and_clears` — create `{ positionType: "fixed" }` → `PositionFixed` + `Node.position_type == Absolute`; update to `"absolute"` → marker gone; `styleUnset` → gone; a `<root>` with `fixed` → no marker; a `hoverStyle: { positionType: "fixed" }` + `Interaction::Hovered` (with `apply_interaction_styles`) → marker.
- T7 decode (`style/decode_tests.rs` / `ui_map/tests.rs::node_covers_layout`) — `"fixed"` decodes; `"sticky"` warns kind `positionType` and falls back to relative. `style::ts::tests::core_style_ts_is_current` covers the TS.

**Layers (`layer.rs` `geometry_world`, `layer/clip.rs` `clip_world`, `pick3d.rs` tests)**

- T8 `fixed_subtree_is_not_a_layer_member` — root L, children M and F(`FixedNode`) with child C: map has L and M only. Promote F: `F→F`, `C→F`, `enclosing[F] == None`. A promoted R under an unpromoted F under L: `enclosing[R] == None`.
- T9 `fixed_flip_rebuilds_membership` — settle, insert `FixedNode` on M with no other change: `rebuilds` bumps and M leaves; remove it: M returns. Moving F afterwards does not put L in `LayerRepaintState.dirty`.
- T10 `fixed_node_has_no_interior_clip_entry` — clipping root with a fixed child: no `interior` entry for it or its child; a promoted nested fixed root's `quads` entry is `None` (run with both spawn orders of the two roots).
- T11 `escaped_fixed_node_keeps_real_pointer_hits` — the `suppression_scopes_hits_per_pointer` shape plus a `FixedNode` child of the member, absent from the map: mouse picks on it survive, the virtual pointer's are dropped.

**Layout transitions (`transition/layout_tests.rs`, `layout_shared_tests.rs`)**

- T12 `fixed_descendant_does_not_ride_an_ancestors_flip` — row `[A][B(layout)[F fixed]]`, remove A: mid-flight B's x is eased, F's global equals bevy's pristine value and F is not in `Written`.
- T13 `fixed_node_eases_in_viewport_space` — parent with `UiTransform` scale 2, fixed child with a layout spec, `left` 0 → 100: change frame shows the old position, halfway is +50 (not +100).
- T14 `fixed_flip_snaps_the_layout_channel` — node with a layout spec, settled; insert `FixedNode`: the flip frame's global is bevy's own, `!in_flight`.
- T15 `fixed_node_restarts_layout_rounding` — the `layout_channel_honors_the_nearest_layout_config_override` shape with a fixed node under a `use_rounding: false` ancestor: the change frame reproduces the _rounded_ rect.
- T16 `shared_seed_lands_in_root_space_on_a_fixed_node` — a `shared_seed_*`-style test: incoming fixed node under a scaled parent; the seed frame shows the seed rect where the outgoing node was.

Not headless-testable (leave to `--shoot` / the user): paint order against
the enclosing layer's quad, retained-extraction behavior when a node leaves a
layer on the flip.

---

## 4. Commit sequence

The engine commits land first: they only reference Bevy's `FixedNode`, are
inert until something inserts it, and their tests insert it by hand. Each
builds and passes on its own.

1. `fix(layer) fixed nodes are not members of their ancestors' layers` — §2.4 (`layer.rs`, `layer/clip.rs`, `layer/pick3d.rs`), T8–T11.
2. `fix(transition) layout transitions treat a fixed node as a layout root` — §2.5, T12–T16.
3. `feat(style) positionType "fixed"` — §2.2 + §2.3 (`props/layout.rs`, `keywords.rs`, `ui_map.rs`, `writers/layout.rs` + `writers/mod.rs`, new `fixed.rs`, `lib.rs`, `plugin.rs`, `bridge.rs` rustdoc), regenerated `js/src/generated/style.ts`, T1–T7.
4. `feat(demos) fixed positioning example` — §5.
5. `docs positioning: fixed` — guide pages, CHANGELOG, `CLAUDE.md`/`CONTEXT.md` lines, `npm run generate` output if any.

Before handing off: `cargo test -p bevy_react_core --lib`, `cargo clippy --workspace --all-targets`, `cargo hack check -p bevy-react --each-feature --no-dev-deps --features bevy/x11` (no new bevy feature is used — `FixedNode` is plain `bevy_ui`, already imported by `layer/clip.rs`), `npm run lint`, `npm run docs`.

---

## 5. Demos and docs

### Demos — `examples/demos/ui/src/demos/layout/PositioningDemo.tsx` (nav label "Positioning", `demos.ts:98`)

Context that shapes the demo: every page sits inside the content column, which
is a scroll container **and** a `morphFilter` layer (`App.tsx` `contentStyle`).
So a fixed node on a page is positioned against the **window**, escapes the
page's scroll clip, and (D5) is outside the page-switch morph.

- New fifth example, "Fixed to the window" (own `DemoRow`): a `Stage` holding a small `overflowY: "scroll"` list and one badge whose `positionType` is a `Radio` `absolute | fixed`, **default `absolute`** (the Details modal mounts a second instance; two fixed badges would overlap, and the page should not start with a stray overlay).
  - `absolute`: the badge is pinned inside the list, scrolls with it, is clipped by it.
  - `fixed`: the badge (label "fixed") jumps to the window's bottom-right (`right: 24, bottom: 24`) and stays there while the list **and** the page scroll.
  - No `globalZIndex`: tree order already puts it above the page and below the modal and the compact nav drawer, which is the behavior to show.
- Extend `POSITION_OPTIONS` / the `Position` type only for this card (the "Out of the flow" card stays two-way).
- Update the page `info` paragraph and the example's `info` (snippet lines ≤ 40 chars, the repo's rule).
- No `bevy.ts` change (core property). No docs screenshot references this page.

### Guide

- `docs/guide/layout/positioning.md` (covers `style.positionType` already): frontmatter description; intro sentence; a `## Fixed` section — containing block = the viewport of the UI camera the node belongs to (window, `<surface>` texture, `<root>`), insets/`%`/stretch as for absolute, set an inset on each axis (no inset = top/left, or `alignSelf`/`justifySelf`), not clipped, not scrolled, takes no space and no scroll range, hidden by an ancestor's `display: none`, paints in tree order (use `globalZIndex` to float); rewrite "Overlays outside the tree" (drop "There is no fixed", keep `<root>` for UI that must also leave the app's _tree_, `<anchor>` for 3D); `## Limits`: differences from CSS (see D5/D8), `"sticky"`/`"static"` still unsupported, the fallback sentence now lists three keywords.
- `docs/guide/styling/z-index.md`: the popover example with `positionType: "fixed"` + `globalZIndex`.
- `docs/guide/styling/layers.md` → Limits: a fixed descendant is outside the layer (no group opacity/filter/3D/morph from ancestors).
- `docs/guide/styling/overflow.md` → Limits: a fixed child is neither clipped nor scrolled.
- `docs/guide/styling/3d-transforms.md` → Limits: a fixed descendant stays flat.
- `docs/guide/animations/style-transitions.md` → layout section: a fixed node eases in viewport space and does not ride an ancestor's layout transition; switching to/from fixed snaps.
- `docs/guide/styling/layout-rounding.md`: inheritance also restarts at a fixed node.
- `docs/guide/elements/root.md`: one line on `<root>` vs a fixed node.

### CHANGELOG (`## [Unreleased]` → `### Added`)

- **`positionType: "fixed"`.** Positions a node against the viewport of its UI
  camera (the window, or a `<surface>`'s texture) instead of its parent: it
  takes no space, ignores ancestor scrolling, clipping and transforms, and
  keeps its place in the tree for stacking, events and lifetime. Built on
  Bevy 0.20's `FixedNode`. Unlike CSS, a filtered or transformed ancestor does
  not become its containing block, and it is not part of an ancestor's
  composited layer.

### Other text

- `CLAUDE.md`: one sentence in "composited layers" (a `FixedNode` is a membership boundary) and one in the layout-transition paragraph (parent frame = identity). `CONTEXT.md`: a "Fixed node" term if the implementer wants the vocabulary pinned.
- Generated: `npm run style:generate` (required), then `npm run generate` and commit whatever else moves (expected: nothing — `docs/reference/bevy.ts` has no core style).

---

## 6. Decisions

| #   | Choice                                       | Recommended                                                                       | Why                                                                                                                                                                                                                                                                                                                                                                                        |
| --- | -------------------------------------------- | --------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| D1  | Key name                                     | `positionType: "fixed"`; **no** `position` alias                                  | The property already exists under that name; a second key for one property is a new merge rule. Cheap to add later.                                                                                                                                                                                                                                                                        |
| D2  | Layout mode                                  | `Absolute` + `FixedNode`                                                          | F6: only then do `right`/`bottom`, `%` and stretching mean what CSS means.                                                                                                                                                                                                                                                                                                                 |
| D3  | Who writes `FixedNode`                       | `PositionFixed` marker (own writer) + `sync_fixed_nodes`                          | Single owner; survives the spawn-bundle path; the only way to react to an _ancestor's_ `display`.                                                                                                                                                                                                                                                                                          |
| D4  | `display: none` ancestors                    | Hide, by withdrawing `FixedNode`                                                  | Web-faithful (a hidden subtree is not laid out, drawn or picked), no `Visibility` ownership fight with `<anchor>`/`<surface>`/apps. Alternative: write `Visibility::Hidden` — needs a restore path and clobbers app writes.                                                                                                                                                                |
| D5  | Fixed node inside a promoted layer           | It **leaves** the layer                                                           | As a member it is cropped to the capture window and re-captures the layer on every scroll — broken, not merely different. CSS would instead make a filtered/transformed ancestor its containing block (not expressible: taffy positions absolute nodes against the direct parent only) and fade it with an `opacity` ancestor (lost; documented). Same story as `<root>`/`<anchor>` today. |
| D6  | Paint order                                  | Tree order; no automatic raise                                                    | F10. CSS paints positioned boxes above in-flow content; bevy_ui has no such tier for `absolute` either. An implicit `GlobalZIndex` would fight `GLOBAL_Z_INDEX_WRITER`. Document `globalZIndex`.                                                                                                                                                                                           |
| D7  | `fixed` on `<root>`/`<surface>`/`<anchor>`   | Same as `absolute`, silently                                                      | They are already viewport-level; avoids two writers on the anchor overlay.                                                                                                                                                                                                                                                                                                                 |
| D8  | Transformed ancestor                         | Document: it never becomes the containing block (2D `transform` or `transform3d`) | F8 is Bevy's model; emulating CSS needs layout Bevy cannot express.                                                                                                                                                                                                                                                                                                                        |
| D9  | `transition: { layout }` across a fixed flip | Snap                                                                              | The two rects are in different spaces. A glide between them is what `sharedTag` is for.                                                                                                                                                                                                                                                                                                    |
| D10 | No inset on an axis                          | Top/left of the viewport; document "set an inset per axis"                        | CSS's static position would need the in-flow position of a node that is not in the flow.                                                                                                                                                                                                                                                                                                   |
| D11 | Viewport inside `<surface>`/`<root>`         | That root's camera target                                                         | F5 — free, and the natural reading.                                                                                                                                                                                                                                                                                                                                                        |
| D12 | Wheel/hover over a fixed node                | Unchanged (geometric)                                                             | Not specific to fixed; an overflowing `absolute` child behaves the same today.                                                                                                                                                                                                                                                                                                             |
| D13 | `FixedNode` on React nodes                   | Bridge-owned (rustdoc)                                                            | `sync_fixed_nodes` removes one it did not ask for.                                                                                                                                                                                                                                                                                                                                         |

**Out of scope — anchors as `FixedNode` children.** It would work mechanically
(F3/F9: no layout or scroll-range contribution, no clip — what `AnchorLayer`
exists for), and the real hierarchy would give `<anchor>` its declaring tree's
visibility and, inside a `<surface>`, that surface's camera. Against it: every
anchor becomes its own taffy root and, to keep today's "below the app tree"
order, its own `GlobalZIndex(-1)` stack root (crowded scenes have hundreds);
the `detached` machinery (`bridge.detached`, recorded React parents, the
anchor crate's reparenting and its `Absolute` self-heal) would be rewritten for
no change a user can see in the window tree. Worth revisiting only if
anchors inside `<surface>` become a request; not as part of this work.

---

## 7. Risks and perf

- **Per-frame cost with no fixed nodes:** one empty query iteration (`sync_fixed_nodes`), two empty gate queries, a bool test per member in `mark_subtree`, one writer bit that only runs when `positionType` is set. Nothing a benchmark should see; if `table-ops`/`layers-stress` move, look at `SubtreeWalk::mark_subtree` first (the `any_fixed` short-circuit), then the child push in `drive_layout_transitions`.
- **Per fixed node:** one extra taffy root — an implicit viewport node, a `compute_layout` call (a cache hit when clean) and a geometry-walk entry every frame (F2). Fine for overlays; a list of hundreds of fixed rows is the wrong tool (say so in the guide). Upside: animating a fixed node's insets/size relayouts only its own tree.
- **`sync_fixed_nodes`** walks each fixed node's ancestors every frame: O(fixed × depth). Gate on `Changed<Node>` only if a profile shows it.
- **Each `FixedNode` insert/remove is a full membership rebuild** (structural signal). Rare by nature (mount, a style flip, a `display` toggle above it).
- **Semantic surprise (the biggest risk):** inside a layer the fixed node ignores the ancestor's opacity/filter/morph. In the demos gallery that means a fixed badge pops on page switch instead of morphing with the page. Acceptable and documented, but it is the behavior most likely to be reported as a bug.
- **Unverified by running:** F14 (`display: none` escape), F7 (no-inset placement), the auto sync point after `sync_fixed_nodes`, and what retained extraction does on the frame a node leaves a layer (expected fine: its transform changes and bevy removes its `CalculatedClip`, both re-extract it). T5a/T3/T5b pin the first three; the last needs eyes.
- **What to look at with `--shoot`** (`cargo run -p demos -- --shoot "Positioning" out.png`, the radio driven with `--record` + `--input` clicks, or a temporary default of `fixed`):
  - the badge sits at the window corner, outside the content column, uncut by the page's scroll clip, at 1280×832 and at `--size 390x844`;
  - scrolled page (wheel `--input` is not available — check by eye or set `scrollTop`): badge unmoved;
  - devtools Layers tab: the page layer's repaint count does not climb while only the badge exists/moves;
  - "Layers", "3D transforms" and "Shared elements" pages unchanged (the engine commits must be invisible without a fixed node).
