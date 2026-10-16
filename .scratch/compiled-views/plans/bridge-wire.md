# Plan: is serde over JSON the right bridge, or shared memory?

Question: CLAUDE.md "Architecture: the Rust↔JS bridge" sends ops as one JSON
string decoded by serde. Would a shared-memory, realtime architecture with no
serialization be better? Everything below was read, not run, except where it
says **measured**. Code is cited by symbol.

## Verdict

1. **Serde stays, as the typing layer.** It is where the element/style
   registry, `DecodeScope`, unknown-key warnings and decode-fallback warnings
   live, and the decode is already format-agnostic. It is not the cost.
2. **JSON text stays for React's ops.** V8's native `JSON.stringify` is hard
   for a JS-written binary encoder to beat on generic objects, the decode
   already runs off the Bevy thread, and the whole bevy-react share of a
   profile is ~1.6%.
3. **Shared memory is the right architecture for _state_, not for
   _commands_.** Build the compiled-views state slots (`../spec.md`) as
   Rust-owned memory both sides read and write in place. That is where
   "realtime, no serialization" pays: scalar writes and Rust-driven values
   with no message at all.
4. **Do not build a shared-memory ring for React ops.** It moves the decode
   onto Bevy's main thread, needs a binary encoder written in JS, needs
   cross-origin isolation for `SharedArrayBuffer` on the web, and targets a
   slice of the frame that is already small.
5. **Small fixes worth doing now**: the double conversion on `emit`/`request`
   (the `TODO(review)` above `op_emit`), and measuring `flushMs` against the
   other legs before any wire work.

## 1. How the bridge works today (facts)

- **JS → Bevy ops** (`js/src/bridge.ts` `flushRaw`, `crates/core/src/js_thread.rs`
  `op_flush`):
  - The reconciler builds plain `Op` objects; `buildUpdateOp` sends deltas only
    (a re-render with no value change sends nothing).
  - `flushRaw` calls `JSON.stringify(batch)` once per commit and passes the
    string to `op_flush`, a `#[op2(fast)]` op taking `&str`.
  - `op_flush` runs `serde_json::from_str::<OpBatch>` **on the JS thread,
    inside the call**, then sends the typed `Vec<Op>` over crossbeam.
  - Bevy's `apply_js_ops` (in `ReactApplySet`, `Update`) receives typed Rust
    values. **Bevy's frame never parses anything.**
  - The comment on `op_flush` records why JSON: a `serde_v8` object walk cost
    60–75% more at the boundary (per-property V8 API traffic).
  - `Op`'s `Deserialize` is hand-written (`OpVisitor`) so it streams without
    serde's `Content` buffering, which was once the dominant cost.
- **The decode is format-agnostic.** `Op`, `Props` and the style store decode
  through `serde::Deserializer`, the style values via erased-serde through the
  thread registry. Some codecs buffer through `serde_json::Value::deserialize`
  (`protocol/animatable.rs`, `protocol/background_image.rs`), which works with
  any **self-describing** format (one that supports `deserialize_any`).
- **App messages**: `op_emit`/`op_request` take `#[serde] serde_json::Value`
  (a `serde_v8` walk), then convert again into the typed struct
  (`TODO(review)` above `op_emit`). `op_animate` decodes `AnimationCommand`
  with `serde_v8`.
- **Bevy → JS**: `op_next_event` is async and returns `#[serde]
Option<Outbound>`, so `serde_v8` builds one JS object per message (UI events,
  element events, named events, responses).
- **Web host** (`host/web.rs`): the same JSON string for ops;
  `serde_wasm_bindgen` for the rest; outbound drained per frame in `Last`.
- **Latency**: UI events are collected in `Update` and sent to the JS thread
  at once; the JS reaction's ops arrive while that frame runs, so they apply
  at the earliest in the next frame's `apply_js_ops`. The devtools
  `preApplyMs` leg measures this wait.

## 2. Where the time goes (measured)

- `.scratch/bevy-0.20/bench-phase1.md`, real app, whole-process `perf`
  profile of the table-ops run: bevy-react's crates 3 864 of 240 348 samples
  (**1.6%**). That count includes `op_flush`'s decode (native code in
  `bevy_react_core`). `bevy_ui_render` was 63 885, layout 45 220.
- Same file, 10k rows p50: `updateText1` total 31.3 ms, layout 19.0 ms.
- This session (stub host, JS only, V8): React's own work for "text 1 of 10k"
  was 4.2 ms; the 1k Create batch was 253 KB of JSON, a one-row update ~65
  bytes per op.
