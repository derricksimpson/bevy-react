# Plan — elliptical `borderRadius` on Bevy 0.20's `CornerRadius { x, y }`

Audit section: `.scratch/bevy-0.20/audit.md` §2.4. Written against `main` at
`b7e408c` (another agent has uncommitted edits in `layer*.rs` / `plugin.rs`,
none in the files this plan touches). Our files are cited by symbol; line
numbers are approximate.

Bevy paths are relative to
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`.

Nothing here was compiled or run (planner brief). Every Bevy claim is from
reading the 0.20.0 source; the ones a test must pin are marked **(pin)**.
What I could not verify by reading is listed in §7.4.

---

## 1. Verified facts

### 1.1 Bevy 0.20 — the types

| #   | Fact                                                                                                                                                                                                                                                                                                                                                                                                                                 | Source                                                                              |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------- |
| B1  | `BorderRadius { top_left, top_right, bottom_right, bottom_left: CornerRadius }`. `BorderRadius::all(impl Into<CornerRadius>)`; `impl<T: Into<CornerRadius>> From<T> for BorderRadius`.                                                                                                                                                                                                                                               | `bevy_ui-0.20.0/src/ui_node.rs:2959, 3005, 3252`                                    |
| B2  | `CornerRadius { x: Val, y: Val }` lives in **`geometry.rs`**, not `ui_node.rs`. Constructors: `circular(v)` = `{ x: v, y: Auto }`; `all(v)` = `{ x: v, y: v }`; `new(x, y)`; consts `ZERO` = `{ Px(0), Px(0) }`, `MAX` = `{ Val::MAX, Auto }`, `MAX_ELLIPTICAL`. `Default` = `{ Auto, Auto }`. `From<Val>` = **circular** (`{ x, y: auto() }`), `From<(Val, Val)>` / `From<[Val; 2]>` = elliptical.                                  | `bevy_ui-0.20.0/src/geometry.rs:1282, 1291-1306, 1339, 1379, 1388, 1436, 1445-1461` |
| B3  | **`Val::Auto` is the "circular" flag.** `resolve`: both `Auto` → zero. Exactly one `Auto` → the other value is a circular radius: resolved against `size.min_element()` and clamped to `0.5 * size.min_element()` (this is 0.19's whole rule, bit for bit — `bevy_ui-0.19.1/src/ui_node.rs:2742-2764`). Neither `Auto` → `x` resolved against `size.x`, `y` against `size.y`, then `.clamp(Vec2::ZERO, 0.5 * size)`.                 | `geometry.rs:1393-1433`                                                             |
| B4  | So `%` is **per axis** in an elliptical corner (`Val::resolve`: `Percent(v)` = `base * v / 100`) and **of the shorter side** in a circular one. `CornerRadius::all(Px(30))` on 100×50 resolves to `(30, 25)`; `circular(Px(30))` to `(25, 25)`. Bevy's own test: `corner_radius_resolve`, and `test_border_radius_updates` (`circular(percent(100))` → `(25, 25)`, `new(percent(100), percent(100))` → `(25, 50)` on a 50×100 node). | `geometry.rs:488-500, 1540-1583`; `layout/mod.rs:2225-2262`                         |
| B5  | **Clamping is per corner, per axis, to half the box — not CSS.** CSS scales _all_ radii by one factor so adjacent corners fit, preserving each corner's aspect ratio. Bevy: `"150px / 10px"` on a 200-wide node → `(100, 10)` (CSS: `(100, 6.67)`); one corner can never span more than half a side even when its neighbour is 0 (CSS allows the full side). The circular half of this is unchanged from 0.19.                       | `geometry.rs:1423, 1431`; rustdoc `ui_node.rs:2917-2920`                            |
| B6  | `CornerRadius: PartialEq` is hand-written: `{ x: r, y: Auto } == { x: Auto, y: r }`; otherwise field-wise. So `circular(Px(8)) != all(Px(8))` and `{ Px(0), Auto } != ZERO` — `Node` compare-before-write tells circular from elliptical.                                                                                                                                                                                            | `geometry.rs:1463-1474`                                                             |
| B7  | `ResolvedBorderRadius { top_left, …: Vec2 }` (physical px, x = horizontal). `From<ResolvedBorderRadius> for [[f32; 4]; 2]` = `[xs, ys]`, each `[tl, tr, br, bl]`.                                                                                                                                                                                                                                                                    | `ui_node.rs:3266, 3282`                                                             |
| B8  | Layout resolves `Node.border_radius` for every node on every layout pass against `node.size` (the border box) and writes `ComputedNode.border_radius` only when it differs.                                                                                                                                                                                                                                                          | `layout/mod.rs:414-425`                                                             |
| B9  | A `Node` change is an unconditional `taffy.set_style` → relayout. **A radius change still re-runs layout**, as in 0.19.                                                                                                                                                                                                                                                                                                              | `layout/ui_surface.rs:134-136`                                                      |

### 1.2 Bevy 0.20 — who honors an elliptical corner

Every consumer reads the resolved `Vec2` pair; a corner with `min(rx, ry) <= 0`
is square everywhere (CSS's rule too).

| Consumer                                                                                   | How                                                                                                                                                                                                                                                                     | Source                                                                                              |
| ------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| `sd_rounded_box`                                                                           | picks the corner's `(rx, ry)`, square if either is ≤ 0, else an ellipse distance (`distance_to_ellipse_approx`, one Newton step). Antialias = `saturate(0.5 - d)`.                                                                                                      | `bevy_ui_render-0.20.0/src/ui.wesl:76-88, 102-121, 186, 287`                                        |
| Background color, `OuterColor`                                                             | `uinode.border_radius()`                                                                                                                                                                                                                                                | `lib.rs:788, 815` (`extract_uinode_background_colors`)                                              |
| Border                                                                                     | outer SDF minus `sd_inset_rounded_box`: inner radii = outer − border width **per axis**, zeroed together when either hits 0 (CSS's inner-edge rule).                                                                                                                    | `lib.rs:1148`; `ui.wesl:123-160, 191`                                                               |
| Outline                                                                                    | `outline_radius()`: both axes + `(width + offset)` when both > 0, else zero.                                                                                                                                                                                            | `ui_node.rs:137-152`; `lib.rs:1191`                                                                 |
| Gradients (background + border)                                                            | same `draw_uinode_border/background` with `radius_x/radius_y`.                                                                                                                                                                                                          | `gradient.rs:498-661`; `gradient.wesl:131-133`                                                      |
| Box shadow                                                                                 | radii × the spread ratio; the shader integrates an elliptical corner.                                                                                                                                                                                                   | `box_shadow.rs:346-351`; `box_shadow.wesl:36-45`                                                    |
| `ImageNode` (our `<image>`, stretch `backgroundImage`, canvas/svg/portal/surface textures) | radius minus the visual-box inset, clamped per axis.                                                                                                                                                                                                                    | `lib.rs:983-991` (`extract_uinode_images`)                                                          |
| Viewport nodes, `UiMaterial`s                                                              | `border_radius()`; `UiVertexOutput.border_radius_x/_y`.                                                                                                                                                                                                                 | `lib.rs:1461`; `ui_material_pipeline.rs:464`; `ui_vertex_output.wesl:7-9`                           |
| 9-slice / tiled images                                                                     | **no radius** (as in 0.19).                                                                                                                                                                                                                                             | `ui_texture_slice_pipeline.rs` (no `radius` at all)                                                 |
| Picking + hover                                                                            | `ComputedNode::contains_point`: inside the straight edges, then `(q / r).length_squared() < 1.` — an ellipse test. Used by `ui_picking` and `ui_focus_system`. **(pin, T7)**                                                                                            | `ui_node.rs:211-231`; `picking_backend.rs:205`; `focus.rs:276`                                      |
| **Clipping**                                                                               | **Rectangular only.** `update_clipping` pushes `resolve_clip_rect` (a `Rect`); `CalculatedClip::Rects` holds `{ rect, world_to_clip_local }`; the renderer clips polygons against those rects. `overflow: clip` does not clip to rounded or elliptical corners in 0.20. | `layout/clipping.rs:99-101`; `ui_node.rs:2757, 2776`; `bevy_ui_render-0.20.0/src/clipping.rs:16-40` |

### 1.3 Our side, as it is now

- **Property**: `BORDER_RADIUS: StyleProperty<Animatable<Rect>>`,
  `Invalidate::Fixed(LAYOUT_PAINT)`, `Codec::serde_as("Animatable<Rect>")`
  (`crates/core/src/style/props/visual.rs`). Generated TS:
  `borderRadius?: Animatable<Rect>` (`js/src/generated/style.ts:56`), with
  `Rect = number | string | { top?, right?, bottom?, left? } | { horizontal?, vertical? }`
  (`js/src/jsx.d.ts:90`).
- **Decode**: `Rect`'s visitor (`crates/core/src/protocol/units.rs`): number →
  uniform px; string → 1–4 whitespace-separated lengths in CSS order; object →
  sides / axis pair. A bad token warns `rect` and is 0; a bad count warns and
  zeroes the rect; nothing aborts. `Animatable<T>` buffers the wire value into
  a `serde_json::Value` first (`protocol/animatable.rs`).
- **To Bevy**: `rect_to_border_radius(Rect) -> BorderRadius`
  (`crates/core/src/ui_map.rs`): `top/right/bottom/left` → TL/TR/BR/BL, each
  `length_to_val(..).into()` = `CornerRadius::circular`. Callers: `node_from`
  (the `LAYOUT_WRITER`), the transition drive block, `scrollbar.rs`
  (`ScrollbarPartStyle.border_radius: Option<Rect>`).
- **Transition**: row `(border_radius, (Rect::default()), radius)` in
  `with_input_channels!`; `TransitionState.border_radius: ProgressChannel<Rect>`
  (`transition/channels.rs`); `impl Lerp for Rect` = per-leaf `Length::lerp`
  (same unit eases, anything else returns the target). The drive block
  (`transition/mod.rs`, "Corner radii") writes `Node.border_radius` and pushes
  `dirt.nodes`. `TransitionInput.border_radius: Option<Rect>`
  (`transition/spec.rs`, a `pub` field of a re-exported type). Mount seed,
  `sync_radius_rows`, `copy_value_channels`, `at_identity`, `running_mask` are
  all generated from the row and only need `T: Copy + Default + PartialEq`.
- **Binding**: `AnimatableProperty::BorderRadius` → write rule
  `(node_radius_all)` → `BorderRadius::all(Val::Px(v))`
  (`animations/apply/node_colors.rs`) — circular in 0.20 by B2. Parks
  `ChannelId::BorderRadius`; excluded from `has_node_props`.
- **Already per axis** (nothing to do): `compensate_seed_frame_radius`
  (`transition/layout.rs`, commit `59544f9`) divides the _resolved_ radii by
  the FLIP scale per axis and clamps to half the box per axis;
  `ExtractedLayer.corner_radius: [[f32; 4]; 2]` → `CompositeUniforms.radius_x /
