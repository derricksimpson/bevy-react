# Compiled views: a Rust-run replacement for the React runtime

Status: proposal, 2026-10-16. Nothing here is built. Measurements are marked
**measured** (with how) or **estimate**. The bridge/serialization question has
its own plan: `plans/bridge-wire.md`.

## Verdict

1. **Keep a JS layer.** Game logic, reducers, handlers, effects and libraries
   stay JavaScript. They are not compilable in general, and they don't need to
   be: they run on discrete events, not every frame.
2. **Compile the views.** A build step turns opted-in TSX components into a
   **view IR** (templates + binding expressions + control-flow blocks). A Rust
   **view runtime** owns the state those views read, evaluates the bindings,
   and writes the result into the bridge's existing per-node props, so every
   engine (writers, transitions, layers, animations) keeps working unchanged.
3. **Semantics: observably React.** A compiled component behaves exactly as if
   React re-rendered it on every state change. It never re-renders: only the
   bindings that read changed inputs re-evaluate. (React Compiler makes the
   same promise for memoization; this goes further.)
4. **Ship as islands inside React trees**, opted in per component. No
   big-bang migration, and every island can be measured against its React
   original.
5. **Do not:** swap the JS engine as the goal, adopt Preact, switch the runtime
   to Solid, or generate Rust source from TSX (verdicts and reasons below).

## Decided in conversation

- A JS layer is a must, because of the JS logic.
- Precompiled, React-like views are the bet.

## Evidence

**Measured** this session with a stub host (no Bevy): the production stress
bundle (`examples/stress`, a js-framework-benchmark-style table), driven
through its own `bench.runStep` protocol, the flushed JSON counted and not
parsed. JS-side time only, ms, median of 3 rounds:

| Step                     | React / V8  | React / V8 jitless | React / QuickJS-ng | Preact / QuickJS-ng | Solid / QuickJS-ng |
| ------------------------ | ----------- | ------------------ | ------------------ | ------------------- | ------------------ |
| Create 1k                | 7.1         | 45                 | 147                | 137                 | 47                 |
| Text every 2nd of 1k     | 1.4         | 8.6                | 38                 | 56                  | 5.3                |
| Swap 2 of 1k             | 2.6         | 36                 | 70                 | 11                  | 2.4                |
| Create 10k               | 263         | 3401               | 9053               | 2025                | 548                |
| Text 1 of 10k            | 4.2         | 35                 | 141                | 133                 | 1.3                |
| Text every 2nd of 10k    | 18.5        | 85                 | 391                | 564                 | 61                 |
| Bundle, minified (bytes) | 146 K + app | same               | same               | 28 K total          | 17 K total         |

- V8 = node 22 (same engine as deno_core, not the same embedding). QuickJS-ng
  via `rquickjs` 0.12. Boa 0.21 was ~4× slower than QuickJS and is out.
- Preact ran the **unchanged** `App.tsx` (`react` aliased to `preact/compat`)
  over a ~150-line fake DOM that emits our ops; the updates on the wire were
  identical op-for-op. Solid was a hand port to `solid-js/universal`, and it
  put _more_ on the wire for Create (separate text nodes, whole styles).
- React's own `getHostSibling` was 58% of a V8 CPU profile: its quadratic
  placement walk when many new children land in an existing parent (why
  Create 10k ≈ 37× Create 1k).
- Cold start, demos bundle (~1.5 MB of JS, parse + run): V8 50 ms, QuickJS
  95 ms.
- The harness lived in the session scratchpad and is gone with the container;
  re-creating it is phase 0 work if we want it in-tree (`examples/stress`).

**Measured earlier** (`.scratch/bevy-0.20/bench-phase1.md`, real app, 10k
rows, p50): `updateText1` total 31.3 ms of which layout 19.0 ms; `updateColor1`
21.5 / 9.7 ms; in a whole-process `perf` profile, bevy-react's crates were
3 864 of 240 348 samples (1.6%), Bevy's UI renderer and layout the rest of the
big items.

What that means for expectations: on V8 the JS share of a surgical update at
10k rows is a few ms out of ~30 ms; Bevy's layout and UI render dominate. The
compiled views win **latency, JS-engine independence, footprint, and per-frame
UI**; a large throughput win on V8 is not expected. Phase 0 confirms this with
the stress capture before anything is built.

## Paths considered

