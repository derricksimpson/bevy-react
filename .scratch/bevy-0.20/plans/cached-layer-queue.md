# Plan: keep clean cached layers' members out of the stock UI queue

Audit sections: §3.3 "Delete `cached.rs`", §1.1.2 "Retained UI rendering vs the
layer system". Measurement: `.scratch/bevy-0.20/bench-phase1.md` ("Ours").

Everything below was read, not run (no cargo). Another agent is editing
`layer/clip.rs`, `layer/render/clip.rs`, `layer.rs` and the `PostUpdate` block
of `plugin.rs` (the interior-clips plan), so our code is cited by symbol, not by
line. `B/` = `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_ui_render-0.20.0/src/`,
`R/` = the same registry dir, other crates.

## Verdict

1. **Park by camera slot, do not move data.** Every stock retained store keeps
   `main entity → (camera render entity, items)`, and every stock `queue_*`
   system skips an entry whose camera resolves to no phase. A system of ours,
   before `RenderSystems::Queue`, writes `Entity::PLACEHOLDER` into that slot
   for the members of clean cached layers and puts the saved camera back the
   frame the layer re-captures. Per-node data is never touched, so Bevy's own
   invalidation keeps working and the 0.20 failure mode (data missing at
   re-capture) cannot occur.
2. **Also make the steal cheaper** (`retain` in place, drop cached items
   without buffering). Small, independent, first commit.
3. Moving entries into our own store (option (a) as briefed) is rejected:
   it needs the same bookkeeping plus render-entity cleanup, and its bugs are
   ghost draws or blank layers.
4. No user-facing API, wire, TS or docs change. No CHANGELOG entry (the
   regression never shipped).

Expected on P1 (15 s, ~56 000 samples): about −2 600 samples, back to the 0.19
frame rate (≈ 525 fps) or slightly above. Estimates, see §8.

## 1. Verified facts

### 1.1 How 0.20 holds node data

- `ExtractedUiNodes` (`B/lib.rs:408-417`, `pub`, both fields `pub`):
  - `uinodes: MainEntityHashMap<(Entity, EntityIndexMap<ExtractedUiNode>)>` —
    key = the node's **main** entity; `.0` = the **render entity of its target
    camera**; `.1` = one `ExtractedUiNode` per drawable, keyed by a render
    entity spawned for it (`commands.spawn_empty().id()`, e.g. `:772`).
  - `changed: MainEntityHashSet` — "UI nodes that changed this frame".
  - Registered with `init_resource` + `allow_ambiguous_resource` (`:239-240`).
- `extract_uinode_changes` (`B/lib.rs:439-711`, set `ExtractChanges`):
  - clears `changed` (`:578`);
  - for every node matching one of ~26 `Changed<…>` filters (`:442-483`), every
    `RemovedComponents` reader (`:602-631`), every changed text span
    (`:487-503`) and, with `bevy_ui_debug`, every node when the global debug
    options changed (`:584`, `:588-598`): `process_changed_entity` (`:672`);
  - `process_changed_entity` inserts the main entity into `changed` (`:682`),
    **removes its whole entry** and despawns the sub-node render entities
    (`:684-688`), and queues the ancestor `Text` for the same treatment
    (`:695-709`).
- Every extractor that feeds this store iterates **only `changed`** and
  re-inserts (`.entry(main).or_insert_with(|| (camera, …))`): backgrounds
  (`:744-768`), inline images (`:845`), images (`:935`), borders/outlines
  (`:1082`), viewport nodes (`:1414`), text sections (`:1509`), text shadows
  (`:1650`), text decorations (`:1831`), cursor and preedit (`B/text.rs:73`,
  `:261`), debug overlay (`B/debug_overlay.rs:195`). Text glyphs are
  `ExtractedUiItem::Glyphs` inside the same store (`B/lib.rs:375-395`); 0.19's
  separate `glyphs` vector is gone.
- So an entry disappears when: any filtered component changed or was removed
  (despawn included), the node became hidden (`!inherited_visibility.get()` →
  extractors `continue`, nothing re-inserted, e.g. `:750-756`), it became
  empty, or its camera no longer maps (`camera_mapper.map` → `None`,
  `:761-763`). A camera change (`Changed<ComputedUiTargetCamera>`) re-creates
  the entry with the new camera. A camera going **inactive** changes nothing in
  the store: the entry stays and is skipped at queue time (below).
- An existing entry's camera is never copied forward: the entry is removed
  before any extractor runs (`ExtractChanges` is first in a weak chain,
  `:245-263`; the extractors conflict with it on `ResMut<ExtractedUiNodes>`,
  so the order is kept).

### 1.2 `queue_uinodes` (`B/lib.rs:2047-2109`)

