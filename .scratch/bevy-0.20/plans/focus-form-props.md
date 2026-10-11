# Plan — focus and form props: `tabIndex`, `onFocus`/`onBlur` everywhere, `focusVisibleStyle`, `readOnly`, `disabled`

Audit section: §2.6 "Focus and form props".

Written against `main` at `0e7fda7`. The `Interaction` retirement
(`plans/interaction-and-capture.md`) has **not** landed yet; this plan is
written against its end state (`ReactInteraction`, `reconcile/picking.rs::
sync_interactions`, one restyle and one click collector for every pointer) and
§2.9 lists what differs if it is implemented before that. Line numbers for our
files are "at the time of writing"; symbols are the anchor. Nothing here was
compiled or run: every Bevy claim is from reading the 0.20 source, every claim
about our behavior from reading ours.

**The design in one paragraph.** `tabIndex` becomes a common prop (a new
`Common::FOCUS` group with `onFocus`/`onBlur`) stamped as Bevy's `TabIndex`;
`<editableText>` keeps its implicit `0`. Bevy's stock click-to-focus cannot be
used as is under bevy-react's pass-through stacks — it asks for focus once per
pressed entity in hash order and every request that reaches the window clears
the focus — so one small system resolves a press the way clicks are already
resolved (_the topmost pressed node that has a focusable on itself or an
ancestor_) and replaces whatever Bevy recorded for that press. A second small
system diffs `InputFocus` each frame into `FocusState` (now `{ focused,
visible }`) and the `focus`/`blur` events, replacing the two bubbling
observers. `disabled` is `InteractionDisabled` plus "not focusable"; on
`<editableText>` it is also `TextReadWriteMode::Static` (the marker alone does
not stop typing — the audit was wrong there). Tab cycling stays the app's
one-line opt-in (`TabNavigationPlugin`); bevy-react only puts the `TabGroup`s
on its roots so that line is enough.

---

## 0. Today's picture

| Piece                                         | Where                                                                                                                                   | State                                                                                                                                                     |
| --------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `focusStyle` prop                             | `protocol/props.rs` (`Common::VARIANTS`), `StyleVariants::focus`, overlaid last in `reconcile/interaction.rs::apply_interaction_styles` | generic: any element with the VARIANTS group                                                                                                              |
| `FocusState(pub bool)`                        | `bridge.rs`; inserted by `reconcile/stamps.rs::apply_style_variants` only when a `focusStyle` is present                                | generic                                                                                                                                                   |
| `on_focus_gained` / `on_focus_lost` observers | `elements/editable.rs`, registered in `plugin.rs`                                                                                       | generic for `FocusState`; act on `original_event_target()` because `FocusGained`/`FocusLost` bubble; run in `PostUpdate`, so the style lands a frame late |
| `onFocus` / `onBlur`                          | `editable.rs::{FOCUS, BLUR}` — **element events** of `<editableText>` only, gated on `EventSubscriptions`                               | not generic                                                                                                                                               |
| A way to _get_ focus                          | `spawn_editable` inserts `TabIndex(0)`; nothing else carries one                                                                        | not generic                                                                                                                                               |
| `focus_pickable`                              | `style/writers/interaction.rs`: `should_block_lower` when `focusPolicy == Block` **or** `flags.text == TextRole::Input`                 | input-only workaround for the race in §1.2                                                                                                                |
| `<button>`                                    | `elements/node.rs`: `ui_widgets::Button` (+ `Interaction`), default `focusPolicy: "block"`; `label_buttons` names it                    | not focusable, no keyboard, no `disabled`                                                                                                                 |
| Keyboard                                      | `keyboard.rs::collect_keyboard_events` reads raw `KeyboardInput` → global `keyDown`/`keyUp`                                             | independent of focus; unchanged by this plan                                                                                                              |
| Tab                                           | nothing; `docs/guide/elements/editable-text.md` says so                                                                                 | —                                                                                                                                                         |

---

## 1. Verified facts

Registry root: `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`.

### 1.1 `bevy_input_focus-0.20.0`

`src/lib.rs`

- `InputFocus` (`:103-120`) is `Clone + PartialEq + Default`: `current_focus`
  plus a private FIFO `recorded_changes` and `original_focus`. `set(entity,
cause)` and `clear()` each **push a record** (`:145-159`).
  `gained_and_lost.rs::process_recorded_focus_changes` (`PostUpdate`,
  `InputFocusSystems::FocusChangeEvents`) replays **every** record as
  `FocusLost`/`FocusGained`, skipping only a record equal to the one before it
  (`gained_and_lost.rs:64-107`, the skip at `:83`). So focus that flaps
  A → none → A inside one frame fires `Lost(A)`, `Gained(A)`.
- `FocusGained`/`FocusLost` are `auto_propagate` entity events
  (`gained_and_lost.rs:32-59`): they bubble through every ancestor.
- `InputFocusVisible(pub bool)` (`:162-182`) is a plain flag "not used by the
  crate itself"; default `false`.
- `AcquireFocus` (`:219-234`) bubbles `ChildOf`, then jumps to
  `event.window` (`WindowTraversal`, `:261-277`) — _any_ non-window entity
  eventually reaches the window. `on_window_acquire_focus_clear` (`:299-312`)
  clears the focus there. It is a global observer added by `InputFocusPlugin`
  (`:322-333`); nothing can veto it for a request whose original target **is**
  the window.
- `set_initial_focus` (`PostStartup`, `:380-387`) focuses the **primary
  window entity**. A click on nothing leaves `None`, not the window.
- `dispatch_focused_input` (`PreUpdate`, `InputFocusSystems::Dispatch`,
  `:394-436`) sends `FocusedInput<KeyboardInput>` to the focused entity,
  bubbling to the window. A despawned focused entity is cleared **only here**,
  i.e. on the next key press (`:405-423`).
- `DefaultPlugins` has `InputFocusPlugin` and `InputDispatchPlugin`
  (`bevy_internal-0.20.0/src/default_plugins.rs:15-18`) and, through
  `UiWidgetsPlugins` (`:98-100`), `PointerFocusPlugin`, `ButtonPlugin` and
  `TextInputPlugin` (`bevy_ui_widgets-0.20.0/src/lib.rs:91-109`).
  **`TabNavigationPlugin` is in none of them.**

`src/pointer_focus.rs`

- `click_to_focus` (`:29-52`) observes `PointerPress` for **any pointer and
  any button**, acts on the original target only (`:39`), sets
  `InputFocusVisible` to `false` (`:41-43`) and triggers `AcquireFocus` on the
  pressed entity (`:45-50`).
- `PointerFocusPlugin` (`:90-100`) also registers
  `tab_navigation::acquire_focus_tab_index`, documented as a "temporary
  bridge" until a `PointerFocusable` component exists (`:65-71`) — this
  pathway will change in 0.21.

`src/tab_navigation.rs`

- `TabIndex(pub i32)` (`:58-64`). `>= 0` = in the Tab order; `< 0` =
  focusable "via direct selection" only (`:3-10`, enforced at `:343-347`).
  Order: by group, then **ascending index**, ties in tree order (stable sort
  after a depth-first gather, `:308-314`). So `1, 2` come **after** `0` —
  the opposite of HTML, where positive values come first.
- `acquire_focus_tab_index` (`:389-403`): the request stops at the first
  entity with a `TabIndex` (any value) and focuses it with
  `FocusCause::Navigated` — clicks do not report `Pressed`.
- `TabGroup { order, modal }` (`:66-100`). Tabbables must descend from one
  (`:12-15`). Non-modal groups form one global cycle sorted by `order`
  (`:282-300`); inside a **modal** group Tab cycles among that group's
  descendants only, and modal groups are left out of the global cycle
  (`:274-281`, `:287`). The group of the focused entity is its nearest
  `TabGroup` **ancestor** (`:202-211`, self excluded). A subtree whose
  `InheritedVisibility` is false is skipped (`:340-342`) — `display: "none"`
  is not checked.