radius_y` → `bevy_ui_render::ui::sd_rounded_box` in `layer/composite.wesl`
  (commit `7ce05a7`).
- **Not radius-aware by construction**: layer / `transform3d` composite quads
  are rectangles over a capture that already contains the rounded paint.
- **Our hit tests** (`scroll.rs`, `touch_scroll.rs`, `reconcile/pointer.rs`)
  all call `ComputedNode::contains_point`.
- **Devtools**: `devtools/style_fields.rs::category` maps the TS string
  `"Rect"` (after stripping `Animatable<…>`) to `"rect"`; `fields.ts`
  `checkCategory("rect")` accepts number | string | object; the editor keeps
  non-JSON input as a string (`edits.ts`). `warnings.ts` `KIND_FIELDS.rect`
  already lists `borderRadius`.
- **Style store**: values ≤ 32 bytes are inline (`SmallBox<_, S4>`,
  `style/store.rs`); larger ones heap-allocate.

### 1.4 Audit check

- Confirmed: `%` per axis for `CornerRadius::all`; seed compensation and the
  backdrop SDF are per axis already.
- Wrong reference: `CornerRadius` is at `geometry.rs:1282` (`new` at `:1388`);
  `ui_node.rs:2959` is `BorderRadius`.
- The proposed TS type ``Animatable<Rect> | `${string}/${string}` | { x: Rect; y: Rect }``
  is redundant: `Rect` already contains `string`. I recommend no type change
  and no object form (§6 D1, D3).
- Missed: `Val::Auto` on one axis silently turns an elliptical corner circular
  (B3); the clamp is not CSS's (B5); circular ↔ elliptical transitions (D7);
  the style-store size step (§7.1).

---

## 2. Design

One wire addition: **the CSS slash inside the existing string form.**

```tsx
borderRadius: "50% / 50%"; // an ellipse
borderRadius: "40px 10px / 10px 40px"; // per corner: horizontal / vertical
borderRadius: 12 | "8px 16px" | { top: 4 }; // unchanged: circular corners
```

### 2.1 The value type — `crates/core/src/protocol/visual.rs`

Next to `BorderColorSpec` (the file is "visual spec wire types", 379 lines;
`units.rs` is already 550).

```rust
/// `borderRadius`: the corners' radii (`Rect` sides are the corners, CSS
/// order) and, when the wire string had a `/`, their vertical radii.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CornerRadii {
    /// The radii of circular corners, or the horizontal radii when `y` is set.
    pub x: Rect,
    /// The vertical radii; `None` = no `/` on the wire = circular corners.
    pub y: Option<Rect>,
}