| #   | Path                                       | Verdict         | Reason                                                                                                                                |
| --- | ------------------------------------------ | --------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| A   | Swap V8 for QuickJS, keep React            | No, as the goal | React runs ~20× slower there. Revisit after E for binary size and no-JIT platforms (iOS, consoles), where V8 would be jitless anyway. |
| B   | Drop-in replacement (Preact)               | No              | Compatible, small, but no faster on a small engine; still a virtual DOM.                                                              |
| C   | Switch the runtime to Solid                | No, as the goal | New programming model, and JS + JSON stay on every update. Kept as the reference for what compiled output should look like.           |
| D   | Generate Rust source from TSX              | No              | The logic can't be compiled; every UI edit becomes a cargo rebuild (hot reload gone).                                                 |
| E   | **Compiled views: TSX → IR, Rust runtime** | **Yes**         | The only path that takes JS off the update path while keeping JS logic and React-like authoring.                                      |

## The design

### Authoring

- Normal TSX, opted in per component with a directive (`"use bevy";` as the
  first statement, name open). Everything else stays React.
- Hooks: `useState`, `useMemo`, `useRef`, `useEffect`, `useCallback`,
  `useSharedValue` (and later `useContext`). That is what the apps use today:
  245 `useState`, 169 `useEffect`, 52 `useRef`, 15 `useCallback`, 7 `memo`,
  6 `useMemo`, 6 `useSyncExternalStore`, no Suspense, transitions or portals
  (grep of `examples/*/ui/src` and `js/src`).
- Typed against the same generated `BevyIntrinsicElements`/`BevyStyle`, so
  the compiler sees the element and style types the Rust registry declared.

### What the compiler emits per component

1. **Template**: the static tree. Element kinds, static props and static
   styles, decoded **at build time** by the Rust registry (the build runs the
   exporter binary in a "decode" mode) into the same `Props` the bridge keeps.
2. **Bindings**: each dynamic hole (a prop, one style property, a text run)
   gets an expression in a small pure IR.
3. **Blocks**: `if`/early return, keyed `for` (from `.map` with a `key`),
   `switch`, child component instances.
4. **JS references**: handlers, effects, and every expression the compiler
   cannot prove pure stay in the normal JS bundle, referenced from the IR by id.

### Runtime ownership

| Rust view runtime                                                    | JS                                           |
| -------------------------------------------------------------------- | -------------------------------------------- |
| state slots (`useState`, props of the island)                        | handlers, effects, logic, libraries          |
| component instances, binding evaluation, dependency tracking         | thunks for non-compilable expressions        |
| keyed lists, conditional blocks                                      | owns structured app data (arrays of records) |
| writes into `JsBridge::props_cache` + `StyleDirty`/`AttrDirty` masks | writes state through a batched channel       |

The output side is the important constraint: the runtime produces the same
merged `Props` and dirty masks `Props::merge_delta` produces today, and then
calls the same writers. It skips the JSON, the decode and the JS-side diff;
it does not skip the writers. So transitions, layers, filters, shared
elements and animated bindings need no changes.

### State and data

- **Scalars** (`useState` numbers, strings, bools, enum strings) live in Rust
  slots. A JS handler writes a slot; Rust re-evaluates dependents in the same
  frame it applies the write. How the write crosses (a batched message or a
  shared-memory slot) is `plans/bridge-wire.md`.
- **Lists of records** stay owned by JS (the reducers build them, often
  immutably: `rs.map(...)`). On change, a JS-side **reference diff** of old vs
  new array (O(n) identity compares, cheap even on QuickJS) emits a patch:
  inserted/removed/moved keys plus the records whose identity changed. Rust
  applies it to the keyed block. This is the job React's `memo` did, without
  the render.
- **Derived values** (`useMemo`, consts computed in the body) compile to memo
  nodes when pure, else to JS thunks re-run when their inputs change.

### IR sketch (Rust)

```rust
struct Component { template: Template, slots: Vec<SlotDecl>, js: Vec<ThunkId> }
struct Template  { nodes: Vec<TNode>, holes: Vec<Hole>, blocks: Vec<Block> }
struct TNode     { kind: ElementId, parent: Option<u16>, props: Box<Props> } // static, pre-decoded
enum Hole  { Prop { node: u16, key: AttrKey, expr: ExprId },
             Style { node: u16, prop: StylePropId, expr: ExprId },
             Text { node: u16, expr: ExprId } }
enum Expr  { Const(ValueId), Slot(SlotId), Prop(PropId), Field(ExprId, FieldId),
             Not(ExprId), Eq(ExprId, ExprId), Cond(ExprId, ExprId, ExprId),
             Concat(Vec<ExprId>), Num(NumOp, ExprId, ExprId), Builtin(Fn, Vec<ExprId>),
             Js(ThunkId, Vec<ExprId>) }            // the escape hatch
enum Block { If { cond: ExprId, then: TemplateId, otherwise: Option<TemplateId> },
             For { list: SlotId, key: FieldId, item: TemplateId },
             Child { component: ComponentId, props: Vec<(PropId, ExprId)> } }
```

