# Bevy 0.20 migration — progress log

> History note (2026-10-12): the port's 115 per-fix commits were squashed
> into 40, one per feature or fix. The hashes below are the squashed
> commits, so a range from the original log now names a single commit;
> commit counts in the log are the original ones.

Goal (user, 2026-10-11): migrate to Bevy 0.20. Apply fixes one by one, one commit
each. Do "1. Required" (`audit.md`) first. After that, every change needs a test
run and a benchmark-regression check. Implement all 3 categories. Focus on the
demos example; fix the other examples only after all library changes. New
features and API changes also update the demos app and the docs guide.

## Ground rules (keep across context compactions)

- Commit on `main` (the user asked for per-fix commits; no branches). Repo style:
  `type(scope) description`. Every commit must build.
- Build env:
  - `export CARGO_PROFILE_DEV_DEBUG=line-tables-only`: smaller test binaries
    (disk); use it on every cargo call.
  - Use `-j 8`: the machine has 31 GB RAM, ~8 GB free, and no swap.
  - Run cargo jobs one at a time.
  - Check `df -h /home` before workspace builds.
- Tests while other examples are unported:
  `cargo test --workspace --exclude arcana --exclude atrium --exclude civilization --exclude cyberpunk --exclude nodes --exclude deadspace`
  (this keeps demos, minimal, stress and layers-stress in).
- The demos bundle tests need `npm run build -w demos` (dev bundle, not prod).
- Benchmarks:
  - `npm run build:prod -w stress-app` then
    `cargo run --release -p stress -- --run table-ops --out benchmark_results/<name>.json`.
    Run it twice and compare per-leg p50s; leave the PROD bundle in place.
  - layers-stress:
    `STRESS_PRESET='<json>' npm run build -w layers-stress-app` then
    `cargo run --release -p layers-stress -- --measure 15`; read the steady-state
    fps from the tail.
    - P1 `{"n":500,"animate":false,"filterMode":"off"}`
    - P2 `{"n":500,"animate":false,"filterMode":"all","blur":true,"animateFilter":true}`
    - P3 `{"n":500,"animate":false,"filterMode":"off","animateSize":true}`
  - Afterwards rebuild layers-stress with no preset; the committed defaults
    regenerate byte-identical.
  - grep the logs for `Path not found` / `ERROR`.
- Temp files: the session scratchpad is gone. Use the gitignored
  `target/port-scratch/` (logs, `shots/`, `benchcmp.py`). Bevy 0.20 sources:
  `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_*-0.20.0`.
- Visual checks: `cargo run -j 8 -p demos -- --shoot "<label>" target/port-scratch/shots/x.png 4`
  (never xdotool). grep the log for ERROR/WARN.
- Decisions taken without the user (the goal said not to pause):
  - Keep the `bevy_react::filter` shader import path (register the prelude at
    `embedded://bevy_react/filter.wesl`).
  - The bump commit makes ALL examples compile (tiny API renames only); their
    shader/camera/runtime fixes wait until the library is done.

## Plan / status

Phase 0: baseline

- [x] 0.19 baseline benchmarks (table-ops ×2, layers-stress P1–P3)
  - table-ops: `benchmark_results/bevy019-{a,b}.json`. The two runs agree within
    ±10% on the 10k legs; 1k `totalMs` noise reaches ±40%. Compare with
    `python3 <scratchpad>/benchcmp.py A,B C,D` (min-of-p50 per side).
  - layers-stress (0.19, steady state): P1 ≈ 565 fps, P2 ≈ 30–48 fps (bimodal),
    P3 ≈ 77 fps.
  - Pre-existing in measure mode: `devtools.styleFields`/`devtools.elements`
    "no handler" ERRORs. Not a regression.

Phase 1: required (audit §1)

- [x] bump bevy 0.20 + naga 30 + compile fixes + test harness (775f69b). Rust
      toolchain updated to 1.99 (bevy MSRV 1.97.1). Found: UI roots sized `100%`
      outgrow the viewport (implicit grid auto row), so roots now use vw/vh.