impl From<Rect> for CornerRadii {
    fn from(x: Rect) -> Self {
        Self { x, y: None }
    }
}

impl<'de> Deserialize<'de> for CornerRadii {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::value::StrDeserializer;
        // Buffered, like `Animatable` — which is where this is always decoded
        // from, so the value already is a `serde_json::Value`.
        let v = serde_json::Value::deserialize(d)?;
        let side = |s: &str| Rect::deserialize(StrDeserializer::<D::Error>::new(s));
        Ok(match v.as_str().and_then(|s| s.split_once('/')) {
            Some((x, y)) => CornerRadii { x: side(x)?, y: Some(side(y)?) },
            None => Rect::deserialize(&v).map_err(de::Error::custom)?.into(),
        })
    }
}
```

`Rect`'s own decoder is reused as a black box for each side, so every existing
rule carries over: 1–4 values per side expanded in CSS order, a bad token warns
`rect` and is 0, a bad count (including an empty side, `"10px / "`) warns and
zeroes that side, nothing aborts. A second `/` lands in the `y` side as a token
`"/"` and warns there. No new diag kind, no `Rect` change, no ts-rs impl.

### 2.2 Property — `crates/core/src/style/props/visual.rs`

```rust
pub static BORDER_RADIUS: StyleProperty<Animatable<CornerRadii>> = StyleProperty {
    invalidate: Invalidate::Fixed(LAYOUT_PAINT),
    ..StyleProperty::with_codec("borderRadius", Codec::serde_as("Animatable<Rect>"))
};
```

The TS literal stays `"Animatable<Rect>"` on purpose (D3): the generated
`style.ts` does not change and devtools keeps the `rect` category. Update the
static's rustdoc (it is the property's documentation): the slash form, `%`
per axis with a slash and of the shorter side without, the binding stays
uniform circular px.

### 2.3 To Bevy — `crates/core/src/ui_map.rs`

Keep `rect_to_border_radius` (the scrollbar still uses it, D9) and add:

```rust
/// Wire corner radii → `BorderRadius`. No vertical radii: circular corners,
/// exactly `rect_to_border_radius`. With them: `CornerRadius::new(x, y)`,
/// each axis against its own side. `auto` reads as 0 there — a `Val::Auto`
/// on one axis makes bevy read the corner as circular with the other radius.
pub fn radii_to_border_radius(radii: CornerRadii) -> BorderRadius {
    let Some(y) = radii.y else {
        return rect_to_border_radius(radii.x);
    };
    let axis = |l: Length| match length_to_val(l) {
        Val::Auto => Val::ZERO,
        v => v,
    };
    let corner = |x: Length, y: Length| CornerRadius::new(axis(x), axis(y));
    let x = radii.x;
    BorderRadius {
        top_left: corner(x.top, y.top),
        top_right: corner(x.right, y.right),
        bottom_right: corner(x.bottom, y.bottom),
        bottom_left: corner(x.left, y.left),
    }
}
```

`node_from`: `node.border_radius = radii_to_border_radius(r)` (the read stays
`s.get(&BORDER_RADIUS).static_val()`).

### 2.4 Transition — `crates/core/src/transition/`

- `channels.rs`: the row's default becomes
  `(crate::protocol::visual::CornerRadii::default())`, the field
  `ProgressChannel<CornerRadii>`, and next to `impl Lerp for Rect`:

  ```rust
  /// Eight leaves: a horizontal and a vertical radius per corner, each a
  /// `Length` lerp (same unit eases, a unit change or `auto` snaps that leaf
  /// alone). Both circular stays circular — the `Rect` lerp, bit for bit. A
  /// circular side facing an elliptical one reads as `r / r`, CSS's meaning
  /// of a single radius, so `8` ↔ `"40px / 20px"` eases.
  impl Lerp for CornerRadii {
      fn lerp(self, other: Self, t: f32) -> Self {
          CornerRadii {
              x: self.x.lerp(other.x, t),
              y: match (self.y, other.y) {
                  (None, None) => None,
                  (a, b) => Some(a.unwrap_or(self.x).lerp(b.unwrap_or(other.x), t)),
              },
          }
      }
  }
  ```

  `LerpInterp::settle` returns the target itself, so an ease toward a circular
  value ends on `y: None` exactly.

- `spec.rs`: `TransitionInput.border_radius: Option<CornerRadii>`;
  `from_style` is textually unchanged.
- `mod.rs` drive block: `radii_to_border_radius(state.border_radius.drive(..))`;
  swap the import. Mount seed, `sync_radius_rows`, shared-element
  `copy_value_channels` / `at_identity` / `running_mask` follow the row with no
  edit.
- Update the three doc comments that say "a whole `Rect` eased per corner".

### 2.5 What does not change

- **System order / sets**: none added, none moved. No new system.
- **`{ animated }` binding** (`node_radius_all`): one px value, all four
  corners, circular (D8).
- **Shared elements**: the radius channel's reading is copied as a
  `CornerRadii`; `compensate_seed_frame_radius` works on resolved `Vec2`s and
  is already right for an elliptical seed.
- **Backdrop frost mask, layer capture, `transform3d`, picking, our hit
  tests**: all read `ComputedNode.border_radius`.
- **Scrollbar parts**: stay `Rect` (D9).
- **JS runtime, devtools, generated TS, `docs/reference/bevy.ts`**: no code
  change (JSDoc only, §5.4).
- **Deleted**: nothing.

### 2.6 Data flow

```
style.borderRadius ── Animatable<CornerRadii> in Style
   ├─ LAYOUT_WRITER → node_from → radii_to_border_radius → Node.border_radius
   ├─ TRANSITION_WRITER → TransitionInput.border_radius
   │     → drive_transitions (radius block) → Node.border_radius (+ content dirt)
   └─ { animated } → AnimatableProperty::BorderRadius → BorderRadius::all(Px)
