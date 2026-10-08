# New showcase examples

Four showcases, built one at a time in the order below. Each section is the
idea, not the design: grill it at kickoff (its open questions are the agenda),
then write the decisions back into the section.

## Why these four

What the existing showcases (arcana, atrium, civilization, cyberpunk) leave
unexercised, from a grep of their `ui/src` on 2026-10-08:

- `<canvas>` and `imageRendering`: never used.
- Gamepad input: never used (cyberpunk only draws a control-scheme picture).
- `sharedTag` (atrium only), `transition: { layout }` and `onWheel`
  (civilization, once each), `editableText` (one input in atrium, one in
  cyberpunk).
- Nothing real-time (civilization is turn-based, the rest are menus), nothing
  that is a tool, nothing portrait or touch-first.

| Example     | Fills                                                                            |
| ----------- | -------------------------------------------------------------------------------- |
| Racing      | per-frame gameplay HUD, gamepad + rumble, layout FLIP, `<anchor>`, `<canvas>`    |
| JRPG        | `imageRendering`, 9-slice image frames, gamepad-first menus, `pixelize` morph    |
| Phone       | touch, portrait, web on phones, `sharedTag` navigation, FLIP reorder, `<canvas>` |
| Node editor | tool UI, `onWheel` zoom, drag-heavy input, `editableText` at scale, app textures |