- [x] demos named_pins: drop the PostLayout→Propagate edge (startup cycle) (775f69b)
- [x] core + demos shaders → WESL, real ShaderCache composition test, guide (0c62e65)
- [x] clip_aabb NaN on an open (Visible) axis: nested layers' quads were clipped away (7ce05a7)
- [x] retained UI: delete layer/render/cached.rs (7ce05a7)
- [x] retained UI: clip swap → set_changed on interior-clip changes + ExtractInlineImages bracket (7ce05a7)
- [x] editableText: TextInput + inputs block lower picks + focus observers act on the original target + blur docs (7b8721c)
- [x] virtual-pointer restyle keeps focusStyle (pre-existing bug found while verifying) (7b8721c)
- [x] scroll_range re-derived for taffy 0.14 (7b8721c)
- [x] backdrop frost: bevy_ui's own `sd_rounded_box`, per-axis radii (7ce05a7)
- [x] glam projection constructors (7ce05a7)
- [x] `<button>` keeps its accessible label (`label_buttons`) (7b8721c)
- [x] demos: minimap marker depths (775f69b)
- [x] docs: version badges/tables, CLAUDE.md, changelog (4ad1b6d)
- [x] benchmark check after phase 1 vs baseline: slower, and it is Bevy's UI
      renderer (`clip_polygon` in `prepare_uinodes`), not our code — see
      `bench-phase1.md` (numbers, perf attribution, an upstream patch)

Not done in phase 1, on purpose: the `Interaction` deprecation stays a warning
until the real migration (phase 3, audit §3.8) — no alias commit.

Phase 2/3: features + improvements (each: tests + bench + demos + docs)