Node.border_radius ── bevy layout ──► ComputedNode.border_radius (Vec2 × 4)
   └─ bevy renderers, picking, our backdrop mask, seed-frame compensation
```

---

## 3. Edge cases and tests

Decode tests in `protocol/visual.rs`'s test module. The transition and
real-layout tests go in a **new** `crates/core/src/transition/radius_tests.rs`
(`#[cfg(test)] mod radius_tests;`): `transition/tests.rs` is 1134 lines, over
the repo's 1000-line cap. Move the two existing radius tests there
(`lerp_rect_eases_per_corner_and_snaps_mixed_units`,
`system_eases_border_radius_and_dirties_content`) and make `drive_world` /
`advance` in `tests.rs` `pub(super)`.

| #   | Behavior                                                                                                                                                                             | Harness / shape                                                                                                                                                                                                                                                                                                                            |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| T1  | Slash decode.                                                                                                                                                                        | `serde_json::from_value::<CornerRadii>`: `"10px 20px / 5px"` → `x` = (10, 20, 10, 20), `y` = `Some(uniform 5)`; `"50%/25%"` (no spaces) decodes the same as with spaces; `8`, `"8px 16px"`, `{ "top": 4 }`, `{ "horizontal": 4 }` → `y: None` and `x` equal to the `Rect` decode of the same JSON.                                         |
| T2  | Through the store and the wrapper.                                                                                                                                                   | `Style` from `{ "borderRadius": "40px / 10px" }` → `get(&BORDER_RADIUS).static_val()`; `{ "animated": { "id": 3 }, "seed": "40px / 10px" }` → binding present, `static_val()` is `None`, seed decodes. Adapt `style/decode_tests.rs::border_radius_reruns_transition_and_decodes_binding` (`uniform8.into()`).                             |
| T3  | Bad values never abort. `#[cfg(all(feature = "devtools", debug_assertions))]`, `decode_batch_start()` + `take_decode_warnings()` (the `ext_tests::unknown_style_key_warns` pattern). | `{ "borderRadius": "10px / ", "width": 40 }` → `y == Some(Rect::default())`, `width` intact, one `rect` warning; `"10px / bogus"` → that side 0 + `rect` warning with value `bogus`; `"1px / 2px / 3px"` warns.                                                                                                                            |
| T4  | Conversion. Extend `ui_map/tests.rs::rect_axis_pair_maps_to_val` (it already pins the circular arm; change its call to `radii_to_border_radius`).                                    | `"10px 20px / 5px"` → `top_left == CornerRadius::new(Px(10), Px(5))`, `top_right == CornerRadius::new(Px(20), Px(5))`; `"auto / 10px"` → `top_left == CornerRadius::new(Val::ZERO, Px(10))` (not circular 10); `12` → `CornerRadius::circular(Px(12))` on all four.                                                                        |
| T5  | Lerp, both elliptical.                                                                                                                                                               | `a.lerp(b, 0.5)`: px leaves halfway on both axes; one `y` leaf changing unit equals the target while its `x` leaf still eases.                                                                                                                                                                                                             |
| T6  | Lerp, circular ↔ elliptical.                                                                                                                                                         | `CornerRadii::from(uniform 8)` → `"40px / 20px"` at 0.5: `x` = 24, `y == Some(14)`; both `None` stays `None`. Then a `ProgressChannel<CornerRadii>` driven from an elliptical current to a circular target for its full duration ends `== target` (`y: None`).                                                                             |
| T7  | **(pin)** Real resolution + hit test, the docs' promises. `layout_tests::layout_app()` (`root_row()`, a 200×80 child, `step`).                                                       | Child radius from `"50%"` → `ComputedNode.border_radius.top_left == Vec2::splat(40.)` (the back-compat rule: shorter side); from `"50% / 50%"` → `(100, 40)`; from `"150px / 10px"` → `(100, 10)` (per-axis clamp); with the ellipse, `contains_point(global, center)` is true and a point 2 px inside the box's top-left corner is false. |
| T8  | System ease lands on `Node`. Extend the moved `system_eases_border_radius_and_dirties_content`.                                                                                      | Keep its circular asserts as they are (`BorderRadius::all(Val::Px(100.0))` at settle — the "existing apps unchanged" pin). Add: retarget to `"100px / 40px"`, halfway `top_left` is `CornerRadius::new(Px(~100), Px(~70))` and content dirt is pushed; settle is exact; a further frame writes nothing.                                    |
| T9  | Shared seed.                                                                                                                                                                         | No new test: `layout_shared_tests.rs::shared_seed_frame_shows_the_seed_corner_radius_through_the_flip_scale` already covers the per-axis compensation; only its two literals change (`px(16.0).into()`, `.init(px(36.0).into())`).                                                                                                         |
| T10 | Binding unchanged.                                                                                                                                                                   | `animations/apply/tests.rs::apply_drives_border_radius_uniformly` and `style_bindings.rs`'s park test pass untouched.                                                                                                                                                                                                                      |
| T11 | Generated TS + devtools unchanged.                                                                                                                                                   | `style::ts::tests::core_style_ts_is_current` and `devtools::style_fields::tests::categories_follow_the_ts_type` pass with **no** regeneration. A diff in `js/src/generated/` after this change means the codec literal was touched by mistake.                                                                                             |

