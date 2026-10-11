# Phase 1 benchmark check: Bevy 0.19 → 0.20

Date: 2026-10-11. Same machine, back-to-back runs of a 0.19 binary (commit 8292389) and the 0.20 binary (commit 4ad1b6d), release builds, prod bundle.
Raw data: `benchmark_results/{ab019,ab020,ab2-*,perf*}.json`, perf profiles
under `target/port-scratch/perf*.data` (not kept in git).

## Result

**0.20 is slower in these benchmarks, and the cause is Bevy's UI renderer, not
bevy-react's code.**

table-ops (10k rows, min of two p50s, ms):

| op             | total 0.19 → 0.20 | layout 0.19 → 0.20 |
| -------------- | ----------------- | ------------------ |
| updateColor1   | 16.4 → 21.5       | 8.5 → 9.7          |
| updateText1    | 25.6 → 31.3       | 17.0 → 19.0        |
| append1        | 26.6 → 32.5       | 16.9 → 19.7        |
| remove1        | 26.8 → 33.7       | 17.0 → 20.0        |
| create (10k)   | 551 → 532         | 118 → 128          |
| insertEvery2nd | 217 → 224         | 75 → 88            |

The JS, flush and translate legs are unchanged (±10%, the noise floor).

layers-stress (steady fps): P1 (500 static layers) 525 → 490, P2 (500 animated
blur chains) 30–45 → 30–45 (bimodal in both), P3 (animated size) 58–80 → 72–75.

## Where the time goes (`perf record`, whole table-ops run)

| samples in         | 0.19    | 0.20    |
| ------------------ | ------- | ------- |
| total              | 190 994 | 240 348 |
| `bevy_ui_render`   | 29 780  | 63 885  |
| `bevy_ui` (layout) | 42 790  | 45 220  |
| `bevy_react_*`     | 3 566   | 3 864   |

- `bevy_ui_render::clipping::clip_polygon` is new in 0.20 and is the top symbol
  of the whole run (36 936 samples, 15%). `prepare_uinodes` runs it for every
  node quad and every glyph of every extracted node, every frame, rows scrolled
  out of view included (the table is an `overflowY: "scroll"` container, so each
  row carries a clip rect). It is a Sutherland–Hodgman clipper that moves two
  inline 16-vertex buffers per clip edge; 0.19 clamped a rect.
- `prepare_uinodes` itself grew (10 150 → 18 144): triangle fans per quad, a
  larger `UiVertex`.
- Retained extraction pays back part of it: `extract_text_sections` 13 137 → 668.
- The layout window is +10–25%: taffy 0.14, `update_clipping` building clip
  lists, per-field compares in `update_uinode_geometry_recursive`,
  `CornerRadius::resolve`.

## Upstream fix (not applied here)

A fast path at the top of `clip_polygon` — classify the vertices against each
region once; return the input when all are inside, nothing when all are outside
one edge — cuts `clip_polygon` from 36 936 to 11 932 samples and the whole run
from 240 348 to 214 562 (0.19: 190 994). Patch against bevy_ui_render 0.20.0:
`clip_polygon.patch` next to this file. bevy-react cannot apply it without
forking `prepare_uinodes`; it belongs in Bevy.

## Ours

- table-ops: nothing. `label_buttons` (new) is 88 samples.
- layers-stress P1: `redistribute_ui_layers` went from 143 to 947 samples. With
  `cached.rs` gone, the members of clean cached layers stay in Bevy's retained
  extraction, so stock `queue_uinodes` queues them every frame (+~1 400 samples
  with its `IndexMap` work) and our steal pass drops them again. 0.19 hid them
  from extraction for 495 samples. Net about −5% fps on 500 static layers.
  Fixed later in the port (f685ce1): the steal is one in-place
  `retain`, and members of clean layers are parked by their camera slot in
  bevy's retained stores. On P1 the three symbol groups went from about 2 000
  to about 400 samples, and fps went from ≈ 505 to ≈ 530 in the agent's
  alternating runs (0.19 read ≈ 525 at the start of the port; not re-measured
  side by side).

## Gate for the following changes

The baseline is the 0.20 state after phase 1: `benchmark_results/bevy020-p1-{a,b}.json`,
P1 ≈ 490 fps, P2 ≈ 30–45, P3 ≈ 72. Latency legs move ±20% when other jobs run
on the machine; the `bevy_react_*` sample count from a `perf record` of the same
run is the sharper signal.