How it runs: one implementation subagent at a time (brief:
`target/port-scratch/AGENT_BRIEF.md`; every cargo/lint call goes through
`target/port-scratch/lk`, a flock wrapper). The coordinator reviews, commits,
then `target/port-scratch/bench.sh build` (release binaries + preset bundles
stashed under `target/port-scratch/{bin,ls-dist}`) and `bench.sh run <name>`
(table-ops ×2 + layers-stress P1–P3 under the same lock, the previous build's
binaries and the new ones alternating — a stale baseline misleads: frame
count and the real cursor's position change the numbers). Trivial changes
share one run.
Scratch checkouts for A/B runs: `target/port-scratch/wt019` (0.19, commit 8292389) and `wt020` (HEAD + a patched bevy_ui_render); binaries in
`target/port-scratch/bin`. Remove both with `git worktree remove --force` at
the end. Plans from the planning agents: `target/port-scratch/plans/*.md`.

Improvements (audit §3)

- [x] delete `pick_clip.rs` (§3.2) (abfc86e)
- [x] exact seed-frame radius (59544f9); image asset dirt through
      `AssetChanged<ImageNode>` (59544f9); GlobalZIndex tie docs (59544f9);
      trilinear variants drop `COPY_DST` (bd9101a); `lerp_val` through
      `try_interpolate_stable` (59544f9)
- [x] naga_oil-era shader notes (59544f9; the core ones went with the WESL
      commit)
- [x] macro rejects `.wgsl` (§3.6): landed after the examples (8539e18)
- [x] cursor follows the hover map (§3.9) (0c972a7): clip-correct, and a
      `focusPolicy: "block"` node now hides the cursors beneath it
- [x] schedule shuffling for ordering tests (§3.7): core feature
      `schedule_shuffle` + `BEVY_REACT_SHUFFLE_SEED` in every test harness and
      in `ReactUiPlugin::build` (99645d3). 15 seeds × 1015 library tests and
      20 demos pages × 4 seeds: no ordering bug found. Use `--lib`; the extra
      Bevy configuration costs ~11 GB
- [x] taffy bugs re-tested (§3.12) (13801ac): the `maxWidth` text
      measure bug is fixed in 0.20 (pinned by real-layout tests; one demos
      workaround removed, the shell-wide ones kept); the fractional-scale
      flex-wrap bug is still there (65 of 432 configurations at 1.1667;
      `Radio` stays no-wrap; an `#[ignore]`d reproduction + TODO entry)
- [x] interior clips are bevy's clip list minus the root's (0e7fda7), rotated
      clippers cut layers at their real edge via `clip_polygon` (0e7fda7),
      tests on bevy's own `update_clipping_system` (0e7fda7, b7e408c), clipped
      reveals with `transition: { layout }` documented + demo card (b7e408c).
      §3.1 already worked on main; no accordion was ever removed from demos,
      and the reveal is center-anchored (the "Hide docs" fold stays as it is)
- [x] retire `Interaction` (§3.8): `ReactInteraction` from the hover map and
      press state for every pointer, one hover collector and one drag
      collector (`virtual_events.rs` gone), `Interaction` and
      `RelativeCursorPosition` no longer inserted (ebf2685); a drag
      owns its click (b6d638c; `Slider.tsx` drops its workaround)
- [x] clean cached-layer members stay out of the stock queue (§3.3): the
      steal is one in-place `IndexMap::retain` (f685ce1) and members of clean
      layers are parked by their camera slot in bevy's retained stores, un-parked
      by the flag that schedules the capture (f685ce1). P1: our samples 2583 →
      1946, the three stock/steal symbol groups ~2000 → ~400, about +5% fps

Features (audit §2)

- [x] em/rem lengths (§2.1): `Length::{Em, Rem}`, grid tracks, `fontSize` and
      `letterSpacing` in em, em on nested text, fontless nodes follow
      `RemSize`, a `RemSize` change repaints cached layers; no bevy-react API
      for `RemSize` (the demos Units page drives the resource with its own
      message) (de1f23b)
- [x] inline images in text (§2.2): an `<image>` inside a `<text>` is the
      node-less `inlineImage` kind → bevy's `InlineImage`; px/em/rem sizes, the
      box reserved before load, layer dirt on asset change (c9553de)
- [x] `positionType: "fixed"` (§2.3): `FixedNode` behind a marker and one
      change-driven sync (withheld under a `display: none` ancestor); a fixed
      node leaves its ancestors' layers, is a layout root for the layout
      transitions, and uses its root's camera viewport (bc74a78)
- [x] elliptical `borderRadius` (§2.4): the CSS slash in the string form; a
      value without a slash is unchanged and stays inline in the style store
      (15cbb50)
- [ ] pointer capture (§2.5): `PointerCaptureMap` would break inner buttons
      and drop-target hover; the fix for "drag release clicks the ancestor" is
      a rule in `collect_ui_events`, done with the `Interaction` retirement
- [x] focus and form props (§2.6): a press focuses the topmost focusable
      under it (replaces bevy's racy click-to-focus; fields no longer need to
      block lower picks), `tabIndex`/`onFocus`/`onBlur` on every element,
      Enter/Space click a focused `<button>`, `readOnly`/`disabled` on
      `<editableText>`, `<button disabled>`, `focusVisibleStyle`; Tab
      navigation is the app's `TabNavigationPlugin` (063446f … efb2888)
- [x] safe alignment keywords (1b825ac), gradient `okhsl`/`okhslLong`
      (3cd4a8a), `fontFamily` fallback lists (c423113; an unregistered name
      warns and is skipped, the default font closes every list)
- [x] devtools paint flashing (§2.9): a "paint" toggle flashes every React
      node bevy re-extracted that frame (`ExtractedUiNodes.changed`), zero
      cost when off, nothing in release builds (0e82ea9)

Skipped on purpose (say so in the final report): `Length` as a `Val` newtype
(§3.10, optional — only `lerp_val` changes), anchors as `FixedNode` children
(§2.3 bonus, unverified).

Phase 4: other examples

- [x] arcana (541aa0d … aadc222): 7 shaders to WESL, `DebandDither::Disabled`
      on the card camera, the held card sized by its camera's viewport (a
      shoot-mode bug, not a 0.20 one); held card vs flat twin: same colour
      within 2/255, edges within 0.5 px
- [x] atrium (1df387d): 9 shaders, `super::common` import and no
      keep-alive handle, `ScreenSpaceTransmission` + `DebandDither::Disabled`
      on the eye, mirror and four lens cameras; 5.2–5.9 ms frames. Its agent's
      commit was refused by the permission system; committed by the coordinator
      after review
- [x] cyberpunk (d627370): 8 shaders, the handle field remounts on
      Enter only, two text-width wrappers removed (the 0.19 measure bug is
      gone), shoot steps `key`/`focus`; 25 screens match the 0.19 captures
      (mean abs diff ≤ 1.7/255)