Edge cases settled by the design, no test of their own: `"0 / 0"` is
`CornerRadius::ZERO`; a negative radius is clamped to 0 by Bevy; a slash inside
a per-corner object value (`{ top: "10px / 5px" }`) is a bad `Length` (warns
`length`, 0) — the slash is only read in the whole-value string.

---

## 4. Commit sequence

1. **`feat(styling) elliptical borderRadius`** — §2.1–2.4, the tests of §3,
   the JSDoc edits of §5.4 and the CLAUDE.md sentence (§5.5). Builds and passes
   alone; no generated file changes. Run `cargo clippy --workspace
--all-targets`, `npm run lint`.
2. **`docs(demos) elliptical corners on the Borders page`** — §5.1–5.3:
   the demo example, the guide pages, the CHANGELOG bullets. `npm run build -w
demos`, `npm run docs`, `npm run lint`.

The repo's precedent (`f04700f`, `e0aa5f5`) folds a feature's demo into its
`feat` commit; the coordinator can squash 2 into 1.

---

## 5. Demos + docs

### 5.1 Demos — `examples/demos/ui/src/demos/styling/BordersDemo.tsx` (nav label "Borders")

Add one example, `EllipticalDemo`, right after "Rounded corners" (rows become
[Rounded corners, Elliptical corners], [Border width and color, Per-side
values], [Outlines]). A **non-square** box is the point: on a square one the
two forms look alike.