- Iterates **all** of `uinodes`, in hash order, every frame (`:2061-2063`).
- `current_camera_entity` starts as `Entity::PLACEHOLDER`, `current_phase` as
  `None` (`:2058-2059`).
- When the entry's camera differs from the previous entry's, it resolves the
  phase: `render_views.get(camera)` (needs `UiCameraView` + `ExtractedView` on
  that entity) → `camera_views.get(ui_view)` → `phases.get_mut(retained)` →
  `pipelines.specialize` (`:2064-2091`).
- **The only early-out per entry:** no phase → `continue` (`:2093-2095`). No
  visibility check, no marker, no `RenderLayers` (the crate never mentions
  `RenderLayers` or `ViewVisibility`).
- Otherwise one `add_transient` per sub-node, unconditionally (`:2096-2107`).
- `SortedRenderPhase::add_transient` (`R/bevy_render-0.20.0/src/render_phase/mod.rs:1930-1934`)
  is an `IndexMap::insert` plus a push to `transient_items`;
  `prepare_for_new_frame` (`:1697-1711`) `swap_remove`s every transient key
  next frame. Stock UI re-queues everything every frame.

### 1.3 `prepare_uinodes` (`B/lib.rs:2116-2431`)

Iterates the **items of every phase** (`:2159`), ours included, and finds each
item's data by `(main entity, render entity)` (`:2165-2168`); the camera slot
is ignored. An item with no data resets the batch and `continue`s. Cost is
per phase item, so an entry that queued nothing costs nothing here. Batches
are rebuilt every frame (`clear_batches`, `:2436`).

### 1.4 The other families

All have the same shape (`main → (camera, EntityIndexMap<…>)`) and the same
queue loop with the same single early-out. None has a `changed` list: each
extractor has its own `Changed<…>` query, drains the entry's items in place,
overwrites the camera slot when the node is visible
(`if let Some((camera_entity, _)) = ….get_mut(&main) { *camera_entity = … }`)
and removes the entry on component removal.

| Family                                                                                                                                        | Store                                                                                                                   | Reachable from our crate                                                                                            | Queue early-out     |
| --------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | ------------------- |
| backgrounds, borders, outlines, images, text glyphs, text shadows, decorations, cursor, preedit, inline images, viewport nodes, debug overlay | `ExtractedUiNodes` (`B/lib.rs:408`)                                                                                     | yes                                                                                                                 | `lib.rs:2093`       |
| box shadows                                                                                                                                   | `box_shadow::ExtractedBoxShadows.box_shadows` (`B/box_shadow.rs:200-208`; camera rewrite `:304`; removal `:390`)        | yes (`pub mod`)                                                                                                     | `box_shadow.rs:446` |
| sliced / tiled images                                                                                                                         | `ui_texture_slice_pipeline::ExtractedUiTextureSlices.slices` (`B/ui_texture_slice_pipeline.rs:209-217`; `:337`; `:402`) | yes (`pub mod`)                                                                                                     | `:458`              |
| gradients                                                                                                                                     | `gradient::ExtractedGradients.items` (`B/gradient.rs:251-258`; `:448`; `:693`)                                          | **no** — `mod gradient;` is private (`B/lib.rs:12`) and only `GradientPlugin` is imported; the type cannot be named | `gradient.rs:740`   |
| `UiMaterial` nodes                                                                                                                            | `ExtractedUiMaterialNodes<M>.uinodes` (`B/ui_material_pipeline.rs:310-319`; `:442`; `:485`)                             | yes, but generic over the app's `M`                                                                                 | `:709`              |

Box-shadow and slice extraction is triggered by a subset of the `uinodes`
filters, plus `ComputedUiRenderTargetInfo` for shadows (`B/box_shadow.rs:226-235`).

### 1.5 Our pipeline now (`crates/core/src/layer/render.rs`, by symbol)

- `extract_ui_layers` (`ExtractSchedule`, after `extract_ui_camera_view`):
  builds `ExtractedUiLayers { layers, membership, enclosing, … }`.
  - `needs_capture = returning || !cached_ok || repaints.dirty.contains(&root)`,
    then propagated outward along `enclosing`, then forced for every layer
    enclosing a backdrop layer. It is final when the system returns and nothing
    writes it afterwards.
  - `cached_ok` reads the slot's `content_valid` (`layer/render/store.rs`,
    `LayerSlot`): set by `prepare_layer_textures` on a capture frame only when
    every item of the layer's phase had a compiled pipeline; cleared by
    `prepare_layer_composites` for layers enclosing a gated quad.
  - `membership: HashMap<MainEntity, usize>` (node → index into `layers`) and
    `enclosing` are re-derived only when the layer roots/order changed or
    `LayerMembership::rebuilds` moved; they are cleared when there are no
    layers.
  - The synthetic view entity carries an `ExtractedView` only while
    `needs_capture`.
