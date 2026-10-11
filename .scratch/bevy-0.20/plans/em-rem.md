# Plan: `em` and `rem` lengths (audit §2.1, §1.5)

Scope: accept `em`/`rem` wherever a style takes a length or a font size, with
CSS meaning as far as bevy-react's model allows, on Bevy 0.20.

Bevy paths below are relative to
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`. Our line numbers
were read on `main` at `4ad1b6d` with another agent editing the tree, so find
the **symbol**, not the line.

## 1. Verified facts

### Bevy 0.20

| #   | Fact                                                                                                                                                                                                                                                                                                                                                              | Source                                                                                                                            |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| B1  | `Val::Em(f32)` / `Val::Rem(f32)` exist. `Val::resolve(scale_factor, base, target, em_size, rem_size)` gives physical px: `Em = v * em * scale_factor`, `Rem = v * rem * scale_factor` (so both follow `UiScale`, like px; viewport units don't). Interpolation is same-unit only. `PartialEq` treats a zero of any unit as equal.                                 | `bevy_ui-0.20.0/src/geometry.rs:58-67, 489-511, 535-556, 138-163`                                                                 |
| B2  | `RemSize(pub f32)` is a plain resource, default `DEFAULT_REM_SIZE_PX = 20.0`, inserted by `TextPlugin`. It and `EmSize` are in `bevy::prelude` (through bevy_ui's prelude).                                                                                                                                                                                       | `bevy_text-0.20.0/src/text.rs:867-881`, `lib.rs:128`; `bevy_ui-0.20.0/src/lib.rs:82`                                              |
| B3  | `EmSize(pub f32)` is a component, default 20, **required by `Node`**. Its only writer in Bevy is `sync_font_size_to_em_size`: for entities with `EmSize + TextFont + ComputedUiRenderTargetInfo` and `With<Node>`, it writes `font_size.eval(viewport, rem)` when the `TextFont`, the target or `RemSize` changes. It runs in `PostUpdate`, `UiSystems::Content`. | `bevy_text text.rs:883-915`; `bevy_ui ui_node.rs:463-477`, `layout/mod.rs:77-101`, `lib.rs:295-297`                               |
| B4  | **A node without `TextFont` keeps `EmSize(20)` forever.** Nothing propagates it: bevy_ui's only `HierarchyPropagatePlugin`s are for `ComputedUiTargetCamera` / `ComputedUiRenderTargetInfo`; bevy_feathers propagates `TextFont`/`TextColor` under its own `ThemedText` marker, not `EmSize`. Bevy's doc says so outright ("does not track changes to it").       | `bevy_ui lib.rs:170-183`; `bevy_feathers-0.20.0/src/lib.rs:82-83`; `bevy_text text.rs:888-893`                                    |
| B5  | `ui_layout_system` re-converts a node into taffy when `target changed \|\| node.is_changed() \|\| content_size.is_changed() \|\| rem_size.is_changed() \|\| em_size.is_changed()`. The conversion bakes em/rem into px lengths, so nothing else re-resolves them.                                                                                                 | `bevy_ui layout/mod.rs:137-170`, `layout/convert.rs:29-32, 49-52`                                                                 |
| B6  | Every frame, for every node, layout copies `EmSize` and `RemSize` into `ComputedNode.em_size` / `rem_size` and resolves the `UiTransform` translation, `border_radius` and `Outline` with them. **So a `RemSize` change ticks `Changed<ComputedNode>` on every UI node**, whether it uses rem or not.                                                             | `bevy_ui ui_node.rs:61-70`, `layout/mod.rs:392-460`, `ui_transform.rs:194-206`                                                    |
| B7  | Gradient stops and box-shadow lengths resolve at extraction from `uinode.em_size` / `rem_size`. Retained extraction re-extracts a node on `Changed<ComputedNode>`.                                                                                                                                                                                                | `bevy_ui_render-0.20.0/src/gradient.rs:300-308, 478-479`, `box_shadow.rs:227, 324-325`, `lib.rs:450`                              |
| B8  | Grid: `GridTrack::em/rem`, `RepeatedGridTrack::em/rem`, `Min/MaxTrackSizingFunction::{Em, Rem}`, resolved against the **grid container's** em size.                                                                                                                                                                                                               | `bevy_ui ui_node.rs:1850-1892, 1957-1972, 2184-2200`, `layout/convert.rs:402-405, 432-435`                                        |
| B9  | `FontSize` is `Px \| Vw \| Vh \| VMin \| VMax \| Rem`. **There is no `FontSize::Em`.** `Mul<f32>` scales inside the variant (`Px(30) * 0.5 == Px(15)`, `Rem(1) * 1.2 == Rem(1.2)`). `TextFont::default().font_size` is `Rem(1.)` (while `FontSize::default()` is `Px(20.)`). Bevy's own `From<Val> for FontSize` maps `Em → Rem`.                                 | `bevy_text text.rs:783-796, 833-845, 762, 853-857`; `bevy_ui geometry.rs:514-532`                                                 |
| B10 | `LetterSpacing` is `Px \| Rem`; `LineHeight` is `Px \| RelativeToFont`. No em letter spacing, no rem line height.                                                                                                                                                                                                                                                 | `bevy_text text.rs:1349-1364, 1377-1394`                                                                                          |
| B11 | Text re-measures on a `RemSize` change only if a section's **font size** is `Rem` (`uses_rem_sizes`). A `LetterSpacing::Rem` on a px font is not re-measured (Bevy gap). `text_system` recomputes on `ComputedNode` change, so (B6) every text node rewrites `TextLayoutInfo` on a `RemSize` change. Editable text checks `rem_size.is_changed()` itself.         | `bevy_text pipeline.rs:106-116`, `text.rs:95-104`; `bevy_ui widget/text.rs:320-327, ~403`, `widget/text_input_layout.rs:103, 211` |
| B12 | `PostUpdate` order: `UiSystems::Prepare → Propagate → Content → Layout → PostLayout`. bevy_text's `detect_text_needs_rerender` runs after `PropagateSet::<TextFont>`.                                                                                                                                                                                             | `bevy_ui lib.rs:158-168`; `bevy_text lib.rs:130-137`                                                                              |
| B13 | `f32::interpolate_stable` is `a*(1-t) + b*t`, not our `a + (b-a)*t`.                                                                                                                                                                                                                                                                                              | `bevy_math-0.20.0/src/common_traits.rs:55-57, 477-485`                                                                            |

### Our tree today

| #   | Fact                                                                                                                                                                                                                                                                                                                                                                                   | Where                                                                                                                            |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| O1  | `Length` is `Auto \| Px \| Percent \| Vw \| Vh \| VMin \| VMax`. `parse_length` goes through `parse_suffixed`: the **first** suffix in the list that matches wins. `"1em"`/`"2rem"` match none, fail the number parse, `decode_warn("length")`, decode as `Px(0)`.                                                                                                                     | `crates/core/src/protocol/units.rs` (`Length`, `parse_length`, `parse_suffixed`)                                                 |
| O2  | The wire `FontSize` **already has `Rem`** (`"1.5rem"` works). `"1.2em"` warns `fontSize` and becomes `Px(0)`.                                                                                                                                                                                                                                                                          | `units.rs` (`FontSize`, `parse_font_size`)                                                                                       |
| O3  | `lineHeight` strings already accept `em` and `rem`, both as `RelativeToFont`. `letterSpacing` strings already accept both, both as `LetterSpacing::Rem` — so `"0.1em"` on 40px text is 2px, not 4px.                                                                                                                                                                                   | `crates/core/src/ui_map/text.rs` (`line_height`, `letter_spacing`)                                                               |
| O4  | `length_to_val` is the single `Length → Val` conversion (exhaustive match). Every `Node` field, rect, radius, outline, shadow, gradient stop and 2D translation goes through it.                                                                                                                                                                                                       | `crates/core/src/ui_map.rs`                                                                                                      |
| O5  | Lerp sites, both with a catch-all that snaps: `impl Lerp for Length` and `lerp_val`. `Rect` lerps per leaf through the first.                                                                                                                                                                                                                                                          | `transition/channels.rs`, `transition/gradient_channel.rs`                                                                       |
| O6  | Px-resolution sites: `length_logical_px` (filters; exhaustive match, 29 call sites, some macro-generated in consumer crates, no node context), `resolve_origin_axis` (transform3d; catch-all → center + `length` warning), `size_currents_px` (catch-all → `None`), and the shared-seed `len` closure (maps only `Px`/`Percent` back from `UiTransform`; anything else is not seeded). | `filters/params.rs`, `layer/transform3d.rs`, `transition/channels.rs`, `transition/shared.rs` (`let len = \|v: Val\|`)           |
| O7  | Grid tracks: `single_track` tries `fr`, `flex`, `px`, `%`.                                                                                                                                                                                                                                                                                                                             | `crates/core/src/protocol/grid.rs`                                                                                               |
| O8  | Text style is resolved **at op time, statically**, into `(TextColor, TextFont, LineHeight, LetterSpacing)`, cached per text node in `JsBridge::text_styles`. A bare-string run (`SpanKind::RawInherited`) copies its parent's cached tuple; a nested `<text>` (`textSpan`, `SpanKind::InlineStyled`) keeps its own and inherits nothing. The copy-down loop exists in **four** places. | `reconcile/apply.rs` (`inherit_text_style`), `reconcile/update.rs`, `reconcile/interaction.rs`, `layer.rs` (`reapply_text_fold`) |
| O9  | The layer capture cache's geometry hash covers each member's root-relative translation, linear part and **size** — not border widths, radii, outline, shadows or gradient stops.                                                                                                                                                                                                       | `layer.rs` (`fold_member_geometry`)                                                                                              |
| O10 | svg, canvas, anchor, portal and surface do not use the `Length` parser (surface only builds `Length::Vw/Vh` defaults). No example matches on `Length` exhaustively.                                                                                                                                                                                                                    | grep                                                                                                                             |
| O11 | TS: `Length = number \| string`, `FontSize = number \| string` (`js/src/jsx.d.ts`); `js/src/generated/style.ts` only references them. The JS runtime has no unit tables.                                                                                                                                                                                                               | grep                                                                                                                             |

### Where the audit is off

- "`FontSize::{Px,Rem,Em?}`": there is no `Em` (B9).
- It does not say that our wire `FontSize` already has `Rem`, nor that
  `lineHeight`/`letterSpacing` already accept `em`/`rem` strings with
  approximate meanings (O2, O3). Making `letterSpacing: "…em"` exact is a
  visible behavior change, not an addition.
- "`letterSpacing: "…em"` can become exact": only for a px or rem font size.
  With a viewport font size Bevy has no unit for it (B10).
- §2.10 suggests `lerp_val` → `a.try_interpolate_stable(&b, t).unwrap_or(b)`.
  That changes the float formula (B13) under tests that assert exact values.
  Add two arms instead.
- It misses the layer cache (O9 + B6): a `RemSize` change that only alters
  paint (a rem border width on a fixed-size box) leaves a cached capture stale.
- It misses B11's gap (`LetterSpacing::Rem` on a px font is not re-measured).
- "Expose `RemSize` through a built-in message or plugin option": not needed
  (Decision 12).
- "transform3d origin: resolve exactly": deferred (Decision 10).

## 2. Design

Five small parts. Nothing changes on the wire's shape or in the TS types:
`"2rem"` and `"1.5em"` are strings that used to warn.

### A. The units (`protocol/units.rs`, `ui_map.rs`, the lerps, filters)

1. `Length` gains `Em(f32)` and `Rem(f32)`.
2. `parse_length`'s suffix list gains `("rem", Length::Rem)` **then**
   `("em", Length::Em)` — `"1.5rem"` also ends in `"em"`. Extend the
   `parse_suffixed` doc comment's list of ordered pairs (`grad`/`rad`,
   `ms`/`s`, now `rem`/`em`).
3. `length_to_val`: `Length::Em(v) => Val::Em(v)`, `Length::Rem(v) => Val::Rem(v)`.
   The compiler forces this one.
4. `impl Lerp for Length`: add `(Em(x), Em(y)) => Em(lerp(x, y))` and the
   `Rem` twin. `lerp_val`: the same two arms on `Val`. These are **not**
   compiler-forced (catch-alls); without them em/rem transitions snap silently.
5. `transition/shared.rs`, the `len` closure: make the match exhaustive
   (`Val::Auto => None`, every other unit `Some(Length::…)`), so an outgoing
   node's `rem`/`em` (and, today's gap, `vw`…) translation seeds the incoming
   flight and a future unit cannot be dropped again.
6. `length_logical_px`: two more arms, `Length::Em(_) => "em"`,
   `Length::Rem(_) => "rem"`, falling into the existing error. Compiler-forced.
   Filters stay px-only (Decision 9).
7. Nothing to do, on purpose: `size_currents_px` (`None` for non-px is the
   documented fallback), `resolve_origin_axis` (Decision 10), the
   `{ animated }` path (always logical px), `Rect` (per-leaf through 4).

### B. `em` on an element with no font: stamp `EmSize` from `RemSize`

`<node>`, `<button>`, `<image>` and the feature elements carry no font and
bevy-react has no font inheritance through them, so the font size such an
element "inherits" is always the root size: **`em` equals `rem` there**. Bevy
does not do this for us (B4): those nodes sit at `EmSize(20)` whatever
`RemSize` says. One system, new file `crates/core/src/em_size.rs`:

```rust
/// `em` on an element with no font of its own is the root size (CSS: the
/// inherited font size; nothing between the root and a `<node>` sets one).
/// Bevy derives `EmSize` only where a `TextFont` sits, so mirror `RemSize`
/// onto every other React node.
pub(crate) fn sync_em_size_to_rem(
    rem: Res<RemSize>,
    mut nodes: Query<(&mut EmSize, Ref<ReactNode>), Without<TextFont>>,
) {
    let all = rem.is_changed();
    // A fresh node's default `EmSize` already equals the default root size.
    if !all && rem.0 == DEFAULT_REM_SIZE_PX {
        return;
    }
    // ponytail: one `is_added` check per node per frame while `RemSize` is
    // non-default; split out an `Added<ReactNode>` query if a profile shows it.
    for (mut em, node) in &mut nodes {
        if all || node.is_added() {
            em.set_if_neq(EmSize(rem.0));
        }
    }
}
```

- Registered in `ReactUiPlugin::build` (`plugin.rs`, next to the
  `MeasureStampSet` block): `PostUpdate`, `.before(bevy::ui::UiSystems::Layout)`.
  `PostUpdate` so a `RemSize` written by any `Update` system lands the same
  frame; before layout so B5's `em_size.is_changed()` re-converts the node.
- `Without<TextFont>` makes it disjoint from Bevy's
  `sync_font_size_to_em_size` (B3): no ambiguity, and no two writers fighting
  over one component.
- `With ReactNode` only: an app's own Bevy UI keeps its `EmSize` ("the value
  is yours to set", B4). Node-less React entities (spans, svg shapes, `<cube>`)
  have no `EmSize` and fall out of the query.
- Add `EmSize` to the bridge-owned list in the `ReactNode` rustdoc
  (`bridge.rs`).

With this, **every other em resolution is Bevy's**: layout (B5), radii,
outline and translation (B6), gradients and shadows (B7), grid tracks (B8) —
all read the node's `EmSize` / `ComputedNode.em_size`, which is now right on
every React node. `length_to_val` stays context-free (`Em → Val::Em`). The
alternative — mapping `Length::Em → Val::Rem` per element kind at convert
time — needs the element kind at every `length_to_val` call, including the
transition and animation systems that have none.

On `<text>` and `<editableText>` (they carry `TextFont` and a `Node`), Bevy's
own sync makes `width: "10em"` / `padding: "0.5em"` follow that text's font
size, hover-variant font sizes included. No code.

### C. Layer cache: a root-size change repaints

In `resolve_layer_repaints` (`layer.rs`), add a param
`rem: Option<Res<bevy::text::RemSize>>` (`Option`: the layer unit tests run
this system in bare schedules) and, before the stats step:

```rust
// A root-size change repaints every layer: `rem`/`em` paint values (border
// widths, radii, outlines, shadows, gradient stops) resolve at extraction
// and sit outside the geometry hash.
if rem.is_some_and(|r| r.is_changed()) {
    state.dirty.extend(registry.layers.values().map(|m| m.entity));
}
```

Everything else already reacts with no dirt from us: sizes and positions
change the geometry hash (the gate opens because of B6), text reshapes hit the
`Changed<TextLayoutInfo>` tap, and an `em` paint value on a `<text>` box only
changes when its font size does, which reshapes.

### D. Grid tracks (`protocol/grid.rs`)

`single_track`: after `px`, try `rem` then `em` (`GridTrack::rem` /
`GridTrack::em`), before `%`. `repeat(3, 10rem)` rides the same function.
Update the doc comment's token list.

### E. Text

**E1. Wire.** `FontSize` gains `Em(f32)`; `parse_font_size` gains
`("em", FontSize::Em)` right after `("rem", …)`. Fix the two doc comments that
say em is not accepted.

**E2. Resolution (`ui_map/text.rs`).** The font size a text node inherits
becomes a parameter; a block root's is the root size.

```rust
/// The font size a `<text>` block inherits: nothing above it carries one.
pub(crate) const ROOT_FONT_SIZE: BevyFontSize = BevyFontSize::Rem(1.0);