Order: racing first (the biggest gap, and its core prerequisite — Bevy-set
shared values — serves the JRPG's gauges too), then the JRPG, the phone, and the
node editor last (the one most likely to hit the keyboard/focus gaps open in
TODO `Features:`). Reorder freely.

## Shared rules

- Homage the look, never the names: original branding, lore and copy, no logos
  or quoted UI text (cyberpunk's precedent: its own wordmark, Sable City).
- Rust owns the world, React owns UI state; 3D is procedural (no model files);
  the example owns its `assets/`.
- `--shoot <out.png> [settle] [--size WxH] [--scale F] [--do "<secs> <step>"]…`,
  the same scheme as the others (`shoot.rs`).
- Desktop + web like cyberpunk/civilization unless the kickoff decides
  otherwise.

### New-example checklist

What adding cyberpunk (928af54) and its deploy (d8b9542) touched:

- `examples/<name>/` cargo package (`publish = false`, the `bevy-react` facade
  with the features it uses) + root `Cargo.toml` `members`.
- `examples/<name>/ui/`: npm `name` = the cargo package name,
  `build`/`watch`/`build:web`/`build:web:prod`, `tsconfig.json`, `index.html`
  for web; root `package.json` `workspaces`.
- `--export-bindings` + the committed `ui/src/bevy.ts`.
- `.gitignore`: `examples/<name>/ui/dist/`.
- Docs: `examples/<name>/README.md`, root `README.md`,
  `docs/guide/getting-started.md`, a `CLAUDE.md` paragraph, `PUBLISHING.md` run
  commands, `CHANGELOG.md`, `screenshots/<name>-*.webp`.
- Web: a `deploy:<name>` script, the folder added to `deploy:docs`'s
  `--remove '{*,!(demo|cyberpunk|civilization)/**}'` (no docs top-level folder
  may start with its name), a link in `docs/guide/tooling/web.md`.

## 1. Racing — `racing` (working title)

Forza Horizon homage: a festival road race running live behind the HUD.

**Bevy**: a procedural closed-loop track (spline ribbon, curbs, barriers, a few
landmarks), terrain and a sunset sky, low-poly cars built from primitives.
Arcade handling on the spline frame (no physics crate), 7 AI cars on a racing
line with per-car pace and rubber-banding, a chase camera, a top-down minimap
camera (civilization's portal precedent). Keyboard + gamepad driving, rumble on
curbs and contact.

**React**: car pick on a 3D turntable (`<portal>`), countdown, then the race
HUD: speedometer + rev gauge (`<svg>`, needles on Bevy-set values), gear, lap
and split deltas, a position tower that reorders on overtakes
(`transition: { layout }`), rival tags with distance (`<anchor>` over each car),
minimap (`<portal>`), telemetry trace (`<canvas>`). Results screen: the player's
tower row flies into the results table (`sharedTag`).

**Core first: Bevy-set shared values.** Today only JS drives shared values
(`SharedValues::set` is `pub(crate)`, ids come from `useSharedValue`), so a
needle driven by Bevy costs an event plus a React re-render every frame. Wanted:
Rust writes a value each frame, JS binds it with `{ animated }` like any shared
value, React never re-renders.

**Open questions**

- The shared-value API: string-keyed (`set("car.speed", v)` + a JS hook) or
  typed through codegen (a `#[react_value]`-style registry, names checked)?
- Text that changes every frame (speed digits, lap timer): throttled events
  (10–20 Hz re-renders) or a text binding?
- Telemetry: throttled events into `getContext()`, or drawn Bevy-side?
- Scope: one track and one race, or a few events on a festival map?
- Menus on the d-pad: app-side focus (cyberpunk's keyboard pattern) or a
  library helper?

**Risks**: the most game-side work of the four (track generation, AI, handling
that feels good).

## 2. JRPG — `jrpg` (working title)

Octopath Traveler (HD-2D) homage: pixel sprites in small 3D dioramas, classic
battle menus.

**Bevy**: 2–3 dioramas (forest clearing, town, cave) built from blocks with
low-res generated pixel textures (nearest sampler), sprite billboards with idle
frames, tilt-shift `DepthOfField` + bloom + vignette, torch lights, fireflies. A
small walkable area for the party leader; an encounter starts a battle in the
same diorama.

**React**: title screen, location banner, dialogue box (portrait, typewriter
text, choices), menus (party, items, equipment, skills), and the battle UI:
command menu, target cursor over the 3D enemies (`<anchor>`), a turn-order bar
that reorders (`transition: { layout }`), HP/MP bars, damage numbers popping
over sprites (`<anchor>` + a spring, unmounted from the completion callback), a
`pixelize` (or custom swirl) battle-intro morph, victory screen (XP count-up,
level up). Gamepad-first with keyboard too: the first showcase you drive with a
gamepad.

Pixel UI: window frames as 9-slice pixel PNGs (`<image>` sliced mode),
portraits and icons as small PNGs, all `imageRendering: "nearest"` (its first
showcase use). An OFL pixel font (Pixelify Sans, Silkscreen, …).

**Open questions**

- Pixel art source: hand-drawn PNGs, or ASCII definitions + a generator script
  with the PNGs committed? It has to be image files: `imageRendering` refuses
  `{ texture }` sources, so Rust-generated images wouldn't exercise it.
- Pixel-perfect scale: fit the UI scale factor to an integer multiple (like
  cyberpunk's `fit_ui_to_1080p`; fractional scales such as the 1.1667 desktop
  make nearest-sampled art shimmer), or keep HD text with pixel frames?
- Scope: one battle loop, or exploration + dialogue + battle?
- Battle intro: the built-in `pixelize` or a custom morph?

**Risks**: the art quality bar, with pixel art drawn by us.

## 3. Phone — `island` (working title)

Animal Crossing NookPhone / iOS homage: a phone UI over a cozy 3D island.
Portrait and touch-first, made to be opened in a phone browser.

**Bevy**: a low-poly island (sea, beach, palms, a house, a dock), day/night
from the real clock, weather, a gently drifting camera, drag to look around
while the phone is down. A phone-camera rig for the Camera app.

**React**: status bar (island time, weather), home-screen grid, apps that open
by the icon growing into the app and close by shrinking back (`sharedTag` +
`transition: { sharedElement }`), a swipe-up home gesture, long-press jiggle
mode with drag reorder (`withRepeat` wiggle + `transition: { layout }`),
notification banners from Bevy events. Apps:

- Camera: live viewfinder (`<portal>`); the shutter makes a
  `RenderMode::Snapshot` target per photo (cyberpunk's `shot-*` pattern).
- Photos: grid, tap for full screen (`sharedTag`), delete with a FLIP reflow.
- Design: 32×32 pixel-pattern editor on `<canvas>` (pencil, fill, eyedropper,
  undo); "Display" sends it to Rust, which paints it on a flag on the island.
- Map: the island from above with pins (`<svg>` from island data, or a top-down
  portal).
- Chat: a message list + an `editableText` input.

On desktop the phone floats beside the island. In a phone browser the phone is
the screen, and swiping it down shows the island.

**Open questions**

- Text input in phone browsers: winit's web backend probably never raises the
  on-screen keyboard (verify). An in-app keyboard component (a showcase in
  itself), or no Chat on mobile?
- wasm on phones: WebGL2 vs WebGPU, the shadow/portal budget. Measure a bare
  island build on a real phone before building apps.
- Gestures: what the library gives (touch scroll + drag arbitration) vs what the
  app builds (swipe velocity into `withSpring`, long-press).
- Which apps make the cut (five is a lot)?

**Risks**: the island's art, phone performance, the virtual keyboard.

## 4. Node editor — `nodes`

Blender node-editor homage: build a procedural texture as a node graph and see
it as a PBR material on a 3D object. The first tool showcase: bevy-react as an
editor-UI kit. Built first (reordered, 2026-10-08) — built the same day: `examples/nodes`.

### Decisions (grilled 2026-10-08)

- **Name** `nodes` (folder, cargo + npm package, UI title "Nodes"). Desktop +
  web, deployed under `/nodes/`.
- **Look: Blender-like.** Rounded nodes with category-colored headers, typed
  colored sockets (gray = value, yellow = color, purple = vector), every
  parameter inline in the node (an unlinked input socket shows its value as a
  field, Blender style), a small live preview inside each node. Original
  palette and copy, Inter.
- **Layout: Blender's Shading workspace.** Top bar (File/Edit/Add/View menus,
  preset tabs styled as workspace tabs, resolution, eval status); 3D preview
  top-left (`<portal>`), 2D view of the active node top-right (tiling 1/2/3×);
  the graph across the bottom with an N-panel (the active node's properties).
- **Evaluator: CPU in Rust.** The node catalog (sockets, params, defaults,
  ranges) is defined in Rust and served by a request, so the UI is generic. A
  budgeted Bevy system evaluates dirty nodes in topological order, each node's
  rows split over the `ComputeTaskPool` (inline on wasm). Content-addressed
  cache (key = hash of type, params, mute and the input keys), so only changed
  nodes and their downstream re-run and undo back to a cached state is free.
  f32 buffers (gray 1 channel, color/vector 3). Resolution 256/512/1024 (512
  desktop, 256 web). Outputs: a 128² thumbnail per node
  (`RenderTargets::register("node-<id>")`), the active node at full size for
  the 2D view, and the material maps (base color, metallic/roughness, normal,
  height → parallax, emission) on a `StandardMaterial`.
- **Nodes (20):** Noise, Voronoi, Bricks, Gradient, Shape; Value, Color; Mix
  (blend modes), Invert, Levels, Color Ramp; Math, Map Range; Blur, Slope
  Blur, Warp, Transform, Edges (curvature), Normal (from height); Reroute;
  Material Output (base color, metallic, roughness, normal, height, emission).
- **Wires: an app-registered `<wire>` element** (the extension API, like the
  demos' `<cube>`): a `UiMaterial` draws the antialiased bezier on the GPU,
  colored from socket type to socket type, crisp at any scale. A `<grid>`
  element draws the background grid the same way.
- **Zoom: hybrid.** While the wheel turns, one `transform: scale` op on the
  graph content (anchored at the cursor); ~150 ms after the last notch the
  graph re-lays out at the zoomed size (every length × zoom) so text is crisp
  at rest.
- **Adding nodes: Blender's Add menu.** Shift+A, right-click on empty space or
  the Add menu: cascading categories, typing searches at once; the new node
  follows the cursor until a click places it. A wire dropped on empty space
  opens it filtered to compatible nodes and links the pick.
- **Interactions:** click / Shift-toggle / box select (Shift adds, Ctrl
  subtracts); drag or G to move (Esc/right-click cancels); drag wires output →
  input with magnetic snapping, drag a linked input to detach or re-route;
  X/Delete; Shift+D duplicate-and-grab; Ctrl+C/V (in-session); Ctrl+Z /
  Ctrl+Shift+Z; A / Alt+A; Home frames all, F frames the selection; MMB drag
  pans, the wheel zooms (trackpad pixel deltas pan). Power tools: Shift+right-
  drag drops reroutes where it crosses wires, Ctrl+right-drag knife-cuts
  wires, M mutes (passes the first input through), H collapses.
- **Fields (editableText at scale):** Blender number fields (drag to scrub,
  click to type into an autofocused `<editableText>` that unmounts on Enter,
  Esc or blur; factor fields draw a fill), color swatches with an HSV picker
  and hex field, enum dropdowns, the Color Ramp stop editor, node rename, the
  Add-menu search.
- **Shortcuts vs fields: app-side.** The app knows when a field is in edit
  mode (React state) and ignores shortcuts then. No core focus work.
- **3D preview:** own render layer, live `preview` target, HDR + bloom (for
  emission); sphere / cube / plane / torus; spin toggle, drag to orbit, wheel
  to zoom; a procedural studio cubemap for reflections (fallback lights where
  WebGL2 can't filter it); height → parallax (`depth_map`).
- **Persistence: session only.** Three presets (Old bricks — the startup
  graph, Rusted metal, Lava rock), undo history, in-session copy/paste. No
  files, no PNG export.
- **Core first (library work):**
  - `shiftKey`/`ctrlKey`/`altKey`/`metaKey` on `PointerEventData` and
    `WheelEventData` (settles the bespoke-pointer TODO(review) note).
  - `onPointerHover`: fires each frame the cursor moved while the pointer is
    over the element (the hover state `onPointerEnter` uses), button held or
    not; `onPointerMove` keeps its drag-only semantics.
- **Skipped:** frames, splice-onto-wire, waves/checker nodes, a wood preset,
  file save/load and PNG export, per-node `onKeyDown`/focusable nodes.

## Parked

Mentioned, not planned: a Pip-Boy-style wrist computer (a CRT `<surface>`, a
synthesized radio + `<canvas>` scope; overlaps atrium and cyberpunk), a product
configurator (a mechanical watch with an exploded view and labelled parts), a
mission-control telemetry dashboard.
