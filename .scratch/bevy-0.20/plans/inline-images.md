# Inline images in text — implementation plan

An `<image>` written inside a `<text>` becomes an inline box in the text flow,
on Bevy 0.20's `InlineImage` / `InlineBox`. Audit section: §2.2.

Everything below was read, not run (no cargo, no shoot). Bevy paths are under
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`. Our line numbers are
as of `b7e408c` plus another agent's uncommitted layer/clip edits — they drift,
so the symbol is the reference.

## 1. Verified facts

### Bevy 0.20

| #   | Fact                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | Source                                                                                               |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| B1  | `InlineBox { kind: InlineBoxKind, size: Vec2 }` — size in **logical px**; `InlineBoxKind::{InFlow, OutOfFlow}`, default `OutOfFlow`. A leaf of the text tree: its children are not walked. `PartialEq`, so `set_if_neq` works.                                                                                                                                                                                                                                                                                                                                                                                                                      | `bevy_text-0.20.0/src/inline_box.rs:5-29`                                                            |
| B2  | `InlineImage { color, image: Handle<Image>, width: Option<f32>, height: Option<f32>, flip_x, flip_y }`, `#[require(InlineBox)]`, `PartialEq`, `AsAssetId`. Default image is `TRANSPARENT_IMAGE_HANDLE`. **No atlas, source rect, image mode, slicing, radius, sampler choice.**                                                                                                                                                                                                                                                                                                                                                                     | `bevy_ui-0.20.0/src/widget/inline_image.rs:20-76`                                                    |
| B3  | Size: `resolve_inline_box_size(image_size)` — both set → that box (texture stretched); one set → the other follows the texture's aspect; none → the texture's pixel size taken as logical px. Px only, nothing relative to the font.                                                                                                                                                                                                                                                                                                                                                                                                                | `inline_image.rs:58-68`                                                                              |
| B4  | `update_inline_image_boxes` runs on `Changed<InlineImage>` or `AssetChanged<InlineImage>`. If the image is **not in `Assets<Image>`** (or the size is not finite and positive) it writes `InlineBox { OutOfFlow, ZERO }` — **even when width and height are both given**. Otherwise `InFlow` + the resolved size. `set_if_neq` both ways.                                                                                                                                                                                                                                                                                                           | `inline_image.rs:78-103`                                                                             |
| B5  | That system is `PostUpdate`, `UiSystems::Content`, `.before(detect_text_needs_rerender)`. Both functions are `pub`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | `bevy_ui-0.20.0/src/lib.rs:241-247`                                                                  |
| B6  | Which entity: any child (by `Children` order) of the text root **or of a `TextSpan`**, at any depth. `TextSpanIter` walks the root, then children depth-first; an entity with `InlineBox` is yielded as `TextElement::Box` and not descended into; a child that is neither span nor box is skipped. So an inline image is ordered among spans exactly by its position in `Children`.                                                                                                                                                                                                                                                                | `bevy_text-0.20.0/src/text_access.rs:138-197` (pinned by `bevy_sprite-0.20.0/src/text2d.rs:398-436`) |
| B7  | In the pipeline a box takes a section slot (`computed.entities` gets a `TextEntity` for it) and is pushed as `parley::InlineBox { id: entity bits, index: byte offset, width/height: size × scale_factor }`. After layout each one lands in `TextLayoutInfo.inline_boxes: Vec<(Entity, InlineBoxKind, Rect)>`, physical px, relative to the layout's top-left.                                                                                                                                                                                                                                                                                      | `bevy_text-0.20.0/src/pipeline.rs:163-176, 270-279, 458-468, 506`                                    |
| B8  | What re-shapes the block: `Changed<InlineBox>` (walks `ChildOf` up through `TextSpan` ancestors to the nearest `ComputedTextBlock`), and `Changed<Children>` on the root or a span. So mount, unmount, reorder and resize all re-measure and re-lay out.                                                                                                                                                                                                                                                                                                                                                                                            | `bevy_text-0.20.0/src/text.rs:1562-1660`                                                             |
| B9  | Vertical placement (parley 0.11.1): the box's **bottom sits on the baseline** (`y = baseline − height`), its whole height counts as ascent (`ascent = max(ascent, height)`), and the line height is at least the box height. No alignment option.                                                                                                                                                                                                                                                                                                                                                                                                   | `parley-0.11.1/src/layout/line.rs:262-280`, `layout/line_break.rs:241-244, 925-937`                  |
| B10 | Wrapping: a box is atomic. If it does not fit, the line breaks before it (unless it is first on the line — then it overflows); a break opportunity follows every box. `NoWrap` never breaks. It contributes to min- and max-content width. Alignment/justify offsets apply (`metrics.offset`).                                                                                                                                                                                                                                                                                                                                                      | `line_break.rs:494-545`, `layout/data.rs:604-615`, `line.rs:263-265`                                 |
| B11 | Extraction (`extract_inline_images`, set `RenderUiSystems::ExtractInlineImages`, last in the weak chain): iterates the **changed text roots**, and for each `inline_boxes` entry with an `InlineImage` inserts an `ExtractedUiNode` **under the root's entry** (`uinodes.entry(root)`), with the root's `UiGlobalTransform`, `CalculatedClip`, camera and visibility, `z_order = root stack index + 0.09` (`stack_z_offsets::INLINE_IMAGE`, above glyphs 0.06 and the cursor 0.08). Whole texture over the box rect, `border_radius` zero. Skipped when the rect is empty, the color fully transparent, or the image is `TRANSPARENT_IMAGE_HANDLE`. | `bevy_ui_render-0.20.0/src/lib.rs:117-130, 245-263, 282, 824-893`                                    |
| B12 | Retained extraction: `Changed<InlineImage>` or `RemovedComponents<InlineImage>` invalidates the entity and then walks `ChildOf` (`With<TextSpan>` or `With<InlineBox>`) up to the ancestor with `Text`, which is re-extracted whole. A box move/resize reaches extraction through `Changed<TextLayoutInfo>` on the root.                                                                                                                                                                                                                                                                                                                            | `lib.rs:486-504, 566-570, 631, 672-712`                                                              |
| B13 | `prepare_uinodes` runs every frame; an item whose `GpuImage` is missing is skipped that frame and drawn once it exists. An item with `AssetId::default()` is drawn untextured in its color.                                                                                                                                                                                                                                                                                                                                                                                                                                                         | `lib.rs:2116-2260` (`2186-2217`, `2253-2258`)                                                        |
| B14 | `ImagePlugin` inserts `Handle::default()` (1×1 white) and `TRANSPARENT_IMAGE_HANDLE` (1×1 transparent) into `Assets<Image>`. So a `Handle::default()` inline image is a solid `color` box; a transparent one reserves its box and draws nothing.                                                                                                                                                                                                                                                                                                                                                                                                    | `bevy_image-0.20.0/src/image.rs:229-232`                                                             |
| B15 | Picking: nothing in `bevy_ui`'s picking backend, layout or accessibility mentions `InlineImage`/`InlineBox` (grep). The entity has no `Node`, so it is never a hit; the block's box is.                                                                                                                                                                                                                                                                                                                                                                                                                                                             | `bevy_ui-0.20.0/src/{picking_backend,layout/mod,accessibility}.rs`                                   |
| B16 | Text shadow, underline, background extraction work on glyph runs only; an inline image casts no `textShadow`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | `bevy_ui_render lib.rs` (`extract_text_shadows`, `extract_text_decorations`)                         |