- `TabNavigationPlugin` (`:406-419`) attaches `handle_tab_navigation` to the
  primary-window entities that exist at `Startup`. On Tab (pressed, not a
  repeat; Shift = backwards) it sets the focus and
  `InputFocusVisible = true` (`:427-453`). Every failure is `warn!`ed
  (`:455-456`): no groups, no focusables — and
  `NoTabGroupForCurrentFocus` (`:248-254`), which fires (and recovers) when
  the focused entity has no `TabGroup` ancestor. **The window is such an
  entity**, so the first Tab after startup logs one warning.

### 1.2 The click-to-focus race (why stock pointer focus is not enough)

`bevy_picking-0.20.0/src/events.rs:1052-1083`: on a press, `PointerPress` is
triggered (and written as a message) for **every entity in that pointer's
hover map**, iterated in `EntityHashMap` order. With pass-through `Pickable`s
the hover map holds the whole stack down to the window: the window backend
always adds the window entity beneath everything (`window.rs:22-58`, depth 0,
order −∞), and `build_hover_map` only stops at a blocking entity
(`hover.rs:265-291`).

So one click on a pass-through node `F` with a `TabIndex`, inside pass-through
ancestors, produces requests from `F` and its descendants (→ focus `F`), from
each ancestor (→ bubble to the window → clear), and from the window itself
(→ clear), in hash order. The final state is `F` or nothing; every
intermediate change is replayed as `FocusLost`/`FocusGained`.

The same happens with **two pointers for one physical click**: inside a
`<surface>` or a 3D-transformed layer the real mouse still presses whatever
lies under the cursor in the window (at least the window entity), while the
remapped pointer presses the UI. `PointerInput`s are read in write order
(mouse in `First`, the remapped pointer in `PreUpdate`), so today a click into
an **already focused** field there records `[None, Some(field)]` →
`FocusLost` + `FocusGained` on every click. Bevy's text input reacts to
`FocusLost` with `CollapseSelection` (`text_input.rs:667-675`), queued after
the press's own edit — by reading, double-click word selection cannot survive
inside a `<surface>` or a transformed layer today. _(Unverified by running;
the first test of commit 1 should show it red.)_

The audit's fix ("these nodes also need the blocking rule") does not hold up:

- Blocking changes hit-testing: every ancestor of a blocking node drops out of
  the hover map, so it loses hover, press and drag begin. The gallery's own
  `Button` (`examples/demos/ui/src/components/Button.tsx`) is a
  `<button focusPolicy: "pass">` inside `Pinchable`'s press surface precisely
  so the surface sees the press; a blocking focusable button would kill the
  pinch. On the web, `tabindex` never changes hit-testing.
- `Pickable` is written by `FOCUS_POLICY_WRITER`, which reads a _style
  property_; `tabIndex` is a prop. Two owners of one component, or a prop a
  global writer cannot see.
- It does nothing for a pass-through overlay _above_ the node, nor for the
  two-pointer case.

### 1.3 `bevy_ui_widgets-0.20.0`

`src/button.rs`

- `Button` requires `AccessibilityNode(Role::Button)` (`:27-31`).
- `button_on_key_event` (`:39-58`): on `FocusedInput<KeyboardInput>` at a
  focused, non-`InteractionDisabled` `Button`, a non-repeat **press** of
  `KeyCode::Enter` or `KeyCode::Space` stops the event and triggers
  `Activate`.
- `button_on_pointer_click` (`:60-76`) triggers the **same** `Activate` for a
  pointer click. So forwarding `Activate` as `onClick` would double-fire
  mouse clicks (ours already come from `PointerClick` messages, with the
  ownership and drag rules).
- The pointer observers (`:60-139`) `propagate(false)` and maintain bevy*ui's
  `Pressed` marker unless disabled. We read neither (we consume picking
  \_messages*, which observers cannot stop). Nothing to forward, nothing to
  suppress.

`src/text_input.rs`

- Everything filters `With<TextInput>`.
- Keyboard (`on_focused_keyboard_input`, `:114-264`). The gate is at
  `:164-172`:
  `mode == Editable || (!edit.is_destructive() && (!disabled || mode == ReadOnly))`.
  - `Editable`: every edit goes through — **`InteractionDisabled` is ignored.**
  - `ReadOnly`: caret moves, selection, copy (`text_edit.rs:226-269` lists
    the destructive edits); no mutation.
  - `Static`: non-destructive edits only while _not_ disabled; `Static` +
    `InteractionDisabled` = nothing.
  - A matched key is consumed even when the edit is dropped (`:171`).
    Unmatched keys (Tab; Enter on a single-line field) propagate (`:258-260`).
    Escape collapses, clears the focus and keeps bubbling (`:237-247`).
- Pointer (`on_pointer_press` `:270-333`, `on_pointer_drag` `:339-385`):
  primary button only; returns early for `Static` (`:299`, `:363`); **never
  checks `InteractionDisabled`**. It no longer focuses the field (`:289-297`).
- IME: skipped when `mode != Editable || disabled` (`:533`, `:598`);
  `window.ime_enabled` follows "focused and `Editable`" (`:624-650`).
- `on_focus_select_all` reads the `FocusCause` (`:693-713`) — only with
  `SelectAllOnFocus`, which we do not use.

`bevy_text-0.20.0/src/editing.rs:316-328`: `TextReadWriteMode { Editable
(default), ReadOnly, Static }`, a required component of `EditableText`
(`:100-108`). `apply_text_edits` does not look at it — the mode only gates
what the _widget_ queues, so our controlled `value` push
(`editor_mut().set_text`) keeps working in every mode.

`bevy_ui_render-0.20.0/src/text.rs`: the selection is drawn in
`selection_color` only when focused **and `Editable`**, else
`unfocused_selection_color` (`:103-107`); `Static` hides selection and caret
(`:109-112`, `:189-193`). The retained-extraction filter
(`lib.rs:450-477`) has `Changed<EditableText>` and `Changed<TextLayoutInfo>`
but **not** `Changed<TextReadWriteMode>`: a mode flip alone does not repaint.

### 1.4 `bevy_ui-0.20.0/src/interaction_states.rs`

- `InteractionDisabled` (`:13-31`): "more like `aria-disabled`" — it does
  **not** prevent focus, and "has no effect on the click-to-focus observer".
- Accessibility: `on_add_disabled` / `on_remove_disabled` (`:33-48`,
  registered in `lib.rs:226-227`) call `set_disabled()` /
  `clear_disabled()` on the entity's `AccessibilityNode` — if it has one when
  the marker is added.
- `Pressed`, `Checked`, `Selected`… are widget state markers; only the widget
  observers write them. Not used here.
- accesskit 0.24.1 has `set_read_only` / `clear_read_only`
  (`accesskit-0.24.1/src/lib.rs:1814`).
- `bevy_winit-0.20.0/src/accessibility.rs:217-223`: while the focused entity
  has **no `AccessibilityNode`**, the whole accessibility-tree update is
  skipped. A focusable React node needs one.

### 1.5 Audit corrections

1. "`InteractionDisabled`, which the text input honours" — only for the IME,
   and for keys only in `Static` mode. `disabled` needs the mode too (§1.3).
2. "Click-to-focus now works for any `TabIndex` entity" — only for a node that
   blocks and has nothing passing above it (§1.2).
3. "These nodes also need the blocking rule" — rejected (§1.2).
4. "The `FocusState` machinery is already generic" — the component and the
   observers are; `onFocus`/`onBlur` are `<editableText>` element events, and
   `FocusState` exists only next to a `focusStyle`.
5. Not in the audit: positive `TabIndex` sorts after `0`; the first-Tab
   warning; the a11y freeze without an `AccessibilityNode`; the missing
   `Changed<TextReadWriteMode>`; `FocusCause` is `Navigated` for clicks.

---

## 2. Design

New file: `crates/core/src/reconcile/focus.rs` (four small pieces, ~200 lines
plus tests), re-exported from `reconcile/mod.rs`.

### 2.1 Press → focus: `resolve_pointer_focus`

The rule is the click rule `collect_ui_events` already implements
("per pointer, the topmost hit that resolves to an owner"):