fn font_size_to_bevy(size: FontSize, inherited: BevyFontSize) -> BevyFontSize {
    match size {
        // … the six one-to-one arms …
        FontSize::Em(v) => inherited * v, // exact inside the inherited unit (B9)
    }
}

/// `resolve_text_font` for a run inside another: an `em` font size is a
/// multiple of `inherited`.
pub(crate) fn resolve_text_font_in(
    style: Option<&Style>, fonts: &Fonts, inherited: BevyFontSize,
) -> (TextFont, LineHeight, LetterSpacing)   // today's body

pub(crate) fn resolve_text_font(style: Option<&Style>, fonts: &Fonts) -> (…) {
    resolve_text_font_in(style, fonts, ROOT_FONT_SIZE)
}
```

So `fontSize: "1.2em"` on a block root is `FontSize::Rem(1.2)` with no special
case: it follows `RemSize`, and Bevy's sync (B3) gives the box the matching
`EmSize`. `resolved_text_style`, `SpawnCtx::text_style`, `TEXT_FONT_WRITER`
and every other caller keep their signatures and stay root-relative.

`letter_spacing(spec, font_size: BevyFontSize)`: split today's shared
`rem`/`em` string arm. `rem` and `{ rem }` stay `LetterSpacing::Rem(v)`. `em`
becomes a multiple of **this run's** font size:

```rust
match font_size {
    BevyFontSize::Px(p) => LetterSpacing::Px(k * p),
    BevyFontSize::Rem(r) => LetterSpacing::Rem(k * r),
    // A viewport font size: Bevy has no letter-spacing unit for it (B10).
    _ => { diag::report("letterSpacing", raw, "…"); LetterSpacing::Rem(k) }
}
```

`resolve_text_font_in` must set `font.font_size` before it resolves the
spacing. Unset `fontSize` + `"0.1em"` still gives `Rem(0.1)`, i.e. today's
result. `line_height` is untouched: `"1.5em"` → `RelativeToFont(1.5)` is
already exact (Decision 6 for `rem`).

**E3. `em` on a nested `<text>`: the cascade.** A `textSpan`'s `em` is a
multiple of the enclosing run's resolved size, which is only known once the
span is attached. Extend the existing op-time cascade rather than add a second
mechanism, and fold its four copies into one helper. New file
`crates/core/src/reconcile/text_cascade.rs`:

```rust
/// Push text node `id`'s resolved style to what inherits from it: every
/// bare-string run copies the halves asked for; with `font_half`, every
/// nested `<text>` sized in `em` re-resolves against it and cascades on.
/// `rebased` collects the re-resolved spans for `JsBridge::text_styles`.
pub(crate) fn cascade(
    commands: &mut Commands, bridge: &JsBridge, fonts: &Fonts, id: NodeId,
    resolved: &ResolvedTextStyle, color_half: bool, font_half: bool,
    rebased: &mut Vec<(NodeId, ResolvedTextStyle)>,
) {
    for kid in bridge.children_of(id) {
        let Some(&e) = bridge.nodes.get(&kid) else { continue };
        match bridge.spans.get(&kid) {
            Some(SpanKind::RawInherited) => apply_resolved_text_style(
                &mut commands.entity(e), resolved, color_half, font_half),
            Some(SpanKind::InlineStyled) if font_half =>
                rebase(commands, bridge, fonts, kid, resolved.1.font_size, rebased),
            _ => {}
        }
    }
}