### Ours

| #   | Fact                                                                                                                                                                                                                                                                                                           | Source                                                                                                                            |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| O1  | JS: `getChildHostContext` turns `inText` on under the first `<text>` and never off. `createInstance` picks the wire kind: `type === "text" && hostContext.inText ? "textSpan" : type`. The instance keeps `kind`, and every update op names it. An `<image>` inside `<text>` is created as kind `image` today. | `js/src/renderer.ts:114-115, 170, 189, 272-278`                                                                                   |
| O2  | `TEXT_SPAN` is the precedent: `flags: { text: TextRole::Span, ..NODE_LESS }`, `common: Common::IDENTITY`, a spawn hook. Node-less = no `Node`, no style bundle (`SpawnCtx::spawn`), `layer_ineligible`.                                                                                                        | `crates/core/src/elements/text.rs:28-53`, `element/decl.rs:157-170`, `ext/mod.rs:48-61, 112-116`                                  |
| O3  | A node-less element gets no global style writers unless it is a text element (`global_mask`), but **its own writers run** and may read style properties and attributes (svg shapes). `ignored_styles` (`styleIgnored`) is only computed for elements with a `Node`.                                            | `element/registry.rs:131-160`                                                                                                     |
| O4  | `style` decodes on every element. A common prop outside the element's `Common` groups is dropped with `propIgnored`; a key that is no attribute of the element is dropped with `unknownProp`. Both messages name the wire kind.                                                                                | `protocol/props.rs:194-216, 240-252, 262-277`                                                                                     |
| O5  | Attribute statics are keyed by address and can be listed by several elements (svg shapes do). `SRC`, `TINT`, `FLIP_X`, `FLIP_Y` are `pub`, `String`/`bool`, all `Invalidation::PAINT`. `<image>` has **no `{ texture }` form** — that is `backgroundImage` and `<portal>`; `src` is a string.                  | `elements/image.rs:19-38`, `element/store.rs:67-72`, `js/src/generated/elements.ts` (`BevyImageProps`)                            |
| O6  | Hierarchy: `Append`/`Insert` attach with `add_child` (order fixed up from the shadow tree), push content dirt on the parent, and call `inherit_text_style`, which only acts on `SpanKind::RawInherited` children. Nothing there is kind-specific.                                                              | `reconcile/apply.rs` (`Op::Append`, `Op::Insert`, `inherit_text_style`)                                                           |
| O7  | Layer membership claims **every** entity reachable through `Children` (`map.insert(node, layer)` is unconditional), so a node-less child's content dirt resolves to its block's layer. A PAINT attribute or style change ends in `mark_content_dirty(ec)`.                                                     | `layer.rs` (`SubtreeWalk::mark_subtree`, `mark_content_dirty`), `ui_map.rs` (`apply_style_masked`), `reconcile/update.rs:117-124` |
| O8  | Layer capture routes phase items by `item.main_entity()`; an inline image's item carries the text root (B11).                                                                                                                                                                                                  | `layer/render.rs` (`extracted.membership.get(&item.main_entity())`)                                                               |
| O9  | `RenderUiSystems::ExtractInlineImages` is already inside the `ExtractUiItems` umbrella the clip swap brackets.                                                                                                                                                                                                 | `plugin.rs:455-470`                                                                                                               |
| O10 | Layer dirt for async textures: `watch_layer_image_assets` taps `AssetChanged<ImageNode>` only. Text reshapes are tapped through `Changed<TextLayoutInfo>`.                                                                                                                                                     | `layer.rs` (`watch_layer_image_assets`, `resolve_layer_repaints` step 3)                                                          |
| O11 | `Length` has no `Em`/`Rem` yet (the em/rem plan adds them). `width`/`height` are `StyleProperty<Animatable<Length>>`.                                                                                                                                                                                          | `protocol/units.rs:14-22`, `style/props/layout.rs:72-79`                                                                          |
| O12 | Svg-mode `<image>` rasters into an element-owned texture sized from `ComputedNode`, with `ContentSize`/`ImageNode`/`SvgSurface` — none of which an inline image has. `imageRendering` binds through `ImageNode` + a global writer's `ImageRenderingMode` stamp.                                                | `svg/raster.rs` (`update_svg_surfaces`), `image_rendering.rs` (`bind_image_rendering`)                                            |
| O13 | No existing test runs Bevy's text shaping; `layout_app()` runs `ui_layout_system` only. The demos e2e harness sees the real op stream from the real reconciler.                                                                                                                                                | `transition/layout_tests.rs:310-375`, `examples/demos/tests/common/mod.rs`                                                        |