- [x] deadspace (8dadb74): 8 shaders; every hologram kind, filter and
      morph seen in shots
- [x] nodes (a10ee3b): 2 `UiMaterial` shaders, the text field
      blocks its own presses, the node menu acts on the node it opens on (an
      old bug), shoot steps for a pointer and keys — every gesture run with
      the scripted pointer; civilization (b9bb3a7): bindings only,
      `OWNS_POINTER` still shields the map; minimal: clean
- [x] after them: the macro guard that rejects `.wgsl` filter shader paths
      (§3.6, 8539e18), `keyDown` pinned before the `blur` it causes (063446f;
      two example agents found the missing edge), no devtools table errors on
      builds without devtools (9277655), CLAUDE.md's example paragraphs, the
      README blurb, the TODO entry (e5f9198)

Not verified anywhere in phase 4: real pointer input on `<surface>` UI
(arcana, atrium, deadspace), touch, and the wasm/WebGL2 builds (a web build
needs ~125 GB of disk).

## Log

- 775f69b bump: 958 tests pass; clippy only deprecations
- 775f69b named_pins cycle
- 0c62e65 WESL: 958 tests; --shoot Filters starts with no shader errors, but layer content is blank (cached.rs + retained UI, next)
- 7ce05a7 / 0c62e65 / 7ce05a7: 958 tests; demos pages render (Home, Filters, Overflow, Layers, Backdrop, Morph, surface, Custom styles/filters)
- 7b8721c / 7ce05a7 / 7b8721c / 775f69b: 963 tests; clippy only `Interaction` deprecations; lint clean
- 4ad1b6d docs; phase-1 benchmark: see `bench-phase1.md`
- abfc86e … 0c972a7 (batch A, 8 commits): 963 tests, clippy/lint clean.
  Bench `batchA`: 6 of 224 legs over ±10%, all JS-side or 1k noise;
  `bevy_react_*` samples 3924 (baseline 3864) of 241k; P1 ≈ 470, P2 28–43,
  P3 50–74 (five reading agents were running). No regression.
- 0e7fda7 (interior clips, 6 commits) + 775f69b (surface demo root
  size): 970 tests. Bench `clips`: 3 of 224 legs over ±10% (1k noise);
  `bevy_react_*` 3841 of 239k; P1 ≈ 460, P2 26–44, P3 52–72 with our share of
  P3 down from 2992 to 2346 samples (no full-node clip scan). No regression.
- ebf2685 (`Interaction` retirement, 9 commits): 989 tests, clippy
  has no warnings left. Bench `interaction`: 4 of 224 legs over ±10% (JS-side
  noise and two improvements); `bevy_react_*` 3716 of 243k (down from 3841);
  P1 ≈ 460, P2 27–47, P3 51–76. No regression. Not checkable with `--shoot`
  (needs a real cursor/touch): the transform3d remap, the in-world surface
  pointer, the cursor leaving the window, touch drags.
- de1f23b (em/rem, 9 commits): 1007 tests, clippy and lint clean.
  The baseline comparison came back mixed (two text-layout legs +20%, many
  legs −20%) because the run itself differed: fewer frames, and the real
  cursor sat over the bench window (`ui_picking` 20 → 675 samples). A clean
  A/B (the commit before vs the head, 3 alternating runs each) shows 2 of
  224 legs over ±10%, both JS-side. No regression. From here on
  `bench.sh run` alternates the previous and the new binaries itself.
- 063446f … efb2888 (focus and form props, 10 commits): 1034 tests, clippy and
  lint clean. Needs a manual pass: Tab walking, the focus ring after Tab,
  Enter/Space in the live app, double-click selection in a field inside a
  `<surface>`.
  Bench `focus` (alternating with the em/rem build): 2 of 224 legs over ±10%
  (1k, by 10–11%); `bevy_react_*` 3678 of 237k; P1 ≈ 470 vs 480, P2 and P3
  bimodal as before. No regression.
- bc74a78 (`positionType: "fixed"`, 7 commits): 1053 tests, clippy
  and lint clean.
  Bench `fixed` (alternating with the focus build): 7 of 224 legs over ±10%,
  five of them JS-side and two improvements; `bevy_react_*` 3917 of 242k and
  3905 on P1 (3901 before). P1 read 485 fps against 515 for the previous
  build, but the new build ran under `perf` and the previous one did not; the
  script now measures both without it. No regression.
