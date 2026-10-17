# bevy_react_views (experimental)

A Rust runtime for **compiled views**: components whose template, bindings and
control flow are data, evaluated in Rust, with JavaScript kept for the logic
(handlers, effects). The plan, the measurements behind it and the open
questions are in `.scratch/compiled-views/spec.md`. Not published, not wired
into `bevy-react` yet; no Bevy dependency unless the `bevy` feature is on.

## The model

A component compiles to a `Template`: a static tree, its **bindings** (pure
expressions over the component's state **slots**), **blocks** (keyed lists,
conditional/nested components) and **effects**. `Runtime::mount` instantiates
one; writes to slots only _mark_ the bindings that read them; `flush` (or a
`poll` once a window has run) re-evaluates each marked binding once and
returns **commands** that mirror the bridge's wire ops one-to-one, sent only
when a value actually changed.

| Piece          | Where                       | What it does                                                                         |
| -------------- | --------------------------- | ------------------------------------------------------------------------------------ |
| IR, evaluator  | `ir.rs`, `eval.rs`          | templates, JS-semantics expressions (`toFixed`, `&&`, `Math.round`, objects, arrays) |
| Runtime        | `runtime.rs`, `compiled.rs` | mount, slot writes, flush, generation-tagged instance ids with slot reuse            |
| Keyed lists    | `list.rs`, `keyed.rs`       | LIS diff (minimal moves), staged lists, `update_item` deltas, selectors              |
| Windowed lists | `list_apply.rs`             | mount only the rows in view; spacers sized from unmounted counts                     |
| Children       | `child.rs`                  | `{cond && <C prop={..} />}`, ternaries, nesting                                      |
| Handlers       | `dispatch.rs`               | Rust-run click actions (`setX(expr)`, on the owner too), or JS                       |
| Effects        | `effects.rs`                | `useEffect` calls for the host, in React's order                                     |
| Batching       | `batch.rs`                  | a flush window anchored to the first write                                           |
| Bevy adapter   | `bevy.rs` (feature `bevy`)  | commands to `bevy_react_core::protocol::Op`; verified against real entities          |

## Running it

```sh
cargo test -p bevy_react_views                    # unit + integration tests, seconds
cargo run --release -p bevy_react_views --example bench    # stress-table timings
cargo test -p bevy_react_views --features bevy --test bevy_e2e    # end to end through the core's op app (builds Bevy)
```

The `bevy` feature links Bevy and V8 into every test binary it builds (hundreds
of MB each), so run just `--test bevy_e2e` with it. The benchmark times the runtime only (building the list, `set_list` /
`update_item`, `flush`): not the JS side, not Bevy's apply and layout.

## What is missing

The TSX compiler that emits templates, the JS side (handlers writing slots,
the identity diff that decides between a delta and a full list, effect
calls), animation bindings, and the conversion of commands that skips JSON
(pre-decoding a template's static props once). See the spike log in the plan.