### What the audit got wrong or left out

- "add `Changed<InlineImage>` to layer dirt": not needed. Every `InlineImage`
  write is our writer's, under an op whose invalidation is PAINT (O7). What
  **is** missing is the asset tap (O10): a texture that loads or reloads without
  changing the box size re-shapes nothing.
- "`\"1em\"` = × the run's font": `Length` has no `em` (O11). Out of v1.
- "bracket `ExtractInlineImages`": already done (O9).
- "0×0 until the asset loads, even with an explicit size": confirmed (B4). We
  can fix it in ~15 lines (§2.4).
- Its extraction line reference is `824-893`, not `839-893`.

## 2. Design

One new **wire kind**, `inlineImage`: a node-less core element whose one writer
builds a Bevy `InlineImage`. The reconciler picks it for an `<image>` inside a
`<text>`, the way it picks `textSpan` for a nested `<text>`. Reusing the `image`
element with a flag is not possible: flags, writers and the `Node` are fixed per
element kind (`<image>` is a styled node that owns an `ImageNode`).

### 2.1 JS — `js/src/renderer.ts`

`createInstance`:

```ts
// Inside a `<text>`: a nested `<text>` is a styled span, an `<image>` an
// inline box in the text flow. Everything else keeps its kind.
const kind = !hostContext.inText
  ? type
  : type === "text"
    ? "textSpan"
    : type === "image"
      ? "inlineImage"
      : type;
```

Update the `Instance.kind` comment. Nothing else: `serializeProps` and
`buildUpdateOp` are generic, and update ops already carry `instance.kind`.

No wire-format change: an ordinary `create`/`update` with `kind:
"inlineImage"`, props `src`, `tint`, `flipX`, `flipY`, `name`, `style`.

### 2.2 The flag — `crates/core/src/ext/mod.rs`

Add `TextRole::InlineBox`: "an inline box in a block's text flow (an `<image>`
inside a `<text>`): `Node`-less, a leaf of the text tree, no text style." The
codebase rule is flags, not kind checks; this is the one bit three places need.
Sites to touch (all the `TextRole` comparisons there are):