- **Not measured**: the stringify vs parse split. The stress capture reports
  `flushMs` (stringify + decode + send) per op; read it in phase 0 of
  `../spec.md` before changing the wire.

Conclusion: removing serialization entirely would save at most the
`flushMs` leg on the JS thread, which runs in parallel with Bevy's frame. It
would not touch the layout/render cost that dominates.

## 3. Options

### W0: keep JSON text + serde (today)

- Pros: native `JSON.stringify` speed on V8; decode off the main thread;
  debuggable (the devtools recorder replays ops as JSON); one wire for
  native and web; tested.
- Cons: number formatting and parsing, string escaping, every key spelled out
  in every op, one big string allocation per commit. On QuickJS,
  `JSON.stringify` is still native C, so the relative cost stays similar.

### W1: binary tape + the same serde decode

A self-describing binary format (tagged values, keys interned to `u16` per
session) written by JS into an `ArrayBuffer`, decoded by a custom
`serde::Deserializer`. Every codec, `DecodeScope` and warning keeps working.

- Saves: float formatting/parsing, escaping, repeated key bytes.
- Costs: the encoder walks generic objects **in JS**, which on V8 is often
  slower than native `JSON.stringify` (**estimate**; the case to measure);
  the devtools recorder needs a tape reader; a second codepath to keep exact.
- Where it clearly wins: when the **compiler** writes the tape, not a generic
  walker. Compiled views know types at build time (`../spec.md`), so their
  structural messages can be emitted pre-encoded.

### W2: shared-memory command ring (JS writes, Bevy reads)

An SPSC ring in Rust-owned memory exposed to JS as an `ArrayBuffer`
(`SharedArrayBuffer` across threads), with commit markers.

- Saves: the channel hop, the string allocation, the copy.
- Costs:
  - needs W1's binary encoding anyway (the ring holds bytes);
  - if Bevy decodes from the ring, the decode **moves onto the main thread**,
    which today never decodes. If the JS thread's Rust side decodes it, that
    is what `op_flush` already does;
  - atomics and memory ordering across threads; a commit must apply atomically;
  - web: `SharedArrayBuffer` needs COOP/COEP headers (cross-origin isolation),
    which GitHub Pages cannot send without a service-worker workaround. (On
    web JS and wasm share one thread, so a plain view of wasm memory works
    there with no SAB, but views detach on `memory.grow` and must be rebuilt.)
- Verdict: not for React ops.

### W3: shared state slots (the realtime part)

Rust-owned typed memory (`f64`/`u32`/interned string ids, plus a dirty bitset)
that JS reads and writes as typed arrays; Rust scans the dirty bits once per
frame.

- Gives exactly what "no serialization" promises, for **scalars**: a handler's
  `setCount(n)` is one typed-array store; Rust-driven values (an animation's
  current value, the synth transport, a pointer position) are readable from JS
  synchronously, with no event and no request round trip.
- Fits compiled views directly: their state slots _are_ these slots.
- Limits: structured values (gradients, filter chains, text runs, record
  lists) still need an encoding (W0 or W1); multi-field values need a
  consistent snapshot (a seqlock or a per-frame double buffer) when JS runs
  on its own thread.
- Engine support: V8 (deno_core) can wrap Rust memory as a backing store;
  QuickJS can create an `ArrayBuffer` over Rust memory with a free callback;
  web uses views over wasm memory (same thread, no atomics). QuickJS-ng's
  `SharedArrayBuffer`/`Atomics` support needs checking before relying on it
  across threads.

### W4: in-frame JS (no JS thread)

Run JS on Bevy's main thread at a fixed point in the frame, as the web host
effectively does. Shared memory then needs no atomics, and an event's reaction
lands in the same frame (deterministic, one frame less latency).

- Cost: JS time adds straight to the frame (today it overlaps on another
  core). Viable only when per-frame JS is small, i.e. with compiled views, and
  most attractive with QuickJS (cheap to embed on the main thread).
- Verdict: a later option (phase 5 of `../spec.md`), not a default.

## 4. Recommendation, in order

1. **Phase 0**: read `flushMs`, `jsMs`, translate, command and layout from the
   stress capture at 1k/10k. If `flushMs` is a large share of `jsMs` on the
   big batches, prototype W1 for `Create` only and compare against
   `JSON.stringify`.
2. **Now, small**: decode `emit`/`request` payloads straight into the typed
   struct (one conversion instead of two), as `op_flush` already does for ops.
3. **With compiled views**: build the state slots as W3 from the start; send
   structural changes (list patches, island props) through the existing
   channel, pre-encoded by the compiler when that measures faster.
4. **Not planned**: W2 for React ops.
