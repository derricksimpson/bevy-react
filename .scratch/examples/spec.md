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

## 5. Dead Space 2 — `deadspace`

Homage to Dead Space 2's diegetic UI: every readout lives in the world, on
the suit, over the gun, projected in front of the engineer, on doors and wall
units. Built from `<surface>`s on meshes. Added from TODO on 2026-10-08 —
built the same day: `examples/deadspace` (the lead plus three parallel agents:
the Rust world, the suit/fixture holograms, the menu holograms).

Where the build moved from the plan below: `Holo` mounts a hologram's content
only while it is open (+ 400 ms), since a `<portal>` in a folded hologram
kept its camera rendering; the requested-open set lives in a resource, since
worn holograms' quads spawn after React's first `holo.open`s; the save
station moved left and its hologram narrowed so the doors and hack shots
frame cleanly; the menu hologram hangs right of the engineer, turned to the
camera; live portals inside the cached hologram layers need
`cache: "never"`; the room's own noises (doors, elevator, impacts) come from
a Rust `station::Noise` message into the sound module; the map's markers use
the legend's colors and the bulkhead turns green on `door.unlock` (new
`map.mark` message for the objective beam); `tour.arrived` was dropped —
holograms unfold with their beat and the dip runs on fixed timing; the
objective is derived from the game (bought, cutter upgraded, door open),
not advanced by events, so the tour can be taken in any order.

### Redesign (2026-10-12): walk it

The showroom became walkable; where this contradicts the decisions below,
this wins. Asked for: no main menu, mouse + W A S D, panels disabled (grey)
until the player approaches, no white-noise transitions, every screen
diegetic and opening smoothly. Decided with the user:

- **Camera: over the shoulder** (Dead Space 2), not first person: the suit's
  own readouts are on his back.
- **The crosshair is the pointer.** Cursor locked; the middle of the screen
  is the in-world pointer (click, wheel), with a reticle drawn on the
  hologram. Needed a core addition: `SurfacePointerRay` (the app's ray
  instead of the window cursor). The view narrows onto the hologram pointed
  at, so small panels read from where he stands.