The stress row, compiled:

```tsx
const Row = ({ row, selected }) => (
  <button
    style={selected ? SEL : { ...ROW, backgroundColor: row.bg }}
    onClick={() => select(row.id)}
  >
    <text>{`${row.id} ${row.label}`}</text>
  </button>
);
```

```
template Row {
  button(style: $0, on click: js#17) { text { $1 } }
  $0 = Cond(Prop.selected, Const(SEL), Merge(Const(ROW), { backgroundColor: Prop.row.bg }))
  $1 = Concat(Prop.row.id, " ", Prop.row.label)
}
```

### What compiles, what falls back, what is rejected

| Code in a compiled component                                | Result                                           |
| ----------------------------------------------------------- | ------------------------------------------------ |
| JSX structure, static props and styles                      | template, decoded at build time                  |
| reads of props/state, ternary, `&&`, template strings, math | binding, evaluated in Rust                       |
| `if`/early return, `.map` with a `key`, `switch`            | blocks                                           |
| object spread of a const + one dynamic field                | binding (`Merge`)                                |
| helper calls, other non-pure expressions                    | JS thunk, re-run when its inputs change (+ note) |
| handlers, `useEffect`                                       | JS, by design                                    |
| conditional hooks, mutating props, `.map` without a key     | compile error                                    |

Every fallback emits a compiler note (file, line, why), so "how much of this
component runs in Rust" is visible.

### Integration with React (islands)

- To React, a compiled component is one host element (working name
  `<compiled component="Row" props={...}>`) whose subtree the Rust runtime
  owns. Its props cross only when they change, as today's update ops.
- Leaf islands first: rows, cards, HUD widgets (the hot spots). React
  children inside an island (slots) come later, as a hole that hosts
  React-managed children.

### Hot reload, devtools, web

- **Hot reload**: the IR is a data file next to `app.js`. Rust reloads it;
  state survives when a component's slot layout (its hook signature) is
  unchanged, the Fast Refresh rule. The JS bundle re-executes as today.
- **Devtools**: compiled nodes are ordinary bridge entities (`ReactNode`), so
  the tree, styles, layers and warnings tabs work. New: a "Compiled" view of
  slots, bindings and fallbacks.
- **Web**: same IR, Rust in wasm, JS in the page; nothing native-specific.

## Phases

| #   | Phase        | Work                                                                                                        | Exit criterion                                                                                     |
| --- | ------------ | ----------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| 0   | Measure      | Stress capture (`--run table-ops`, release + prod) at 1k/10k: `jsMs`, `flushMs`, translate, command, layout | Numbers recorded here; the expected win per leg written down                                       |
| 1   | Spike        | Hand-written IR for the stress table, minimal Rust runtime, `<compiled>` island, list patch channel         | Same ops and pixels as React; JS-thread time on update steps ≥ 5× lower; end-to-end total no worse |
| 2   | Compiler MVP | Templates, bindings, blocks, handlers to JS, fallback notes                                                 | The stress `App` compiles with only the directive added                                            |
| 3   | DX           | IR hot reload, devtools "Compiled" view, errors mapped to TSX source                                        | Edit a compiled row, keep its state; a binding error points at the TSX line                        |
| 4   | Showcases    | Synth cards, civilization HUD panels                                                                        | Measured against their React versions                                                              |
| 5   | Options      | QuickJS engine feature; in-frame JS; IR → Rust for release builds                                           | Each its own plan                                                                                  |

## Open questions

1. Directive name and file-level vs component-level opt-in.
2. Compiler language: TypeScript first (TS type info, fits `build-lib.mjs`'s
   esbuild/SWC pipeline), Rust (oxc) later if build speed demands it.
   Recommendation: TypeScript.
3. Source syntax: TSX first. TSRX (statement-based `@if`/`@for`, easier to
   compile) could become a second front-end, since the IR is the contract.
4. Context across the island boundary: React context values crossing into
   compiled components (read-only props first).
5. `useSyncExternalStore` stores (`globalThis` stores in cyberpunk/coldwake):
   map a store to slots, or keep it a JS thunk.
6. Build-time decoding: the compiler needs the app's registry (its own
   elements and styles), i.e. the exporter binary run at build time, as
   `bevy:generate` does.

## Risks

- **It is a framework**: compiler, IR, runtime, keyed lists, instances,
  effects lifecycle, error mapping, devtools. Phases 1–2 are the cheapest
  point to stop.