- `element/registry.rs` `global_mask`: `if decl.flags.text == TextRole::Span {
styles.masks.span } else { WriterMask::NONE }` (today: `== None → NONE`, which
  would hand an inline box the span's text writers).
- `element/registry.rs` `ignored_styles`: gate becomes `!decl.flags.node_less ||
decl.flags.text == TextRole::InlineBox`. An inline box takes a `style`, so
  every property its writer does not read reports `styleIgnored` through the
  existing `warn_ignored_styles` (create and update already call it). Svg shapes
  are untouched.
- `style/writer.rs` `WriterCtx::text()`: `!matches!(…, TextRole::None |
TextRole::InlineBox)`.
- `element/ts.rs` `is_intrinsic`: `!matches!(element.flags.text, TextRole::Span
| TextRole::InlineBox)` — a wire kind the reconciler picks is no JSX
  intrinsic.

`create.rs`/`update.rs` compare against `Block`/`Span` explicitly and need
nothing: an inline box gets no `text_styles` entry and no `SpanKind`.

### 2.3 The element — new `crates/core/src/elements/inline_image.rs`

```rust
pub static INLINE_IMAGE: Element = Element {
    flags: ElementFlags { text: TextRole::InlineBox, ..ElementFlags::NODE_LESS },
    attrs: &[&SRC, &TINT, &FLIP_X, &FLIP_Y],      // elements::image's statics
    common: Common::IDENTITY,
    writers: &[&INLINE_IMAGE_WRITER],
    spawn: Some(spawn_inline_image),
    ..Element::new("inlineImage")
};

pub static INLINE_IMAGE_WRITER: Writer = Writer {
    reads: &[&WIDTH, &HEIGHT, &OPACITY],
    attrs: &[&SRC, &TINT, &FLIP_X, &FLIP_Y],
    writes: &[owns::<InlineImage>],
    apply: |ctx, s, ec| {
        if ctx.fresh { return; }                   // the spawn built it (svg-shape precedent)
        ec.queue(set_if_neq_or_insert(inline_image(ctx.attrs, Some(s), ctx.assets)));
    },
};

fn spawn_inline_image(ctx: &mut SpawnCtx) -> Entity {
    let image = inline_image(&ctx.props.attrs, ctx.style.as_ref(), ctx.assets);
    ctx.spawn(image)                               // + required InlineBox (OutOfFlow, zero)
}

fn inline_image(attrs: &Attrs, style: Option<&Style>, assets: &AssetServer) -> InlineImage {
    let image = match attrs.get(&SRC) {
        None => Handle::default(),                 // solid `tint` fill, like <image> (B14)
        Some(path) if crate::svg::is_svg_src(path) => {
            crate::diag::report("inlineImage", path, "…svg sources are not supported inside <text>…");
            TRANSPARENT_IMAGE_HANDLE               // box stays reserved, nothing drawn (B11, B14)
        }
        Some(path) => assets.load(path.clone()),
    };
    InlineImage {
        color: crate::ui_map::image_tint(
            Color::WHITE,
            attrs.get(&TINT).map(String::as_str),
            style.and_then(|s| s.get(&OPACITY).static_val()),
            false,                                 // never a layer root
        ),
        image,
        width: px(style, &WIDTH),
        height: px(style, &HEIGHT),
        flip_x: attrs.get(&FLIP_X).copied().unwrap_or(false),
        flip_y: attrs.get(&FLIP_Y).copied().unwrap_or(false),
    }
}

/// `InlineImage` sizes are logical px.
fn px(style: Option<&Style>, prop: &'static StyleProperty<Animatable<Length>>) -> Option<f32> {
    match style?.get(prop)? {
        Animatable::Static(Length::Px(v)) => Some(*v),
        Animatable::Static(Length::Auto) => None,
        _ => { crate::diag::report("inlineImage", prop.name, "…takes px or \"auto\"…"); None }
    }
}
```

Reused helpers: `crate::ui_map::{image_tint, set_if_neq_or_insert}`,
`crate::svg::is_svg_src`, `elements::image::{SRC, TINT, FLIP_X, FLIP_Y}`. The
wildcard arm in `px` keeps it compiling whichever order this and the em/rem
plan land in.

Register: `pub mod inline_image;` and `&inline_image::INLINE_IMAGE` in
`CORE_ELEMENTS` (`elements/mod.rs`). Add `InlineImage`/`InlineBox` to the
bridge-owned list in the `ReactNode` rustdoc (`bridge.rs`).

Data flow, mount: `create inlineImage` → spawn `(ReactNode, ElementFlags,
InlineImage, InlineBox)` → `append` under the text or span entity →
`Changed<Children>` (B8) → Bevy sizes the box (B4), re-measures, lays out,
extracts under the root (B11). Update: the delta re-runs the writer only if
`src`/`tint`/`flipX`/`flipY`/`width`/`height`/`opacity` changed; an unchanged
rebuild ticks nothing (`set_if_neq`). A tint change re-extracts the block
without re-shaping; a size change re-shapes it.

### 2.4 Reserving the box before the texture loads

Same file:

```rust
/// Bevy keeps an inline image out of flow until its texture is loaded, even
/// when both dimensions are given (the line would reflow on load). Reserve
/// the box, as the web does for `<img width height>`.
pub(crate) fn reserve_inline_image_boxes(
    images: Res<Assets<Image>>,
    mut boxes: Query<
        (&InlineImage, &mut InlineBox),
        (With<ReactNode>, Or<(Changed<InlineImage>, AssetChanged<InlineImage>)>),
    >,
) {
    for (image, mut inline_box) in &mut boxes {
        if let (Some(w), Some(h)) = (image.width, image.height)
            && w > 0.0 && h > 0.0
            && !images.contains(&image.image)
        {
            inline_box.set_if_neq(InlineBox { kind: InlineBoxKind::InFlow, size: Vec2::new(w, h) });
        }
    }
}
```

`plugin.rs`, `PostUpdate`:
`.after(bevy::ui::widget::update_inline_image_boxes).before(bevy::text::detect_text_needs_rerender)`.
Bevy exposes no set for that gap (B5), so these two `pub` function names are
the only handle; our own systems still order by sets.

Same filter as Bevy's system, so both run on the same frames and it settles:
when the texture arrives Bevy writes the identical `InFlow (w, h)`, `set_if_neq`
ticks nothing, and the block does not re-shape. The item was extracted with the
unloaded image id from the start and draws when the `GpuImage` exists (B13). A
failed load leaves the reserved blank box, as a browser does.

### 2.5 Layers

- Captured by the block's layer with no new code (O8, O9), so a `filter`,
  `opacity` group, `transform3d` or `morphFilter` on the `<text>` or any
  ancestor includes it.
- Dirt from our writes: free (O7).
- Dirt from reflow (first load without a reserved box, wrap changes): free
  (`Changed<TextLayoutInfo>`, O10).
- Dirt from the texture itself: add a second query to
  `watch_layer_image_assets`:

  ```rust
  inline: Query<Entity, AssetChanged<bevy::ui::widget::InlineImage>>,
  …
  dirt.nodes.extend(&inline);
  ```

  Covers a reserved box whose texture arrives, and a hot reload at the same
  size. The entity is in `node_to_layer` (O7).

### 2.6 Diagnostics

One new kind, `inlineImage` (values: the svg path; `width`/`height`):

- `js/src/devtools/warnings.ts` `KIND_FIELDS`:
  `inlineImage: { props: ["src"], style: ["width", "height"] }`.
- `crates/core/src/devtools/js_tables.rs`: add `"inlineImage"` to the list.

Everything else reuses existing kinds: `unknownProp` (`imageMode`,
`sourceRect`, `atlas`, `visualBox`), `propIgnored` (handlers, state styles,
scroll props), `styleIgnored` (every style property but `width`, `height`,
`opacity`), `color` (a bad `tint`).

### 2.7 TypeScript

Nothing changes. JSX cannot type by parent, so an `<image>` inside `<text>`
takes `BevyImageProps`; `inlineImage` is not an intrinsic and renders no
interface (`is_intrinsic`), exactly like `textSpan` under `BevyTextProps`.
`js/src/generated/elements.ts`, `js/src/jsx.d.ts` and `docs/reference/bevy.ts`
must come out byte-identical (`core_element_ts_is_current` and
`library_ts.rs` prove it). The subset that applies is documented and
warned at runtime.

### 2.8 Deleted

The guide's "There are no inline elements" bullet. No code.

## 3. Edge cases and tests

Headless, in `elements/inline_image.rs` unless noted. Harness:
`crate::reconcile::test_util` (`op_app`, `op_app_with`, `create`, `update`,
`ent`, `children_of`); `Props::decode_for("inlineImage", json)`.

1. **Spawn, order, Bevy's walk.** Ops: `create text 1`; `CreateTextSpan 2 "a "`;
   `create(3, "inlineImage", { src: "x.png", tint: "#f00", flipX: true, style:
{ width: 16, height: 12 } })`; `CreateTextSpan 4 " b"`; append 2, 3, 4 → 1;
   1 → root. Assert `children_of(text) == [e2, e3, e4]`; `e3` has `InlineImage {
width: Some(16.), height: Some(12.), flip_x: true, color: red }`, an
   `InlineBox`, `ElementFlags.node_less`, no `Node`. Then
   `world.run_system_once(|mut r: bevy::ui::widget::TextUiReader| …)` and assert
   the walk is `[(root, 0), (e2, 1), (e3, 1, Box), (e4, 1)]` — Bevy's own text
   access, no fonts needed (B6).
2. **Inside a span.** Same with the image appended to a `textSpan`; walk depth
   is 2.
3. **Updates.** `tint` delta → color changes; `unset: ["src"]` →
   `Handle::default()`; `style_unset: ["width"]` → `None`; `opacity: 0.5` → alpha
   halves; an identical re-send leaves `Changed<InlineImage>` unticked (compare
   `get_ref().last_changed()`).
4. **Insert before / reorder / remove.** `Insert { child: image, before: span }`
   puts it first in `Children`; removing it drops it from `Children`.
5. **Warnings** (`#[cfg(all(feature = "devtools", debug_assertions))]`, the
   `span_layer_styles_and_handlers_warn` pattern: `diag::test_lock`, filter by
   node id — the runtime sink is global). Decode: `onClick` → `propIgnored`,
   `hoverStyle` → `propIgnored`, `imageMode` → `unknownProp`. Apply: `src:
"a.svg"` → `inlineImage` and the handle is the transparent one; `style: {
width: "50%" }` → `inlineImage`/`width` and `width: None`; `style: { margin: 4
}` → `styleIgnored`/`margin`; `style: { width: 8 }` alone → no warning.
6. **Reserve.** `op_app_with(|app| app.add_systems(PostUpdate,
(bevy::ui::widget::update_inline_image_boxes,
reserve_inline_image_boxes).chain()))`. Mount two images, then point both at
   `images.reserve_handle()` (unloaded). A (`16×12`) → `InlineBox { InFlow, (16,

12) }`; B (width only) → `OutOfFlow, ZERO`. `images.insert(id, 4×2 image)`,
two updates (`AssetChanged`lags a frame): B →`InFlow (16, 8)`; A's
`InlineBox` change tick did not move.

7. **Layer dirt on asset change** (`layer.rs`, copy
   `image_asset_change_repaints_the_nodes_layer`): an entity with `InlineImage`
   mapped to a root → dirty when the asset arrives, clean next frame, dirty
   again on `Modified`.
8. **Layer dirt on a prop change.** `op_app()` plus
   `app.init_resource::<LayerContentDirt>()` (the svg crate's `raster_tests.rs`
   pattern; `layer.rs`'s own `layer_app()` has no dirt resource). Drain the
   resource after the mount frame, send a `tint` update, and assert
   `LayerContentDirt.nodes` holds the image entity. That it maps to the block's
   layer is O7 plus the existing `repaint_resolution_and_propagation` test.
9. **Element registry.** `core_registry().element_info("inlineImage")` exists;
   its `ignored_styles` contains `margin` and not `width`; `textSpan`'s and an
   svg shape's are still empty (the gate did not widen).
10. **TS unchanged.** `element::ts::tests::core_element_ts_is_current` and the
    facade's `library_ts` pass without regenerating.
11. **The kind swap, end to end** (`examples/demos/tests/roundtrip.rs`, needs
    the bundle): `h.click_label("<text>")`, then `h.wait_op(…, |op|
matches!(op, Op::Create { kind, .. } if kind == "inlineImage"))`. This is
    the only test that runs `renderer.ts`.

Not planned: a parley layout test (rect in `TextLayoutInfo.inline_boxes`,
wrap, baseline). It would need a new harness — `TextPlugin`, fonts,
`measure_text_system`, `text_system` on top of `layout_app()` (O13) — to test
Bevy's code, not ours. Check those with `--shoot` (§7).

Edge cases with no test, by design:

- A block whose only child is an image: root `Text("")` plus one box; parley
  lays it out (`line_break.rs:467`).
- Children of an inline image are attached and ignored by Bevy (B1).
- An `<image>` under a `<node>` that is itself inside a `<text>` still gets the
  kind (O1) and draws nothing — a `<node>` inside `<text>` was never supported.

## 4. Commit sequence

Rust first, so the kind exists before the reconciler emits it.

1. `feat(core) inlineImage: an image as an inline box of a text block` —
   `TextRole::InlineBox` and its four sites, `elements/inline_image.rs`
   (element, writer, builder), `CORE_ELEMENTS`, the `inlineImage` diag kind in
   both tables, the `ReactNode` rustdoc, tests 1–5, 8, 9, 10.
2. `feat(core) an inline image with both sizes reserves its box before load` —
   `reserve_inline_image_boxes` + its `plugin.rs` registration, test 6.
3. `feat(layer) inline image textures repaint the block's cached layer` —
   `watch_layer_image_assets`, test 7. (Test 8 belongs to commit 1: it pins the
   writer's dirt push.)
4. `feat(js) an <image> inside <text> is an inline image` — `renderer.ts`.
5. `feat(demos) inline images on the <text> page` — the demo, its assets,
   test 11.
6. `docs inline images in text` — guide pages, CHANGELOG, `CONTEXT.md`,
   `CLAUDE.md`, `TODO` follow-ups.

Each builds and passes alone (4 changes behavior only for JSX that nests an
image in a text; no example does — grepped).

## 5. Demos + docs

### Demos — `examples/demos/ui/src/demos/elements/TextDemo.tsx` (nav label `<text>`)

Add an `Example` titled **Inline images** in a third `DemoRow` (the file stays
under 500 lines), and one sentence about it in `PAGE.info`.

- Card: a `Stage` about 280 px wide holding one wrapping sentence with icons in
  it — "Built with [bevy] Bevy and [react] React. Harvest: +3 [wheat] per turn,
  says [parrot]." — plus a `src`-less tinted square as a bullet.
- Controls: a `Slider` for the icon height (10–40 px; past the text's ascent
  the line visibly grows, B9) and a `Radio` for `tint` (none / amber / cyan).
- Snippet (lines ≤ 40 chars):

  ```tsx
  <text style={{ fontSize: 18 }}>
    Costs 3 <image src="images/inline/wheat.png" style={{ height: 18 }} /> per
    turn
  </text>
  ```

- Assets: the existing logos are 243 px and `imageRendering` does not reach
  inline images, so drawing them at 18 px aliases. Add pre-scaled copies:
  `magick images/<name>.png -resize x48 images/inline/<name>.png` for `bevy-logo`,
  `react-logo`, `wheat`, `parrot` (under `examples/demos/assets/`).
- No `bevy.ts` regeneration (no binding changed). No docs screenshot uses this
  page.

### Guide

- `docs/guide/elements/text.md`
  - Frontmatter `description`: mention inline images.
  - "Content": replace the "There are no inline elements…" bullet with: an
    `<image>` inside a `<text>` is an inline image; boxes still go beside the
    text in a `<node>`.
  - New `## Inline images` section after "Spans": the usage snippet; what
    applies (`src`, `tint`, `flipX`, `flipY`, `name`; `style.width`,
    `style.height` in px, `style.opacity`); sizing (one texel per logical px by
    default, one axis keeps the aspect, both stretch); placement (bottom edge
    on the baseline, a taller image grows its line, it wraps as a unit, a line
    may break before or after it); loading (both sizes given → space reserved;
    otherwise zero until loaded, then the text reflows); no `src` → a solid
    `tint` box; spacing with `{" "}` (there is no margin); part of the block for
    clicks, `opacity`, `filter` and the other layer styles; `textShadow` does
    not shadow it.
  - "Limits": no `.svg` source, no `imageMode`/`sourceRect`/`atlas`/
    `visualBox`, no handlers or state styles, no `{ animated }` bindings, no
    `imageRendering`, no vertical alignment, px sizes only, children ignored —
    each with its warning kind.
- `docs/guide/elements/image.md`: a short `## Inside <text>` section pointing
  to the above and listing the attributes that carry over.
- `docs/guide/styling/image-rendering.md`: one limit line — not applied to
  inline images; use a source close to the drawn size.
- `covers`: no change (no new element or style key in the reference).

### Other

- `CHANGELOG.md`, `[Unreleased]` → Added:
  "**Inline images in text.** An `<image>` written inside a `<text>` or a span
  flows with the glyphs as an inline box (Bevy 0.20's `InlineImage`). It takes
  `src`, `tint`, `flipX`, `flipY` and a px `width`/`height`; it sits on the
  baseline, wraps as a unit, and belongs to the block for clicks and layers. A
  box with both sizes is reserved before the texture loads."
- `CONTEXT.md`, "Elements": **Inline image** — an `<image>` inside a `<text>`;
  the wire kind `inlineImage`; a `Node`-less inline box of the block.
- `CLAUDE.md`: add `inlineImage` to the core element list in "What this is".
- `TODO`: the follow-ups in §6 (svg sources, `em` sizes, `imageRendering`,
  vertical alignment, generic inline boxes).
- Generated files: none change. Run `npm run generate` and expect no diff.

## 6. Decisions

| #   | Question                                                                         | Recommended                                                                                                                                                                                                          | Why                                                                                                                         |
| --- | -------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| D1  | New kind or a flag on `image`?                                                   | New wire kind `inlineImage`.                                                                                                                                                                                         | Flags, writers and the `Node` are per kind.                                                                                 |
| D2  | Where does the swap apply?                                                       | Any `<image>` whose host context is `inText` — directly in the block or in a span.                                                                                                                                   | Matches B6; same rule as `textSpan`.                                                                                        |
| D3  | Size props                                                                       | `style.width` / `style.height`, as on a block `<image>`.                                                                                                                                                             | CSS sizes an inline `<img>` the same way; no new prop on `BevyImageProps`.                                                  |
| D4  | Units                                                                            | Static px (a number or `"…px"`) or `"auto"`. Anything else — `%`, viewport units, `{ animated }` — warns `inlineImage` and counts as unset.                                                                          | `InlineImage` is px-only (B3).                                                                                              |
| D5  | `em`                                                                             | Not in v1. Once `Length::Em` exists: resolve against the parent run's cached font size at append and on the parent's font change (the bare-run cascade), only when that size is `Px`.                                | Needs the em/rem plan first.                                                                                                |
| D6  | Natural size                                                                     | The texture's pixel size as logical px; one axis keeps the aspect; both stretch.                                                                                                                                     | Bevy's rule (B3) and what `<image>` auto mode documents.                                                                    |
| D7  | Reserve the box before load?                                                     | Yes, when both sizes are given.                                                                                                                                                                                      | Web behavior; the `<image>` guide already promises "give it an explicit size if that jump matters".                         |
| D8  | No `src`                                                                         | A solid `tint` box (white by default).                                                                                                                                                                               | Same as `<image>`; free (B14).                                                                                              |
| D9  | `.svg` source                                                                    | Unsupported in v1: warn, draw nothing, keep an explicitly sized box.                                                                                                                                                 | The svg raster is built on `ComputedNode`/`ImageNode` (O12). A second raster path keyed on the box size is its own feature. |
| D10 | `imageMode`, `sourceRect`, `atlas`, `visualBox`                                  | Not attributes of `inlineImage` → `unknownProp`.                                                                                                                                                                     | Bevy has no equivalent (B2).                                                                                                |
| D11 | Events, state styles                                                             | `Common::IDENTITY` only: handlers, `hoverStyle`/`pressStyle`/`focusStyle`, scroll and wheel props warn `propIgnored`. A click goes to the block.                                                                     | Not pickable (B15); same as a span.                                                                                         |
| D12 | `name`, `sharedTag`                                                              | Accepted, like a span. `name` stamps a Bevy `Name`; `sharedTag` is inert (no box to fly).                                                                                                                            | Parity with `textSpan`; no extra code.                                                                                      |
| D13 | Other style properties (`margin`, `borderRadius`, `filter`, `imageRendering`, …) | Ignored with `styleIgnored`.                                                                                                                                                                                         | They cannot apply; the warning is three lines (§2.2).                                                                       |
| D14 | `opacity`                                                                        | The image's own `opacity` folds into its tint. The block's fades it through the block's layer (a block with a child and `opacity` is always promoted unless `groupAlpha: false` — then it does not reach the image). | Cheapest correct; the leftover case is documented.                                                                          |
| D15 | Vertical alignment                                                               | Baseline, no option.                                                                                                                                                                                                 | Parley has none (B9); also CSS's default.                                                                                   |
| D16 | Children of an inline image                                                      | Ignored, no warning.                                                                                                                                                                                                 | A warning costs a lookup on every append, the hot path, for a misuse.                                                       |
| D17 | TS typing                                                                        | Unchanged (`BevyImageProps`).                                                                                                                                                                                        | JSX cannot type by parent; `textSpan` precedent.                                                                            |
| D18 | Crate                                                                            | Core, no cargo feature.                                                                                                                                                                                              | `InlineImage` is `bevy_ui`, which core already needs.                                                                       |
| D19 | Layer dirt                                                                       | No `Changed<InlineImage>` tap; add `AssetChanged<InlineImage>`.                                                                                                                                                      | See "audit" above.                                                                                                          |
| D20 | Flag or kind check for "not an intrinsic"                                        | `TextRole::InlineBox`.                                                                                                                                                                                               | The repo's "flags, not kind checks" rule. Alternative: an `Element::jsx: bool` field.                                       |

**Out of scope: a generic `InlineBox` for `<node>` children of text.** The node
must be laid out by taffy and placed by the text layout, and a text node is a
taffy leaf with a measure function — it cannot hold `Node` children. So it
takes: a placeholder `InlineBox` entity in the text tree; the real node
detached elsewhere (a new detached-style element flag, with its own layer
membership, clipping and picking); its measured size fed into `InlineBox.size`
before shaping; and its position read back from `TextLayoutInfo.inline_boxes`
after `text_system` (`PostLayout`) — either one frame late, or composed into
`UiGlobalTransform` after layout like the FLIP driver. Content whose size
depends on the line (wrapped text in the node) loops. A feature of its own.

## 7. Risks / perf

- **Per frame:** two change-filtered queries over inline images
  (`reserve_inline_image_boxes`, the second `watch_layer_image_assets` query).
  `AssetChanged` checks each inline image entity each frame, as Bevy's own
  system does. Nothing on the op hot path: no new lookup in `Append`/`Insert`.
- **Per change:** a tint/flip/opacity change re-extracts the whole block (B12)
  but does not re-shape it. A size change, mount or unmount re-shapes and
  re-lays out the block, like a font-size change. An unreserved image re-shapes
  once on load.
- **Layers:** any inline image change re-captures the block's layer. A cached
  layer over a block with a reserved image depends on the `GpuImage` being
  ready on the frame the asset tap fires — the same assumption
  `watch_layer_image_assets` already makes for `<image>`.
- **Benchmarks:** none should move. If `table-ops` regresses, look at
  `ElementInfo` construction (`registry.rs`, once per kind) and the
  `global_mask` rewrite — a wrong mask there would run text writers on every
  node.
- **Not verified by reading; check with `--shoot`** (`npm run build -w demos`,
  then `cargo run -p demos -- --shoot "<text>" out.png`):
  - icons sit on the baseline and wrap with the sentence; also at
    `--size 390x844` (compact shell, narrower wrap) and `--scale 1.5`;
  - an inline image under a `filter` and under a group `opacity` (temporarily
    add them to the demo's `<text>`) is captured, tinted once, and clipped by a
    scroll container like the glyphs around it;
  - `textAlign: "justify"` and `"center"` place the box correctly (B10 is from
    parley's code, not from a run).
- **Soft pixels:** the box lands at a fractional x (after the preceding
  advances) and, at fractional DPI, a fractional height. Bevy draws at the
  layout rect as is, so pixel art softens. Not fixable from our side.
- **Aliasing:** a large texture drawn small has one mip level and no
  `imageRendering` (D13). The demo ships pre-scaled icons for that reason.
- **Behavior change:** JSX that already nested an `<image>` in a `<text>` (an
  unsupported `Node` child until now) becomes an inline image. No example does;
  a wrapper component rendering `<image>` under a `<text>` would be caught the
  same way.
- **Bevy coupling:** the reserve system orders against two Bevy function names
  and duplicates Bevy's filter. If a 0.20.x patch makes Bevy reserve the box
  itself, ours becomes a no-op and can go.
- **Concurrent work:** the clip-swap files are being rewritten by another
  change. This plan touches `layer.rs` only in `watch_layer_image_assets` and
  its test, and `plugin.rs` only with one added `PostUpdate` system.