/// Re-resolve styled span `id` against `inherited` if its `fontSize` is in
/// `em` (anything else does not depend on its parent), then cascade on.
pub(crate) fn rebase(
    commands: &mut Commands, bridge: &JsBridge, fonts: &Fonts, id: NodeId,
    inherited: BevyFontSize, rebased: &mut Vec<(NodeId, ResolvedTextStyle)>,
) {
    let style = bridge.props_cache.get(&id).and_then(|p| p.style.as_ref());
    if !style.is_some_and(|s| matches!(s.get(&FONT_SIZE), Some(FontSize::Em(_)))) {
        return;
    }
    let (Some(&e), Some(mut own)) =
        (bridge.nodes.get(&id), bridge.text_styles.get(&id).cloned()) else { return };
    (own.1, own.2, own.3) = resolve_text_font_in(style, fonts, inherited);
    apply_resolved_text_style(&mut commands.entity(e), &own, false, true);
    cascade(commands, bridge, fonts, id, &own, false, true, rebased);
    rebased.push((id, own));
}
```

Plus one bridge helper:

```rust
/// The font size `id` inherits: its text parent's resolved one, else the root's.
pub(crate) fn inherited_font_size(&self, id: NodeId) -> BevyFontSize {
    self.parent_of.get(&id).and_then(|p| self.text_styles.get(p))
        .map_or(ROOT_FONT_SIZE, |s| s.1.font_size)
}
```

Call sites (an op-path caller writes `rebased` back into
`bridge.text_styles`; the two that hold the bridge read-only or change color
only pass a throwaway `Vec`):

1. `apply.rs` `inherit_text_style` (called on every `Append`/`Insert`; give
   it `&mut JsBridge` and `fonts`): a `RawInherited` child copies the parent's
   cached tuple as today; an `InlineStyled` child calls
   `rebase(…, child, parent_cached.1.font_size, …)`. This is where an `em`
   span first gets its real size. React attaches a span's own children before
   it attaches the span, so the recursion is what fixes runs and nested spans
   that copied the root-relative placeholder a few ops earlier.
2. `update.rs` `apply_update`:
   - the cache refresh uses `resolve_text_font_in(style, fonts, inherited)`
     with `inherited = bridge.inherited_font_size(id)` read **before** taking
     the `text_styles` entry (a block root gets `ROOT_FONT_SIZE`);
   - for `TextRole::Span` with `font_half`, after `apply_style_masked`, queue
     `apply_resolved_text_style(&mut ec, &resolved, false, true)`: the writer
     has no parent and wrote the root-relative value a moment earlier in the
     same command buffer; this lands the real one. Both are
     compare-before-write, the double write only happens on a delta that
     touches a font property (which reshapes anyway), and it avoids threading
     a parent font size through `WriterCtx` (six construction sites, five of
     which would pass the root constant);
   - the trailing loop over ECS `Children` becomes `cascade(…)` over the
     shadow tree. (Side effect: a run appended earlier in the same batch,
     which ECS `Children` does not show yet, now follows too.) The `children`
     and `rnodes` params stay; the reset path uses them.
3. `interaction.rs` `apply_interaction_styles`: its bare-run loop becomes
   `cascade(…)` with the merged style's resolution. A hover variant that
   changes a block's `fontSize` now re-sizes its `em` spans too, and restores
   them on hover-out. No cache write (it has none today either).
4. `layer.rs` `reapply_text_fold`: its loop becomes `cascade(…, true, false, …)`.

Create stays as is: a span is created before it is attached, resolves
root-relative, and call site 1 corrects it in the same batch.

**Not changed:** a nested `<text>` with no `fontSize` still does not inherit
its parent's size (it is `1rem`). `fontSize: "1em"` is now the way to say
"same as the enclosing text" (Decision 4).

### F. What gets deleted

Three hand-rolled bare-run loops (update, interaction, layer) and the body of
`inherit_text_style`, replaced by `cascade`/`rebase`. The doc sentences that
say em is unsupported. Nothing else.

### System ordering summary

| System                      | Schedule / order                                              | Why                                                            |
| --------------------------- | ------------------------------------------------------------- | -------------------------------------------------------------- |
| `sync_em_size_to_rem` (new) | `PostUpdate`, before `UiSystems::Layout`                      | sees this frame's `RemSize`; layout sees this frame's `EmSize` |
| `resolve_layer_repaints`    | unchanged                                                     | reads `RemSize` change, same frame as the relayout             |
| text cascade                | inside `apply_js_ops` / `apply_interaction_styles` (`Update`) | op time, as today                                              |

## 3. Edge cases and tests

All headless.

**Parser — `protocol/units.rs` tests**

- `length_em_rem`: `"1.5rem"` → `Length::Rem(1.5)` (pins the suffix order:
  with `em` first it would warn), `"2em"` → `Em(2.0)`, `"0.5 em"` → `Em(0.5)`,
  `"rem"` alone → warn + `Px(0)`.
- Rect shorthand `"1rem 2em"` → top/bottom `Rem(1)`, left/right `Em(2)`.
- `fontSize`: `"1.2em"` → `FontSize::Em(1.2)`, `"1.2rem"` still `Rem(1.2)`.

**Conversion and lerp — `ui_map/tests.rs`, `transition/tests.rs`, `gradient_channel.rs` tests**

- `length_to_val`: `Em → Val::Em`, `Rem → Val::Rem`.
- `Length::Rem(1).lerp(Rem(3), 0.5) == Rem(2)`; same for `Em`;
  `Em(1).lerp(Rem(3), 0.5) == Rem(3)` and `Px(10).lerp(Rem(1), 0.5) == Rem(1)`
  (unit change snaps).
- `Rect` lerp: one corner `Rem → Rem` eases while another `Px → Rem` snaps.
- `lerp_val(Val::Rem(1), Val::Rem(3), 0.5) == Val::Rem(2)`.
- Shared seed: `transition::shared::snapshot(world, e)` on a static node with
  `ComputedNode`, `UiGlobalTransform` and `UiTransform.translation.x =