- **A dead panel shows its real UI, grey**: desaturated, dim, under the
  bloom, deaf to input; live within reach (3.6 m for the big ones, 2.8 m
  for the far wall's). The bulkhead lock and the floor indicator are status
  lights: always live.
- **Only the static cut went** (`holoStatic`, the `static` cue, the dip).
  The grain, the glitch bursts and the `scanSwap` tab sweep stay.
- **Controls**: the alcove terminal (was the title) lists them and he starts
  in front of it; a prompt hologram beside his arm says what the panel he
  stands at answers to, the basic keys for the first seconds, and how to
  take the mouse. No beat strip, no flat UI.
- **H (take a hit) and V (vent) stay keys.** RMB aims, LMB fires. The suit
  menu (Tab) stops him; a step folds it.

**Second pass, same day: the camera locks on screens.** Pointing with the
middle of the screen was "not comfortable". Decided with the user:

- **Dead Space staging.** Walking up to a panel, facing it, locks the camera
  on it head-on; he steps to a post beside it and his back stays at the
  frame's left edge. The mouse moves a cursor on the screen (camera still).
- **Back away to leave**: W A S D keep walking him, relative to the screen;
  S out of range releases; a sidestep along the far wall hands the lock on.
- **Near and facing** triggers it (walking past doesn't); panels still wake
  to color from farther out.
- **The suit menu locks the same way.**

This replaced the crosshair pointer and its zoom (no pointer at all while
the camera is free) and the dead panels' blocking pane.

**Third pass: F, not proximity.** The user: "F to activate the screen, F
again or walk in any direction to deactivate it." The lock no longer takes
hold on approach: before a panel, looking at it, the prompt offers it and F
gives it the camera; F again or a freshly pressed W A S D lets go (keys held
since before F don't count, and he walks nowhere of his own while held). No
handover between neighbours. F also folds the suit menu. Nothing is
activated at startup (the user, after I had him start held by the controls
terminal): he stands before it and the prompt offers it.

Gone: the tour (`tour.rs`, `beats.ts`, `tour.beat`), the title screen and
`app.quit`, the beat strip, the dip, `fit_ui_to_1080p`. New: `player/`,
`layout::{confine, inside, reach, spot}`, `holo.active`, `player.stance`,
`player.fire`, `player.sync`, the `prompt` surface (fourteen now), a walk
cycle, footsteps, the corridor behind the bulkhead is walkable once open.
Core, found on the way: surface UI cameras saw the scene's meshes, so no
live surface near the origin ever idled (they are on no render layer now).

### Decisions (grilled 2026-10-08)

- **Name** `deadspace` (folder, cargo + npm package). In game: our own title
  and lore, original copy. Desktop + web, deployed under `/deadspace/`.
- **Era: Dead Space 2.** Cyan holograms, the spine health bar with the stasis
  arc on the shoulder blade, the ammo hologram over the gun, the projected
  menu, the store, the bench, the rotary hack.
- **A showroom, not a game.** No walking, no enemies. A camera tours fixed
  beats of one procedural station deck; keys act inside a beat. The world
  never pauses; holograms project in real time.
- **Tour: beat strip + keys.** A small flat strip, the only non-diegetic UI,
  lists the beats (←/→ or 1–8) and the current beat's key hints. Beats at the
  same spot glide; a beat at another fixture cuts through a short
  hologram-static dip while the engineer is moved.
- **Title: diegetic.** Title and menu are a screen on a wall terminal; BEGIN
  glides the camera to the first beat.
- **Holograms are real `<surface>` meshes**, driven by the mouse ray-cast
  into them (the surface pointer) and by keys. No gamepad.
- **Menus:** the suit menu (Inventory, Map, Objectives, Logs), Store, Upgrade
  bench. **Extras:** locator line, doors + wall panels (lock holograms, save
  station, elevator panel), hacking panel. No video calls.
- **Engineer: hard-surface procedural.** Beveled and extruded plates, a
  ribbed spine housing, segmented limbs, a helmet with lit visor slits; a
  breathing idle and an aim pose.
- **Tone: eerie, no gore.** Flicker, steam, emergency red, scorch marks,
  scrawled warnings.
- **Sound: synthesized** (cyberpunk's `sound/`): hologram open/close, select,
  buy, deny, stasis, shots, reload, hack ticks, a station drone.
- **Core first:**
  1. **Layers under every UI camera.** Promoted layers (group opacity,
     `filter`, `morphFilter`, `transform3d`, `cache`) composite on the camera
     that renders them, so they work inside `<surface>` (today they're skipped
     with a `layerCamera` warning). `ExtractedLayer` carries its camera's UI
     phase and render entity; `redistribute_ui_layers` steals from and injects
     into that camera's phase; `ui_layer_capture_pass` runs each camera's own
     layers; the backdrop samples its own camera. `stock_camera.rs` and the
     `layerCamera` kind go. Known limit kept: `transform3d` picking stays
     window-cursor only.
  2. **Wheel into surfaces.** Wheel hit-tests per camera: the window cursor
     against window-camera nodes, each virtual pointer's texture position
     against its surface camera's nodes. Scroll containers and `onWheel` then
     work inside a `<surface>`. This also fixes a latent mis-hit: today the
     window cursor is tested against surface nodes in texture space.
     Both update `docs/guide/elements/surface.md` "Limits", the CHANGELOG, and the
     demos `<surface>` page (a filtered window and a scrolling list).

### Lore (original)

- Wordmark **COLDWAKE**, our own lettering. Eurydice Station, an ice-mining
  station on a gas giant's moon; the deck is Maintenance Concourse C. The
  engineer is T. Okafor of Halden Ice & Ore.
- Credits buy things, **power cells** buy upgrades. Weapons: Arc Cutter
  (tri-beam starter), Pulse Driver, Rail Spike. Items: medpacks S/M/L, stasis
  pack, air canister, three ammo kinds, power cell, semiconductor (sells).
  Three text logs, two audio logs (a garbled synthesized transmission and its
  transcript), a three-step objective chain.

### The deck and the beats

One procedural deck: a ribbed octagonal hall with floor grating, pipes, light
strips, a store kiosk, an upgrade bench, a save station, an elevator, a sealed
bulkhead with a hack panel beside it, a side corridor, and the title terminal
in a dark alcove.

| #   | Beat      | Where                 | Diegetic UI                                                        | Keys                                  |
| --- | --------- | --------------------- | ------------------------------------------------------------------ | ------------------------------------- |
| 0   | Title     | alcove terminal       | title screen: BEGIN / CONTROLS / QUIT                              | Enter, click                          |
| 1   | Suit      | center, over shoulder | spine health, stasis arc, air countdown in vacuum                  | H hit, M medpack, S stasis, V vacuum  |
| 2   | Weapon    | center, aiming        | ammo hologram over the gun (clip/reserve, name on swap), tri-laser | F/LMB fire, R reload, Q swap          |
| 3   | Suit menu | center                | projected menu: Inventory, Map, Objectives, Logs                   | Tab cycles tabs, click, wheel         |
| 4   | Store     | kiosk                 | Buy / Sell / Safe, item preview, credits                           | click, wheel                          |
| 5   | Bench     | bench                 | circuit grid per item, power cells                                 | click                                 |
| 6   | Doors     | bulkhead wall         | bulkhead lock, save station (save/load), elevator panel + floor    | click                                 |
| 7   | Hack      | hack panel            | rotary dial, three locks, a timer                                  | mouse around the dial or A/D, E/click |
| 8   | Locator   | center                | the line traced along the floor                                    | L project, Shift+L next destination   |

State carries across beats: a medpack used on the suit leaves the inventory,
a capacity node grows the clip, a purchase lands in the grid, the hack unlocks
the bulkhead (its lock goes green, the door opens), saves restore all of it.

### Surfaces

| Surface    | Mesh                                    | Logical size |
| ---------- | --------------------------------------- | ------------ |
| `spine`    | inset strip in the suit's spine housing | 48×360       |
| `stasis`   | arc plate on the right shoulder blade   | 160×160      |
| `air`      | floating quad over the left shoulder    | 220×110      |
| `ammo`     | quad above the gun                      | 220×130      |
| `menu`     | curved panel ~1.2 m in front of him     | 1280×720     |
| `store`    | panel in front of the kiosk             | 1280×760     |
| `bench`    | panel over the bench                    | 1280×760     |
| `lock`     | the bulkhead's center                   | 200×200      |
| `save`     | the save station's screen               | 480×320      |
| `elevator` | the call panel                          | 240×360      |
| `floor`    | above the elevator door                 | 240×80       |
| `hack`     | the hack panel's face                   | 420×420      |
| `title`    | the alcove terminal                     | 960×540      |

Every mesh carries `SurfacePointer` (it also gates the Live render on
visibility). One `HologramMaterial` draws them: premultiplied UI, an emissive
gain into the HDR camera's bloom (surface textures are LDR), scanlines,
flicker, edge fade, and a `presence` uniform Rust eases for the unfold-and-
flicker open/close. Wall screens use it with an opaque backing and less
flicker.

Inside the holograms (needs core 1): tab and page switches morph through a
custom `scanSwap` (bands redraw line by line), a time-driven `holoNoise`
filter on each hologram root (interference lines, jitter), `glitch` bursts on
a hit or a denied action, group-opacity fades. `<portal>`s inside them: the
item preview (a turntable camera on its own layer, one procedural model per
item), the map (a camera over a hologram miniature of the deck; drag rotates,
wheel zooms), item icons (`Snapshot` targets of the same models). `<canvas>`:
the audio-log waveform. `<svg>`: the bench circuit, the hack dial, the stasis
arc.

### Ownership and wire

React owns the fake game (`ui/src/game/`, one reducer: health, stasis, air,
ammo per weapon, inventory, safe, credits, power cells, upgrades, door,
saves, logs, the locator destination). Rust owns the presentation: camera
shots, glide and dip, the engineer's poses and effects (sparks, shake, muzzle
flash, stasis pulse), doors and the elevator, the locator path, the
turntable, the map miniature, sound.

Messages: `tour.beat`, `holo.open { surface, open }`, `suit.hit/heal/stasis`,
`suit.vacuum { on }`, `weapon.equip/fire/reload`, `preview.show { item }`,
`map.orbit/zoom`, `door.unlock`, `elevator.call { floor }`,
`locator.show { to }`, `hack.shock`, `sound.cue`. Events: `tour.arrived`
(holograms open once the camera settles), `elevator.arrived`,
`door.opened`, `debug.act`.

### Look

Hologram cyan `#7fe3ff` on blue-black, white for the selection, amber for
prices and costs, red for damage and locks, green for unlocked. Michroma
(headers) + Saira Semi Condensed (body), both OFL. The world: dark steel and
painted panels, cold light strips (two flicker), a rotating emergency beacon,
distance fog, bloom, dust, steam.

### Shoot

`--shoot <out.png> [settle] [--size WxH] [--scale F] [--do "<secs> <step>"]…`.
Verbs: `beat <n>`, `tab <name>`, `select <item>`, `hit`, `fire`, `reload`,
`stasis`, `vacuum`, `buy <item>`, `install <slot>`, `hack`, `locator <to>`,
`elevator <floor>`. The surface pointer is window-only, so hover in captures
goes through verbs.

### Build order

1. Core: layers under every UI camera.
2. Core: wheel into surfaces.
3. Skeleton: crate, ui package, bindings, shoot, the hologram material on a
   test quad.
4. In parallel: the station, the engineer and weapons (Rust); the game
   reducer and the holograms (React).
5. Sound, locator, dip, polish; a shoot per beat; a web smoke run.
6. Docs and the deploy checklist.

**Risks**: the hard-surface suit's art bar in code; up to 13 surface cameras
(only visible ones render, a beat shows ≤ 6); `transform3d` picking inside a
surface stays unsupported.

## Parked

Mentioned, not planned: a Pip-Boy-style wrist computer (a CRT `<surface>`, a
synthesized radio + `<canvas>` scope; overlaps atrium and cyberpunk), a product
configurator (a mechanical watch with an exploded view and labelled parts), a
mission-control telemetry dashboard.