```tsx
function EllipticalCard() {
  const [x, setX] = useState(50);
  const [y, setY] = useState(50);
  return (
    <ControlColumn>
      <Box
        style={{
          width: 144,
          border: 3,
          borderColor: Colors.amber,
          borderRadius: `${x}% / ${y}%`,
        }}
      />
      <Slider value={x} min={0} max={50} onChange={setX} name="horizontal %" />
      <Slider value={y} min={0} max={50} onChange={setY} name="vertical %" />
    </ControlColumn>
  );
}
```

Info text: a `/` splits the value into horizontal and vertical radii, as in
CSS; with a slash a percentage follows its own axis, so `"50% / 50%"` is an
ellipse while plain `"50%"` stays a pill. Snippet (lines ≤ 40 chars):

```tsx
<node
  style={{
    width: 144,
    height: 72,
    borderRadius: "50% / 50%",
  }}
/>
```

Also add half a sentence to the page's `PAGE.info` paragraph. The border in
the card shows the ring following the ellipse. No change to "Style
transitions": its radius example stays circular.

### 5.2 Guide — `docs/guide/styling/borders.md` (says "elliptical corners are not supported")

In "Corner radius":

- Replace the bullet "One radius per corner: elliptical corners are not
  supported." with a subsection **"Elliptical corners"**: the two code lines
  from §2, then
  - the string form takes one `/`: horizontal radii before it, vertical after,
    each side 1–4 lengths expanded like the shorthand, corner by corner; the
    number and object forms are always circular;
  - with a slash a percentage is of the node's width (horizontal) or height
    (vertical); without one it is of the shorter side — so CSS's `50%` ellipse
    is written `"50% / 50%"` here;
  - each radius is capped at half the node's size on its own axis (CSS scales
    all radii together instead; `"150px / 10px"` on a 200-wide node is
    `100 / 10`);
  - a radius of `0` or `"auto"` on either axis squares that corner; a side
    that fails to parse counts as `0` and reports a `rect` warning.
- Extend the "rounds the background, border, …" bullet: the border's inner
  edge is the outer radius minus the border width per axis, and pointer
  hit-testing follows the rounded (or elliptical) shape.
- "Transitions and animation": every radius eases, eight per value with a
  slash; between a value with a slash and one without, the plain radius `r`
  counts as `r / r` — exact for pixel radii that fit the box, while a
  percentage or an oversized pill radius (`999`) means something else with a
  slash, so that shape jumps at the start; ease a pill as its real radius.
  The `{ animated }` binding still drives all four corners with one circular
  px value.
- "Limits" keeps "Overflow clipping is rectangular" (still true, §1.2).
- Frontmatter `description`: "rounded and elliptical corners…".

Smaller edits: `docs/guide/styling/units.md` ("Four-sided values": on
`borderRadius` the string also takes a `/`, link to Borders);
`docs/guide/animations/style-transitions.md` (the `borderRadius` bullet: with
a slash both radii of each corner ease); `docs/guide/animations/animated-values.md`
(table cell → "Logical px, all four corners, circular");
`docs/guide/styling/overflow.md` (scrollbar parts: "`borderRadius` (no `/`
form)").