Val::Rem(2)` returns a seed whose `translate_x.current == Length::Rem(2)`
  (add a `#[cfg(test)]` accessor beside `SharedSeed::opacity_current`).

**Filters — `filters/builtin/mod.rs` tests**

- `length_logical_px("blur", "radius", Length::Rem(1.0))` is `Err` and the
  message names `"rem"`; same for `em`.

**Grid — `protocol/grid.rs` tests (`grid_templates_and_placement_decode`; the
grid tests compare `Debug` strings)**

- `"2em 10rem 1fr"` → `[GridTrack::em(2.), GridTrack::rem(10.), GridTrack::fr(1.)]`;
  `"repeat(3, 10rem)"` → `RepeatedGridTrack::repeat_many(3, [GridTrack::rem(10.)])`
  (what `parse_template` builds).

**Real layout — new tests in `em_size.rs`, on `crate::transition::layout_tests::layout_app()`**
(it already has `RemSize` and `sync_font_size_to_em_size`; add
`sync_em_size_to_rem.before(bevy::ui::ui_layout_system)` the way
`reconcile/clip_pick_tests.rs` adds its systems). Build the `Node` through the
real path: `serde_json::from_str::<Style>` → `crate::ui_map::node_from`.

- `rem_and_em_follow_the_root_size`: `(ReactNode(1), node_from({ "width":
"2rem", "height": "1em" }))` under `root_row()`. Update → 40×20. Insert
  `RemSize(10.)`, update → 20×10. The `em` half fails without the stamp.
