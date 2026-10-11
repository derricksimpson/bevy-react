# Plan: interior clips by prefix strip, exact quad clipping, clip reveals under `transition: { layout }`

Audit sections: §3.4 "Simpler interior clips", §3.1 "The layout-transition clip
limitation is gone", background §1.1.2.

Everything below was read, not run (no cargo). Our files were being edited by
another agent while I read (`pick_clip.rs` removal, `layer.rs`,
`transition/layout.rs`), so our code is cited by symbol, not by line.
`B/` = `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`.

## Verdict

1. **Prefix strip: do it.** It is correct for every case the cascade handles
   that can produce pixels, it is about 130 lines smaller, and it makes the
   extract-window swap sparse (today the swap rewrites every clipped member of
   every layer every frame, even when the interior clip equals the real one —
   the demos shell's page layer is exactly that case).
2. **`clip_polygon` for composite quads: only for rotated clippers.** It does
   not replace `clip_aabb`.
3. **§3.1 already works**, outside and inside promoted layers, with no engine
   change. It needs tests, a guide recipe and a demo. The reveal is anchored
   at the box's **center**, which the audit did not notice.

## 1. Verified facts

### How 0.20 builds `CalculatedClip`

| Fact                                                                                                                                                                                                                                         | Source                                                                                            |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `CalculatedClip` is `Rects(SmallVec<[CalculatedClipRect; 2]>)` or `FullyClipped`. `Default` is an empty `Rects` list.                                                                                                                        | `B/bevy_ui-0.20.0/src/ui_node.rs:2774-2787`                                                       |
| A `CalculatedClipRect` is `{ rect: Rect, world_to_clip_local: Affine2 }`. **No clipper entity is stored.** It derives `PartialEq` (plain `f32` equality).                                                                                    | `ui_node.rs:2755-2762`                                                                            |
| A node's component holds its **inherited** list. Its own overflow rect is appended only to the list handed to its children.                                                                                                                  | `B/bevy_ui-0.20.0/src/layout/clipping.rs:73-87`, `:89-106`                                        |
| **Order is outermost first.** The child list is the inherited list plus one `push_rect` at the end; each child gets a `clone()`.                                                                                                             | `clipping.rs:97-103`, `:108-115`; `ui_node.rs:2832-2839`                                          |
| The appended rect is `ComputedNode::resolve_clip_rect` (object-centered, physical px, padding box by default) with `world_to_clip_local = transform.try_inverse()`: the clipper's **full** `UiGlobalTransform`, rotation and scale included. | `clipping.rs:97-102`; `ui_node.rs:247-275`, `:1699-1703`                                          |
| A `Visible` axis has an infinite extent in the rect.                                                                                                                                                                                         | `ui_node.rs:265-272`                                                                              |
| **Resets:** `OverrideClip` or `FixedNode` on a node discards the inherited list (the node itself gets no component; its descendants start from its own overflow).                                                                            | `clipping.rs:63-66`                                                                               |
| **`FullyClipped` has exactly two sources:** `display: none` on the node, and a clipping node whose transform is not invertible (`determinant == 0`). It propagates to every descendant until a reset.                                        | `clipping.rs:68-71`, `:90-96`, `:104-106`; `B/bevy_ui-0.20.0/src/ui_transform.rs:239-241`         |
| The component is written only when the value differs (`!=`), inserted or removed through `Commands` otherwise. So `Changed<CalculatedClip>` and `RemovedComponents<CalculatedClip>` are exact signals.                                       | `clipping.rs:74-87`                                                                               |
| The system walks the whole tree every frame (not retained).                                                                                                                                                                                  | `clipping.rs:13-35`                                                                               |
| It runs in `PostUpdate`, `UiSystems::PostLayout`. The sets are `chain_weak`, which still orders it after `ui_layout_system` (they conflict on `UiGlobalTransform`).                                                                          | `B/bevy_ui-0.20.0/src/lib.rs:157-167`, `:205`; `B/bevy_ecs-0.20.0/src/schedule/config.rs:579-603` |
| Picking and focus test the node's inherited clip with `CalculatedClip::contains_point`, through each rect's `world_to_clip_local`.                                                                                                           | `B/bevy_ui-0.20.0/src/picking_backend.rs:205-209`, `focus.rs:276-279`, `ui_node.rs:2819-2828`     |
| Text input fields append their content box to whatever clip the node carries, at extraction. So a swapped clip composes with it for free.                                                                                                    | `B/bevy_ui_render-0.20.0/src/text.rs:19-35`                                                       |
| `bevy::ui::update::update_clipping_system` is public, so tests can run it.                                                                                                                                                                   | `B/bevy_ui-0.20.0/src/update.rs:8`                                                                |

**Recognizing an equal prefix.** With no reset between a layer root R and a
member M, `M.rects` is `R.rects` plus the clippers from R down, and the leading
part is a clone of the very values R received in the same pass. So
`m.starts_with(r)` (slice `PartialEq`) is bit-exact. `NaN` never compares
equal; a `NaN` only arises from a near-singular clipper (inverse overflow),
and then Bevy itself rewrites the component every frame.

### `clip_polygon`

```rust
// B/bevy_ui_render-0.20.0/src/clipping.rs:16-20, module public at lib.rs:11
pub fn clip_polygon<T: Copy>(
    clip: Option<&CalculatedClip>,
    vertices: &[(Vec2, T)],                    // convex, boundary order
    interpolate: impl Fn(T, T, f32) -> T + Copy,
) -> SmallVec<[(Vec2, T); 16]>                 // convex polygon = a triangle fan
```

- `None` returns the input; `FullyClipped` returns empty; a result under 3
  vertices returns empty (`clipping.rs:26-31`, `:61-63`).
- Sutherland-Hodgman per clip rect, four edges each, positions lerped and `T`
  interpolated at each crossing (`:36-59`, `:68-109`).
- **Infinite edges are skipped** (`if edge.is_finite()`, `:47`), so an open
  `Visible` axis needs no sentinel. The NaN bug of `clip_aabb` cannot occur
  here.
- Stock prepare pushes the result as a fan: indices `(0, i, i+1)`
  (`B/bevy_ui_render-0.20.0/src/lib.rs:2304-2341`, glyphs `:2364-2411`).
- A quad that only touches a clip edge yields a zero-area polygon with 3 or
  more vertices, not an empty one (`0. <= distance` is inclusive, `:85-92`).

### Retained extraction

- A node is re-extracted when `Changed<CalculatedClip>` fires or the component
  is removed (`B/bevy_ui_render-0.20.0/src/lib.rs:439-457`, `:454`, `:520`).
  Gradients, box shadows, texture slices and `UiMaterial`s filter the same way
  (`gradient.rs:366`, `box_shadow.rs:232`, `ui_texture_slice_pipeline.rs:240`,
  `ui_material_pipeline.rs:351`).
- `gradient` is still a private module (`lib.rs:12`), so the main-world swap
  stays the only way to reach gradient items.
- `extract_ui_material_nodes::<M>` is in `RenderUiSystems::ExtractBackgrounds`
  (`ui_material_pipeline.rs:70`), which our `ExtractUiItems` umbrella brackets.
  The "Known gap: a `UiMaterial` extractor is … unordered" note in
  `crates/core/src/layer/render/clip.rs` is stale.

### Our code today

- `crates/core/src/layer/clip.rs`: `sync_layer_clips` re-runs Bevy's cascade
  per layer root with no inherited clip (`cascade`, `children_clip`), gated by
  `ClipSyncMemory`, `Changed<Node>`, `OverrideClip` add/remove and
  `LayerMembership::clip_touched`. Output is `LayerClips { interior, quads,
changed }`; `interior` has an entry for every member, `quads` one per root
  as an `Option<Rect>` (`clip_aabb`). `mark_reclipped_members` calls
  `set_changed()` on every entity in `changed`.
- `crates/core/src/layer/render/clip.rs`: `swap_in`/`swap_out` replace each
  member's `CalculatedClip` with `interior[member]` for the extract window,
  unconditionally, clone per member per frame. `clip_quad` clamps an
  axis-aligned quad to a `Rect` and shifts UVs.
- `crates/core/src/layer/render.rs` `prepare_layer_composites`: content and
  backdrop quads go through `clip_quad(.., layer.quad_clip)` and
  `push_quad_vertices` (6 non-indexed vertices; `DrawLayerComposite` draws
  `batch.range` with `pass.draw`). A `transform3d` quad keeps its full
  geometry and passes `quad_clip` as a screen rect uniform that
  `composite.wesl` tests per fragment.
- `crates/core/src/render_target.rs` `clipped_out`: a portal's box against
  `clip_aabb` of its inherited clip, to idle the portal camera.
- `crates/core/src/plugin.rs`: `drive_layout_transitions`
  `.after(UiSystems::Layout).before(UiSystems::PostLayout)`;
  `sync_layer_clips` and `mark_reclipped_members` `.after(PostLayout)`.
  `update_clipping_system` has `Commands`, so the strict `.after` edge gets a
  sync point: the inserted and removed components are visible to
  `sync_layer_clips` the same frame (the top-level quad clip already relies
  on this).

What each consumer needs:

| Consumer                   | Needs                                                                                                                   |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Swap                       | member → the clip to extract with inside the capture                                                                    |
| Retained flagging          | the members whose in-capture clip differs from what their render data holds, with their real clip unchanged             |
| Content and backdrop quads | root → the clip of the quad: the root's inherited clip (top-level), or the part of it below the enclosing root (nested) |
| `transform3d` quad         | the same, as one screen-space rect                                                                                      |
| Portal culling             | "is this box entirely outside its clip", erring on "visible"                                                            |

### Where the audit is wrong or out of date

- **"Handles rotation and scale correctly" is already true.** The bump commit
  (`775f69b`) ported `cascade`/`children_clip` to clip lists with
  `transform.try_inverse()`. Interior clips are already affine. The only
  inexact piece left is the quad clamp (`clip_aabb`: a rotated clipper
  contributes its bounding box).
- **"Removes the documented `OverrideClip` divergence" is wrong.** An
  `OverrideClip` escapee inside a layer subtree is painted into the layer's
  texture, and the texture is drawn as one clipped quad. No clip bookkeeping
  changes that. The divergence stays documented.
- **§3.1 "inside promoted layers it also needs the affine interior clips from
  item 4" is already satisfied** (same port). §3.1 does not depend on §3.4.
- **§3.1 "restore the accordion, drop the documented limitation":** nothing
  was removed. `git log -S"Hide docs" -- examples/demos` finds one commit,
  `afcc42e`, which shipped the fold in its workaround form (fade, out of
  flow, body glide). No guide page documents the limitation; only the memory
  note `flip-clip-ignores-scale` does. The guide's "one that collapses to
  zero size snaps" is a different rule and stays true.
- **§3.1 misses the anchor.** See "§3.1 precisely" below.
- "Use it for `clipped_out` too": possible, not worth it (Decision 4).

### §3.1 precisely

Order in `PostUpdate`: `ui_layout_system` writes pristine `UiGlobalTransform`
→ `drive_layout_transitions` composes `G' = P'·T(Δ)·P⁻¹·G·S(s)` into the
animating node → `update_clipping_system` reads `G'`.

The clip a node hands its children is its **laid-out** local rect (the new
size) mapped through `G'`. `S(s)` is shown size over laid-out size, so that
is exactly the **eased box**. Rendering clips there (`clip_polygon` in stock
prepare) and so does picking (`contains_point`). In 0.19 the clip used the
translation only, so the box clipped at its new size at once.

- **Outside layers: yes**, an `overflow` clip container with
  `transition: { layout }` reveals and hides at the eased box.
- **Inside a promoted layer: yes.** `children_clip` uses the same
  `try_inverse` + `resolve_clip_rect`. The clipper's `UiGlobalTransform`
  changes every flight frame, which re-cascades its root (`clip_touched`) and
  changes the member geometry hash (`fold_member_geometry` folds the linear
  part and the root-relative translation), so the layer re-captures every
  flight frame and caches again on settle. The prefix strip keeps this by
  construction.

Three things an author must know (they shape the recipe, tests and docs):

1. **Children ride the box's center.** `Δ` is shown center minus laid-out
   center, and children compose from the translated, unscaled frame
   (`drive_layout_transitions`; pinned by
   `children_ride_translation_but_not_scale`). Content pinned to the top edge
   is therefore displaced by **half the size change** on the change frame:
   expanding, it slides in from above at half speed; collapsing, it **jumps
   down by half the height on the first frame**. Content **centered** in the
   wrapper (`justifyContent: "center"`, `flexShrink: 0`; taffy centers with
   negative free space, `B/taffy-0.14.0/src/compute/common/alignment.rs:85`)
   has the same center as the wrapper in both states and is continuous both
   ways. The reveal then crops top and bottom equally.
2. **To zero snaps** (`LayoutChannel::drive`, `to_zero`; a zero laid-out size
   has no scale). Collapse to `1`, not `0`.
3. **From zero grows about the final center** (`from_zero`), not from the top
   edge. Starting from `1` keeps the top edge fixed.

A wrapper clips shadows that reach outside its child (the demos `Card` has a
50 px shadow below it).

## 2. Design

No wire format or TS typing changes. No generated files change.

### 2.1 Interior clips: a strip count per member (`crates/core/src/layer/clip.rs`)

A member's clip inside its layer's capture is its real `CalculatedClip` minus
the leading rects that belong to its layer root's inherited list. Store the
**count**, not a second clip, and only where it is not zero.

```rust
#[derive(Resource, Debug, Default)]
pub struct LayerClips {
    /// Member → how many leading rects of its real clip come from above its
    /// layer root. Only members with at least one: the rest extract as is.
    pub strip: EntityHashMap<usize>,
    /// Active root → the clip of its composite quad. Absent = unclipped.
    pub quads: EntityHashMap<QuadClip>,      // a plain `Rect` until commit 4
    /// Members whose strip count appeared, changed or went away.
    pub changed: Vec<Entity>,
}

/// How many leading rects of `member` are `above`'s whole list. 0 when
/// `above` is unclipped or fully clipped, when `member` is fully clipped, or
/// when the lists differ (an `OverrideClip`/`FixedNode` reset in between).
fn inherited_len(member: &CalculatedClip, above: Option<&CalculatedClip>) -> usize {
    match (member.rects(), above.and_then(CalculatedClip::rects)) {
        (Some(m), Some(a)) if m.starts_with(a) => a.len(),
        _ => 0,
    }
}
```

`sync_layer_clips` (same slot: `PostUpdate`, after `sync_layer_geometry` and
`UiSystems::PostLayout`):

```rust
pub fn sync_layer_clips(
    real: Query<&CalculatedClip>,
    changed: Query<Entity, Changed<CalculatedClip>>,
    mut removed: RemovedComponents<CalculatedClip>,
    membership: Res<LayerMembership>,
    mut seen_rebuilds: Local<u64>,
    mut clips: ResMut<LayerClips>,
) {
    let refresh = |e: Entity, clips: &mut LayerClips| {
        let own = real.get(e).ok();
        // As a member (a root is a member of itself: its own paint is
        // unclipped inside its own capture).
        if let Some(&root) = membership.node_to_layer.get(&e) {
            let k = own.map_or(0, |c| inherited_len(c, real.get(root).ok()));
            let prev = if k == 0 { clips.strip.remove(&e) } else { clips.strip.insert(e, k) };
            if prev.unwrap_or(0) != k { clips.changed.push(e); }   // compare first
        }
        // As an active root: its quad draws in the enclosing capture, or on
        // screen.
        if let Some(&outer) = membership.enclosing.get(&e) {
            let quad = own.and_then(|c| {
                let k = outer.map_or(0, |o| inherited_len(c, real.get(o).ok()));
                QuadClip::new(c, k)
            });
            /* insert/remove, compare before write */
        }
    };
    if *seen_rebuilds != membership.rebuilds {
        *seen_rebuilds = membership.rebuilds;
        removed.clear();
        // Rows of nodes that left every layer: flag, drop.
        // strip.retain(member still in node_to_layer, else push to changed)
        // quads.retain(root still in enclosing)
        for &member in membership.node_to_layer.keys() { refresh(member, &mut clips); }
        return;
    }
    if membership.node_to_layer.is_empty() { removed.clear(); return; }
    let mut touched = changed.iter().chain(removed.read())
        .filter(|e| membership.node_to_layer.contains_key(e)).peekable();
    if touched.peek().is_none() { return; }       // idle: no `ResMut` deref
    for e in touched { refresh(e, &mut clips); }
}
```

Why a per-entity refresh is enough on non-rebuild frames: a member's count
depends on its own list and its root's. If the root's list changes with no
reset in between, the member's list changes too (it contains it), so the
member is in `changed` itself. With a reset in between, the count is 0 before
and after.

`mark_reclipped_members` is unchanged.

Deleted: `cascade`, `children_clip`, `ClipSyncMemory`, the `nodes`,
`changed_nodes`, `changed_root_clips`, `added_overrides`, `removed_overrides`,
`roots`, `root_markers`, `children` params, `InteriorClips`. In
`crates/core/src/layer.rs`: `LayerMembership::{syncs, clip_touched}`, the
`clip_set` bookkeeping and `clip_touched` argument of
`LayerGeometryGate::take_work`, and the `Option<&Node>` in `changed_geometry`
if nothing else reads it. `clip_aabb` and the test helper `screen_clip` stay.

### 2.2 The swap (`crates/core/src/layer/render/clip.rs`)

`swap_in` iterates `clips.strip` only (every row is a real swap, so the
"whichever side is smaller" branch goes):

```rust
for (&entity, &k) in &clips.strip {
    let Ok(mut e) = world.get_entity_mut(entity) else { continue };  // stale row
    let Some(mut clip) = e.get_mut::<CalculatedClip>() else { continue };
    let Some(rects) = clip.rects() else { continue };
    let mut interior = CalculatedClip::default();                    // empty = unclipped
    for r in &rects[k.min(rects.len())..] { interior.push_rect(r.rect, r.world_to_clip_local); }
    stash.push((entity, std::mem::replace(clip.bypass_change_detection(), interior)));
}
```

`swap_out`, `SwappedClips`, the schedule bracket and `ExtractUiItems` are
unchanged. Delete the stale `UiMaterial` "known gap" sentence.

### 2.3 Quad clips (`layer/clip.rs`, `layer/render/clip.rs`, `layer/render.rs`)

```rust
/// What clips a layer root's composite quad.
#[derive(Debug, Clone, PartialEq)]
pub enum QuadClip {
    /// Axis-aligned clippers only (translate/scale, the FLIP scale
    /// included): exactly this screen rect. Empty = draw nothing.
    Rect(Rect),
    /// A rotated or skewed clipper: the list itself, for `clip_polygon`.
    Rotated(Arc<CalculatedClip>),
}

impl QuadClip {
    /// `clip` minus its first `skip` rects. `None` = unclipped.
    fn new(clip: &CalculatedClip, skip: usize) -> Option<Self>;
    //  FullyClipped            → Some(Rect(Rect::default()))
    //  nothing left            → None
    //  every matrix2 has x_axis.y == 0 && y_axis.x == 0 → Rect(bounds)
    //  else                    → Rotated(Arc::new(list))
    /// Exact for `Rect`, the bounding box for `Rotated`.
    pub fn bounds(&self) -> Rect;
}
```

`clip_aabb` is split over a slice helper (bounds of `&[CalculatedClipRect]`)
in commit 3 already, because a nested root's quad needs the bounds of a
tail of its list. `QuadClip::new` and `render_target::clipped_out` both end
up on it. Its `UNBOUNDED` clamp stays (commit `7ce05a7`).

`ExtractedLayer::quad_clip` becomes `Option<QuadClip>`
(`clips.quads.get(&root).cloned()`: a `Rect` copy or an `Arc` clone, no
allocation).

Render side, in `layer/render/clip.rs`:

```rust
/// The layer's quad under a rotated clip, staged as a triangle list.
/// Returns the vertex count, 0 when nothing shows.
pub fn clip_quad_rotated(
    min: Vec2, size: UVec2, inset: f32, clip: &CalculatedClip,
    mut push: impl FnMut(Vec2 /* position */, Vec2 /* uv */),
) -> u32 {
    let Some(q) = clip_quad(min, size, inset, None) else { return 0 };
    let corners = [q.pos_min, vec2(q.pos_max.x, q.pos_min.y), q.pos_max,
                   vec2(q.pos_min.x, q.pos_max.y)].map(|p| (p, ()));
    let poly = bevy::ui_render::clipping::clip_polygon(Some(clip), &corners, |_, _, _| ());
    if poly.len() < 3 { return 0 }
    // UV is affine in position (the capture rect maps to 0..1): recompute
    // it per vertex, as `clip_quad` does, instead of interpolating.
    let uv = |p: Vec2| (p - min) / size.as_vec2().max(Vec2::ONE);
    for i in 1..poly.len() - 1 {
        for p in [poly[0].0, poly[i].0, poly[i + 1].0] { push(p, uv(p)); }
    }
    3 * (poly.len() as u32 - 2)
}
```

The result is indexed as a slice, so `smallvec` is never named and no
dependency is added. Winding matches `push_quad_vertices`' two triangles.

In `prepare_layer_composites`, the two untransformed call sites (content
quad, backdrop quad) go through one local helper that returns the vertex
count:

- `None` or `Some(QuadClip::Rect(r))` → today's `clip_quad` +
  `push_quad_vertices`, byte-identical output, count 6.
- `Some(QuadClip::Rotated(c))` → `clip_quad_rotated`.

`StagedQuad::range` becomes `start..start + count`. The `transform3d` branch
keeps `clip_quad(.., -EDGE_AA_INFLATE_PX, None)` and passes
`layer.quad_clip.as_ref().map(QuadClip::bounds)` as the fragment rect.
`composite.wesl`, `CompositeUniforms` and the pipeline do not change.

### 2.4 Not changed

`render_target::clipped_out`, `transform3d`'s fragment clip, `pick3d`,
`drive_layout_transitions`, the layout channel's rules.

## 3. Edge cases and tests

How the strip behaves, case by case:

| Case                                                            | Result                                                                                                                                 |
| --------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| Member under clippers inside the layer, root under a scrollport | count = root's list length; the capture keeps the interior clippers only                                                               |
| Root with no clipping ancestor                                  | count 0 for all, no rows, no swap (today: every clipped member swapped every frame)                                                    |
| Root that is itself a scroll container                          | its own rect is not in its inherited list, so children keep it: clipped inside the capture, as today                                   |
| Nested layers                                                   | a member strips its **nearest** root's list (`node_to_layer`); a nested root's quad strips the **enclosing** root's list (`enclosing`) |
| `OverrideClip`/`FixedNode` between root and member              | lists differ → count 0 → the member's whole list applies. The quad still clamps the escapee (unchanged divergence)                     |
| Rotated or scaled interior clipper (FLIP scale)                 | untouched: the member keeps Bevy's own affine rect                                                                                     |
| Member fully clipped by an interior cause (`display: none`)     | a `FullyClipped` member always has count 0, so it stays fully clipped in the capture                                                   |
| Root fully clipped from outside                                 | see below                                                                                                                              |
| Inactive nested root (zero-sized)                               | same rows as today: members map to it in `node_to_layer`, it has no `enclosing` entry, so no quad                                      |

**Fully clipped from outside.** The interior part of the list is gone, so
members extract fully clipped and the capture is empty. This cannot be seen:
`display: none` above the root gives the root a zero size (inactive, no
capture); a non-invertible clipping ancestor gives the root a singular
transform (zero-area paint, usually a zero capture window). When the cause
lifts, the root's size or linear part changes with it, so the layer
re-captures (activation is a membership rebuild; the linear part is in the
geometry hash).

**The one unsound corner.** After a reset, a member's first rects could
equal the root's inherited rects bit for bit: a clipper below an
`OverrideClip`/`FixedNode` with the same transform and padding box as a
clipper above the root. The count is then wrong, the capture is under-clipped,
and the quad clamp applies the identical rect instead, so the pixels match
until the two clippers stop coinciding. bevy-react inserts neither component
today. If `position: "fixed"` (§2.3) lands, its plan already stops layer
membership at a `FixedNode`. Document it in the module docs; do not add a
reset walk.

### Tests

Convert first, then swap the implementation (commits 1 and 3), so the same
assertions are green on both.

`crates/core/src/layer/clip.rs` — `clip_world()` gains
`bevy::ui::update::update_clipping_system` at the head of the chain, and the
tests build real clipper ancestors instead of hand-inserting clips. Two
helpers hide the storage, so only they change in commit 3:
`interior_clip(world, e) -> Option<CalculatedClip>` (today
`LayerClips::interior`, afterwards the real list minus the strip count) with
`interior()` as its bounds, and `quad()` (bounds of the quad clip, `None`
when unclipped).

| Test                                                                | Shape                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| ------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ancestor_clip_stripped_and_moved_to_quad` (keep)                   | scrollport → root → child: both interiors unclipped, quad = scrollport rect; an unclipped root has no quad clip                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `root_overflow_clips_children_only` (keep)                          | add an outer scrollport so the count is 1: children keep the root's own rect and deeper ones                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `nested_roots_get_quad_clips_and_restart_interior` (keep)           | outer scrollport → O → clipper → I → clipper2 → innermost: each quad carries only the clippers below its enclosing root                                                                                                                                                                                                                                                                                                                                                                                                             |
| `override_clip_and_display_none` (keep)                             | escapee has no row; hidden subtree stays `FullyClipped`; add a clipper below the escapee: its child keeps the whole list                                                                                                                                                                                                                                                                                                                                                                                                            |
| `scroll_holds_cache_and_updates_quad_clip` (keep)                   | translate the subtree under a fixed scrollport: no dirt, quad unchanged; resize the scrollport: no dirt, quad follows                                                                                                                                                                                                                                                                                                                                                                                                               |
| `membership_transitions_flag_clips_for_reextraction` (keep, extend) | demote and promote flag members under a scrollport; **a second frame after each flags nothing** (the `set_changed` → `Changed` echo must not loop). In commit 3 add: a root with no clipping ancestor, with an interior clipper, flags nothing on promote/demote (today it flags)                                                                                                                                                                                                                                                   |
| `single_axis_clip_bounds_stay_unbounded_on_the_open_axis` (keep)    | unchanged                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| new `interior_keeps_a_rotated_clippers_affine_rect`                 | scrollport → root → clipper rotated 30° → member: `interior_clip` has one rect and its `world_to_clip_local` is the clipper's inverse                                                                                                                                                                                                                                                                                                                                                                                               |
| new `clip_reveal_follows_the_flip_scale_inside_a_layer`             | `layout_app()` + clipping + `sync_layer_geometry`, `sync_layer_clips`, `resolve_layer_repaints` chained after `drive_layout_transitions`. Scrollport → promoted root → clip wrapper with `layout_spec(1.0)` (width 200 → 100) → child. Change frame: the child's interior is one rect spanning x 0..200 and the root is in `LayerRepaintState::dirty`; at 0.5 s it spans 0..150 and is dirty again; settled plus one frame: 0..100 and clean. Needs `step` and `layout_spec` raised to `pub(crate)` in `transition/layout_tests.rs` |
| new `quad_clip_is_a_rect_unless_a_clipper_rotates` (commit 4)       | scaled scrollport → `QuadClip::Rect` with the scaled bounds; rotated → `QuadClip::Rotated`; `display: none` ancestor → empty `Rect`                                                                                                                                                                                                                                                                                                                                                                                                 |

`crates/core/src/layer/render/clip.rs`:

- `clip_quad_table`, `swap_in_clears_previous_stash`: unchanged.
- `swap_roundtrip`: rewrite for counts. Row 1 on a two-rect clip leaves the
  second rect; row 1 on a one-rect clip leaves an empty list; no row leaves
  the clip alone; a dead entity's row is skipped; restore is exact.
- new `rotated_clip_cuts_the_quad_to_a_polygon` (commit 4): a 100×100 quad
  under a 45° square clip. The count is a multiple of 3 and above 6; every
  position satisfies `clip.contains_point` within 1e-3; every UV equals
  `(p - min) / size`; a clip that contains the quad yields 6 vertices equal
  to `push_quad_vertices`' set; a disjoint one yields 0.

New `crates/core/src/transition/layout_clip_tests.rs` (`layout_tests.rs` is
at 988 lines), harness `layout_app()` plus
`update_clipping_system.after(drive_layout_transitions)`, as
`reconcile/clip_pick_tests.rs` does:

- `overflow_clip_follows_the_eased_box`: clip wrapper 200 → 100 wide with
  `layout_spec(1.0)`, a 200-wide `flex_shrink: 0` child. Change frame: the
  wrapper's x scale is 2 and the child's `CalculatedClip` contains
  (150, 50) but not (210, 50). At 0.5 s: contains (140, 50), not (160, 50).
  Settled: not (110, 50).
- `start_pinned_content_shifts_by_half_the_size_change`: same scene, the
  child's center on the change frame is 150 (it was 100). Pins the documented
  anchor rule.
- `centered_content_does_not_jump`: wrapper with
  `justify_content: Center`; the child's center is 100 before and on the
  change frame.

Unchanged tests that pin what this relies on: `transition/layout_tests.rs`
`size_change_scales_about_the_old_rect`,
`children_ride_translation_but_not_scale`, `layout_channel_zero_size_rules`,
`from_zero_delta_is_never_singular`; `reconcile/clip_pick_tests.rs`
`a_node_outside_an_ancestors_clip_is_not_hovered` (uncommitted when I read
it); `layer/render/backdrop.rs`
`backdrop_quad_shrinks_to_border_box_with_inflated_uvs`; `layer/render.rs`
`grown_transform_quad_extends_uvs_proportionally`; `render_target.rs`
`live_camera_renders_only_while_a_portal_is_showing`.

## 4. Commit sequence

Each builds and passes the suite. Benchmark (`bench.sh build && bench.sh run
<name>`) after 3 and after 4.

1. `test(layer) clip tests run bevy's own clipping` — `clip_world()` on
   `update_clipping_system`, the seven tests rebuilt on real ancestors (in
   today's vocabulary: "no row" reads "interior unclipped"), plus
   `interior_keeps_a_rotated_clippers_affine_rect` and
   `clip_reveal_follows_the_flip_scale_inside_a_layer`. Green on the current
   cascade. If one fails here, that is a real bug in the cascade: stop and
   report it.
2. `test(transition) overflow clips follow the layout flip` —
   `layout_clip_tests.rs`.
3. `ref(layer) interior clips are bevy's clip list minus the root's` — §2.1,
   §2.2, the `layer.rs` gate cleanup, `swap_roundtrip`, module docs,
   CLAUDE.md's clip-independent-capture sentence ("interior-clip cascade" →
   prefix strip). `quads` still holds a `Rect`.
4. `fix(layer) a rotated clipping ancestor cuts layers at its real edge` —
   §2.3, its two tests, the `overflow.md` and `layers.md` lines, the
   changelog bullet's first sentence.
5. `docs(guide) clipped reveals with layout transitions` — the
   `style-transitions.md` and `sizing.md` text, the changelog bullet's second
   sentence, CLAUDE.md's `transition: { layout }` paragraph, the example
   card on Style transitions.
6. `impr(demos) the docs fold is a clipped accordion` — `HeaderCard.tsx`
   only. Separate so it can be dropped if the user prefers today's fold.

## 5. Demos + docs

No nav label changes, no `bevy.ts`/`style.ts`/`elements.ts` regeneration, no
screenshot retune (`layout-animations.webp` is shot on Flexbox).

**Style transitions page**
(`examples/demos/ui/src/demos/animations/TransitionDemo.tsx`, 708 lines):
add a "Clipped reveals" `Example` next to `LayoutDemo` in the third
`DemoRow`, in its own file beside it. A button and an auto-height panel:

```tsx
<node
  style={{
    height: open ? "auto" : 1,
    overflowY: "hidden",
    justifyContent: "center",
    flexDirection: "column",
    transition: {
      layout: { duration: 300 },
    },
  }}
>
  <node style={{ flexShrink: 0 }}>{details}</node>
</node>
```

Snippet lines stay at or under 40 characters. The copy says: the height is
`auto`, no frame relayouts, the content is centered so it does not jump, the
box collapses to 1 because zero snaps, and what follows the panel needs its
own `layout` entry to glide. Give the footer below the panel that entry.

**Docs fold** (`examples/demos/ui/src/HeaderCard.tsx`): replace the fade and
`leavingStyle` with a wrapper around the `Card`. `useFold` stays as is
(`mounted`, `visible`; the 50 ms beat is what lets the collapsed mount reach
Bevy before the flip).

- Wrapper: `flexDirection: "column"`, `justifyContent: "center"`,
  `overflowY: "hidden"`, `height: docs.visible ? "auto" : 1`,
  `margin: { bottom: docs.visible ? -SHADOW_REACH : 0 }`,
  `transition: { layout: DocsFold }`.
- `Card`: `flexShrink: 0`,
  `margin: { top: HEADER_GAP, bottom: SHADOW_REACH }` (50, the card's shadow
  reach, so the wrapper does not cut the shadow at rest); `headerStyle.gap`
  becomes 0. The expanded layout is pixel-identical to today's.
- `App.tsx` keeps the body's `transition: { layout: DocsFold }`.
- Update the `DocsFold` comment in `theme.ts`.

The page content node is a morph-promoted layer, so this runs the in-layer
path. Check the expanded state with a before/after `--shoot` of one page
(must be identical). The motion itself is for the user to judge live.

**Guide**

- `docs/guide/animations/style-transitions.md`, "Layout transitions": a bullet
  that an `overflow` clip follows the eased box, in drawing and pointer hits;
  a short "Clipped reveals" subsection with the snippet above and the three
  rules (center the content, collapse to 1, neighbours need `layout`); one
  sentence on choosing between it and the `size` accordion (fixed pixel ends
  and neighbours pushed by real layout, versus auto height and no relayout).
- `docs/guide/styling/sizing.md`: after "the tool for accordions", link to
  the auto-height variant.
- `docs/guide/styling/overflow.md`: a clipping node that is rotated or scaled
  clips along its transformed padding box.
- `docs/guide/styling/layers.md`, "Clipping and scrolling": a `transform3d`
  layer under a rotated clipping ancestor is cut at that ancestor's bounding
  box.

**CHANGELOG** (`[Unreleased]`, Changed):

- **`overflow` clips follow transforms and layout transitions.** A rotated or
  scaled clipping node clips its children along its transformed box (Bevy
  0.20), composited layers included. With `transition: { layout }` the clip
  eases with the box, so a clipping wrapper can reveal auto-height content.

`LayerClips` is not part of the extension contract; no changelog line for its
fields.

## 6. Decisions

1. **Strip or cascade?** Strip. Fewer lines, no mirror of Bevy's cascade
   rules to keep in step, sparse swap. Cost: it depends on the list being
   append-only and outermost first, which commit 1's tests guard on every
   Bevy upgrade (a type change breaks the build; a reorder fails
   `nested_roots…` and `root_overflow…`).
2. **Store counts, not clips.** A count is exactly "what differs from the
   real clip". It makes `changed` precise and the swap sparse.
3. **Quad clamp: two paths.** Keep `clip_quad` for axis-aligned clips and use
   `clip_polygon` only for rotated ones. One path for everything would run
   Sutherland-Hodgman for every clipped layer every frame, and the
   benchmarks cannot see that (layers-stress has no clipping ancestor).
4. **`clipped_out` stays on `clip_aabb`.** Its over-approximation only keeps
   a portal camera on, and `clip_polygon` reports a merely touching box as
   visible, which would need an area test.
5. **`transform3d` under a rotated clipper: bounding box.** The fragment test
   takes one rect. An exact version needs a homography clip or a clip-list
   uniform. Documented as a limit.
6. **`OverrideClip` escapees stay clamped by the layer quad.** Inherent.
7. **Reveal anchor: keep center-riding children.** A top-anchored reveal
   needs the children's frame anchored at the shown top-left instead of the
   center. That changes the layout channel's composition (and
   `children_ride_translation_but_not_scale`, shared-element seed frames, the
   Home card), a design the user already settled. Ask the user before
   touching it; do not bundle it here.