- Main world: `resolve_layer_repaints` (`layer.rs`) turns `LayerContentDirt`,
  text reshapes, `Changed<InheritedVisibility>`, focused editables, the
  geometry hash (`sync_layer_geometry` / `fold_member_geometry`) and
  `cache: "never"` into `LayerRepaintState::dirty`. `LayerMembership` walks
  Bevy `Children`, so a member's computed target camera is its root's.
- `redistribute_ui_layers` (`RenderSystems::PhaseSort`, before
  `sort_phase_system::<TransparentUi>`): for each camera phase that has
  layers, `std::mem::take`s the phase's `IndexMap`, re-inserts every
  non-member, pushes every member item into a `stolen` vector, records the
  minimum sort key per layer, then moves items of `needs_capture` layers into
  the layer's synthetic phase and **drops the rest**. One composite quad per
  layer is injected at that minimum key.
- `ExtractedLayer::fallback_sort_key`: today only for a morphing layer that
  stole nothing. Commit 7ce05a7 removed its cached branch and the
  `root_visible` field with `cached.rs`.
- Extract-window clip swap (`layer/render/clip.rs`): unchanged by this plan.

### 1.6 System order

- `ExtractSchedule`: `swap_interior_clips_in` → stock `ExtractChanges`,
  `ExtractCameraViews`, `extract_ui_layers`, the item sets (our
  `ExtractUiItems` umbrella) → `swap_interior_clips_out`.
- `Render` (sets are chained **weakly** in 0.20,
  `R/bevy_render-0.20.0/src/lib.rs:332-347`: only conflicting systems are
  ordered): `ExtractCommands` → … → `Queue` (`queue_uinodes`, `queue_shadows`,
  `queue_ui_slices`, `queue_gradient`, `queue_ui_material_nodes::<M>`) →
  `PhaseSort` (`redistribute_ui_layers`, then the stock sort) →
  `PrepareResources` (`prepare_layer_textures`) → `PrepareBindGroups` (stock
  prepares; `prepare_layer_morphs` → filters/backdrops → mips →
  `prepare_layer_composites`) → the camera's `Core2d`/`Core3d`
  (`ui_layer_capture_pass` before `ui_pass`) → `Cleanup`.

### 1.7 The measured cost (P1, 15 s, `perf` at 1499 Hz)

`target/port-scratch/perfls0{19,20}-p1.sym.txt`; the later `bench-batchA-p1`
profile agrees (1025 / 714 / 623 / 312 / 240).

| Symbol                                                      | 0.19      | 0.20 |
| ----------------------------------------------------------- | --------- | ---- |
| `redistribute_ui_layers`                                    | 143       | 947  |
| `bevy_ui_render::queue_uinodes`                             | 7         | 766  |
| `IndexMap<…TransparentUi>::insert_full` (both monomorphs)   | 99        | 625  |
| `RawTable<usize>::reserve_rehash` (same map)                | 102       | 332  |
| `IndexMap<…TransparentUi>::swap_remove_full`                | 130       | 258  |
| `malloc` + `cfree`                                          | ~600      | ~720 |
| `cached::hide_cached_layer_members` + `cached::restore_out` | 495 + 243 | —    |

P1 has about 6 retained entries and 7 phase items per layer (banner
background + border, image, glyphs, three dots): ~3 000 entries, ~3 500 items,
all queued, stolen and dropped every frame. That is roughly 70 ns per item per
frame.

### 1.8 Corrections

- **The brief's 0.19 figure is low.** The hide cost 495 + 243 (`restore_out`
  is a separate symbol), so 0.19 paid 888 samples on this path, not 645.
- **Audit §3.3 / §1.1.2** call deleting `cached.rs` a pure deletion because
  "Bevy now skips unchanged nodes itself". Bevy skips their _extraction_;
  queue, our steal and the transient cleanup still run for every retained
  member. That is the regression.
- **`layer/render/clip.rs` "Known gap: a `UiMaterial` extractor is … unordered"**
  is wrong for 0.19 and 0.20: `extract_ui_material_nodes::<M>` is in
  `RenderUiSystems::ExtractBackgrounds` (`B/ui_material_pipeline.rs:70`). The
  interior-clips plan already removes the sentence.
- `fill_fallback_sort_keys`' doc still says "one served from cache (members
  hidden from extraction)". Stale since 7ce05a7; true again after this plan
  with "parked" for "hidden".