### 5.3 CHANGELOG — `[Unreleased]`

- **Added** — **Elliptical `borderRadius`.** The string form takes the CSS
  slash: `"50% / 25%"`, `"40px 10px / 10px 40px"`, horizontal radii before it
  and vertical after. With a slash a percentage is of the node's width or
  height, so `"50% / 50%"` is an ellipse; without one, corners are circular
  exactly as before. Backgrounds, borders, gradients, shadows, outlines,
  backdrop filters, images and pointer hit-testing follow the ellipse, and
  `transition: { borderRadius }` eases every radius.
- **Changed** — (Rust, breaking for extension code) `style::props::BORDER_RADIUS`
  holds `Animatable<protocol::visual::CornerRadii>` and
  `TransitionInput::border_radius` is `Option<CornerRadii>`; a `Rect` converts
  with `.into()`.

### 5.4 TypeScript (JSDoc only) — `js/src/jsx.d.ts`

- `Rect`: after "On `borderRadius` the sides name the four corners…", add
  that there the string also takes the CSS slash (`"50% / 25%"`): horizontal
  radii before it, vertical after.
- `BevyTransition.borderRadius`: a value with a slash eases both radii of each
  corner; a value without one facing it reads as `r / r`.
- `ScrollbarPartVisual.borderRadius`: "(no `/` form)".

### 5.5 Generated files / project docs

- **Nothing to regenerate.** `npm run generate` must be a no-op (T11).
- `CLAUDE.md`, "Runtime behavior worth knowing", the `transition: { borderRadius }`
  sentence: add that `borderRadius` takes the CSS slash for elliptical corners
  (`protocol::visual::CornerRadii` → `CornerRadius::new`), no slash = circular
  (`%` of the shorter side), and a circular side facing an elliptical one
  eases as `r / r`.
- No screenshot in `screenshots/screenshots.json` shows the Borders page.

---

## 6. Decisions

| #   | Choice                                           | Recommended default                                                                                                                                                                | Reason                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| --- | ------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D1  | Wire form                                        | **The CSS slash string, and only it.** No `{ x, y }` object, no per-corner pairs.                                                                                                  | Web-faithful, composes with the `Rect` shorthand for free (each side _is_ a `Rect` string), and is already a `string`. An object `{ x, y }` next to `{ horizontal, vertical }` would be a trap: on `borderRadius` `horizontal` already means "the top-right + bottom-left corners". Computed values are a template literal (`` `${rx}px / ${ry}px` ``). Add `{ x: Rect; y: Rect }` later only on demand — it is ~6 lines in the same `Deserialize`.                                                      |
| D2  | `"50%"` without a slash                          | **Keep our circular rule** (percent of the shorter side, capped at half of it). Do not switch to CSS's per-axis percent.                                                           | (1) Published behavior: 0.7 apps use `"50%"` for circles and pills on non-square boxes (ours: `sharedElements/galleryCard.tsx`, a 72×92 thumb); switching silently turns them into ellipses. (2) It is Bevy's own convention (`From<Val>` = circular), so the binding path and Rust-side code agree. (3) Pixel radii — the bulk of real use — are identical to CSS either way; only bare `%` differs, and the CSS shape has an exact spelling, `"50% / 50%"`. Document the difference on the guide page. |
| D3  | TS type                                          | **Unchanged: `Animatable<Rect>`.** JSDoc only.                                                                                                                                     | A slash string is a `string`, which `Rect` already admits; a template-literal member adds nothing. Zero generated diff, devtools category untouched.                                                                                                                                                                                                                                                                                                                                                     |
| D4  | Oversized radii                                  | **Accept Bevy's per-axis clamp**; no CSS proportional scaling.                                                                                                                     | Reproducing CSS needs the laid-out size, i.e. a post-layout rewrite of `ComputedNode.border_radius` fighting Bevy's own writer every frame. The circular case already differs from CSS today. Document it.                                                                                                                                                                                                                                                                                               |
| D5  | `auto` in a slash value                          | **0** (square corner).                                                                                                                                                             | Bevy would read `{ Auto, r }` as a circular `r` (B3) — a surprise. CSS has no `auto` here; in our non-slash form `auto` already resolves to 0.                                                                                                                                                                                                                                                                                                                                                           |
| D6  | One axis 0                                       | Square corner (Bevy = CSS).                                                                                                                                                        | Nothing to implement; documented.                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| D7  | Transition between a non-slash and a slash value | **Promote the circular radius to `r / r` and ease per leaf.** Fallback if the artifact below is judged worse than a snap: return `other` when exactly one side has `y` (one line). | CSS interpolates the pair, and it is the right answer for the common case (`12` ↔ `"40px / 20px"`, unset ↔ slash). It degrades to a snap where meanings differ: a `%` leaf against a px leaf is a unit change. Known artifact: a circular `%`, or a px radius above half the shorter side (`999` pills), is _not_ `r / r` under Bevy's clamp, so the shape jumps on the first or last frame.                                                                                                             |
| D8  | `{ animated }` binding                           | **Unchanged**: one px value, four circular corners. No per-axis bindings.                                                                                                          | The wrapper replaces the whole field with a number; there is no slot for a second axis, and nobody asked. `seed` accepts a slash string but, as today, is not rendered for this property.                                                                                                                                                                                                                                                                                                                |
| D9  | Scrollbar part `borderRadius`                    | **Stays `Rect`** (circular); a slash there warns `rect`.                                                                                                                           | No use for elliptical thumbs; keeps `scrollbar.rs` untouched.                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| D10 | Malformed slash values                           | Warn under the existing `rect` kind; the bad side is 0 (so those corners are square).                                                                                              | Same contract as every other rect; `KIND_FIELDS.rect` already flags the `borderRadius` row.                                                                                                                                                                                                                                                                                                                                                                                                              |
| D11 | Rounded `overflow` clipping                      | **Out of scope.**                                                                                                                                                                  | Bevy 0.20 clips to rectangles only (§1.2).                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| D12 | Rust type name / home                            | `CornerRadii` in `protocol::visual` (public, like `Rect`).                                                                                                                         | Mirrors Bevy's `CornerRadius`; `units.rs` is full.                                                                                                                                                                                                                                                                                                                                                                                                                                                       |