- **Two-language debugging**: a binding is written in TSX and evaluated in
  Rust; source mapping is not optional.
- **Semantic drift** from React in edge cases (effect timing, batching). The
  spike must pin the stress table's exact op stream against React's.
- **Expectation**: on V8 the throughput gain is bounded by Bevy's own layout
  and render cost (Evidence). The case rests on latency, engine independence
  and per-frame UI, so phase 0 matters.

## Spike log (`crates/views`, `bevy_react_views`)

See `crates/views/README.md` for the map. 119 tests (+4 end to end with `--features bevy`); `cargo run --release -p
bevy_react_views --example bench` for the numbers. The numbers time the
runtime only: building the list, `set_list`/`update_item`, `flush`. They
exclude the JS side (building the list, the identity diff, the boundary) and
Bevy's apply and layout, so they are **not** comparable to the React table in
Evidence beyond the shape.

| Piece          | What it does                                                                                                                                                                       |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| IR + evaluator | templates, bindings as pure expressions over slots; JS semantics for `+`, `&&`, `Math.round`, `toFixed` (ties checked against node), number printing; objects and arrays as values |
| Runtime        | mount, slot writes mark bindings, flush re-evaluates each once and emits a command only when the value changed; generation-tagged ids, slots reused                                |
| Keyed lists    | LIS diff, so a swap of 2 of 1,000 rows is 2 moves (React's `lastPlacedIndex` rule moves 997); 500 random-edit rounds pin minimal moves                                             |
| Batching       | `poll()` flushes after a window anchored to the first write; staged lists coalesce into one diff; injectable clock                                                                 |
| Delta path     | `update_item` changes one record in place; an unchanged key order skips hashing and positioning                                                                                    |
| Selectors      | `selected === row.id` as a flag slot; a selection change flips only the old and the new row                                                                                        |
| Child blocks   | conditional/nested components with props bound to parent expressions; placement before a static sibling; ternaries; a badge inside a list row                                      |
| Handlers       | a click runs `setX(expr)` in Rust, on the owner too (a row selects itself in its table), then optionally JS; all actions read pre-click state                                      |
| Effects        | `useEffect` calls for the host in React's order (children first, cleanup before re-run, undone writes run nothing)                                                                 |
| Windowed lists | data stays in Rust, entities only for the rows in view; spacers sized from unmounted counts; scrolling is the same diff over the mounted slice                                     |
| Bevy adapter   | `bevy.rs` (feature `bevy`): commands to `bevy_react_core`'s `Op`; e2e tests in `tests/bevy_e2e.rs`                                                                                 |

Measured (release, median; run-to-run spread about 30%):

| Step                                 | Runtime (ms)     | Commands | Note                                                               |
| ------------------------------------ | ---------------- | -------- | ------------------------------------------------------------------ |
| create 1k / 10k rows                 | 0.6 / 11         | 4k / 40k | React on V8: 7.1 / 263 (JS only)                                   |
| text every 2nd of 1k / 10k           | 0.25 / 3         | 500 / 5k | React: 1.4 / 18.5                                                  |
| swap 2 of 1k                         | 0.06             | 2        | React: 2.6                                                         |
| text 1 of 10k, via `set_list`        | 0.16             | 1        | was 3-4 ms before the unchanged-order fast path                    |
| 10 one-row edits of 10k              | 2.0 / 1.6 / 0.35 | 10       | flush each / `set_list` x10 + one flush / `update_item` x10        |
| select / move selection (10k)        | 0.001-0.005      | 1-2      |                                                                    |
| click a row (dispatch + flush, 10k)  | 0.0015           | 2        | handler runs in Rust, no JS                                        |
| clear 10k                            | 2.6              | 10k      | React: 5.8                                                         |
| **windowed** create 10k (30 mounted) | 2.2              | 127      | most of it is the benchmark building the list; was 40,000 commands |
| windowed scroll 1 row / jump 5000    | 0.01 / 0.03      | 7 / 152  |                                                                    |
| windowed change every 2nd of 10k     | 2.0              | 15       | was 5,000 commands                                                 |

Findings:

- **A flush window does little for full-list sets.** Ten `set_list` calls in
  one window cost 1.6 ms against 2.0 ms flushing each: each call still builds
  and compares the whole list. The win is the **delta API** (`update_item`):
  cost follows the number of changed records, about 0.2-0.3 us each at scale.
  So the JS side should send records that changed in place as deltas and a
  full list only for structural change. The window still pays for writes to
  the same slot or list.
- **Windowing is the large win for big lists**, in commands (127 vs 40,000)
  as much as in time: Bevy's per-row layout and render cost, which dominated
  the earlier profile, is paid for rows in view only.
- **Allocations were the cost of mass create/teardown.** A mounted row went
  from 10 allocations to 4 (text reuses the evaluated `Arc<str>`, `Concat`
  reuses a buffer, exact-size prop/style vectors), keys hash with Fx, the
  diff returns early when a side is empty, and handlers resolve by id range
  instead of a per-node table. Swap 0.15 -> 0.06 ms, clear 10k 6.8 -> 2.6 ms.
- **The instance table leaked** under churn (every mounted row left an entry
  forever, so a list that scrolls forever grew without bound). Found by
  reasoning, fixed with slot reuse behind generation-tagged ids; a stale id
  names nothing.
- **Structured values were a blocker** (a transform, a shadow list): added.

**End to end (first real number).** Commands go through the adapter into the
core's real `apply_js_ops` (the headless op app: apply only, no layout or
render). A 10k-row table, one run (`cargo test -p bevy_react_views --features
bevy --test bevy_e2e -- --ignored --nocapture`):