- Audit §1.1.2's "future risk" line is right and now covers this plan too:
  the day Bevy retains `TransparentUi` items (PR #24893), `queue_uinodes`
  stops walking the store every frame and both the steal and the parking need
  a redesign.

## 2. Candidates

|                                                                                                                                 | Correct under retained extraction?                                                                                                                                                                                                                                                                                                    | Per-frame cost                                                                                                                                                                                          | Verdict                                                                                                                                       |
| ------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| (a) move clean members' entries into our own store, merge back on re-capture                                                    | Only with bookkeeping: a parked member Bevy re-extracts is not found by `process_changed_entity`, so its old render entities leak and a fresh entry appears beside ours; a parked member Bevy removes (hidden, despawned) would come back as a ghost at merge. Both need a `changed`-driven reconcile to be _correct_, not just fast. | Same as the recommended design                                                                                                                                                                          | Rejected: more code, worse failure modes, no gain                                                                                             |
| (b) cheaper steal                                                                                                               | Yes (pure refactor)                                                                                                                                                                                                                                                                                                                   | Saves the re-insert of non-members, the per-frame regrowth of the camera phase map and the `stolen` buffer for cached layers. Does not touch `queue_uinodes` or its inserts. Est. −700 of ~2 800 on P1. | **Do it** as commit 1; not enough alone                                                                                                       |
| (b') skip the partition when all layers are clean                                                                               | No: gradient and `UiMaterial` items of members may still be in the phase and would draw unlayered                                                                                                                                                                                                                                     | —                                                                                                                                                                                                       | Rejected                                                                                                                                      |
| (b'') steal by sort-key range, or keep our own per-layer item lists                                                             | Ranges break on the pre-sort order and on `GlobalZIndex` members; kept lists are pointless for clean layers and stale for dirty ones                                                                                                                                                                                                  | —                                                                                                                                                                                                       | Rejected                                                                                                                                      |
| (c) park by camera slot                                                                                                         | Yes, by construction (§3.5)                                                                                                                                                                                                                                                                                                           | Static frame: one bool compare per layer. Change frame: a few hash lookups per re-extracted member. Flip/membership frame: one pass over the stores.                                                    | **Recommended**                                                                                                                               |
| (c') anything else stock offers                                                                                                 | Nothing: no marker, no visibility check, no `RenderLayers` in queue. Changing `UiTargetCamera` in the main world re-runs layout and re-extracts. `InheritedVisibility` is the deleted hack.                                                                                                                                           | —                                                                                                                                                                                                       | —                                                                                                                                             |
| (d) do nothing                                                                                                                  | Yes                                                                                                                                                                                                                                                                                                                                   | −5% on 500 static layers; ~70 ns per member item per frame for every cached layer, e.g. a whole page inside a morph carrier                                                                             | Rejected: cached layers stop being "an optimization rather than a tax"                                                                        |
| (e) point member entries at the layer's synthetic view (give it a `UiCameraView`) so stock queues straight into the layer phase | Plausible, and it would also remove the steal for dirty layers (P3)                                                                                                                                                                                                                                                                   | Lowest                                                                                                                                                                                                  | Not now: needs `UiAntiAlias`/`BoxShadowSamples` mirroring and a new quad-sort-key rule for every layer. Revisit if dirty-layer scenes matter. |

Why (c) is safe where the old hide was not: the hide changed what Bevy
_extracted_ (an empty node), and Bevy keeps what it extracted. Parking changes
only where a complete, current entry is _routed_; the frame the layer needs
its items, the same system routes them back before queueing.

Families that stay queued for clean layers: gradients (private store) and
`UiMaterial` nodes (generic store). Acceptable: their items are stolen and
dropped exactly as today, the cost is per gradient/material member only, and a
layer whose items are partly parked and partly stolen is fine because routing
is per item by main entity.

## 3. Design

### 3.1 Steal in place (`redistribute_ui_layers`)

Replace the `mem::take` + re-insert loop with `IndexMap::retain`, which is in
place, order-preserving, keeps the allocation and rebuilds the index table
without growing only when something was removed
(`R/indexmap-2.14.0/src/inner.rs:451-465`). `TransparentUi` is not `Clone`
(`B/render_pass.rs:77`); copy it field by field (every field is `pub`, the ids
are `Copy`).

```rust
let mut stolen: Vec<(usize, TransparentUi)> = Vec::new();
// per camera phase:
camera_phase.items.retain(|_, item| {
    let Some(&idx) = extracted.membership.get(&item.main_entity()) else {
        return true; // not a member: stays, untouched
    };
    let best = &mut quad_sort_keys[idx];
    if best.is_none_or(|b| item.sort_key < b) {
        *best = Some(item.sort_key);
    }
    // A cached layer's item ends here: its pixels are in the persistent texture.
    if extracted.layers[idx].needs_capture {
        stolen.push((idx, TransparentUi {
            sort_key: item.sort_key, entity: item.entity, pipeline: item.pipeline,
            draw_function: item.draw_function, batch_range: item.batch_range.clone(),
            extra_index: item.extra_index.clone(), indexed: item.indexed,
        }));
    }
    false
});
// after fill_fallback_sort_keys + propagate_quad_sort_keys, as today:
for (idx, item) in stolen {
    if let Some(phase) = phases.get_mut(&extracted.layers[idx].retained) {
        phase.add_transient(item);
    }
}
```

Factor the closure body's phase part as
`fn steal_members(items, membership, needs_capture: impl Fn(usize) -> bool, quad_sort_keys, stolen)`
so one unit test can drive it (§4). Nothing else in the function changes.

### 3.2 Parking (`crates/core/src/layer/render/park.rs`, new)

```rust
/// Camera slot of a parked entry. No entity has this id, and it is the value
/// stock `queue_*` loops start from, so a parked entry resolves no phase.
const PARKED: Entity = Entity::PLACEHOLDER;

/// One stock retained store: node main entity → (camera render entity, items).
type Store<T> = MainEntityHashMap<(Entity, T)>;

#[derive(Resource, Default)]
pub struct ParkedUiNodes {
    /// Parked node → the camera render entity stock extraction gave it.
    cameras: MainEntityHashMap<Entity>,
    /// What the last full pass was derived from.
    epoch: u64,
    clean: Vec<bool>, // per layer index: !needs_capture
}

pub fn park_cached_layer_members(
    extracted: Res<ExtractedUiLayers>,
    mut parked: ResMut<ParkedUiNodes>,
    mut nodes: ResMut<ExtractedUiNodes>,
    mut shadows: ResMut<ExtractedBoxShadows>,
    mut slices: ResMut<ExtractedUiTextureSlices>,
) {
    park(&mut parked, extracted.membership_epoch, &extracted.membership,
         extracted.layers.iter().map(|l| !l.needs_capture),
         &mut nodes, &mut shadows.box_shadows, &mut slices.slices);
}

fn park<S, T>(
    state: &mut ParkedUiNodes,
    epoch: u64,
    membership: &HashMap<MainEntity, usize>,
    clean: impl Iterator<Item = bool> + Clone,
    nodes: &mut ExtractedUiNodes,
    shadows: &mut Store<S>,
    slices: &mut Store<T>,
) {
    let ParkedUiNodes { cameras, epoch: seen, clean: seen_clean } = state;
    let full = *seen != epoch || !seen_clean.iter().copied().eq(clean.clone());
    if full {
        *seen = epoch;
        seen_clean.clear();
        seen_clean.extend(clean);
    }
    // No layers, or every layer re-captures, and nothing is parked.
    if cameras.is_empty() && !seen_clean.contains(&true) {
        return;
    }
    let want = |main: &MainEntity| {
        membership.get(main).is_some_and(|&idx| seen_clean.get(idx) == Some(&true))
    };
    let ExtractedUiNodes { uinodes, changed } = nodes;
    if full {
        // Membership or a layer's clean/dirty state moved: route every entry.
        route_all(uinodes, &want, cameras);
        route_all(shadows, &want, cameras);
        route_all(slices, &want, cameras);
        cameras.retain(|main, _| want(main));
    } else {
        // Same routing as last frame: only entries bevy re-created can be off,
        // and those come back unparked. `changed` lists all of them.
        for main in changed.iter().filter(|main| want(main)) {
            if let Some(e) = uinodes.get_mut(main) { route(*main, e, true, cameras); }
            if let Some(e) = shadows.get_mut(main) { route(*main, e, true, cameras); }
            if let Some(e) = slices.get_mut(main) { route(*main, e, true, cameras); }
        }
    }
}

fn route_all<T>(store: &mut Store<T>, want: &impl Fn(&MainEntity) -> bool,
                cameras: &mut MainEntityHashMap<Entity>) {
    for (main, entry) in store.iter_mut() {
        route(*main, entry, want(main), cameras);
    }
}

fn route<T>(main: MainEntity, entry: &mut (Entity, T), park: bool,
            cameras: &mut MainEntityHashMap<Entity>) {
    match (park, entry.0 == PARKED) {
        (true, false) => { cameras.insert(main, entry.0); entry.0 = PARKED; }
        (false, true) => match cameras.get(&main) {
            Some(&camera) => entry.0 = camera,
            None => debug_assert!(false, "parked UI entry without a saved camera"),
        },
        _ => {}
    }
}
```

Imports: `bevy::ui_render::{ExtractedUiNodes, box_shadow::ExtractedBoxShadows,
ui_texture_slice_pipeline::ExtractedUiTextureSlices}`,
`bevy::render::sync_world::{MainEntity, MainEntityHashMap}`. No new cargo
feature (the crate already uses `bevy::ui_render`).

Recommended, optional: a `#[cfg(debug_assertions)]` tail that asserts, over
the three stores, that no entry is `PARKED` unless `want(main)`. It runs every
frame in debug builds only and catches the one dangerous bug class (the
routing key not bumped when membership moved).

### 3.3 `extract_ui_layers` / `ExtractedLayer`

- `ExtractedUiLayers` gains `pub membership_epoch: u64`, bumped inside the
  existing `if roots_changed || extracted.membership_rebuilds != membership.rebuilds`
  block (the only place `membership` and the layer indices are re-derived). The
  no-layers early return needs no bump: the clean set going empty is a key
  change by itself.
- Restore from `git show 7ce05a7` (reverse those hunks): the
  `Option<&InheritedVisibility>` query term, `ExtractedLayer::root_visible`,
  and the cached branch of `fallback_sort_key`:

  ```rust
  /// Whether a layer that stole nothing still draws its quad at the root's
  /// own stacking position.
  pub const fn wants_root_key(morphing: bool, needs_capture: bool, root_visible: bool) -> bool {
      morphing || (!needs_capture && root_visible)
  }
  pub fn fallback_sort_key(&self) -> Option<FloatOrd> {
      wants_root_key(self.morph.is_some(), self.needs_capture, self.root_visible)
          .then_some(self.root_sort_key)
          .flatten()
  }
  ```

  A parked layer steals nothing, so without this its quad would not draw.
  The root key orders the quad like the minimum stolen key did:
  `update_uistack_recursive` pushes the parent before its children
  (`R/bevy_ui-0.20.0/src/stack.rs:137-173`), so a subtree's stack indices are
  contiguous from the root and no foreign item lies between the root's key
  and the subtree's first item. `fill_fallback_sort_keys` still lets a stolen
  key win (a gradient item of a parked layer), which is equivalent for the
  same reason. This is the 0.19 rule, unchanged.

### 3.4 Wiring (`plugin.rs`, the render sub-app block)

```rust
.init_resource::<lr::park::ParkedUiNodes>()
.add_systems(Render, lr::park::park_cached_layer_members.before(RenderSystems::Queue))
```

A strict `.before(RenderSystems::Queue)` edge, so it holds for every queue
system including a generic `queue_ui_material_nodes::<M>`. It runs in the
`Render` schedule on purpose: `ExtractedUiLayers` is final there, and
`ExtractSchedule` is the serial section both worlds wait on.

### 3.5 Invariants

1. Only `park_cached_layer_members` writes `PARKED`, and only into the camera
   slot of an entry whose main entity is, **in this frame's
   `ExtractedUiLayers`**, a member of a layer with `needs_capture == false`.
2. Any frame where `membership_epoch` or the per-layer clean set differs from
   the last pass, every entry of the three stores is re-routed before
   `Queue`. So no entry is parked on a frame its layer captures, and none
   stays parked after leaving a layer (demotion, unmount, the layer's camera
   going inactive).
3. Parking never touches per-node data. Bevy removes and re-creates entries
   regardless of the camera slot; a re-created entry is unparked by
   construction and is parked again through `ExtractedUiNodes::changed`.
4. A missed park is harmless: the item is queued, stolen and dropped as
   before. `redistribute_ui_layers` stays authoritative; removing the park
   system's registration must leave every frame's pixels identical. (Parked
   items are exactly a subset of what the steal drops: same predicate, same
   `ExtractedUiLayers`.)
5. `cameras` holds a row for every parked entry; rows are dropped only in a
   full pass, after that pass un-parked the entries.

### 3.6 Docs in code

- `layer/render.rs` module header, step 1: replace the parenthesis with "a
  clean cached layer's members are parked out of the stock queue ([`park`]);
  what still reaches a camera phase is stolen and dropped".
- Add to the "Re-verify on Bevy upgrades" list: every stock `queue_*` resolves
  its phase from the entry's camera entity and `continue`s without one; the
  retained stores keep `(camera, items)` per main entity with `pub` fields;
  `ExtractedUiNodes::changed` lists every entry removed or re-created this
  frame; `gradient` still private.
- `ExtractedLayer::needs_capture` and `fill_fallback_sort_keys` docs: "parked"
  wording.
- `park.rs` module doc: the why (measured numbers), the how, the five
  invariants, the two uncovered families.

Nothing is deleted. `render.rs` is far over the file-size rule already; this
plan adds about 15 lines to it and puts the rest in `park.rs`.

## 4. Edge cases and tests

There is no headless render-world harness: every test under `layer/render*`
is a pure-function or `&mut World` unit test (`clip::swap_roundtrip`,
`needs_filter_run_decision_table`, `quad_sort_keys_propagate_…`). The three
stock stores are `Default` with `pub` fields, so `park` is fully testable on
plain values; entries need only `(camera, Default::default())`.

`park.rs` tests (each builds the stores, calls `park`, inspects camera slots):

1. `parks_only_members_of_clean_layers` — members of a clean layer become
   `PARKED`; a dirty layer's member and a non-member keep their camera.
2. `a_dirty_layer_gets_its_members_back` — flip the clean bit; the entry's
   camera is the exact entity it had; `cameras` is empty. This is the test for
   the 0.20 blank-capture failure.
3. `a_demoted_member_is_unparked_without_reextraction` — drop the node from
   `membership`, bump the epoch, leave `changed` empty; the camera is back.
   (With the interior-clips change a demotion no longer re-extracts members.)
4. `a_reextracted_member_is_parked_again` — same key; overwrite the entry
   with a new camera and put the node in `changed`; it is `PARKED` and the
   saved camera is the new one.
5. `an_entry_bevy_removed_leaves_no_row` — remove the entry, list it in
   `changed`, run; then flip to dirty and run; `cameras` is empty, no panic.
6. `nothing_is_touched_while_every_layer_captures` — all dirty, `changed`
   full: stores unchanged (the early return).
7. `shadow_and_slice_entries_follow_their_node` — including a node that has
   only a shadow entry (no `uinodes` entry): parks and restores.
8. `random_walk_keeps_the_routing_invariants` — seeded, ~200 steps of {toggle
   a clean bit, edit membership + bump epoch, re-create or remove an entry and
   log it in `changed`, run}. After every run: no entry is `PARKED` unless its
   node is a member of a clean layer; every unparked entry holds the camera
   the model last gave it; after a full pass every clean member is `PARKED`.

`render.rs` tests:

9. `wants_root_key_decision_table` — the `needs_filter_run` pattern.
10. `steal_members_in_place` — a small `IndexMap` of `TransparentUi`
    (`DrawFunctionId(0)`, `CachedRenderPipelineId::INVALID`): non-members keep
    their order and the map its capacity; a dirty layer's items land in
    `stolen`; a clean layer's items are gone and only its minimum key is
    recorded.

Cannot be tested headless (needs a GPU `PipelineCache`): that stock
`queue_uinodes` / `queue_shadows` / `queue_ui_slices` skip a `PARKED` entry.
It rests on the source lines in §1.2 and §1.4 and on the runs below.

Verify by running, after commit 2 (build the bundle first; settle ≥ 4 s so
layers have been parked for many frames before the capture):

- `cargo run -p demos -- --shoot "<label>" out.png 4` for "Layers",
  "Filters", "Morph filters", "Backdrop filters", "Home", "<surface>" (a
  second UI camera), "<portal>" (`cache: "never"`), "Overflow". Compare with
  the same shots taken on the parent commit.
  - A blank or partly empty card/panel: an entry parked on a capture frame, or
    left parked.
  - A layer above or below the wrong neighbour: the root sort key.
- Ask the user for the live checks `--shoot` cannot do: hover a control inside
  a filtered or faded layer (clean → dirty → clean), scroll a list of layers,
  and in layers-stress toggle **groupAlpha** (demote and re-promote 500
  layers), **20/100/500**, **filters** and **animations**.
- `target/port-scratch/bench.sh build && bench.sh run park`: fps tails for
  P1/P2/P3 and the P1 symbol counts (§8).

## 5. Commit sequence

1. `perf(layer) steal promoted items in place` — §3.1 and test 10. Builds and
   passes alone. Expect P1 `reserve_rehash` and `redistribute_ui_layers` down,
   everything else unchanged.
2. `perf(layer) park cached layers' members out of the stock ui queue` —
   `park.rs` with tests 1–8, `membership_epoch`, `root_visible` +
   `wants_root_key` with test 9, the wiring, the in-code docs of §3.6, the
   CLAUDE.md sentence of §6.

The `root_visible` restore belongs in commit 2: before parking it is dead code
(a clean layer always steals something).

## 6. Demos + docs

- **Demos:** none. No page shows or explains this.
- **Guide (`docs/guide/`):** none. `styling/layers.md` says a cached layer is
  not re-rendered, which stays true.
- **CHANGELOG:** none. 0.7 (Bevy 0.19) did not have the regression and no
  release shipped with it.
- **Generated files:** none.
- **CLAUDE.md**, "Architecture: composited layers & filters", after "a clean
  layer skips its capture pass": add "and its members are parked out of the
  stock UI queue (`layer/render/park.rs`: the camera slot of their retained
  entries is a sentinel until the layer re-captures; gradients and
  `UiMaterial` nodes are still queued and dropped)".
- `.scratch/bevy-0.20/bench-phase1.md` is the user's untracked notes; offer
  the after-numbers for its "Ours" bullet, do not edit it unasked.

## 7. Decisions

| #   | Choice                                        | Default                                                     | Why                                                                                                                                       |
| --- | --------------------------------------------- | ----------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Park in place or move entries out             | In place, by camera slot                                    | Bevy's invalidation keeps owning the data; a bug cannot lose it                                                                           |
| 2   | Sentinel                                      | `Entity::PLACEHOLDER`                                       | Stock's own "no camera yet" value: a leading run of parked entries costs one compare each, and `Query::get` on it fails without panicking |
| 3   | Where the camera comes back from              | A saved camera per main entity                              | Exact stock value; the layer's `camera_render` is unknown for a demoted node                                                              |
| 4   | Schedule                                      | `Render`, before `RenderSystems::Queue`                     | Keeps the extract window short; `needs_capture` is final                                                                                  |
| 5   | When to walk the stores                       | Full pass on a routing-key change, `changed` only otherwise | Static frames cost nothing; no per-layer member lists to keep in sync                                                                     |
| 6   | Stores covered                                | `ExtractedUiNodes`, box shadows, texture slices             | The bulk of every UI; `boxShadow` and tiled `backgroundImage` exist as styles                                                             |
| 7   | `UiMaterial` stores                           | Not covered, no opt-in API                                  | Generic over the app's type; add `park_material_nodes::<M>` when an app shows the cost                                                    |
| 8   | Gradients                                     | Not covered                                                 | Private store. Worth an upstream PR making `gradient` `pub`; do not wait for it                                                           |
| 9   | Quad sort key of a parked layer               | The root's key, gated on `root_visible` (0.19 rule)         | Already shipped and equivalent in order; tracking "had content" is a second state to get wrong                                            |
| 10  | Kill switch                                   | None                                                        | A/B by removing the system registration (invariant 4)                                                                                     |
| 11  | Debug-build invariant check                   | Add it                                                      | Six lines, debug only, catches a missed epoch bump                                                                                        |
| 12  | Redirect into the layer phase (candidate (e)) | Not now                                                     | Larger change for scenes nobody measured as slow                                                                                          |

## 8. Risks / perf

Per-frame cost of the park system:

- No layers, or all layers dirty and nothing parked: two checks.
- Static frame: one bool compare per layer.
- Frame with re-extracted nodes and at least one clean layer: per `changed`
  node one membership lookup, then for members up to three store lookups and
  one map insert (~30–50 ns each, est.). Bevy's own re-extraction of that
  node costs far more.
- A layer flipping clean ↔ dirty, or a membership rebuild: one pass over the
  three stores, one hash lookup per entry (~25–40 µs for 3 000 entries, est.).
  A layer that is dirty every frame (`cache: "never"`, a focused field, a
  nested backdrop) does not flip, so it triggers nothing.
- Memory: one map row per parked node.

Expected P1 counts after commit 2 (estimates from the 0.19 profile, where the
same items never reached the phase):

| Symbol                      | now   | expected                           |
| --------------------------- | ----- | ---------------------------------- |
| `redistribute_ui_layers`    | 947   | ~150                               |
| `queue_uinodes`             | 766   | < 100 (still walks ~3 000 entries) |
| `insert_full`               | 625   | ~100                               |
| `reserve_rehash`            | 332   | < 100                              |
| `swap_remove_full`          | 258   | ~130                               |
| `park_cached_layer_members` | —     | < 50                               |
| `bevy_react_*` total        | 4 014 | ~3 200                             |

P1 fps: 490 → about 525. P2 (filter params only; layers stay clean) gains the
same CPU time but is GPU-bound. P3 (every layer dirty every frame) hits the
early return; only commit 1 applies there, expect no visible change.

Risks:

- **A parked entry on a capture frame** would freeze a blank or partial layer
  in the cache. Guarded by invariant 2, tests 2 and 8, the debug check. Look
  at: "Layers" and "Filters" after a hover or any content change.
- **An entry left parked after demotion** makes the node vanish. Tests 3 and 8. Look at: layers-stress with groupAlpha off.
- **Camera re-resolve in `queue_uinodes`.** The store iterates in hash order,
  so a scene with parked and unparked entries interleaved re-resolves the
  phase on each switch back to a real camera (two `Query::get`s, a phase
  lookup, a `specialize` lookup; at most `min(parked, unparked)` times a
  frame). Stock already behaves this way with several UI cameras. Estimated
  net win even at a 50/50 mix, not measured: if `queue_uinodes` samples rise
  on a mixed page (the demos shell: nav outside the page layer), that is the
  cause.
- **Quad order.** A member with its own `GlobalZIndex` is not contiguous with
  its root; the quad sits at the root's key as it did under 0.19.
- **Bevy upgrade.** One more reliance on stock internals (§3.6 checklist).
  If Bevy starts retaining `TransparentUi` items, parking and the steal are
  both redesigned.
- **Benchmark noise.** fps moves ±5% with other jobs on the machine; the
  symbol counts from `perf` are the signal.
- **Blind spots.** No preset mixes clean and dirty layers or layered and
  plain UI, and none has box shadows, tiled images or a second UI camera.
  The unit tests and the "<surface>" shot are the only cover for those.

Not verified by running: that the stock queue systems skip `PARKED` entries
with no side effect, every sample estimate above, and the claim that no
in-tree code reads the camera slot of a stock store (grep found none).