> A press focuses **the topmost pressed entity that has a focusable on itself
> or an ancestor**; if no pressed entity resolves, the focus clears.

It follows bevy-react's own pass-through model (`"pass"`: "nodes beneath still
see the pointer"): a decorative overlay over a field does not eat the focus,
the innermost of two nested focusables wins, a `<text>` label resolves to its
`<button>`, and an app's own bevy_ui widget beneath pass-through React UI
still gets focus.

```rust
/// `InputFocus` as it was before this frame's picking events ran.
#[derive(Resource, Default)]
pub(crate) struct FocusBeforePress(InputFocus);

/// PreUpdate, `.before(PickingSystems::Hover)`.
pub(crate) fn snapshot_focus(focus: Option<Res<InputFocus>>, mut before: ResMut<FocusBeforePress>) {
    if let Some(focus) = focus { before.0.clone_from(&focus); }
}

/// PreUpdate, `.after(PickingSystems::Hover)`.
pub(crate) fn resolve_pointer_focus(
    mut presses: MessageReader<PointerPress>,
    before: Res<FocusBeforePress>,
    focus: Option<ResMut<InputFocus>>,
    react: Query<(), With<ReactNode>>,
    focusable: Query<(), (With<TabIndex>, Without<InteractionDisabled>)>,
    child_of: Query<&ChildOf>,
    virtual_pointers: Option<Res<VirtualPointers>>,
    transform3d: Option<Res<crate::layer::pick3d::Transform3dPointer>>,
) {
    // Per pointer, in first-seen order: the shallowest pressed entity that
    // resolves, and whether the pointer pressed React UI at all.
    // ponytail: depth only, like the click rule — two UI cameras on one
    // window would need the hit's camera order too.
    struct Press { pointer: PointerId, ours: bool, best: Option<(f32, Entity, PointerButton)> }
    let mut seen: Vec<Press> = Vec::new();
    for ev in presses.read() {
        let p = /* find or push by ev.pointer.id */;
        p.ours |= react.contains(ev.entity);
        if let Some(target) = climb(ev.entity, &child_of, |e| focusable.contains(e))
            && p.best.is_none_or(|(d, ..)| ev.hit.depth < d)
        {
            p.best = Some((ev.hit.depth, target, ev.button));
        }
    }
    // Presses that never touched React UI are bevy's (the app's own widgets).
    if !seen.iter().any(|p| p.ours) { return; }
    let Some(mut focus) = focus else { return };
    // One physical click can press through two pointers (the mouse, and the
    // pointer remapped into a 3D-transformed layer or a <surface>): the
    // remapped one is on what the user sees, so it is asked first.
    let remapped = |id| virtual_pointers.as_ref().is_some_and(|v| v.contains(id))
        || transform3d.as_ref().is_some_and(|t| t.id == id);
    let target = seen.iter().filter(|p| remapped(p.pointer))
        .chain(seen.iter().filter(|p| !remapped(p.pointer)))
        .find_map(|p| p.best);
    // Bevy's pointer focus asked once per pressed entity, in hash order, and
    // every request that reached the window cleared the focus. Drop what it
    // recorded for this press, then record the one real change (if any).
    if *focus != before.0 {
        *focus.bypass_change_detection() = before.0.clone();
    }
    match target {
        Some((_, entity, button)) if focus.get() != Some(entity) => focus.set(
            entity,
            if button == PointerButton::Primary { FocusCause::Pressed } else { FocusCause::Navigated },
        ),
        None if focus.get().is_some() => focus.clear(),
        _ => {}
    }
}
```

- `climb` is `reconcile/events.rs::climb` (inclusive, already `pub(crate)`).
- Why restore rather than only write the right final value: the recorded
  flapping would still be replayed as `FocusLost`/`FocusGained`, and Bevy's
  text input collapses its selection on `FocusLost` (§1.2). After the restore
  a click into a focused field records nothing at all.
- Scope of the override: only frames in which a pointer pressed a React node.
  Any other `InputFocus` write made between the two systems on such a frame
  (an app observer on a picking event) is discarded too — see Risks.
- `InputFocusVisible` is untouched: `click_to_focus` already hid it.
- **`focus_pickable` is not extended to `tabIndex` nodes.** It keeps its
  `|| TextRole::Input` arm, but no longer for focus: a field consumes pointer
  gestures (press = caret, drag = selection), and those must not also drag or
  press what is beneath. Update its doc comment.
- So that the two presses of one click land in the same frame, order both
  pointer drivers after the input they read:
  `layer::pick3d::drive_transform3d_pointer` and
  `bevy_react_surface::drive_surface_pointer` gain
  `.after(bevy::input::InputSystems)` (both call `VirtualButtons::forward`,
  which reads `ButtonInput<MouseButton>::just_pressed`; today the order
  against the system that updates it is unspecified). Saves a frame of
  latency on those presses either way.

### 2.2 Focus state and events: `sync_focus`

Replaces `editable.rs::{on_focus_gained, on_focus_lost, set_focus_state}`
(deleted, with their two `add_observer` lines).

```rust
// bridge.rs
/// Whether a node with a `focusStyle`/`focusVisibleStyle` holds the input
/// focus, and whether the focus indicator shows. Mutated, never inserted or
/// removed on a focus change.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FocusState { pub focused: bool, pub visible: bool }
```

```rust
/// Update, `.in_set(InteractionSyncSet).after(ReactApplySet)`.
pub fn sync_focus(
    focus: Option<ResMut<InputFocus>>,
    ring: Option<Res<InputFocusVisible>>,
    out: Option<Res<OutboundResource>>,
    bridge: Option<Res<JsBridge>>,
    entities: &Entities,
    nodes: Query<(&ReactNode, Has<InteractionDisabled>, Has<EditableText>)>,
    mut states: Query<&mut FocusState>,
    mut last: Local<Option<Entity>>,
) {
    let Some(mut focus) = focus else { return };
    let mut current = focus.get();
    // A removed or disabled element cannot stay the active element.
    if let Some(e) = current
        && (!entities.contains(e) || nodes.get(e).is_ok_and(|(_, disabled, _)| disabled))
    {
        focus.clear();
        current = None;
    }
    if let Some(e) = current && let Ok(mut state) = states.get_mut(e) {
        // A text field shows its indicator however it was focused (CSS
        // `:focus-visible`); anything else only under keyboard navigation.
        let visible = ring.as_ref().is_some_and(|r| r.0) || nodes.get(e).is_ok_and(|(.., input)| input);
        state.set_if_neq(FocusState { focused: true, visible });
    }
    if *last == current { return; }
    let send = |e: Entity, event: &str, wants: fn(&Props) -> bool| {
        let (Ok((node, ..)), Some(out), Some(bridge)) = (nodes.get(e), &out, &bridge) else { return };
        if bridge.props_cache.get(&node.0).is_some_and(|p| wants(p)) {
            let _ = out.0.send(Outbound::ElementEvent {
                id: node.0, event: event.to_owned(), payload: serde_json::Value::Null,
            });
        }
    };
    if let Some(old) = *last {
        if let Ok(mut state) = states.get_mut(old) { state.set_if_neq(FocusState::default()); }
        send(old, "blur", |p| p.on_blur);
    }
    if let Some(new) = current { send(new, "focus", |p| p.on_focus); }
    *last = current;
}
```

What this buys over the observers, in one place:

- at most one `blur` and one `focus` per frame, **blur first** (the DOM
  order), whatever was recorded in between;
- `focusStyle` lands in the frame the focus moved (the system runs before the
  restyle; the observers ran in `PostUpdate`);
- a node that gains a `focusStyle` while focused gets it (the per-frame
  `get_mut(current)` is one lookup);
- a focused node that unmounts releases the focus at once — today
  `InputFocus` names a despawned entity until the next key press, and
  accessibility updates stall meanwhile (§1.1, §1.4);
- the `visible` bit follows `InputFocusVisible` even when the focused entity
  does not change (a Tab-focused button, then a click on it).

`Outbound::ElementEvent` with a `null` payload calls the handler with no
argument — exactly what `<editableText>`'s `onFocus`/`onBlur` receive today;
`js/src/bridge.ts` routes by node id + event name and needs no change.