| 10k-row table      | runtime | adapter (via JSON) | Bevy `apply_js_ops` | commands | entities |
| ------------------ | ------- | ------------------ | ------------------- | -------- | -------- |
| all rows           | 12.6 ms | 12.1 ms            | **122 ms**          | 40,002   | 20,001   |
| windowed (30 rows) | 1.1 ms  | 0.2 ms             | **1.2 ms**          | 127      | 63       |

This settles the phase 0 question: the runtime is not where the time goes.
Bevy's own apply is about 5x the runtime's cost for the same table (and layout
and render come on top), so the lever is **how many entities exist**, which is
what windowing changes by 160x here. A faster reconciler alone would move the
total by less than a tenth. The adapter's cost (JSON, decoded against the
element) is as large as the runtime's own: pre-decoding a template's static
props once is the next saving on the Rust side.

**Where Bevy's apply goes** (callgrind on the same 10k-row create, core at
`opt-level=3`, no debug assertions). The build profile did not matter: apply
was 117 ms optimized against 122 ms at the dev profile, so Bevy's apply is
about **10x** the runtime's 12 ms, not 5x. Of the 404 M instructions in
`App::update`, **217 M (54%) are `EntityCommands::add_related`**: every
`Op::Append` calls `commands.entity(p).add_child(c)` (`reconcile/apply.rs`,
the first-time attach) on an entity that was already spawned with its full set
of UI components, and inserting `ChildOf` moves the whole row to a new
archetype table (`Tables::move_row`, `Column::initialize_from_unchecked`,
13% of all instructions in `memcpy`). About 10,000 instructions per attach.
That cost is paid by every path that mounts many nodes at once, React's
included. Candidate fix, in the core: put `ChildOf(parent)` in the spawn
bundle (reserve the entities for a batch's creates first, so a parent that is
created after its children is still nameable), so no attach moves a row.

**Done: spawn-time parenting** (`crates/core/src/reconcile/spawn_plan.rs`, 10
tests in `spawn_plan_tests.rs`, the rules in CLAUDE.md). A pre-pass over the
batch plans only the parents it can prove pure (appends of fresh nodes, once
each, in creation order), reserves the entity of a parent created after its
children, and spawns the children with `ChildOf` in the bundle; the `Append`
arm keeps its bookkeeping and skips the attach. Result on the same 10k-row
create, instructions (callgrind, whole test): **683 M -> 587 M (-14%)**; the
plan's own pre-pass costs 3 M with dense tables over the batch's id range
(a first version with hash maps cost 47 M and ate half the saving). Wall
clock for `apply_js_ops` is noisy (90-115 ms either way on this machine);
the median moved from about 102 ms to about 96 ms. The expectation above
(54% of the apply) was too high: the archetype moves were removed, but the
remaining cost is the spawn commands themselves (large bundles copied into
the command queue, then into the table; `memcpy` is still 10% of all
instructions), which only a direct-world apply path or fewer components per
node would cut. Windowing remains the lever that matters (160x fewer
entities). Full core suite (896) and the feature crates pass.

Not done: animation bindings (`{ animated }` values need a shared-value id
allocated per instance), the JS side, the TSX compiler front-end, pre-decoding
a template's static props once for the adapter (commands still copy them per
instance), context, refs.

Environment note: the container shipped rustc 1.97.0; Bevy 0.20 needs 1.97.1,
so the Bevy build needed `rustup update stable` (now 1.99.1). An earlier
statement in the session that the `stress` release build had finished was
wrong (it had failed on the compiler version).