- `fresh_node_under_a_non_default_root_size`: `RemSize(10.)` first and one
  update, then spawn `{ "width": "2em" }`, update → 20 (the `is_added` path).
- `a_text_box_em_is_its_own_font_size`: `(ReactNode(2), node_from({ "width":
"2em" }), TextFont { font_size: FontSize::Px(30.), ..default() })` → 60, and
  still 60 after `RemSize(10.)` (Bevy's sync owns it; ours must not touch it).
- `foreign_nodes_keep_their_em_size`: a `Node` with no `ReactNode` and
  `EmSize(7.)` keeps 7 after a `RemSize` change.

**Layer cache — `layer.rs` tests (the bare-schedule `resolve_layer_repaints` pattern)**

- `root_size_change_repaints_cached_layers`: one registered layer, no other
  dirt. Without `RemSize`: clean. Insert `RemSize(24.)`, run → dirty. Run
  again → clean. Mutate it, run → dirty.

**Text — `ui_map/tests.rs`**

- `font_size_units`: add `"1.2em"` → `BevyFontSize::Rem(1.2)` on a root;
  `resolve_text_font_in(…, Px(30.))` → `Px(36.)`; `(…, Vw(2.))` → `Vw(2.4)`.
- `letter_spacing_px_vs_rem` (signature change): `"0.1em"` with font `Px(30)`
  → `LetterSpacing::Px(3.0)`; with `Rem(1.5)` → `Rem(0.15)`; with `Vw(2)` →
  `Rem(0.1)` plus a `letterSpacing` runtime warning (assert with a filter
  narrow to this value — `take_runtime_warnings()` is process-global);
  `"0.1rem"` and `{ rem: 0.1 }` → `Rem(0.1)` whatever the font.
- `line_height_px_vs_relative`: unchanged, still passes.

**Cascade — `reconcile/update/tests.rs` (op harness, next to the bare-run
tests there), reading `TextFont.font_size` off the entities**

- `em_span_resolves_against_its_parent`: create text 1 `{ fontSize: 30 }`,
  textSpan 2 `{ fontSize: "0.5em" }` with inline text, append 2→1, 1→root →
  span is `Px(15)`.
- `em_span_runs_and_nested_spans_follow`: span 2 (`"0.5em"`) holding a bare
  run 3 and a span 4 (`"2em"`), all appended bottom-up before 2→1 → run 3
  `Px(15)`, span 4 `Px(30)`.
- `em_span_follows_parent_delta`: update 1 `{ fontSize: 40 }` → span `Px(20)`,
  its run too.
- `em_span_own_delta`: update 2 `{ fontSize: "0.8em" }` → `Px(24)`; then
  `{ fontSize: 12 }` → `Px(12)` and a later parent delta leaves it alone.
- `em_span_follows_rem_parent`: parent `"1.5rem"` → span `Rem(0.75)` (stays
  root-relative, so it tracks `RemSize` with no cascade).
- `em_block_root_is_root_relative`: text `{ fontSize: "1.2em" }` → `Rem(1.2)`.
- `same_batch_append_then_update`: `[Append(text, run), Update(text,
fontSize)]` in one batch → the run has the new size (the shadow-tree fix).
- `reconcile/interaction.rs` tests: a block with `hoverStyle: { fontSize: 40 }`
  and an `em` span → `Px(20)` while hovered, `Px(15)` after.

**Edge cases the tests above do not cover — accepted**

- A span appended while its block is hovered with a font-size variant gets the
  base size until the next restyle (bare runs behave the same today).
- `Suspense`-style two-commit mounts: the append is still what resolves.

## 4. Commit sequence

Each builds and passes `cargo test` + `cargo clippy --workspace --all-targets`.

1. `feat(core) fontless nodes take their em size from RemSize` — `em_size.rs`
   (+ tests on `Val::Em` nodes), `lib.rs` mod line, plugin registration,
   `ReactNode` rustdoc. Invisible until a `Val::Em` exists.
2. `fix(layer) a RemSize change repaints cached layers` — part C + test.
3. `feat(style) em and rem length units` — part A, its tests, the `Length`
   JSDoc in `js/src/jsx.d.ts`, rustdoc on `Length`.
4. `feat(grid) em and rem tracks` — part D + test.
5. `feat(text) fontSize in em; letterSpacing em follows the font size` — E1,
   E2, tests, the `FontSize` JSDoc, rustdoc on `FONT_SIZE` / `LETTER_SPACING`
   (`style/props/text.rs`) and `LetterSpacingSpec` (`protocol/visual.rs`).
   Spans are root-relative for one commit.
6. `feat(text) em on a nested text is relative to the enclosing text` — E3
   (`text_cascade.rs`, the four call sites) + cascade tests. The riskiest
   commit; it can be dropped without affecting 1–5.
7. `feat(demos) em and rem on the Units page` — §5, `examples/demos/units.rs`,
   regenerated `examples/demos/ui/src/bevy.ts`.
8. `docs: em and rem units` — guide pages + CHANGELOG.

## 5. Demos + docs

### Demos (`examples/demos/ui/src/demos/styling/UnitsDemo.tsx`, nav label "Units", unchanged)

- `PAGE.info`: add `width: "12rem"` and `padding: "0.5em"` lines to the code
  block (lines ≤ 40 chars); list becomes "Lengths: px, %, vw/vh/vmin/vmax,
  rem, em, auto." and "fontSize takes px, viewport units, rem and em."
- `LENGTHS`: add `"6rem"`.
- `FONT_SIZES`: add `"1.2em"`.
- New `Example` "Root font size" (third `DemoRow`, or beside "Font sizes"):
  the existing `Slider` (`@/components`, `name="RemSize" unit="px"`, 12–32,
  state starts at 20) calling `bevy.units.setRemSize({ px })`, and under it:
  a bar `width: "8rem"`; a `<text style={{ fontSize: "1rem" }}>`; a chip
  `<text style={{ fontSize: 28, padding: "0.5em", borderRadius: "0.25em" }}>`
  (em of its own font: does **not** move with the slider) next to a `<node>`
  chip with the same em values (does move: em = rem on a node); and
  `<text style={{ fontSize: 28 }}>Big <text style={{ fontSize: "0.5em"
}}>half</text></text>`. A `useEffect` cleanup resets to 20 on unmount.
  Info text: tsx snippet + the Rust message snippet.
- `examples/demos/ui/src/demos/elements/TextDemo.tsx` (~line 78): the
  font-size sentence mentions `em`.
- Rust: `examples/demos/units.rs`, modeled on `clipboard.rs`:
  `#[react_message(name = "units.setRemSize")] pub struct SetRemSize { pub px: f32 }`,
  handler `fn(on: On<SetRemSize>, mut rem: ResMut<RemSize>)` writing
  `px.clamp(8.0, 48.0)` only when different, `pub fn register_bindings`.
  Register in **both** places in `main.rs`: beside
  `clipboard::register_bindings(&mut app)` in the live app and in
  `register_react_bindings` (the exporter). Then `npm run bevy:generate -w demos`.
- The shell's typography sets px sizes (`FontSizes.*`), so the slider should
  move only the rem/em samples. Check with `--shoot "Units"`; a scripted
  `--input` drag on the slider shows the non-default case.

### Guide

- `docs/guide/styling/units.md`
  - Lengths table: add `"2rem"` ("2 × the root font size, Bevy's `RemSize`,
    20px by default") and `"1.5em"` ("1.5 × the element's font size").
  - New bullets: rem and em are scaled by the scale factor and `UiScale` like
    px; em is the element's own `fontSize` on `<text>` and `<editableText>`,
    and the root size on every other element (there is no font size to
    inherit through a `<node>`), so `em` and `rem` are the same there.
  - Remove `"1em"`, `"2rem"` from the "Anything else … becomes `0`" bullet.
  - "Font sizes": accepts `em`; on a `<text>` it is relative to the root size,
    on a nested `<text>` to the enclosing text. Still no percentages.
  - New section "The root font size": it is Bevy's `RemSize` resource;
    `app.insert_resource(RemSize(16.0))` or mutate it in any system; lengths,
    text and layers follow the same frame; bevy-react adds no API. A
    tsx-then-rust fence pair (renders as tabs) showing a `#[react_message]`
    that sets it from React. Unset `fontSize` is `1rem`, so it follows too.
  - "Limits": drop `em` from "No `calc()`, `em`, or unit arithmetic"; add:
    filter params and `transform3d.origin` do not take `em`/`rem`; `em` inside
    a `scrollbar` style resolves against 20px (use `rem`); a `{ animated }`
    binding is px.
- `docs/guide/elements/text.md`: table row `fontSize` gains `"em"`, default
  becomes "`1rem` (20px)"; "Size and weight" bullet: replace "CSS `em` is not
  supported" with the root/nested rule and the "`1em` to inherit" tip;
  `letterSpacing` bullet: `"em"` is a multiple of the run's font size, `"rem"`
  and `{ rem }` of `RemSize` (exact for px and rem font sizes; with a viewport
  font size it reads as rem and warns); `lineHeight` bullet stays (rem is
  still a multiple of the font size).
- `docs/guide/layout/grid.md`: tracks accept `"10rem"` and `"2em"` (of the
  grid container); viewport units stay in the "not accepted" list.
- `docs/guide/animations/style-transitions.md` (~line 121): "px to px, `%` to
  `%`, `vw` to `vw`, `rem` to `rem`"; `em` to `rem` snaps.
- `docs/guide/extending/custom-filters.md` (~line 114): add `"1rem"` to the
  rejected examples.
- `docs/guide/styling/3d-transforms.md`: no change ("another unit falls back
  to the center" already covers it).

### CHANGELOG (`[Unreleased]`)

Added:

- **`em` and `rem` lengths.** Every length — sizes, insets, margins, padding,
  borders, radii, gaps, grid tracks, translations, shadows, outlines, gradient
  stops — takes `"2rem"` (multiples of Bevy's `RemSize` resource, 20px by
  default) and `"1.5em"` (multiples of the element's font size: its own
  `fontSize` on `<text>`/`<editableText>`, the root size elsewhere).
  Transitions ease between values of the same unit. Change `RemSize` and the
  UI follows.
- **`fontSize` in `em`**: relative to the enclosing text on a nested `<text>`,
  to the root size on a block.

Changed:

- **`letterSpacing: "…em"` is a multiple of the text's font size**, as in CSS.
  It used to be a multiple of `RemSize`; use `"…rem"` for that.
- **An unset `fontSize` is `1rem`** (Bevy 0.20's default): 20px as before, but
  it now follows `RemSize`.
- **`Length` and `FontSize` (`bevy_react::protocol::units`) gained variants**
  (breaking for Rust code that matches them exhaustively, e.g. a hand-written
  filter `pack`).

### Generated files

- `examples/demos/ui/src/bevy.ts` — regenerate (new message).
- `js/src/generated/{style,elements}.ts`, `docs/reference/bevy.ts` — expected
  unchanged; run `npm run generate` and confirm an empty diff.
- `npm run docs` to validate links; the reference pages pick up the new JSDoc
  at build time.
- `js/src/devtools/warnings.ts` `KIND_FIELDS` — no new kind (reuses `length`,
  `fontSize`, `letterSpacing`, `gridTrack`, `filterParams`).

## 6. Decisions

| #   | Choice                                                        | Recommended                                                                                               | Reason                                                                                                                                                                                                                           |
| --- | ------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `em` on a `<node>` / `<button>` / `<image>` / feature element | The root size (= `rem`), via the `EmSize` stamp                                                           | CSS em is the inherited font size; nothing between the root and a node sets one here. Bevy's fixed 20 would silently stop matching `rem` the moment `RemSize` changes.                                                           |
| 2   | `em` on a `<text>` / `<editableText>` box property            | The text's own font size (Bevy's sync, no code)                                                           | CSS.                                                                                                                                                                                                                             |
| 3   | `fontSize: "Nem"`                                             | Block root: `FontSize::Rem(N)`. Nested `<text>`: N × the enclosing run, in that run's unit                | CSS: em of the parent's size. Exact, and a rem/viewport parent keeps the child responsive without a cascade.                                                                                                                     |
| 4   | Nested `<text>` with no `fontSize`                            | Unchanged: no inheritance (`1rem`). Document `"1em"` as the way to inherit                                | Changing default inheritance is a separate, breaking decision; this ships the tool to opt in.                                                                                                                                    |
| 5   | `letterSpacing: "…em"`                                        | × the run's font size (px and rem fonts exact; viewport font: reads as rem + `letterSpacing` warning)     | CSS. It is a visible change for apps using em letter spacing on non-20px text → CHANGELOG "Changed". No example uses it.                                                                                                         |
| 6   | `lineHeight: "…rem"`                                          | Unchanged (a multiple of the font size)                                                                   | Bevy has no rem line height (B10); exactness would need a per-frame resolver. Cheap middle ground if wanted: `RelativeToFont(k / r)` when the font is `Rem(r)`.                                                                  |
| 7   | `fontSize: "120%"`                                            | Not added                                                                                                 | Not asked for; one line later (`% → Em(v / 100)`).                                                                                                                                                                               |
| 8   | Transitions                                                   | Same-unit lerp for `em` and `rem`; any unit change snaps, `em ↔ rem` included                             | The existing rule. They differ on text, so treating them as one unit on nodes only would be a special case.                                                                                                                      |
| 9   | Filter `Length` params                                        | Reject `em`/`rem` with the existing `filterParams` message                                                | `pack`/`outset`/`resolve` have no node and no `RemSize`, are called from macro-generated code in consumer crates, and chains are resolved once per style change. Compute px in JS.                                               |
| 10  | `transform3d.origin`                                          | Unchanged: `em`/`rem` fall back to center with a `length` warning, like viewport units                    | Zero code, already documented. If wanted later: resolve each axis with `length_to_val(len).resolve(scale, extent, target, computed.em_size, computed.rem_size)` in `sync_transform3d_matrices` — that also fixes viewport units. |
| 11  | Grid tracks                                                   | Add `rem` and `em`; leave viewport tracks out                                                             | Scope. Bevy has `GridTrack::vw` etc. if someone asks.                                                                                                                                                                            |
| 12  | Exposing `RemSize`                                            | No bevy-react API, no built-in message. Document the resource; the demos own a 10-line `#[react_message]` | It is one public Bevy resource in the prelude; a wrapper would be a second way to do the same thing. Default stays Bevy's 20px (CSS's 16 would resize every unstyled text).                                                      |
| 13  | Layers on a `RemSize` change                                  | Repaint every layer that frame                                                                            | Rare event, three lines, and the alternative (hashing borders, radii, shadows, gradients per member) costs every frame.                                                                                                          |
| 14  | `{ animated }` on a length                                    | Unchanged: logical px                                                                                     | Documented contract.                                                                                                                                                                                                             |
| 15  | `lerp_val`                                                    | Add two arms; do not switch to `try_interpolate_stable`                                                   | B13: different rounding under exact-value tests.                                                                                                                                                                                 |
| 16  | `Length` as a thin wrapper over `Val` (audit §2.10)           | Not now                                                                                                   | Orthogonal refactor; `Val`'s zero-equality (B1) needs its own look at compare-before-write.                                                                                                                                      |
| 17  | `em` in `scrollbar` style rects                               | Known limit: resolves against 20px                                                                        | Track/thumb are not `ReactNode`s; widening the stamp to them is not worth a query term.                                                                                                                                          |
| 18  | Span parent size in `WriterCtx`                               | No; the update path corrects the span after the writer                                                    | See E3 call site 2.                                                                                                                                                                                                              |

## 7. Risks / perf

- **Idle cost: none** at the default root size (`sync_em_size_to_rem` returns
  on its second line; the layer tap is one `is_changed`). With a non-default
  `RemSize` the stamp does one `is_added` check per fontless React node per
  frame — the `ponytail:` note. If `table-ops` or `layers-stress` regress,
  look there first (they run at the default, so they should not).
- **A `RemSize` change is a full-UI event, by Bevy's design** (B5, B6): every
  node is re-converted into taffy, relaid out, re-extracted, every text node
  rewrites its layout info, and (part C) every layer re-captures. A slider
  drag does that every frame. Fine for a settings control; do not animate
  `RemSize`.
- **Layout transitions follow**: nodes with `transition: { layout }` FLIP the
  re-flow a `RemSize` change causes, and lag during a continuous drag (the
  documented "driver the engine can't attribute" limit).
- **The cascade refactor (commit 6)** touches the op hot path and the
  interaction restyle. Same work per text delta as before plus one
  `spans.get` per child; `rebase` early-outs on one `props_cache` lookup for
  a non-em span. The double `TextFont` write on an `em` span's own font delta
  is ordered inside one command buffer — not a cross-stage fight.
- **Pre-existing, not fixed here:** the same cache hole as part C exists for
  paint-only viewport-unit values on a window resize; `LetterSpacing::Rem` on
  a px font is not re-measured on a `RemSize` change (B11); surface
  (virtual-pointer) hover restyles do not propagate to bare runs.
- **What to look at with `--shoot`:** `"Units"` at the default (the `6rem` bar
  = 120px, `"1.2em"` text = 24px, the half-size nested span); `"Units"` after
  a scripted slider drag (node chip and rem bar scale, the text chip's padding
  does not, the shell does not move); `"<text>"`, `"Layers"` and
  `"Shared elements"` unchanged against current shots (cascade and layer
  regressions).

### Not verified by running (read only)

- That `layout_app` accepts the extra `.before(ui_layout_system)` system (the
  `clip_pick_tests` precedent says yes).
- React's append order for nested text (bottom-up `appendInitialChild`); the
  cascade is order-independent either way.
- Retained gradient extraction on a `RemSize` change: read for the main node
  and box-shadow filters (`Changed<ComputedNode>`), assumed for gradients
  since they share the node filter in `bevy_ui_render/src/lib.rs`.
- Whether any shell text in the demos is unstyled (and so would scale with the
  slider): typography components all set px; a grep for bare `<text>` found
  only snippet strings and nav items with explicit styles.