---

## 7. Risks / perf

### 7.1 Style-store size (the one measurable cost)

`Animatable<Rect>` is 32 bytes by my reading of rustc's niche layout (`Rect` =
4 × 8 bytes, the `Box` variant fits beside a `Length` tag niche) and so sits
**inline** in the store's `SmallBox<_, S4>`. `CornerRadii` is 64 bytes, so
`Animatable<CornerRadii>` **heap-allocates**: one extra ~64-byte allocation per
node that sets `borderRadius`, at decode and at every `clone_value` (variant
overlays). `borderRadius` is on most styled nodes of a real app.

- Expected impact: noise (a color string already costs an allocation per
  node). The table-ops benchmark rows carry no `borderRadius`
  (`examples/stress/ui/src/App.tsx` — only the toolbar buttons), so it will
  not show there at all; do not read a clean table-ops run as proof.
- If a radius-heavy create path ever matters: keep the channel type as is and
  store `enum { Circular(Rect), Elliptical(Box<[Rect; 2]>) }` in the property
  (32 bytes inline for the common case), with a `fn radii(&self) -> CornerRadii`
  for the three readers. Not worth doing up front.
- Check during implementation: a throwaway
  `assert_eq!(size_of::<Animatable<Rect>>(), 32)` tells whether the "before"
  was really inline (if it prints 40, there is no regression at all).

`TransitionState` grows by 96 bytes and `TransitionInput` by 32, on
transitioning nodes only.

### 7.2 Per-frame cost and layers

- No new system, query or per-frame work. An elliptical value costs the same
  as a circular one everywhere: one `Node` write per changed frame.
- A radius ease is still a relayout per frame (B9) and still pushes layer
  content dirt from the drive block — unchanged, and unchanged for elliptical.
  Capture rects are unaffected (corners live inside the border box, no outset).
- A benchmark regression after this change would point at §7.1 (create /
  restyle legs) and nothing else; layers-stress presets set `borderRadius` on
  a handful of nodes only.

### 7.3 What to look at with `--shoot`

`npm run build -w demos`, then `cargo run -j 8 -p demos -- --shoot "<label>" target/port-scratch/shots/<x>.png 4`:

- **"Borders"**: the new card is a clean ellipse with an even amber ring
  (bevy's ellipse distance is a one-step approximation — look at the ring's
  antialiasing where the curvature is tightest); the three older cards are
  pixel-identical to before.
- **"Shared elements"**: the gallery thumbs (72×92, `borderRadius: "50%"`) are
  still pills — the D2 pin, visually.
- **"Backdrop filters"**, **"Style transitions"**: unchanged.

### 7.4 Not verified by reading

- The byte sizes in §7.1 (reasoned from the layout algorithm, not compiled).
- That the `Deserialize` sketch compiles as written (`StrDeserializer::<D::Error>`,
  `Rect::deserialize(&v)`); the fallback is a plain `Visitor` whose
  `visit_str` splits on `/` and whose other arms delegate to `Rect` through
  `serde::de::value::{F64Deserializer, MapAccessDeserializer, …}`.
- Visual quality of Bevy's elliptical antialiasing with thick borders on very
  eccentric corners (`"50% / 10%"`, `border: 6`).
- The D7 artifact's look in motion — it needs a human eye on a pill easing
  into a slash value; no staged capture.

### 7.5 Interplay with the other plans

`em`/`rem` (`plans/em-rem.md`) extends `Length`, `length_to_val` and
`Length::lerp`; this plan only calls them, so the two land in either order.
Nothing here touches `FixedNode`, interior clips or interaction capture.