8. **Collapse to 1, keep the to-zero snap.** A zero laid-out size cannot be
   scaled.
9. **"Hide docs":** rebuild it as in §5, as its own commit. If the centered
   crop reads worse than today's fade and glide, drop commit 6 and keep the
   example card. This is the user's call.
10. **`overflowY: "hidden"` in the recipe**, so the collapsed size never
    depends on the content's minimum size. `"clip"` works too with an
    explicit height.

## 7. Risks / perf

- **Idle frame:** one `Changed<CalculatedClip>` scan over clipped entities
  (tick compares, no allocation), skipped when there are no layers. Today it
  is a `Changed<Node>` scan over every node. No per-frame allocation is
  added: `strip` and `quads` persist, `stash` keeps its capacity, the quad
  clip crosses to the render world as a copy or an `Arc` clone.
- **Structural frame:** one pass over `node_to_layer` with two component
  reads per member, no clip clones (today: a full cascade with a clone per
  member).
- **Swap:** rows only for members under a root that has clipping ancestors.
  A layer inside a scroll view costs what it costs today; a root with no
  clipping ancestor costs nothing.
- **Fewer re-extractions:** promoting or demoting a root with no clipping
  ancestor (a hover opacity) no longer flags its members.
- **The echo to watch:** `mark_reclipped_members`' `set_changed` shows up in
  next frame's `Changed` scan. The compare before `changed.push` ends it; the
  extended `membership_transitions…` test pins that. A miss here re-extracts
  the same members every frame forever, and no benchmark would show it (they
  have no clips): the test is the only guard.
- **Benchmark blind spot:** table-ops has no layers; layers-stress P1–P3 have
  no clips. Expect no change there. For the clipped path, run layers-stress
  once before and after with `overflowY: "scroll"` on `fieldStyle` in
  `examples/layers-stress/ui/src/App.tsx` (do not commit it).
- **`--shoot` after commits 3 and 4:** "Overflow", "Layers", "Filters",
  "Backdrop filters", "Morph filters", "Shared elements", "<portal>", "Home".
  Look for content drawn outside a scroll area, layers cut at the wrong edge,
  missing frost. After 5 and 6: "Style transitions" and any page's expanded
  header.
- **Not verified by running:** the in-layer reveal, taffy's centering of
  overflowing content, and that the rebuilt clip tests are green on the
  current cascade. Commits 1 and 2 answer all three before any code is
  deleted.