- 15cbb50 … c423113 (elliptical radius + three style additions, 6 commits):
  1066 tests, clippy and lint clean.
  Bench `styles` (alternating with the fixed build): the previous build's
  totals pinned at the 17.6 ms frame floor, so only the in-frame legs count:
  two legs up by 10–11% (1k insertEvery2nd layout, 10k create translate), the
  rest equal or lower; P1 ≈ 495 both, P2/P3 as before. No regression.
- c9553de (inline images, 9 commits): 1076 tests, clippy and lint
  clean.
  Bench `inline` (alternating with the styles build): 7 of 224 legs over ±10%,
  all at 1k and by 10–15% both ways; the 10k text legs are within ±7%;
  `bevy_react_*` 3822 of 234k; P1 ≈ 500 both. No regression.
- f685ce1 (cached-layer queue cost): 1087 tests, clippy and lint
  clean. Bench `park` (alternating with the inline build): 2 of 224 legs over
  ±10% (JS-side); P1 CPU 38.1k → 34.9k samples, ours 2583 → 1946; P2/P3
  unchanged. Disk filled up during this task (an unwrapped cargo call built a
  full-debuginfo copy, ~13 GB; padded perf reports, ~6 GB): cleaned, the
  scratch checkouts `wt019`/`wt020` are removed, 29 GB free.
- 99645d3, 13801ac, 0e82ea9 (shuffle feature, taffy re-tests, paint
  flashing): 1094 tests, clippy and lint clean. Library work is complete here.
  Bench `final` (alternating with the park build): 11 of 224 legs over ±10%,
  ten of them faster and one 1k flush leg +13%; `bevy_react_*` 3816 of 241k on
  table-ops (3864 after phase 1) and 2013 on P1 (about 3900 after phase 1).
- eab49d7 fix(js) a ref never crosses the wire: found on the demos `<canvas>`
  page (`unknownProp` for `ref`); React 19 passes `ref` as an ordinary prop.
  48 JS tests; the canvas shot logs no warning now.
- a10ee3b … d627370 (the six examples, 22 commits, five agents in parallel,
  each in its own directory): every agent ran the full suite once, 0 failed.
- 063446f, 9277655, 8539e18, e5f9198 (root-level follow-ups): full suite 0
  failed, clippy no warnings, lint clean, docs build 62 pages with 0 uncovered
  keys. Every facade feature alone and all together checks clean (CI's
  `cargo hack` step, emulated with plain `cargo check` in a throwaway target
  dir). Bench `closing` (alternating with the last library build): 4 of 224
  legs over ±10%, all at 1k by 11–13% both ways; P1 ≈ 530 both; `bevy_react_*`
  3906 of 239k on table-ops, 2050 on P1.

## Final state

Everything in `audit.md` is done or decided against (see "Skipped on purpose"
above), on `main`, unpushed. The plans the implementation agents worked from
are in `plans/` next to this file.

Needs a person:

- Real input: pointer on `<surface>` UI (arcana, atrium, deadspace) and on
  3D-transformed elements, touch drags, the cursor leaving the window, Tab
  navigation and Enter/Space in the live app, double-click selection in a
  field inside a surface.
- The wasm/WebGL2 builds (a web build needs ~125 GB of disk) and the deploys.
- `npm run screenshots` if the docs shots should show the new demo cards.
- Upstream: the `clip_polygon` fast path (`clip_polygon.patch`,
  `bench-phase1.md`) and an epsilon in taffy's `collect_flex_lines` for the
  fractional-scale wrap.

Left as found, noted by the agents: `layer.rs` (~2.6k lines), `plugin.rs`
(~1.3k) and `reconcile/apply.rs` (~1.5k) are over the 1000-line cap; the demos
shell's px widths could become `width: 100%` + `maxWidth` now; arcana's
`inspect.rs` comment about presses on the card's buttons is wrong (a press on
the heart also grabs the card); there is no `focus()`/`blur()` from React (a
press on a non-focusable part of a popup blurs its search field for good);
the devtools panel asks for a "Noto Sans Mono" font an app may not register.
