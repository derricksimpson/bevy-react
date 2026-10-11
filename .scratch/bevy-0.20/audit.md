# Bevy 0.20 port audit

- **Date:** 2026-10-11. **Status:** audit only; the port has not started. The repo is on Bevy 0.19.0 (`Cargo.lock`), bevy-react 0.7.0.
- **Method:** five parallel read-only audits (layer/render, shaders, picking/input, style/text, ECS/features/examples) against the real 0.20.0 crate sources, with the riskiest claims re-checked by hand. Nothing was built or run, so claims that need a runtime check are marked _(unverified)_.
- **Sources:** [release notes](https://bevy.org/news/bevy-0-20/), [migration guide](https://bevy.org/learn/migration-guides/0-19-to-0-20/). Re-download a crate's source with `curl -sSfL https://static.crates.io/crates/NAME/NAME-0.20.0.crate | tar xz`.
- **Paths:** repo paths are relative to the repo root. Bevy references like (bevy_ui_render `lib.rs:439`) point into that crate's 0.20.0 `src/`.

## Background: what `bevy_ui_render` is

Bevy splits UI into two crates:

- **`bevy_ui`** is the logic. It holds the components (`Node`, `ComputedNode`, `BackgroundColor`…), layout via taffy, clipping (`CalculatedClip`), stacking order, and hit-testing for picking and focus. It doesn't touch the GPU.
- **`bevy_ui_render`** turns that into pixels, once per frame per camera:
  1. **Extract:** copies each visible UI node's draw data from the game world into the render world (`ExtractedUiNodes`).
  2. **Queue:** turns that data into draw items in the `TransparentUi` render phase, sorted by stacking order.
  3. **Prepare:** builds vertex buffers and bind groups.
  4. **Draw:** runs them in the UI render pass, using its own shaders (`ui.wesl` and friends) for backgrounds, borders, images, text, gradients and box shadows.

**Why it matters to bevy-react:** our composited layers (`opacity` groups, `filter`, `transform3d`, `morphFilter`, `backdropFilter`) are built on top of it without forking it.

- For a promoted subtree, we take its already-queued `TransparentUi` items out of the camera's phase and render them into an offscreen texture.
- Then we run the filter passes and draw the result back as one quad.
- To capture without ancestor clipping, we briefly swap each member's `CalculatedClip` during the extract step.

That's why the 0.20 change to "retained" UI rendering hits us hardest. Step 1 now only re-extracts nodes whose data changed and keeps the rest from the previous frame. Our tricks that edit node data during extraction without registering a change (the clip swap, and the cached-layer hiding in `cached.rs`) no longer get picked up reliably.

## Summary

- The port is mostly mechanical, and no fork of `bevy_ui_render` is needed. Every seam the layer system uses is still public: `TransparentUi`, `SetUiViewBindGroup`, `stack_z_offsets`, `RenderUiSystems`, `extract_ui_camera_view`, `ui_pass`.
- The danger is the breaks that compile cleanly and then fail at runtime:
  1. All shaders stop working. Layers, filters and custom materials go invisible on native, and the app crashes on web.
  2. Retained UI rendering makes cached layers render blank and leaves stale clips.
  3. `<editableText>` ignores all input, and clicking it may not focus it.
- The one real engineering item is the clip-swap change-tick fix (§1.1.2). Everything else is mechanical or a deletion.

## 1. Required to work on 0.20

### 1.1 Silent breaks (compile fine, fail at runtime)

#### 1.1.1 Shaders → WESL

**What changed.**

- 0.20 removed naga_oil. A `.wgsl` file is passed to wgpu verbatim through `Shader::from_wgsl`, with no import processing (bevy_shader `shader.rs:135-147`, `:296`). Only `.wesl` files are composed (`shader.rs:294-303`, `shader_cache.rs:221-330`).
- Every file using `#import`, `#ifdef` or `#{…}` therefore fails at runtime, and nothing fails `cargo build`.
- **Native consequences:**
  - The composite pipeline never compiles, and `SetItemPipeline` skips silently (bevy_render `render_phase/mod.rs:2277`). So every promoted layer is invisible: opacity groups, filters, transform3d, morph, backdrop.
  - Filter shaders gate their subtrees, which go invisible.
  - Custom materials don't draw.
- **Web consequence: the app crashes.** On wasm Bevy doesn't catch shader-module errors (bevy_render `pipeline_cache.rs:165-178`). In 0.19, naga_oil caught them as processing errors, so the web build is newly fragile to any WGSL mistake.
- A wrong module path logs one warning and is then retried forever (`shader_cache.rs:294-302`). Our filter gate only warns after 600 frames.

**Inventory:** 72 tracked shaders, and none is plain WGSL, so all need converting. The other `.wgsl` files are build copies under `examples/*/ui/dist` and `node_modules/.cache/gh-pages`; leave those alone.

| Where     | Count | Contents                                                                                                                                                                                        |
| --------- | ----: | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| core      |    15 | prelude (`#define_import_path bevy_react::filter`), 11 built-in filters, `layer/composite.wgsl` (imports `bevy_render::view`), `mip_blit`/`backdrop_blit` (import the fullscreen vertex shader) |
| demos     |    23 | 19 filters/morphs, 3 PBR materials, 1 UiMaterial (`spotlight`)                                                                                                                                  |
| arcana    |     7 | 4 filters, 3 materials                                                                                                                                                                          |
| atrium    |     9 | shared `common` (`#define_import_path atrium::common`), 2 filters, 6 materials                                                                                                                  |
| cyberpunk |     8 | 4 filters, 4 materials                                                                                                                                                                          |
| deadspace |     8 | 4 filters, 4 materials                                                                                                                                                                          |
| nodes     |     2 | 2 UiMaterials                                                                                                                                                                                   |

**Which old features they use:**

- The filter prelude is imported by 44 files.
- `bevy_pbr` imports and `#{MATERIAL_BIND_GROUP}` appear in 20 files each.
- `bevy_ui` imports: 3 files. Imports of `atrium::common`: 3 files.
- `#ifdef` appears in 7 files:
  - `PREPASS_PIPELINE`: foil, crt, diorama_paint, station_paint.
  - `DISTANCE_FOG`: the 2 particle shaders.
  - `VERTEX_COLORS`: land.
  - `VERTEX_UVS_A`: the 2 paint shaders.
- No Rust code pushes shader defs (`ShaderDefVal`/`shader_defs`); every def comes from Bevy.

**Per-file conversion:**

- Rename `.wgsl` → `.wesl`. `#import X` → `import X;` (imports must come first; all 72 already do). `#ifdef`/`#else` → `@if`/`@else`. Drop `#define_import_path`.
- `#{MATERIAL_BIND_GROUP}` → `@group(constants::MATERIAL_BIND_GROUP) @binding(n)`. The migration guide's `@group(2) @binding(constants::MATERIAL_BIND_GROUP)` is a doc bug; Bevy itself writes the former (bevy_pbr `pbr_bindings.wesl:38`).
- UiMaterial shaders keep a hard-coded `@group(1)` with no defs (bevy_ui_render `ui_material_pipeline.rs:224`).
- Gotcha: `@if(X) { … }` creates a block scope, so a `var out` declared inside it can't be returned. Put one `@if(!PREPASS_PIPELINE)` on each statement instead, as bevy_pbr `render/pbr.wesl:76-92` does.
- The 33 example filter files are sed-convertible.

**Bevy module path changes** (all verified in the 0.20 sources):

| 0.19                                                                                                              | 0.20                               |
| ----------------------------------------------------------------------------------------------------------------- | ---------------------------------- |
| `bevy_pbr::{forward_io, mesh_view_bindings, mesh_view_types, mesh_functions, pbr_fragment, pbr_functions, utils}` | `bevy_pbr::render::…`              |
| `bevy_pbr::prepass_io`                                                                                            | `bevy_pbr::prepass::io`            |
| `bevy_pbr::pbr_deferred_functions`                                                                                | `bevy_pbr::deferred::functions`    |
| `bevy_ui::ui_vertex_output`                                                                                       | `bevy_ui_render::ui_vertex_output` |
| `bevy_core_pipeline::fullscreen_vertex_shader`, `bevy_render::view`                                               | unchanged                          |

**Bevy shader APIs that changed:**

- `alpha_discard(material, c)` → `alpha_discard(flags, alpha_cutoff, c)` (bevy_pbr `render/pbr_functions.wesl:98`). Affects foil, crt, diorama_paint and station_paint. Bevy's own `render/pbr.wesl:1-20, 64-92` is a ready template.
- `UiVertexOutput.border_radius` → `border_radius_x` + `border_radius_y` (elliptical corners). Breaks `spotlight.wgsl:65`.
- `view_transmission_texture`/`_sampler` exist only under `@if(VIEW_TRANSMISSION_TEXTURE)` (`mesh_view_bindings.wesl:83-85`). That flag is set only when the camera has `ScreenSpaceTransmission`, which is now opt-in (see §1.1.6).

**Rust-side references:**

- Embedded paths fail at compile time: `crates/core/src/plugin.rs:297-311`, 14 `load_embedded_asset!` sites, and the tests in `filters/builtin/mod.rs:276-378` and `registry.rs:648-696`.
- Asset paths fail silently: 26 `ShaderRef` returns, 33 `shader = "…wgsl"` macro arguments, and `examples/atrium/world/mod.rs:76`.
- In all, 138 `.wgsl` references across 56 Rust files.

**Public API break: the filter prelude path.**

- `#define_import_path` is gone, and there is no alias mechanism. The resolver only uses the path-derived name plus the `constants` module (`shader_cache.rs:408-421, 479-507`).
- An embedded `.wesl` gets its module path from its crate (`module_path!`) and file path (bevy_asset `io/embedded/mod.rs:228-240`; `shader.rs:180-191`).
- Only `.wesl` sources get a module path at all (`shader_cache.rs:408`).
- **Option 1, zero plumbing:** move the prelude to `crates/core/src/filter.wesl`, which gives `bevy_react_core::filter`. Its module path then matches a real file, which suits the wgsl-analyzer language server.
- **Option 2, keep the documented name (about 2 lines at `plugin.rs:297`):** add `Shader::from_wesl(include_str!("layer/filter_prelude.wesl"), "embedded://bevy_react/filter.wesl")` to `Assets<Shader>` and forget the handle, as `load_shader_library!` does. Alternative: `EmbeddedAssetRegistry::insert_asset` (bevy*asset `io/embedded/mod.rs:50`). Both are *(unverified end-to-end)\_.
- `filter` is a WGSL reserved word but is legal as a module name (wgsl-parse `grammar.lalrpop:991-993`). Cross-crate imports resolve (`shader_cache.rs:509-521`).
- **New import line** for custom filter authors (shader at `assets/shaders/x.wesl`, `#[react_filter(shader = "shaders/x.wesl")]`): `import bevy_react::filter::{FullscreenVertexOutput, sample_source, uniforms};`
- **What a user sees if they get it wrong:**
  - File left as `.wgsl`: an immediate gate warning and an invisible subtree on native; a crash on web.
  - Wrong module path: an invisible subtree, warned only after 600 frames.

**atrium `common`:** use `import super::common::{Atmos, apply_fog, noise2};`. Asset shaders are addressed by their asset path (`shader.rs:190-201`).

#### 1.1.2 Retained UI rendering vs the layer system

0.20 re-extracts only nodes whose change ticks fired and keeps all other render data from earlier frames.

- The change filter is in `extract_uinode_changes` (bevy_ui_render `lib.rs:439-457`). It covers `Changed<ComputedNode | ComputedStackIndex | UiGlobalTransform | InheritedVisibility | CalculatedClip | ComputedUiTargetCamera | BackgroundColor | ImageNode | BorderColor | Outline | TextLayoutInfo | …>`.
- `ExtractedUiNodes.changed` lists the main entities re-extracted this frame.

**`crates/core/src/layer/render/cached.rs` (cached-member hide) now blanks layers. Small.**

- During the extract window it overwrites cached-layer members' `InheritedVisibility` with `HIDDEN`, bypassing change ticks.
- Now suppose a member changes (any scroll, or a mount earlier in paint order, since stack indices use `set_if_neq`, bevy_ui `stack.rs:132`) while its layer is cached. Bevy re-extracts it while it is hidden, so its retained data is empty.
- The next re-capture only re-extracts members that changed again, so the layer draws blank or partial.
- **Fix:** delete `cached.rs`, its schedule entries (`plugin.rs:483-489`), and the cached branch of `fallback_sort_key`. Bevy now skips unchanged nodes itself.

**`crates/core/src/layer/render/clip.rs` (extract-window clip swap) leaves stale clips. Medium.**

- The swap also bypasses change ticks, so retained items keep whatever clip they were last extracted with.
- Promoting an unchanged subtree (for example a hover opacity) captures the real ancestor clip. The capture is then wrong after a scroll, because a translation never re-captures.
- Demoting leaves the interior clip, so content draws outside its scroll container.
- **Fix:** on membership transitions (diff `node_to_layer`), call `set_changed()` on the member's `CalculatedClip` in PostUpdate. Every extractor family filters on `Changed<CalculatedClip>` (bevy_ui_render `lib.rs:454`, `gradient.rs:366`, `box_shadow.rs:232`, `ui_texture_slice_pipeline.rs:240`, `ui_material_pipeline.rs:351`).
- Keep the swap in the main world: the `gradient` module is still private (bevy_ui_render `lib.rs:12`), so a render-side rewrite can't reach gradient items.

**`ExtractUiItems` misses two new sets. Small.**

- `plugin.rs:441-456` lists the 0.19 `RenderUiSystems` sets.
- 0.20 adds `ExtractChanges` and `ExtractInlineImages` (bevy_ui_render `lib.rs:133-149, 245-263`) and chains the sets weakly. So the clip swap and the cached-member hide no longer wrap every extractor.

**Future risk.**

- Taking stolen items out of the camera's phase relies on Bevy re-queuing stock `TransparentUi` items every frame. That is still true (`add_transient` at bevy_ui_render `lib.rs:2097`, vertices rebuilt at `:2146`), and is what the guide's "transparent UI still re-extracted" caveat refers to.
- PR #24893 defers retaining phase items to later work. Once that lands, the per-frame steal loses items and needs a redesign.

#### 1.1.3 `<editableText>`

**It ignores all input. Small.**

- Every keyboard, pointer and IME system in 0.20 filters `With<TextInput>` (bevy_ui_widgets `text_input.rs:73, 116-123, 280, 349, 515, 582`).
- `TextInput` requires `EditableText`, but our spawn (`crates/core/src/elements/editable.rs:104-155`) inserts only `EditableText`, so the field renders but ignores typing and clicks.
- **Fix:** add `bevy::ui_widgets::TextInput` to the spawn bundle. Our explicit `AccessibilityNode` overrides its required default.
- Also rename the doc mention `EditableTextInputPlugin` → `TextInputPlugin`.

**Clicking it may not focus it. Small.** The mechanism is confirmed in source; the symptoms are _(unverified at runtime)_.

- The text input no longer focuses itself on press (`text_input.rs:289-296`).
- Focus now comes from `PointerFocusPlugin`, which `UiWidgetsPlugins` adds to DefaultPlugins (bevy_ui_widgets `lib.rs:107`). It triggers `AcquireFocus` for each pressed entity's own `PointerPress` (bevy_input_focus `pointer_focus.rs:29-50`; one press per hovered entity, bevy_picking `events.rs:1060-1077`).
- A focus request that reaches the window clears focus (bevy_input_focus `lib.rs:299-312, 327`).
- Our nodes don't block clicks to what lies beneath them (`focus_pickable` in `crates/core/src/style/writers/interaction.rs:33-41`; the UI root at `plugin.rs:1217`), and the window hit sits under everything (bevy_picking `window.rs:54-57`).
- So one click can focus the input _and_ clear it again via the window. The final state follows `EntityHashMap` iteration order, and every intermediate change is replayed as `FocusGained`/`FocusLost` (`gained_and_lost.rs:72-100`).
- **Likely symptoms:** the field sometimes won't focus, clicking inside a focused field blurs it, and `onFocus`/`onBlur` flicker.
- **Fix:** in `focus_pickable`, set `should_block_lower` when `policy == Block || flags.text == TextRole::Input`, and add a headless test.
- Inside surfaces, the real mouse's window hit may clear focus before the virtual press re-focuses it, giving a spurious blur/focus pair per click _(unverified)_.

**Escape and clicking elsewhere now blur the field.**

- Escape collapses the selection, clears `InputFocus` and keeps propagating (`text_input.rs:237-247`).
- JS `keyDown` is unaffected: `crates/core/src/keyboard.rs:75` reads raw `KeyboardInput`, which focus dispatch doesn't consume.
- `keyDown` (Update) reaches React before `blur` (PostUpdate), so the nodes example's `TextEdit` cancel still wins through its `done` guard.
- `docs/guide/elements/editable-text.md:112-115` ("Clicking elsewhere does not blur…") and `:123-125` (Escape only collapses the selection) are now wrong.
- The cyberpunk `IdCard.tsx:38,137` remount-to-blur hack becomes unnecessary.

#### 1.1.4 Scroll range drift

- taffy went 0.10 → 0.14 (bevy_ui `Cargo.toml`).
- `ComputedNode.content_size` is now the scrollable-overflow rect (`scrollable_overflow_rect.right/bottom`, bevy_ui `layout/mod.rs:370-376`).
- Bevy now clamps the applied offset to taffy's `layout.scroll_width()`/`scroll_height()` (`layout/mod.rs:493-496`), which per the style audit adds border widths (taffy `layout.rs:377`).
- `scroll_range()` (`crates/core/src/scroll.rs:119`) says it "mirrors `ui_layout_system`'s own formula" (`content_size - size + scrollbar_size`), and no longer does. Re-derive it, or our scroll writers clamp to a different range than Bevy applies.

#### 1.1.5 Backdrop frost corners

- `crates/core/src/layer/composite.wgsl:60-81` is a verbatim port of 0.19's circular corner SDF. 0.20 paints corners with ellipse math (bevy_ui_render `ui.wesl:102-121, 287`), so the frost edge drifts slightly from the painted background even for circular corners, and is wrong for elliptical ones.
- Pass both x and y radii: `From<ResolvedBorderRadius>` now yields `[[f32; 4]; 2]`.
  - The uniform `CompositeUniforms` in `layer/render/transform3d.rs` grows from 128 to 144 bytes; update its layout test.
  - Rust sites: `layer/render.rs:342/630/1356`.
- See §3.5 for importing Bevy's SDF instead of porting it.

#### 1.1.6 Examples

**atrium transmission.**

- In 0.19 every `Camera3d` got `ScreenSpaceTransmission` automatically (bevy_pbr-0.19.1 `transmission/mod.rs:35`); in 0.20 it is opt-in.
- Without it, the view has no transmission texture binding or shader def (bevy_pbr `render/mesh.rs:501-503, 3698`; `mesh_view_bindings.rs:869-879`), and materials fall back to their alpha-mode phase (`material.rs:1301`).
- Readers: `examples/atrium/panes/material.rs:32` and `examples/atrium/globe.rs:43`.
- Add `ScreenSpaceTransmission::default()` to the eye (`look.rs:73`), mirror (`world/water.rs:107`) and lens (`lenses.rs:169`) cameras, or gate the reads with `@if(VIEW_TRANSMISSION_TEXTURE)`.
- Whether the failure shows as an error or a blank pane is _(unverified)_.

**`Tonemapping::None` is now a full passthrough.**

- `ColorGrading`/`DebandDither` no longer apply under it, and negative channels are no longer clamped.
- `Camera3d` requires `DebandDither::Enabled` (bevy_core_pipeline `core_3d/mod.rs:104`), and 0.20 warns about `None` combined with `Enabled` (`tonemapping/mod.rs:111-146`).
- Add `DebandDither::Disabled` at `examples/arcana/inspect.rs:194`, `examples/atrium/look.rs:82`, `examples/atrium/lenses.rs:181` (four lenses) and `examples/atrium/world/water.rs:115`.
- The HDR cameras render as before. The non-HDR ones (the arcana card lens and the atrium lenses) lose the clamp and dither that built-in PBR shading applied, which is the behaviour we want.

**Sprite.** Sprites now render as `Mesh2d`, so same-z sprites may draw in a different order. The only use is the minimap markers at `examples/demos/scenes/crowded_cubes.rs:326`, all at z = 0; give each a tiny z offset. The demos cursor arrow is an `ImageNode` (`screenshot.rs:245`) and is unaffected.

#### 1.1.7 Tests that break at runtime

- `ui_layout_system` now takes `Res<RemSize>` (bevy_ui `layout/mod.rs:137`). Add `init_resource::<RemSize>()` to `layout_app` (`crates/core/src/transition/layout_tests.rs:311-330`), as Bevy's own harness does (`layout/mod.rs:562`). Without it every layout and shared-element test panics.
- `PointerId` gained insert/remove hooks that need the `PointerMap` resource (bevy_picking `pointer.rs:27-57`). Add `init_resource::<PointerMap>()` in:
  - `crates/core/src/reconcile/events.rs:330` (2 tests)
  - `crates/core/src/scroll.rs:611`
  - `crates/core/src/layer/pick3d.rs:618`
  - `crates/svg/src/interact.rs:226-337` (5 tests)
  - `crates/svg/src/pick/tests.rs:173-183`
- The built-in shader test text-splices `#import`/`#define_import_path` lines (`crates/core/src/filters/builtin/mod.rs:252-271`; it matches on them at `:258, :278, :357`). Rewrite it; see §3.6.
- Bump the `naga` dev-dependency from 29 to 30 (`crates/core/Cargo.toml:127`): Bevy 0.20 uses wgpu 30, which requires naga ^30. Left at 29, a second naga copy builds and the test validates against a different naga than runtime uses.

### 1.2 Compile errors (mechanical)

- **Bevy version:** set the pin to `"0.20"` (`Cargo.toml:38`).
  - The feature lists need no change. Every feature we list still exists; only `shader_format_glsl`/`shader_format_wesl` were removed, and we use neither.
  - `bevy_shape` is non-optional and re-exported by the prelude. `bevy_curve` is opt-in, but no library crate uses curves or easing.
  - `bevy_render` now implies `bevy_extract`. `custom_cursor` now also turns on `bevy_picking/custom_cursor`; it still pulls in winit, so keep `bevy/x11` in standalone checks.
- **Flat pointer events.**
  - `MessageReader<Pointer<Click>>` → `MessageReader<PointerClick>`, and likewise Press, Release, Drag, Enter, Leave and Move. They are still `Message`s, registered by `InteractionPlugin` (bevy_picking `lib.rs:429-445`).
  - `ev.pointer_id` → `ev.pointer.id`; `ev.pointer_location.position` → `ev.pointer.position` (now a `Vec2`; use `pointer.location()` for a `Location`).
  - Production sites: `crates/core/src/reconcile/virtual_events.rs` (the import, 11 readers, 17 field reads), `crates/core/src/reconcile/events.rs:5,41,61,73,84`.
  - Test constructors become `PointerClick { entity, pointer: Pointer::new(id, loc), … }`:
    - `crates/core/src/reconcile/events.rs` (7 constructors)
    - `crates/core/src/reconcile/pointer.rs:355,370`
    - `crates/core/src/touch_scroll.rs:275,749,762`
- **The picking `PointerPress` component is now `PointerPressState`** (bevy_picking `pointer.rs:120`); the name now belongs to the event.
  - `crates/core/src/layer/pick3d.rs:35,324,618`
  - `crates/svg/src/interact.rs:23,48` and tests at `:209-337`
  - `crates/svg/src/pick/hover_tests.rs:22,136`
  - `examples/demos/cube/mod.rs:29,360`
- **`CalculatedClip` is an enum:** `Rects(SmallVec<[CalculatedClipRect; 2]>) | FullyClipped`. Each rect is in the clipping node's local space, with a `world_to_clip_local: Affine2` (bevy_ui `ui_node.rs:2757-2840`). Medium; about 57 sites.
  - `contains_point(p)` returns true when the point is visible.
  - `crates/core/src/layer/render/clip.rs`: `SwappedClips`, `clip.clip` ×4; `UNCLIPPED` → `CalculatedClip::default()`.
  - `crates/core/src/layer/clip.rs`: the `Option<Rect>` maps, `:137`, and the translation-only `children_clip` at `:235-248`.
  - `crates/core/src/layer/render.rs`: `quad_clip`, `clip_quad`, the transform3d AABB.
  - `crates/core/src/render_target.rs:376` (plus tests at `:642, :652`).
  - `crates/core/src/pick_clip.rs` (×4; better to delete it, §3.2).
  - `examples/demos/scenes/named_pins.rs:256`: `!c.clip.contains(p)` → `!c.contains_point(p)`.
- **`TransparentUi.index` was removed** (bevy_ui_render `render_pass.rs:77-85`): delete the field at `crates/core/src/layer/render.rs:953` and `:964`.
- **Border radius types.**
  - `BorderRadius` corners are now `CornerRadius` (bevy_ui `ui_node.rs:2959`). In `rect_to_border_radius` (`crates/core/src/ui_map.rs:90-97`), write `length_to_val(..).into()`, which gives a circular corner and keeps our "% of the shorter side" rule. The `BorderRadius::all(Val)` call sites still compile.
  - `ResolvedBorderRadius` corners are now `Vec2` (`ui_node.rs:3266`).
    - `crates/core/src/layer/render.rs:630`.
    - `compensate_seed_frame_radius` in `crates/core/src/transition/layout.rs:497-510`: divide each axis by its own scale and clamp to `0.5 * size`. Also drop its `bypass_change_detection`: extraction is retained now, so an untracked write is never re-extracted (it only works today because seeds are fresh nodes), and Bevy compares before writing the radius (`layout/mod.rs:413-425`).
    - The test at `crates/core/src/transition/layout_shared_tests.rs:363-372`.
- **Observer generics:** `IntoObserverSystem<E, B, M>` → `IntoObserverSystem<E: EventPattern, M, Out = ()>` (bevy_ecs `system/observer_system.rs:28`).
  - Sites: `crates/core/src/message.rs:198-203, 221-226, 378-383, 400-405`.
  - `add_react_handler`/`add_react_request_handler` lose the `B` parameter. Callers infer it; none use turbofish.
- **Gamepad:** `GamepadButton/GamepadAxis::Other(u8)` → `Other(u32)` (bevy_input `gamepad.rs:628,693`). Our wire enums at `crates/core/src/gamepad.rs:93,109`; the TypeScript type stays `number`.
- **Examples:**
  - `NextState::PendingIfNeq` → `PendingIfDifferent` at `examples/demos/scene.rs:133` (bevy_state `resources.rs:190`; enum variants get no deprecated alias).
  - Delete the `bevy::math::primitives` import at `examples/arcana/dioramas/figures.rs:6`; the prelude re-exports `bevy_shape`.

### 1.3 Deprecations (warnings only; CI clippy runs without `-D warnings`, `ci.yml:85`)

- **`bevy::ui::Interaction`.**
  - 219 lines mention it: 94 are comments and 125 code. Of the code, 36 lines are production code in 9 files (`reconcile/{pointer,hover,interaction,stamps}.rs`, `layer/pick3d.rs`, `svg/interact.rs`, `elements/editable.rs`, `demos/cube/mod.rs`, `named_pins.rs`) and 89 are tests.
  - It still works: `ui_focus_system` is still registered (bevy_ui `lib.rs:183-186`) and now hit-tests with clipping (`focus.rs:276-279`).
  - Quick fix: `use bevy::ui::DeprecatedInteraction as Interaction;`. Only the alias is deprecated (`focus.rs:65`), not the enum (`:50`).
  - Bevy will likely remove it in 0.21; see §3.8.
- **`bevy::ui::Button`** (`crates/core/src/reconcile/events.rs:22`, `crates/core/src/elements/node.rs:24`): add `#[allow(deprecated)]` and keep it for now. It also gets `Role::Button` plus a label from its child text (bevy_ui `accessibility.rs:98-127`); `ui_widgets::Button` only gets an unlabelled role.
- **glam 0.33 deprecates `Mat4::orthographic_rh`** (`crates/core/src/layer/render.rs:789`, anchor tests): use `bevy::math::proj::orthographic`, as stock Bevy now does.

### 1.4 Docs to update

- **Bevy version:** `README.md:12` (badge), `README.md:144` (compatibility table), `docs/guide/index.md:73`, CLAUDE.md ("Bevy 0.19", the `#define_import_path` note, the GpuImage note).
- **Shader authoring:**
  - `docs/guide/extending/custom-filters.md` (L2, 36, 55-58, 85, 122), `docs/guide/styling/filters.md:393`, `docs/guide/styling/morph-filters.md:101`.
  - `docs/guide/tooling/web.md:152-155`: add that a broken shader now crashes the web build.
  - `docs/src/markdown.ts:21`: add a `wesl: "wgsl"` highlighting alias (refractor has no WESL grammar).
- **Demos snippets:** `CustomFiltersDemo.tsx:36-51, 94-113, 171, 226`, `MorphFilterDemo.tsx:227`.
- **Rustdoc and comments:** `crates/macros/src/lib.rs:198-212, 230, 303-306, 355`, `crates/core/src/plugin.rs:282-286`, `crates/core/src/layer/render.rs:43-52, 1638-1642, 1674`, `examples/demos/filters.rs:4`. The `TextScroll` mention at `layer/clip.rs:26`, and `Pointer<Click>` in comments.
- **Behaviour docs:** `docs/guide/elements/editable-text.md:112-125` (blur on Escape and on click elsewhere). Unit docs (`units.md` says `"1em"` becomes 0 with a warning) once em/rem land.
- **Example READMEs:** 4 of them describe shaders.

### 1.5 Breaking changes for bevy-react users (changelog)

- Custom filter and morph shaders must be WESL: `.wesl` files, the new import line (§1.1.1), and `.wesl` paths in `#[react_filter(shader = …)]`/`#[react_morph_filter]`.
- `add_react_handler`/`add_react_request_handler` have one fewer generic parameter.
- Escape and clicking elsewhere now blur `<editableText>`, so `onBlur` fires.
- Unset `fontSize` now follows `RemSize`: `TextFont::default()` is `FontSize::Rem(1.)` (bevy_text `text.rs:762`), and `resolve_text_font` (`ui_map/text.rs:136`) starts from it. It looks the same at the default 20 px.

### 1.6 Checked, no action needed

- **Dependencies:** accesskit stays 0.24 (bevy_a11y 0.20). parley 0.9 → 0.11: the editor calls we use are unchanged. There are no third-party Bevy plugins in the examples.
- **Rendering:**
  - wgpu 30 texture, pass and sampler descriptors.
  - The render-world APIs we use are unchanged: `SortedRenderPhase`, `ExtractedView`, `ViewUniform`/`UiPipeline.view_layout`, `RetainedViewEntity`, `CachedTexture`, `ViewTarget::main_texture_view`, `ExtractedCamera`, `CameraMainPassTextureFormats`, `FullscreenShader`, `PipelineCache`, `TrackedRenderPass`.
  - We don't use any of the changed render APIs: MainEntityHash\*, TemporaryRenderEntity, Extract/SyncComponent, compositing_space, SortedCamera::hdr, ExtractedWindows, DepthAttachment, WgpuWrapper, ShaderBuffer, BindGroupBuilder, DownsampleShaders, UiCameraMapper.
- **Weak ordering:**
  - Render sets: safe for us. Our render systems talk only through ECS resources (no Mutex, atomics, channels or NonSend) and carry explicit edges (bevy_render `render_context.rs:173`, `lib.rs:469`).
  - The PostUpdate `UiSystems` chain is now weak (bevy_ui `lib.rs:157-168`). Every edge we rely on is explicit: `plugin.rs:995-1022`, `devtools.rs:250-255`, `named_pins.rs:78`, `wire.rs:37`, `spotlight.rs:139`, `table_ops.rs:195-201`.
- **Cameras:**
  - Mixed-HDR camera stacks now alpha-composite. Only arcana has one, and it sets an explicit `blend_state`, which 0.20 honours (`upscaling/mod.rs:64-80`).
  - `Camera2d` now defaults to `Tonemapping::Linear`; no change for our non-HDR UI and surface cameras.
- **Host:** the web host (`crates/core/src/host/web.rs`) uses only `First`/`Last` systems and `Res`, so it is unaffected.
- **ECS and asset APIs we don't use:** `iter_many`/`par_iter_many`, exclusive-system changes, `WorldId`, manual `WorldQuery`, `FilteredResources`, `DeferredWorld::query`, `ComponentInfo::id`, `ComponentId` constants, `AssetId::invalid`, `AssetLoadError` variants, `Ptr::as_ptr`, `ReflectFromPtr`, `FromType`, `to_dynamic`, `TypeIdMap`, `define_label!`, `ScheduleBuildSettings` literals, `MeshTag`, `compute_aabb`, `CustomAttributes`, `RenderAppChannels`.
- **Style and text:**
  - `Name` is already built with `Name::new(String)` (`names.rs:91`).
  - There are no exhaustive matches on `Val`, track sizing, alignment enums, `InterpolationColorSpace` or `Color`.
  - Not used: `Val::resolve`, `compute_affine`, `resolve_font_source`, the generic `FontSource` variants, `Font::from_bytes`, `TextReader`, `TextScroll`, `with_luminance`.
  - `ComputedNode` literals use `..default()`, and `get_layout` only changed its error type.
- **Input and cursor:** `FocusCause::Auto` (no exhaustive match). `cursor.rs` uses only unchanged `bevy_window` types.
- **Picking APIs:** unchanged — `HoverMap`, `PointerHits`, `PointerInput`, `Location`, `HitData`, `PickingSystems`, `MessageMutator`, `RelativeCursorPosition`, `UiStack`, the mesh ray-cast API, the Scrollbar widget.
- **Example APIs:** audio `Decodable`, the gizmos civilization uses, `Image` constructors, `AssetLoader`, `WorldAssetRoot`.
- **Unused features:** no OIT, Atmosphere, DLSS/Solari or compressed image saver.
- **DespawnOnExit:** behaviour is preserved, and despawn hooks still run.
- **`ComputedNode` writes are now change-detected** (border, padding, scroll, outline, radius), so our `Changed<ComputedNode>` consumers fire more often. They are all compare-guarded, so this is a perf nit only.

## 2. New Bevy 0.20 features worth supporting

1. **`em`/`rem` lengths (medium).** A `Val::Em`/`Val::Rem` pair, `RemSize` resource and per-node `EmSize` (bevy_ui, bevy_text).
   - **Parsing:** add `Length::Em`/`Length::Rem`. In the `units.rs` suffix list, `"rem"` must come before `"em"` (the grad/rad rule). Grid tracks: two more suffixes in `grid.rs:43-60` → `GridTrack::em/rem`.
   - **Interpolation:** add cases to `length_to_val`, the Length lerp (`channels.rs:592`), filter `length_logical_px` (reject them), `transition/shared.rs:286` and gradient `lerp_val` (`gradient_channel.rs:217`). The last two have catch-all cases that would otherwise silently snap or drop the new units.
   - **transform3d origin:** resolve exactly via `ComputedNode.em_size`/`rem_size`.
   - **TypeScript:** `Length` is unchanged; update `jsx.d.ts` and `units.md`.
   - **Caveat:** Bevy only derives `EmSize` on nodes that have a `TextFont` (bevy_ui `layout/mod.rs:81-101`). Everywhere else it is a constant 20 that ignores `RemSize` (bevy_text `text.rs:883-905`), and propagating it is the app's job.
   - **Making `em` web-like:**
     - Only `<text>`/`<editableText>` carry a font, so a `<node>`'s inherited font size is always the root size. One small system can set `EmSize = RemSize` on fontless React nodes, on `RemSize` change and on `Added`.
     - `fontSize: "1.2em"` on a text root is exactly `FontSize::Rem(1.2)`.
     - On a styled span it is 1.2 × the enclosing run's resolved `FontSize`, exact within each `FontSize` variant (`text.rs:783-796`). Resolve it at append time and re-resolve when the parent's font changes, reusing the bare-run inheritance cascade.
     - Bevy's `PropagateSet<TextFont>` propagates whole components, so it doesn't fit spans. `letterSpacing: "…em"` can become exact the same way.
   - **Expose `RemSize`** through a built-in message or plugin option. Default text now follows it, which makes a global text-size / accessibility knob.
2. **Inline images in text (medium).** New `InlineImage`/`InlineBox` (bevy_ui `inline_image.rs`).
   - In `renderer.ts`, swap kinds as for `textSpan`: an `<image>` inside `<text>` becomes a node-less `inlineImage` element.
   - It spawns `InlineImage` from `src`/`tint`/`flipX`/`flipY` plus px `width`/`height` (`"1em"` = × the run's font).
   - It works inside layers, because it is extracted under the text root (bevy_ui_render `lib.rs:839-893`). Bracket `ExtractInlineImages` and add `Changed<InlineImage>` to layer dirt.
   - **Caveats:** it is 0×0 and out of flow until the asset loads, even with an explicit size, so the text reflows on load. No events, `.svg`, atlas/imageMode or `imageRendering`.
   - Defer a generic `InlineBox` for `<node>`s: it would need positioning from `TextLayoutInfo.inline_boxes`, with a one-frame lag.
3. **`position: "fixed"` → `FixedNode` (medium).** New marker in bevy_ui `ui_node.rs:3532-3539`.
   - Use `FixedNode` plus `Absolute`. The layout writer owns `FixedNode`, and the keyword table needs our own enum.
   - **Caveats:**
     - It stays in its parent's stacking partition; pair it with `globalZIndex`.
     - It ignores transformed ancestors, whereas CSS makes them the containing block.
     - It escapes `display: none` ancestors: the inherited clip is discarded (`clipping.rs:64`) and it is laid out as its own root (read, not run). The bridge would have to hide it.
     - Layer membership must stop at it (it escapes the capture rect), and the layout-transition and shared-element walks must treat it as a root.
   - **Bonus:** `<anchor>` overlays could be `FixedNode` children instead of detached roots under an `AnchorLayer` _(unverified)_.
     - A fixed child is excluded from its parent's layout children and content size (`layout/mod.rs:239, 334`), and its clip is discarded.
     - Keeping the real hierarchy gives despawn, `Visibility` and the camera inside a `<surface>` for free. Give each anchor `GlobalZIndex(-1)`.
     - The same could apply to `<root>`, if it uses the parent's camera.
4. **Elliptical `borderRadius` (medium).** `CornerRadius::new(x, y)` per corner (bevy_ui `ui_node.rs:2959`).
   - Accept the CSS slash form `"10px 20px / 5px"` or `{ x: Rect, y: Rect }`. Without a slash, keep today's circular corners.
   - CSS `50%` is an ellipse, and `CornerRadius::all` resolves % per axis. Switching to it would change existing pill shapes, so decide deliberately.
   - The transition channel goes from `ProgressChannel<Rect>` to a pair of rects with per-leaf lerp. Seed compensation and the backdrop SDF are already per-axis after §1.
   - TypeScript: ``Animatable<Rect> | `${string}/${string}` | { x: Rect; y: Rect }``.
5. **Pointer capture (small to medium).** New `PointerCaptureMap` (bevy_picking `hover.rs:85`), which releases itself on Release/Cancel (`:172-184`).
   - Call `capture(pointer, entity, hit)` when a drag begins in `collect_pointer_events`/`collect_virtual_pointer_events`. That gives DOM-style implicit capture: hover and click stay on the dragged node.
   - This fixes the known "drag release clicks the ancestor" bug in the framework, and retires the `focusPolicy: "block"` workaround in the demos `Slider.tsx:68`. An explicit `setPointerCapture` can come later.
6. **Focus and form props (small each).**
   - `<editableText readOnly disabled>` → `TextReadWriteMode::ReadOnly` (bevy_text `editing.rs:319`) and `InteractionDisabled`, which the text input honours (`text_input.rs:120, 165-166, 533, 598`) and which also marks the node disabled for accessibility (`interaction_states.rs:33-38`). That is two `Attribute<bool>` plus an insert/remove in `EDITABLE_WRITER`; the same marker would serve `<button disabled>`.
   - A common `tabIndex` prop → `TabIndex`. Click-to-focus now works for any `TabIndex` entity (`pointer_focus.rs:92-98`), so `focusStyle`/`onFocus`/`onBlur` can extend to every element; the `FocusState` machinery is already generic. Tab cycling needs an opt-in `TabNavigationPlugin`. These nodes also need the blocking rule from §1.1.3.
   - `focusVisibleStyle`, driven by `InputFocusVisible` (a pointer press clears it, `pointer_focus.rs:42`).
7. **Small style additions (small each):**
   - Safe alignment keywords `safeCenter|safeStart|safeEnd|safeFlexStart|safeFlexEnd` → the `*Safe` variants; the TS union regenerates.
   - Gradient `colorSpace: "okhsl" | "okhslLong"` (bevy_ui `gradients.rs:691`).
   - `fontFamily` as a CSS fallback list → `FontSource::List` of our registered handles. Unknown names pass through as `Family(name)`, and generic keywords map (bevy_text `text.rs:283-345`). Caveats: generic families need Bevy's `system_font_discovery` feature, and one unloaded handle makes the whole list fail (`NoSuchFont`).
8. **WESL for filter authors (docs).**
   - `@if` and `constants::` come for free. Bevy's global defs reach our filter pipelines (`SIXTEEN_BYTE_ALIGNMENT`, `NO_ARRAY_TEXTURES_SUPPORT` on WebGL2; bevy_render `pipeline_cache.rs:249-254`). For example, wrap `countOneBits`, which kills WebGL2 (`web.md:152`), in `@if(!SIXTEEN_BYTE_ALIGNMENT)`.
   - `import … as …` and inline qualified calls work.
   - Editor support via wgsl-analyzer (Bevy ships a `constants` stub package in `bevy-0.20.0/constants/`); the language server resolving our prelude favours prelude Option 1.
   - Per-library shader defs (`load_shader_library!(app, path, |s: &mut ShaderSettings| …)`, `shader.rs:257-300`) could feed engine constants into the prelude; not needed now.
9. **Devtools paint flashing (small to medium).** `ExtractedUiNodes.changed` is a public, exact "re-extracted this frame" set.
10. **Schedule shuffling for our ordering tests (small to medium).** See §3.7.

### Not worth supporting

- **Feathers widgets and contextual theming:** a different UI toolkit.
- **Headless tabs:** React already owns selection.
- **`Hovered`/`DirectlyHovered`:** mouse-only.
- **A `FocusCause` payload:** clicks report `Navigated` (`tab_navigation.rs:400`).
- **BSN and its `Ready` event, sprite materials, `ExtendedMaterial2d`, mesh shaders (native-only), Solari/DLSS:** unrelated to UI.
- **`PanOrbitCamera` for the demos camera:**
  - It has no auto-orbit, and the plugin group ships no input plugin (`pan_orbit_camera/mod.rs:197-209`; `controller/mod.rs:15`).
  - It pivots on the picked point, while the demos camera always looks at the origin.
  - We would still need our `PointerCapture` gating.
- **bevy_picking's `CursorIconPlugin`:**
  - It can't replace `cursor.rs`, which needs the `UiStack` hit test and virtual pointers.
  - It is mouse-only, and `hover_set.keys().find_map` (`cursor.rs:98-103`) picks an arbitrary entity from our pass-through hover stacks.
  - Don't add both, since both write the window cursor.
- **`despawn_all`:** it takes only a query filter (bevy_ecs `world/mod.rs:1691`), and all our despawns target specific entities. DespawnOnExit and bevy_audio get the speed-up for free.
- **Library-level panic catching:**
  - `App::set_error_handler` asserts it is called once (`app.rs:1493`), and a half-applied op batch would desync the shadow tree.
  - An example (for instance the nodes editor) could call `set_error_handler(error)` itself.
- **`BevyError::context`:** only the svg loader returns `BevyError` (`svg/asset.rs:46`), and the asset server already logs the path.

## 3. Improvements 0.20 makes possible

1. **The layout-transition clip limitation is gone.**
   - A child's clip is now the local rect mapped through its parent's full `UiGlobalTransform` (bevy_ui `layout/clipping.rs:97-103`), computed in PostLayout (`lib.rs:205`), after our FLIP compose.
   - So `overflow: clip` with `transition: { layout }` reveals at the scaled box, in rendering and picking. That makes accordion reveals possible.
   - Restore the demos "Hide docs" accordion and drop the documented limitation. Inside promoted layers it also needs the affine interior clips from item 4. Not run yet; confirm with `--shoot`.
2. **Delete `crates/core/src/pick_clip.rs` (209 lines)** and its slot at `plugin.rs:660` (plus the `.after` references at `:671`, `:685`).
   - 0.20 UI picking and `ui_focus_system` test the node's inherited `CalculatedClip::contains_point` (bevy_ui `picking_backend.rs:205-209`, `focus.rs:276-279`), and `clip_check_recursive` is gone.
   - Move `pointer_physical_position` into `raster/`, since the svg picking code uses it.
   - **Can land before the port:** Bevy 0.19.1 already fixed the recursion (diffed `clip_check_recursive` 0.19.0 vs 0.19.1), and our lockfile pins 0.19.0. After `cargo update -p bevy`, re-verify with a scrolled devtools tree.
3. **Delete `crates/core/src/layer/render/cached.rs` (240 lines).** Bevy's retained extraction already skips unchanged nodes (also the required fix in §1.1.2).
   - The capture cache itself stays, because queue, prepare and draw still run every frame. Measure before parking `uinodes` entries render-side; that couldn't cover the private gradient items anyway.
4. **Simpler interior clips.**
   - Clip lists are ordered outermost first (`push_rect`, bevy_ui `layout/clipping.rs:98-103`). So a member's interior clip is `rects()[K..]` when its first K rects equal the layer root's; otherwise it is the whole list (an `OverrideClip` or `FixedNode` reset).
   - That replaces the cascade mirror in `crates/core/src/layer/clip.rs` (`cascade`, `children_clip`, `ClipSyncMemory`; about 170 lines), handles rotation and scale correctly, and removes the documented `OverrideClip` divergence (`layer/clip.rs:37-39`).
   - Clamp composite quads with the public `bevy::ui_render::clipping::clip_polygon` (bevy_ui_render `clipping.rs:16`), with position + UV lerp. Rotated or scaled clippers then clip exactly as stock does. Use it for `render_target.rs` `clipped_out` too.
5. **Shader dedupe and cleanup.**
   - Replace the copied rounded-box functions (`composite.wgsl:60-81`, `spotlight.wgsl:34-44`) with `import bevy_ui_render::ui::sd_rounded_box;`.
     - Bevy registers `ui.wesl` as an importable library (bevy_ui_render `lib.rs:213`) and imports it itself (`box_shadow.wesl:3`, `gradient.wesl:2`). WESL strips unused declarations (wesl `frontend.rs:137`), so `ui.wesl`'s bindings don't come along.
     - That matches Bevy exactly, elliptical corners included. `CompositeParams` gains `radius_x`/`radius_y`; its Rust mirror is in `transform3d.rs`.
   - Delete naga_oil-era rules: the prelude's NAMING CONSTRAINT comment (`filter_prelude.wgsl:52-59`), the checklist at `layer/render.rs:43-53`, the doc at `:1638-1642`, and the `pad_a`/`pad_b` naming rule. WESL hands WGSL text straight to wgpu, so naga no longer renames identifiers.
   - Delete atrium's `ShaderLibrary` keep-alive (`examples/atrium/world/mod.rs:70-77`); the 0.20 loader loads and holds relative-import dependencies itself (bevy_shader `shader.rs:306-329`).
6. **A real shader test.**
   - Compose WESL on the CPU with the public `ShaderCache::<String, ()>::new((), |_, src, _| …)` plus `Shader::from_wesl`, as Bevy's own tests do (`shader_cache.rs:148-166, 551-575`). Then validate the composed output with naga.
   - Extend the walk to every example's shader folder; today it only covers demos (`builtin/mod.rs:337-366`).
   - This becomes the only pre-ship guard against the new web crashes.
   - Also make `#[react_filter]`/`#[react_morph_filter]` reject a `shader` path ending in `.wgsl` (`crates/macros/src/lib.rs` around line 350). That turns an invisible subtree or web crash into a build error.
7. **Schedule shuffling for ordering tests.** `ScheduleBuildSettings::shuffle_seed` exists only with `bevy_ecs/debug` (`schedule.rs:1887`).
   - Add a core feature `schedule_shuffle = ["bevy/debug"]`. Don't enable `debug` on the dev-dependency instead: that rebuilds all of Bevy on every `cargo test`.
   - In `op_app_with` (`reconcile/test_util.rs:22`) and `layout_app` (`transition/layout_tests.rs:310`), when `BEVY_REACT_SHUFFLE_SEED` is set: `edit_schedule(Update | PostUpdate, …)` with `set_build_settings(ScheduleBuildSettings { shuffle_seed: Some(seed), ..default() })` plus `set_executor_kind(SingleThreaded)`, and print the seed.
   - Loop N seeds over the svg `raster_tests.rs` same-frame tests, `ElementOverrideSet`, and the animation and transition tests. This also exercises the new weak chains.
   - Caveat: automatic sync points still hide some orderings.
8. **Retire `Interaction` before 0.21 removes it (large).**
   - `Hovered`/`Pressed` are not drop-in. `Hovered` is mouse-only (bevy_picking `hover.rs:470`) and counts descendants; `Pressed` is only inserted by widget observers (bevy_ui_widgets `button.rs:94`).
   - Better: drive hover and press for every pointer from picking messages, as `crates/core/src/reconcile/virtual_events.rs` already does. That deletes:
     - the `Interaction` paths in `reconcile/hover.rs`/`interaction.rs`;
     - pick3d's `correct_transformed_interactions`;
     - the `Interaction` half of svg's `sync_shape_interactions`;
     - the cube example's `sync_cube_interactions`.
   - It changes the extension contract (`InteractionSyncSet`), so third-party element authors would notice.
   - Fold `virtual_events.rs`'s 11 near-identical reader loops into one helper over the new `PointerEvent` trait (bevy_picking `events.rs:127`).
   - Keep `sync_editable_a11y`: Bevy still doesn't sync the accessibility value.
9. **Cursor clipping fix.** `window_cursor` in `crates/core/src/cursor.rs` ignores clipping, so a node scrolled out of view can still set the cursor. Replace it with the existing `surface_cursor_for(PointerId::Mouse, …)` over `HoverMap`, which is correct for clipping, Block and transform3d. That also drops the `visually_transformed_members` call.
10. **`Length` as a thin wrapper over `Val` (optional, medium).**
    - `Val` now has every unit we parse, a `FromStr` (bevy_ui `geometry.rs:89`; keep our bare-number branch) and interpolation. That deletes `length_to_val` and the duplicate lerp.
    - Beware: `Val` treats `Px(0) == Percent(0)` as equal, which matters for compare-before-write.
    - At minimum, `lerp_val` (`gradient_channel.rs:215-229`) can become `a.try_interpolate_stable(&b, t).unwrap_or(b)` (`geometry.rs:535-556`), which covers Em/Rem.
11. **Smaller wins:**
    - **Exact seed-frame radius:** per-axis `Vec2` radii remove the "mean of an anisotropic scale" approximation in shared-element seeding.
    - **Simpler asset watch:** `watch_layer_image_assets` (`crates/core/src/layer.rs:1091`) can become `Changed<ImageNode>` ordered after `ImageNodeAssetChangedSystems`, since 0.20 marks the node changed on asset change (bevy_ui_render `image.rs:53`).
    - **Stale comments:** `GlobalZIndex` ties are now stable across frames (bevy_ui `ui_node.rs:2872-2883`), so the "no tiebreak" reasoning in `node.rs` `root_style` and the `AnchorLayer` docs is out of date.
    - **No aliasing with stock data:** 0.20's prepare systems match items by `(main_entity, render_entity)` (bevy_ui_render `lib.rs:2167`), so our composite quads can't alias stock data.
    - **Free perf:**
      - Idle frames re-extract nothing, which rewards our compare-before-write writers.
      - No per-item `TemporaryRenderEntity` churn (0.19 had 15 sites in bevy_ui_render `lib.rs`, 0.20 has none).
      - Indexed quads, and weak-chain parallelism.
      - DespawnOnExit scene switches use `despawn_all`.
12. **Re-test after taffy 0.14 _(unverified)_:** the `maxWidth` wrapped-text measurement bug and the fractional-scale flex-wrap bug may be fixed.

### Checked, not a win

- **`#[component(summary_tick)]` is not a drop-in speed-up.**
  - Plain `Changed<T>` filters still check every row's tick (bevy_ecs `query/filter.rs:1133-1160`). The column summary is only read through `Query::contiguous_iter`/`summary_tick_is_changed()` (`component/mod.rs:514`, `params.rs:234`).
  - Our hot wake filters (`transition/mod.rs:171`, `animations/apply/mod.rs:79`) mix in Bevy components we can't annotate.
  - The only candidate is `SvgShape`, scanned every frame at `crates/svg/src/surface.rs:123`; measure first.
- **`chain_weak` for our own chains:** the only runtime chain is `AnimationSet` Drain→Tick→Apply (`animations/mod.rs:84`). It depends on a channel and shared resources, so keep it strict.
- **The GpuImage reuse bug is not fixed in 0.20.** bevy_render `texture/gpu_image.rs` is byte-identical between 0.19.1 and 0.20.0, and the reuse path (`:70-101`) still rewrites mip 0 only.
  - Keep the nodes workaround (`examples/nodes/graph/textures.rs:105-108`).
  - Core has the same latent bug: trilinear `imageRendering` variants keep `COPY_DST` (`crates/core/src/image_rendering/pyramid.rs:119`) and are re-inserted over the live handle (`image_rendering.rs:415`), so a same-size hot reload leaves mip levels 1 and up stale.
  - One-line fix, independent of the port: `usage = TEXTURE_BINDING` on mipped variants.
- **An SDR UI camera over an HDR camera now composites** (`upscaling/mod.rs:72-76`), but it doesn't help atrium: its UI lives on in-world `<surface>` panes rendered by the eye camera itself.

## Verification status

- **Checked by hand in the 0.20 source:**
  - `.wgsl` loads verbatim (`shader.rs:135-147, 296`), and the wasm shader-module crash (`pipeline_cache.rs:165-178`).
  - `UiVertexOutput` radius split.
  - The `With<TextInput>` filters, and `PointerFocusPlugin` in `UiWidgetsPlugins` with its `click_to_focus`.
  - The retained-extraction change filter vs `cached.rs`'s tick bypass.
  - taffy 0.10 → 0.14 and the new scroll clamp.
  - The `clip_check_recursive` fix in 0.19.1, and `CalculatedClip::contains_point` semantics.
  - accesskit 0.24 in both versions.
- **Checked by the audits, not re-checked by hand:** every other file:line reference above.
- **Needs a runtime check:**
  - the focus-race symptoms;
  - the accordion reveal under a layout transition;
  - anchors as `FixedNode` children;
  - the `bevy_react::filter` embedded registration end-to-end;
  - how atrium fails without transmission;
  - `FixedNode` escaping `display: none` ancestors;
  - the taffy bug fixes.