`layer.rs::resolve_layer_repaints` reads `focus.0` → `focus.focused`.

### 2.3 `tabIndex`, `onFocus`, `onBlur`: the `Common::FOCUS` group

`element/decl.rs`:

```rust
/// `tabIndex` + `onFocus` / `onBlur`.
pub const FOCUS: Self = Self(1 << 5);
pub const ALL: Self = Self(0b111111);
```

Every element declared with `Common::ALL` gets it (`node`, `button`, `text`,
`image`, and the feature crates' elements that keep the default);
`EDITABLE_TEXT` becomes `Common::IDENTITY.with(Common::VARIANTS).with(Common::FOCUS)`
and its `events` shrink to `&[&CHANGE, &SELECT]` — required, because
`ElementInfo::build` panics when an element event's handler prop collides with
a common prop. `editable::{FOCUS, BLUR}` are deleted. `<root>`, `<surface>`,
text spans and SVG shapes stay out (a `tabIndex` there warns `propIgnored`,
the existing mechanism).

`protocol/props.rs`:

- fields `tab_index: Option<i32>`, `on_focus: bool`, `on_blur: bool`;
- `Key::{TabIndex, OnFocus, OnBlur}`, three `common_key` rows with
  `Some(Common::FOCUS)`;
- decode `tab_index` leniently —
  `map.next_value::<Option<f32>>()?.map(|v| v as i32)` — a fractional value
  from JS must not fail the whole op batch;
- `PropsDirty::focus: bool`.

`protocol/merge.rs`: `merge_option!(tab_index => focus)`,
`merge_bool!(on_focus => focus, on_blur => focus)`, three `unset_match!` rows.

`reconcile/stamps.rs`:

```rust
/// `tabIndex`, else the element's own default (a text field is focusable);
/// nothing while disabled.
fn tab_index(props: &Props, flags: ElementFlags) -> Option<i32> {
    if props.attrs.get(&crate::elements::DISABLED).copied().unwrap_or(false) { return None; }   // from commit 4
    props.tab_index.or((flags.text == TextRole::Input).then_some(0))
}

pub(crate) fn apply_focus(ec: &mut EntityCommands, props: &Props, flags: ElementFlags, fresh: bool) {
    match tab_index(props, flags) {
        Some(n) => {
            ec.insert(TabIndex(n));
            // bevy_winit skips accessibility updates while the focused
            // entity has no node. An element's own node (a button's, a
            // field's) is already there.
            // ponytail: left behind if `tabIndex` is removed later.
            ec.insert_if_new(AccessibilityNode(accesskit::Node::new(accesskit::Role::GenericContainer)));
        }
        None if !fresh => { ec.remove::<TabIndex>(); }
        None => {}
    }
}
```

- `stamp_common`: `if common.contains(Common::FOCUS) { apply_focus(ec, props, flags, true) }`.
  A node without `tabIndex` queues nothing on create.
- `reconcile/update.rs`, beside the other group stamps:
  `if common.contains(Common::FOCUS) && (dirty.focus || disabled_dirty) { apply_focus(.., false) }`
  where
  `disabled_dirty = info.attr("disabled").is_some_and(|(i, _)| dirty.attrs.0 & (1 << i) != 0)`.
- `spawn_editable` drops its `TabIndex(0)` (the stamp owns it now).
- `TabIndex` becomes **bridge-owned** on elements with the group: fix the
  list in `bridge.rs`'s `ReactNode` rustdoc and in
  `docs/guide/communication/named-nodes.md` (both name `TabIndex` as "yours").

`onFocus`/`onBlur` need no stamp and no component: `sync_focus` reads the two
flags from `bridge.props_cache`.

TypeScript: `js/src/jsx.d.ts` gains

```ts
/** Keyboard focus (the `FOCUS` common group). */
export interface BevyFocusProps {
  tabIndex?: number;
  onFocus?: () => void;
  onBlur?: () => void;
}
```

and `element/ts.rs` one row `(Common::FOCUS, "BevyFocusProps")`. Regenerate
`js/src/generated/elements.ts` (`BevyEditableTextProps` loses its own
`onFocus`/`onBlur` lines and extends the group; the others gain it).

Devtools: `devtools/element_fields.rs` one row `(Common::FOCUS, "focus")` (its
two tests list the groups), `js/src/devtools/api.ts` doc line,
`js/src/devtools/fields.ts` `COMMON_EDITABLE.focus = [["tabIndex", "number"]]`.
No new diag kind → `KIND_FIELDS` unchanged.

### 2.4 Tab navigation: groups from us, the plugin from the app

bevy-react does **not** add `TabNavigationPlugin` and gets no plugin knob
(D5). It only makes the app's one line sufficient by marking its roots:

| Root                   | Where                                                      | Component           |
| ---------------------- | ---------------------------------------------------------- | ------------------- |
| The window tree's root | `plugin.rs` setup (the `UiRoot` bundle)                    | `TabGroup::new(0)`  |
| `<root>`               | `elements/node.rs` `ROOT.spawn`                            | `TabGroup::new(1)`  |
| The anchor layer       | `crates/anchor/src/position.rs` (the `AnchorLayer` bundle) | `TabGroup::new(1)`  |
| `<surface>`            | `crates/surface/src/element.rs::spawn_surface`             | `TabGroup::modal()` |

Tab then walks the window tree, then overlays; a `<surface>` is its own
scope: Tab never enters it from the screen, and cycles inside it once a click
put the focus there. Without the plugin the four components do nothing. The
`anchor` and `surface` crates add `"bevy_input_focus"` to their bevy feature
lists (repo convention; `cargo hack` builds each alone).

**Focus inside a `<surface>`, in one place.** Today: a click on a field in a
surface does focus it (`click_to_focus` reacts to any pointer's press, and
`InputFocus` and the key dispatch are global), and typing works — but the same
physical click also presses the window through the real mouse, so each click
into an already focused field blurs and refocuses it (§1.2, by reading).
After commits 1–2: the remapped pointer's press decides and the pre-press
focus is restored, so there is no pair; a `tabIndex` node inside a surface
focuses the same way, gets `focusStyle`/`onFocus`/`onBlur`, and Enter/Space
click a focused `<button>` there. Tab moves among the surface's own
focusables only. The same holds inside a 3D-transformed layer (which stays in
its tree's tab group). Not covered: the IME candidate window is positioned
for the primary window, not for a point on a mesh.

### 2.5 Enter and Space click a focused `<button>`

```rust
/// A focused `<button>` clicks on Enter or Space (bevy's widget `Button`
/// test: a press, not a repeat). Bevy's own `Activate` also fires for
/// pointer clicks, which `collect_ui_events` already reports.
pub(crate) fn click_focused_button(
    ev: On<FocusedInput<KeyboardInput>>,
    buttons: Query<&ReactNode, (With<bevy::ui_widgets::Button>, Without<InteractionDisabled>)>,
    out: Option<Res<OutboundResource>>,
) {
    let key = &ev.input;
    if ev.focused_entity != ev.original_event_target()          // it bubbles
        || key.repeat || key.state != ButtonState::Pressed
        || !matches!(key.key_code, KeyCode::Enter | KeyCode::Space)
    { return; }
    if let (Ok(node), Some(out)) = (buttons.get(ev.focused_entity), out) {
        let _ = out.0.send(Outbound::UiEvent {
            event: UiEvent { id: node.0, kind: "click".into(), ..default() },
        });
    }
}
```

Same wire event as a pointer click (`onClick` takes no argument). The global
`keyDown` still fires for the key (`keyboard.rs` reads raw input).

### 2.6 `readOnly` and `disabled` on `<editableText>`

`elements/editable.rs`:

```rust
/// The text can be focused, selected and copied, not changed by the user.
pub static READ_ONLY: Attribute<bool> = Attribute {
    invalidate: Invalidation::PAINT,
    ..Attribute::new("readOnly")
};

fn mode(attrs: &Attrs) -> TextReadWriteMode {
    let on = |a: &'static Attribute<bool>| attrs.get(a).copied().unwrap_or(false);
    if on(&DISABLED) { TextReadWriteMode::Static }            // bevy's key gate needs the mode too
    else if on(&READ_ONLY) { TextReadWriteMode::ReadOnly }
    else { TextReadWriteMode::Editable }
}
```

- `EDITABLE_TEXT.attrs` += `&READ_ONLY, &DISABLED`; `writers` =
  `&[&EDITABLE_WRITER, &DISABLED_WRITER]`.
- `spawn_editable`: `mode(attrs)` joins the bundle; `a11y_node` takes
  `read_only` and calls `set_read_only()`.
- `EDITABLE_WRITER`: `attrs` += `&READ_ONLY, &DISABLED`, `writes` +=
  `owns::<TextReadWriteMode>`; in the existing queued closure:

  ```rust
  if entity.get::<TextReadWriteMode>() != Some(&mode) {
      entity.insert(mode);
      // bevy re-extracts a text node on `Changed<EditableText>`, not on a mode change
      if let Some(mut editable) = entity.get_mut::<EditableText>() { editable.set_changed(); }
  }
  // …and next to the label: set_read_only() / clear_read_only()
  ```

The shared half, in `elements/node.rs` (exported from `elements/mod.rs`):

```rust
/// The control is inert: no focus, no click, no edits.
pub static DISABLED: Attribute<bool> = Attribute {
    invalidate: Invalidation::PAINT,
    ..Attribute::new("disabled")
};

/// `disabled` → bevy_ui's `InteractionDisabled` (which also marks the
/// accessibility node disabled).
pub static DISABLED_WRITER: Writer = Writer {
    reads: &[],
    attrs: &[&DISABLED],
    writes: &[owns::<InteractionDisabled>],
    apply: |ctx, _s, ec| {
        let want = ctx.attr(&DISABLED).copied().unwrap_or(false);
        if ctx.fresh { if want { ec.insert(InteractionDisabled); } return; }
        // Compare first: a full restyle re-runs every element writer.
        ec.queue(move |mut entity: EntityWorldMut| {
            if entity.contains::<InteractionDisabled>() == want { return; }
            if want { entity.insert(InteractionDisabled); } else { entity.remove::<InteractionDisabled>(); }
        });
    },
};
```

What `disabled` means, on both elements (D8, D9):

| Aspect                            | Behavior                                                                                                        | Mechanism                                                                                                                                                          |
| --------------------------------- | --------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Focus                             | not focusable by click, Tab, or while already focused                                                           | no `TabIndex` (`stamps.rs::tab_index`); `resolve_pointer_focus` filters `Without<InteractionDisabled>`; `sync_focus` clears a focused disabled node                |
| Typing, caret, selection (field)  | none; caret and selection hidden                                                                                | `Static` + the marker (§1.3)                                                                                                                                       |
| `focusStyle`, `focusVisibleStyle` | never apply                                                                                                     | follows from "not focusable"                                                                                                                                       |
| `pressStyle`                      | never applies                                                                                                   | `sync_interactions` skips disabled entities in its `pressing` loop — one `Query<(), With<InteractionDisabled>>` and one condition; the node reads `Hovered`/`None` |
| `hoverStyle`, `onPointer*`        | unchanged (CSS `:hover` still matches a disabled control; a tooltip on a disabled button needs the enter/leave) | —                                                                                                                                                                  |
| Accessibility                     | disabled                                                                                                        | bevy_ui's `on_add_disabled`/`on_remove_disabled`                                                                                                                   |
| The controlled `value`            | still applies                                                                                                   | `apply_text_edits` ignores the mode                                                                                                                                |

`readOnly`: focusable and in the Tab order, caret moves, selection, copy; no
edits and no IME; `onChange` never fires from the user, `onSelect` still does;
`focusStyle` applies. Bevy draws a read-only field's selection in
`TextCursorStyle::unfocused_selection_color` even while focused.

No `disabledStyle` (D8).

### 2.7 `disabled` on `<button>`

- `BUTTON.attrs = &[&DISABLED]`, `BUTTON.writers = &[&DISABLED_WRITER]`.
- `reconcile/events.rs::collect_ui_events`: a disabled owner still **owns**
  the click and drops it —

  ```rust
  for (_, (_, target)) in topmost {
      if disabled.contains(target) { continue; }   // swallowed, like a DOM disabled button
      …
  }
  ```

  with `disabled: Query<(), With<InteractionDisabled>>`. An ancestor's
  `onClick` does not fire (today's documented workaround, "drop its `onClick`
  — it still swallows the click", becomes the attribute).

- Enter/Space: already excluded by `click_focused_button`'s filter, and the
  button cannot hold the focus.
- Everything else from the table in §2.6.

### 2.8 `focusVisibleStyle`

A fifth style slot, overlaid after `focusStyle`:

- `Props::focus_visible_style`, `Key::FocusVisibleStyle`
  (`Common::VARIANTS`), `PropsDirty::focus_visible_style`,
  `PropsDirty::any_style_variant`.
- `merge.rs`: the set arm, the `"focusVisibleStyle"` unset row, and
  **`Props::all_styles()`** — the one definition of "every style slot", so
  layer promotion's union and the create-time layer seed follow.
- `bridge.rs`: `StyleVariants::focus_visible: Option<Style>`.
- `stamps.rs`: `apply_style_variants` (presence test, the field, and
  `FocusState` inserted when **either** focus variant is present),
  `variant_keys`; `update.rs`: the two variant-presence tests
  (`old_variant_keys` fallback, `layer_dirty`);
  `style_bindings.rs::warn_variant_bindings` one row.
- `interaction.rs::apply_interaction_styles`:

  ```rust
  let state = focus.as_deref().copied().unwrap_or_default();
  if state.focused { style = overlay_style(style.as_ref(), variants.focus.as_ref()); }
  if state.visible { style = overlay_style(style.as_ref(), variants.focus_visible.as_ref()); }
  ```

  (`focused` in the idle-skip test becomes `state.focused`.)

- JS: `BevyVariantProps.focusVisibleStyle` in `jsx.d.ts`;
  `devtools/warnings.ts` `VARIANT_STYLE_PROPS`; the comment in `animated.ts`.

`visible` is computed by `sync_focus` (§2.2): `InputFocusVisible`, or always
for a text field. Tab sets the flag, any pointer press clears it (§1.1) — so
without `TabNavigationPlugin` (or app code writing `InputFocusVisible`, e.g. a
gamepad menu) it only ever shows on fields. Documented.

### 2.9 System ordering, and what differs before the `Interaction` plan lands

`PreUpdate`

- `snapshot_focus.before(PickingSystems::Hover)`
- `resolve_pointer_focus.after(PickingSystems::Hover)` (the ordering edge
  gives the sync point that applies the picking observers' commands)
- the two pointer drivers: `.after(bevy::input::InputSystems)` added to their
  existing `.before(PickingSystems::ProcessInput)`

`Update`

- `sync_focus.in_set(InteractionSyncSet).after(ReactApplySet)` — after the op
  drain (a same-commit `autofocus` and a freshly stamped `FocusState` are
  visible), before the restyle and the collectors.

Observer: `click_focused_button`. Resource: `FocusBeforePress`.

If this is implemented **before** `interaction-and-capture.md`:

- the press mask (§2.6) has no `sync_interactions` to live in: mask in the
  restyle instead — `Has<InteractionDisabled>` in both
  `apply_interaction_styles` and `virtual_events.rs::
apply_virtual_interaction_styles`, `Pressed → Hovered` when set — and have
  `DISABLED_WRITER` poke `StyleVariants` (`set_changed()`) when the marker
  flips, so the restyle runs;
- the focus-visible overlay (§2.8) and the disabled click swallow (§2.7) need
  their twins in `virtual_events.rs` (`apply_virtual_interaction_styles`,
  `collect_virtual_clicks`);
- tests insert `Interaction` where this plan says `ReactInteraction`.

Nothing else depends on it: `resolve_pointer_focus` reads `VirtualPointers`
and `Transform3dPointer` directly.

### 2.10 What is deleted

- `editable.rs`: `FOCUS`, `BLUR`, `on_focus_gained`, `on_focus_lost`,
  `set_focus_state`, the `TabIndex(0)` in the spawn, and the `FocusGained`/
  `FocusLost` imports; `plugin.rs`: the two `add_observer` lines.
- Guide limits that stop being true (§5).

---

## 3. Edge cases and tests

Harnesses: bare `World` + `run_system_once` for the resolver; `reconcile::
test_util::{op_app_with, create, update, ent}` with `observe_outbound` (as in
`element/apply_tests.rs`) for stamps, `sync_focus` and the key click; the
plugin list of `apply_tests.rs::typed_keys_reach_a_focused_editable` for
anything that types; `reconcile::clip_pick_tests::clip_scene`'s setup (real
layout, `ui_picking`, `PickingPlugin` + `InteractionPlugin`, a mouse pointer on
a headless window) for the one end-to-end pin against stock picking.

`reconcile/focus.rs` — resolver (write `PointerPress` messages by hand; run
`snapshot_focus`, mimic bevy with `focus.set(..)`/`focus.clear()`, run
`resolve_pointer_focus`, then `process_recorded_focus_changes` with counting
`FocusGained`/`FocusLost` observers):

1. `the_topmost_resolving_press_wins` — leaf (depth 0) under a focusable
   child under a focusable parent, plus an unrelated non-resolving entity →
   focus = the child; exactly one `FocusGained`, no `FocusLost`.
2. `a_press_on_the_focused_node_records_nothing` — focus already there,
   mimic clear + set → `InputFocus == snapshot`; no events.
3. `a_press_that_resolves_nowhere_clears` — a React node, nothing focusable.
4. `a_node_beneath_a_passing_overlay_takes_the_focus` — shallower React
   overlay (no focusable ancestor), deeper focusable.
5. `presses_off_react_ui_are_left_to_bevy` — only non-React entities pressed:
   the mimicked state stands.
6. `a_remapped_pointer_is_asked_first` — mouse resolves to A, a registered
   virtual pointer to B → B; mouse resolves, virtual does not → A.
7. `a_disabled_node_is_skipped` — pressed disabled focusable inside a
   focusable ancestor → the ancestor.
8. `a_pass_through_focusable_keeps_its_focus` (the `clip_scene` setup +
   `InputFocusPlugin` + `PointerFocusPlugin` + the two systems): a
   `TabIndex` + `ReactNode` node with a passing `Pickable` inside a passing
   parent; write `PointerInput::new(PointerId::Mouse, .., Press(Primary))`
   twice → focused both times, zero `FocusLost`. Pins the override against
   the real stock chain; fails loudly if 0.21 moves it.

`reconcile/focus.rs` — `sync_focus`: 9. `blur_then_focus_once_per_frame` — two nodes with `onFocus`/`onBlur`;
A → B gives `blur(A)`, `focus(B)` in that order; A → B → A in one frame
gives nothing; a node without the handler gets nothing. 10. `focus_style_lands_the_same_frame` — with `apply_interaction_styles`
ordered after it: one `update()` after setting `InputFocus`. 11. `a_focus_style_added_while_focused_applies`. 12. `an_unmounted_focused_node_releases_the_focus` — `Op::Remove` →
`InputFocus::get() == None`, no `blur` sent. 13. `the_indicator_follows_input_focus_visible` — `visible` false after a
pointer focus, true with `InputFocusVisible(true)`, back to false when the
flag drops while the same node stays focused; always true on an
`<editableText>`.

Props / stamps (`protocol/merge.rs`, `reconcile/stamps.rs` tests): 14. `tab_index_stamps_and_unsets` — create with `tabIndex: 0` → `TabIndex(0)`
and an `AccessibilityNode`; `-1` kept; `unset` removes it; a `<node>`
without it has neither. 15. `an_input_is_focusable_by_default` — replaces the `TabIndex` assertion of
`editable_is_a_focusable_text_input`; `tabIndex: -1` overrides. 16. `a_fractional_tab_index_decodes` (`1.5` → `1`, no error);
`tab_index_on_a_root_warns_prop_ignored`. 17. `merge_delta_focus_props_set_and_unset` (the `name` test's shape).

Keyboard (the `typed_keys…` harness; `TabNavigationPlugin` where noted; spawn
the primary window before the first `update()` — the Tab observer attaches at
`Startup`): 18. `enter_and_space_click_a_focused_button` — `UiEvent { kind: "click" }`
once per key; none for a repeat, a release, a `<node tabIndex onClick>`,
or a disabled button. 19. `tab_walks_the_focusables` (+ `TabNavigationPlugin`, a `TabGroup` on the
harness root): three `tabIndex: 0` nodes and one `-1` → tree order,
skipping `-1` and a disabled one; `InputFocusVisible` is true afterwards. 20. Root markers: the plugin's UI root, a `<root>` and (surface crate op
test) a `<surface>` carry their `TabGroup` (`modal` on the surface).

`<editableText>` modes (`element/apply_tests.rs`): 21. `read_only_and_disabled_set_the_text_mode` — `readOnly` → `ReadOnly`;
`disabled` → `Static` + `InteractionDisabled`, no `TabIndex`; both →
`Static`; unset → `Editable`, `TabIndex` back; the a11y node
`is_read_only()`. 22. `a_read_only_field_keeps_its_text` — type `a` → nothing destructive
queued; `ArrowRight` still queues `TextEdit::Right`; a `value` update
still replaces the text. 23. `a_disabled_field_ignores_keys` — forced `InputFocus::from_entity` + a
key → `pending_edits` empty; after `sync_focus` the focus is gone.

`disabled` (`reconcile/events.rs`, `reconcile/picking.rs`): 24. `a_disabled_button_swallows_its_click` — disabled `<button>` inside a
`<node onClick>`, `PointerClick` on the button → no event at all. 25. `a_disabled_node_is_never_pressed` — in `PointerState.pressing` and the
hover map → `Hovered`; in `pressing` only → `None`.

`focusVisibleStyle` (`reconcile/interaction.rs`, `layer` tests): 26. `the_focus_visible_overlay_wins` — `FocusState { focused: true, visible:
    false }` → `focusStyle` only; `visible: true` → its property overrides. 27. Promotion union: add `"focusVisibleStyle"` to the three
`for variant in ["hoverStyle", "pressStyle", "focusStyle"]` loops in
`layer.rs`'s tests; and the `styleBinding` warning for a binding inside
it (`style_bindings.rs` tests).

Generated / tables: `element::ts::tests::core_element_ts_is_current`,
`crates/bevy-react/tests/library_ts.rs`, the two
`devtools/element_fields.rs` tests (`"focus"` in the group lists).

Not testable headless — by hand: Tab through the Focus demo page (ring on
Tab, none on click, always on the field); a double-click word selection in
the `<surface>` demo's field and on a 3D-transformed card; a screen reader
announcing a disabled button. The `on_add_disabled` observers are
`pub(crate)` in bevy*ui, so the a11y \_disabled* bit is only reachable in a
full-plugin test (skip, or one assert in a demos bundle test).

---

## 4. Commit sequence

The task lists the features as 1 `readOnly`/`disabled` on the field, 2
`disabled` on the button, 3 `tabIndex`, 4 `focusVisibleStyle`. The order below
puts focus first: "disabled = not focusable" lives in the focus stamp and
`sync_focus`, so doing it earlier would mean interim code deleted two commits
later. Each commit builds, passes `cargo test` (workspace, the unported
examples excluded as in `progress.md`), `cargo clippy --workspace
--all-targets`, `cargo hack check -p bevy-react --each-feature` and
`npm run lint`, and carries its own guide/demos/CHANGELOG lines.

1. **`fix(focus) a press focuses the topmost node that takes it`** — §2.1:
   `reconcile/focus.rs` with `FocusBeforePress`, `snapshot_focus`,
   `resolve_pointer_focus`; the two driver edges; `focus_pickable`'s doc.
   Tests 1–8. Stands alone with today's only focusable (`<editableText>`):
   no blur/focus pair per click in a `<surface>` or a transformed layer, no
   race under a passing overlay.
2. **`feat(focus) tabIndex, onFocus and onBlur on every element`** — §2.2,
   §2.3, §2.4: `Common::FOCUS`, the props, `apply_focus`, `sync_focus`
   (observers deleted), `FocusState { focused, visible }` (`visible` computed
   already, unused until commit 6), the four `TabGroup`s, TS + devtools
   tables, regenerated `elements.ts` / `docs/reference/bevy.ts` / demos
   `bevy.ts`. Tests 9–17, 19–20. The new guide page and demos page.
3. **`feat(button) Enter and Space click a focused button`** — §2.5. Test 18.
4. **`feat(editableText) readOnly and disabled`** — §2.6: `READ_ONLY`,
   `DISABLED`, `DISABLED_WRITER`, the mode, the `tab_index` disabled arm, the
   press mask. Tests 21–23, 25. Regenerate `elements.ts`.
5. **`feat(button) disabled`** — §2.7. Test 24. Regenerate `elements.ts`.
6. **`feat(style) focusVisibleStyle`** — §2.8. Tests 26–27 (13 already
   covers the bit).

---

## 5. Demos and docs

**Demos** (`examples/demos`)

- `main.rs`: `.add_plugins(bevy::input_focus::tab_navigation::
TabNavigationPlugin)` (commit 2) — the gallery is where Tab is shown.
- New page **"Focus"** in the Events section of `ui/src/demos.ts`, after
  "Keyboard": `ui/src/demos/events/FocusDemo.tsx`. Three examples (snippet
  lines ≤ 40 chars):
  1. _Tab order_ — a row of plain `<button tabIndex={0}>`s, one
     `tabIndex={-1}`, and an `<editableText>`; a readout fed by
     `onFocus`/`onBlur`. `focusStyle` tints the focused one (commit 2);
     `focusVisibleStyle` adds the ring only under Tab (commit 6).
  2. _Keyboard activation_ — a focused button counting Enter/Space clicks
     (commit 3).
  3. _Disabled_ — a checkbox toggling `disabled` on a button and a field in
     the middle of the row: Tab skips them (commits 4–5).
- `ui/src/demos/elements/EditableTextDemo.tsx` ("<editableText>"): a second
  example, _Modes_ — the same text in an editable, a `readOnly` and a
  `disabled` field (commit 4). Fix the page copy "onBlur for the old field
  fires AFTER onFocus for the new one": the order is now blur, then focus
  (the guard shown stays correct).
- `ui/src/demos/elements/ButtonDemo.tsx` ("<button>"): the basic example
  gains a `disabled` toggle (commit 5) and `tabIndex={0}` + a
  `focusVisibleStyle` (commits 2, 6).
- `ui/src/components/Button.tsx`: forward `tabIndex` and `disabled` to the
  inner `<button>`. The inner button is pass-through inside `Pinchable` — the
  case §2.1 exists for. Check with `--shoot` that a ring drawn with `outline`
  is not cut by the press surface's layer capture; if it is, use
  `borderColor` for the ring there.
- `screenshots/screenshots.json`: optional `focus` shot (the page with a
  field focused by a scripted press).

**Guide** (`docs/guide`)

- New `events/focus.md` (`demo: Focus`; no `covers` — common props are not
  reference keys) and its `SUMMARY.md` line after Keyboard. Contents: what is
  focusable (`tabIndex`; fields by default; buttons only with it — D3); the
  press rule of §2.1 in one paragraph; `onFocus`/`onBlur` (no bubbling, blur
  first, at most once per frame); `focusStyle` vs `focusVisibleStyle`; Tab
  navigation — the one `add_plugins` line (TSX/Rust tab pair), the order
  (tree order; positive values come _after_ `0`, unlike HTML — use `0` and
  `-1`), the scopes (`<root>` after the window tree, a `<surface>` on its
  own), Bevy's one warning on the first Tab and its warning per Tab when
  nothing is focusable; Enter/Space; focus from Rust (`InputFocus`,
  `InputFocusVisible` for a gamepad menu); limits (§6 D16).
- `elements/editable-text.md`: attribute rows `readOnly`, `disabled`,
  `tabIndex` (default `0`); _Focus and keyboard_ (blur-then-focus order —
  drop the "don't rely on the order" bullet; Tab with the plugin); _Limits_:
  drop "read-only, disabled" and rewrite the Tab bullet.
- `elements/button.md`: sections _Disabled_ and _Keyboard_; the comparison
  table gets a "Focusable" row; _Limits_ loses the first two bullets.
- `elements/node.md`: the common-props table (`tabIndex`,
  `onFocus`/`onBlur`, `focusVisibleStyle`); _State styles_ — the overlay
  order ends "…`focusStyle`, then `focusVisibleStyle`", and the last bullet
  ("Only `<editableText>` takes focus on its own") is rewritten.
- `events/keyboard.md`: _Text input and focus_ — Tab and Enter/Space are
  also delivered as `keyDown`; a link to Focus.
- `styling/focus-policy.md`: the `<editableText>` paragraph's reason (a field
  consumes the press and the drag; focus no longer depends on it).
- `elements/surface.md`, `elements/root.md`: one line each on the Tab scope;
  surface: clicking a field or a `tabIndex` node focuses it.
- `communication/named-nodes.md`: `TabIndex` moves from "yours" to the
  bridge-owned list.
- `extending/custom-elements.md`: the `Common::FOCUS` row; `tabIndex` is a
  reserved attribute name, `focus`/`blur` reserved event names.
- `docs/src/reference.ts`: `COMMON_GROUPS` += `["BevyFocusProps", "Focus"]`;
  the style-properties intro names `focusVisibleStyle`.

**Generated**: `npm run elements:generate` (commits 2, 4, 5),
`npm run reference:generate` and `npm run bevy:generate -w demos` (commit 2;
the group list changes every element's `extends`). No `style.ts` change. The
other examples' `bevy.ts` regenerate in the examples phase.

**CHANGELOG (`## [Unreleased]`)**

- _Added_: **`tabIndex`, `onFocus` and `onBlur` on every element** — any
  element can take the keyboard focus by click; `focusStyle` applies to it.
  **Tab navigation** with one line (`TabNavigationPlugin`); bevy-react marks
  its roots as tab groups. **Enter and Space click a focused `<button>`.**
  **`focusVisibleStyle`.** **`<editableText readOnly>` and `disabled`**,
  **`<button disabled>`**.
- _Changed_: `onBlur` fires before the next element's `onFocus`, each at most
  once per frame; `focusStyle` applies in the frame the focus moves. An
  element that unmounts or becomes disabled gives the focus up. `TabIndex` on
  a React entity is bridge-owned. Edit the 0.20 `<editableText>` entry: a
  field still keeps picks from what is beneath it, now because it consumes
  pointer gestures. Rust API (breaking): `elements::editable::{FOCUS, BLUR,
on_focus_gained, on_focus_lost}` are gone, `FocusState` is `{ focused,
visible }`, `Common::ALL` includes `FOCUS`, and an element may no longer
  declare an attribute named `tabIndex` or an event named `focus`/`blur`.
- _Fixed_ (if test 8 / the manual check confirm): clicking inside a focused
  field in a `<surface>` or a 3D-transformed layer blurred and refocused it,
  collapsing the selection; a click on a field under a pass-through overlay
  could blur it.

**Other text**: CLAUDE.md — the element-registry bullet listing the common
props, a line for `reconcile/focus.rs` (the press rule; `sync_focus`), the
`<editableText>`/`<button>` mentions. `CONTEXT.md` — "Common prop" and "Style
variant" entries; a new **Focusable** entry (an element with a `tabIndex`, or
a text field; never a disabled one). `bridge.rs` `ReactNode` rustdoc.
`ext/mod.rs` `InteractionSyncSet` doc (the core's `sync_focus` runs there).

---

## 6. Decisions

- **D1 — Who resolves a press.** bevy-react, with the click rule (§2.1),
  overriding what Bevy's pointer focus recorded for that press. The
  alternative — "focusable nodes block lower picks" — changes hit-testing
  (unlike the web), breaks pass-through buttons inside press surfaces, and
  still loses under an overlay or a second pointer (§1.2). Fallback if the
  snapshot is judged too invasive: keep the stock chain, skip the restore and
  only write the resolved target; then fields keep their blocking arm _for
  focus_ and the surface/transformed-layer pair stays.
- **D2 — Text fields keep blocking the picks beneath them.** A field
  consumes press and drag; an ancestor with `onPointerDown` must not move
  while the user selects text. No behavior change, a new reason.
- **D3 — `<button>` is not focusable unless it has a `tabIndex`.** The web
  makes buttons focusable by default, but there a clicked button then takes
  Space and Enter — in a game those keys are bound to gameplay, and every
  existing app would change. `<editableText>` defaults to `0`.
- **D4 — `tabIndex` is Bevy's `TabIndex` unchanged**: `0` = in tree order,
  negative = click/Rust only, positive = _after_ `0` in ascending order.
  Document the HTML difference; recommend `0` and `-1`. No remapping layer.
- **D5 — Tab navigation is the app's opt-in; no `ReactUiPlugin` knob.** The
  plugin `warn!`s on every Tab press when nothing is focusable and games bind
  Tab; a knob would wrap one `add_plugins` call. bevy-react ships the
  `TabGroup`s: window tree `0`, `<root>` and anchors `1`, `<surface>` modal.
- **D6 — Keyboard activation: `<button>` only**, on the key press, not on
  repeats (Bevy's rule; the web clicks Space on release). A `<node tabIndex
onClick>` is not activated — as on the web. Through our own observer, not
  `Activate` (it also fires for pointer clicks).
- **D7 — `onFocus`/`onBlur` are common props without a payload, and do not
  bubble** (React's bubble; ours follow the rest of the event model, where
  nothing bubbles). Blur, then focus.
- **D8 — `disabled` exists on `<button>` and `<editableText>` only** (HTML:
  form controls). No `disabledStyle`: unlike hover or focus, `disabled` is a
  prop React already holds — `style={disabled ? dim : base}` costs no round
  trip, and a sixth variant slot costs every node.
- **D9 — A disabled element keeps `hoverStyle` and its `onPointer*` events,
  and loses `pressStyle`, focus and its click.** CSS: `:hover` matches a
  disabled control, `:active` and `:focus` do not. Keeping hover is also the
  expressive choice: dropping it is a JSX ternary, adding it back would need
  a round trip. The click is swallowed, not passed to an ancestor.
- **D10 — A disabled field is `Static`**: no caret, no selection, no copy —
  what browsers do for a disabled input. A read-only field is the way to keep
  the text selectable.
- **D11 — `focusVisibleStyle` follows `InputFocusVisible`, and is always on
  for a focused text field** (CSS `:focus-visible`). It stacks on top of
  `focusStyle`. Without keyboard navigation it never shows elsewhere — by
  design, documented.
- **D12 — The override covers every press that touched a React node**,
  including the choice among non-React entities in the same stack (an app's
  own `TabIndex` widget beneath pass-through React UI is focused by the same
  rule). Presses that touch no React node are left to Bevy.
- **D13 — A focusable gets an `AccessibilityNode`** (`GenericContainer`)
  when it has none. Needed for bevy_winit to keep updating the tree; a real
  role/label API for non-button focusables is out of scope.
- **D14 — A new demos page and a new guide page ("Focus")** rather than
  spreading the topic over `<button>`, `<editableText>` and Keyboard.
- **D15 — Commit order**: focus before `disabled` (§4).
- **D16 — Not in this plan** (say so in the guide's limits): `autofocus` on
  other elements, an imperative `focus()`/`blur()` from JS, per-element key
  handlers, a focus-trap prop for dialogs (a modal `TabGroup`), arrow-key or
  gamepad navigation (`bevy_input_focus::directional_navigation`),
  `ariaLabel` beyond `<editableText>`, `disabled` on other elements.

---

## 7. Risks and perf

**Per-frame cost.** `snapshot_focus`: one `InputFocus` clone (two
`Option<Entity>` and an almost always empty `Vec` — no allocation).
`sync_focus`: one lookup. `resolve_pointer_focus`: nothing without a press;
on a press, O(pressed stack × ancestor depth). Create path: one branch per
node, nothing queued without a `tabIndex` — the 10k-row table benchmark
(`<button onClick>`) must not move. `Props` grows by an `Option<i32>`, two
bools and one `Option<Style>` handle per cached node; `StyleVariants` by one
handle. If table-ops regresses, look first at `apply_focus` queuing a remove
for nodes without the prop (the `fresh`/`dirty.focus` gates), then at the
five new arms in `common_key` (the decode hot path).

**The override is invasive by design.** On a frame where React UI was
pressed, every `InputFocus` write between the two systems is replaced — also
one made by an app observer on a picking event, or by a widget that
intercepts `AcquireFocus` (feathers' number scrubber focuses on release; under
pass-through React UI it would now focus on press). It leans on `InputFocus`
being `Clone + PartialEq` and on `PointerPress` messages; Bevy calls this
pathway a temporary bridge, so expect to revisit it on 0.21 — test 8 is the
tripwire.

**Unverified by running** — check these first:

- the blur/focus pair in a `<surface>` / transformed layer today, and that
  the fix removes it (needs both presses in one frame: the `InputSystems`
  edge; if the two still land a frame apart, the pair remains);
- that the new `PreUpdate`/`Update` edges introduce no schedule cycle (any
  full-plugin test — the demos bundle tests — fails at startup if they do);
- `set_changed()` on `EditableText` being enough to repaint a mode flip
  (§1.3). It cannot echo to JS: `TextEditChange` is triggered by
  `apply_text_edits` (`bevy_text` `editing.rs:366`), not by change
  detection; the only other reader is our `sync_editable_a11y`;
- `On<FocusedInput<KeyboardInput>>::original_event_target()` (the same
  accessor `editable.rs` uses on `FocusGained` today);
- the depth comparison picking the right node when two UI cameras overlap on
  one window (same ceiling as clicks).

**Behavior drift to watch** (each intended; each a changelog line):

- a field under a pass-through overlay now takes the focus on a click on the
  overlay; clicking a disabled control inside a focusable container focuses
  the container;
- `onBlur`/`onFocus` arrive from `Update` instead of `PostUpdate`, blur
  first;
- with `TabNavigationPlugin`: one Bevy warning on the first Tab (the window
  has no tab group), one per Tab while nothing is focusable; devtools fields
  join the Tab order when the panel is open; a `display: "none"` subtree
  stays tabbable (Bevy checks `InheritedVisibility` only — use
  `visibility: "hidden"` or unmount);
- a focusable `<button>` takes Enter and Space while focused, on top of any
  global `keyDown` binding the app has for them;
- a node that is _only_ focusable (no handlers, no hover style) has no
  `ReactInteraction`, so `PointerCapture::over_ui` stays false over it and a
  scene camera still reacts to the press. Give it an `onClick`; making
  focusables interactive is a separate decision.

**Layers / caching.** `focusVisibleStyle` joins the promotion union through
`all_styles()`: a `filter` or `opacity` in it promotes the node permanently,
like any variant. The focus restyle goes through `apply_style_masked`, so its
invalidation repaints cached layers as hover does; `READ_ONLY`/`DISABLED`
declare `PAINT`. An `outline` ring on a node inside a promoted layer is cut at
that layer's capture rect (border box + filter outset) — the demos `Button`
check in §5.

**`--shoot` checks.** `--shoot "Focus" out --record 2 --input "0.5 move X Y"
--input "1 press" --input "1.1 release"` on a button: `focusStyle` shows, the
ring does not, the readout names it. The same on the `<editableText>` page's
_Modes_ example: the read-only field takes the caret, the disabled one does
not. `--shoot "<surface>"` with a press on its field: focused, no flicker in
the frames. Tab cannot be scripted in shoot mode — the ring is a manual check.
