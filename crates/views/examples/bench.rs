//! The stress table's steps through the Rust view runtime, release build:
//!
//!     cargo run --release -p bevy_react_views --example bench
//!
//! Times building the list, `set_list` and `flush`. What the real path adds on
//! top (JS building the list and its identity diff, crossing the boundary,
//! Bevy's apply and layout) is not here.

use std::sync::Arc;
use std::time::Instant;

use bevy_react_views::{
    Binding, Block, Command, Compiled, ListItem, Runtime, TNode, Target, Template, Value, e,
};

const ROW_BG_A: &str = "#45475a";
const ROW_BG_B: &str = "#585b70";

/// A table row: `<button style={{ backgroundColor: bg }}><text>{`${id} ${label}`}</text></button>`.
fn row() -> Arc<Compiled> {
    Compiled::new(Template {
        nodes: vec![
            TNode::new("button", None).on("click", 1),
            TNode::new("text", Some(0)),
        ],
        bindings: vec![
            Binding {
                target: Target::Text(1),
                expr: e::concat([e::slot(0), e::lit(" "), e::slot(1)]),
            },
            Binding {
                target: Target::Style(0, "backgroundColor"),
                expr: e::slot(2),
            },
        ],
        slots: vec![Value::Num(0.0), Value::str(""), Value::str("")],
        ..Default::default()
    })
}

#[derive(Clone)]
struct Data {
    id: i64,
    label: String,
    bg: &'static str,
}

impl Data {
    fn fields(&self) -> Vec<Value> {
        vec![
            Value::Num(self.id as f64),
            Value::str(&self.label),
            Value::str(self.bg),
        ]
    }

    /// An item of `data` fields (id, label, bg) minus the ones a template
    /// does not take: the first `n`.
    fn item_fields(&self, n: usize) -> ListItem {
        ListItem::new(self.id, self.fields().into_iter().take(n).collect())
    }

    fn item(&self) -> ListItem {
        ListItem::new(
            self.id,
            vec![
                Value::Num(self.id as f64),
                Value::str(&self.label),
                Value::str(self.bg),
            ],
        )
    }
}

const ADJ: [&str; 5] = ["pretty", "large", "big", "small", "tall"];
const COL: [&str; 5] = ["red", "yellow", "blue", "green", "pink"];
const NOUN: [&str; 5] = ["table", "chair", "house", "bbq", "desk"];

fn build(n: usize, next_id: &mut i64) -> Vec<Data> {
    (0..n)
        .map(|i| {
            *next_id += 1;
            Data {
                id: *next_id,
                label: format!("{} {} {}", ADJ[i % 5], COL[(i / 5) % 5], NOUN[(i / 25) % 5]),
                bg: ROW_BG_A,
            }
        })
        .collect()
}

/// Items for `rows`, full fields only where `changed(i)` (the JS identity diff).
fn items(rows: &[Data], changed: impl Fn(usize) -> bool) -> Vec<ListItem> {
    rows.iter()
        .enumerate()
        .map(|(i, r)| {
            if changed(i) {
                r.item()
            } else {
                ListItem::same(r.id)
            }
        })
        .collect()
}

fn main() {
    type Run = fn(&mut Vec<Data>, &mut i64) -> Vec<ListItem>;
    let create = |n: usize| -> Run {
        match n {
            1000 => |rows, id| {
                *rows = build(1000, id);
                items(rows, |_| true)
            },
            _ => |rows, id| {
                *rows = build(10_000, id);
                items(rows, |_| true)
            },
        }
    };
    let text_every_2nd: Run = |rows, _| {
        for r in rows.iter_mut().step_by(2) {
            r.label.push_str(" !!!");
        }
        items(rows, |i| i % 2 == 0)
    };
    let text_1: Run = |rows, _| {
        let mid = rows.len() / 2;
        rows[mid].label.push_str(" !!!");
        items(rows, |i| i == mid)
    };
    let color_every_2nd: Run = |rows, _| {
        for r in rows.iter_mut().step_by(2) {
            r.bg = if r.bg == ROW_BG_A { ROW_BG_B } else { ROW_BG_A };
        }
        items(rows, |i| i % 2 == 0)
    };
    let swap_1: Run = |rows, _| {
        let last = rows.len() - 2;
        rows.swap(1, last);
        items(rows, |_| false)
    };
    let append_1k: Run = |rows, id| {
        let old = rows.len();
        rows.extend(build(1000, id));
        items(rows, |i| i >= old)
    };
    let remove_every_2nd: Run = |rows, _| {
        let mut i = 0;
        rows.retain(|_| {
            i += 1;
            i % 2 == 1
        });
        items(rows, |_| false)
    };
    let clear: Run = |rows, _| {
        rows.clear();
        vec![]
    };
    let plan: Vec<(&str, Run)> = vec![
        ("create 1k", create(1000)),
        ("text every 2nd (1k)", text_every_2nd),
        ("color every 2nd (1k)", color_every_2nd),
        ("swap 2 (1k)", swap_1),
        ("append 1k (to 1k)", append_1k),
        ("remove every 2nd (2k)", remove_every_2nd),
        ("clear (1k)", clear),
        ("create 10k", create(10_000)),
        ("text 1 (10k)", text_1),
        ("text every 2nd (10k)", text_every_2nd),
        ("clear (10k)", clear),
    ];

    const ROUNDS: usize = 9;
    // Per step: [build the list, set_list, flush] samples, in ms.
    let mut samples: Vec<[Vec<f64>; 3]> = vec![Default::default(); plan.len()];
    let mut cmd_counts = vec![0usize; plan.len()];
    for round in 0..ROUNDS {
        let mut rt = Runtime::with_id_base(1);
        let table = rt.mount(
            &Compiled::new(Template {
                nodes: vec![TNode::new("node", None)],
                blocks: vec![Block::list(0, row())],
                ..Default::default()
            }),
            0,
        );
        rt.flush();
        let (mut rows, mut next_id) = (Vec::<Data>::new(), 0i64);
        for (s, (_, run)) in plan.iter().enumerate() {
            let t0 = Instant::now();
            let list = run(&mut rows, &mut next_id);
            let t1 = Instant::now();
            rt.set_list(table, 0, list).unwrap();
            let t2 = Instant::now();
            let cmds: Vec<Command> = rt.flush();
            let t3 = Instant::now();
            std::hint::black_box(&cmds);
            cmd_counts[s] = cmds.len();
            if round > 0 {
                let ms = |a: Instant, b: Instant| (b - a).as_secs_f64() * 1000.0;
                samples[s][0].push(ms(t0, t1));
                samples[s][1].push(ms(t1, t2));
                samples[s][2].push(ms(t2, t3));
            }
        }
    }
    println!(
        "{:<24} {:>9} {:>9} {:>9} {:>9} {:>9}",
        "step", "total ms", "build", "set_list", "flush", "commands"
    );
    for (s, (name, _)) in plan.iter().enumerate() {
        let med: Vec<f64> = samples[s]
            .iter_mut()
            .map(|v| {
                v.sort_by(f64::total_cmp);
                v[v.len() / 2]
            })
            .collect();
        let total: f64 = med.iter().sum();
        println!(
            "{:<24} {:>9.3} {:>9.3} {:>9.3} {:>9.3} {:>9}",
            name, total, med[0], med[1], med[2], cmd_counts[s]
        );
    }

    burst();
    selection();
}

/// Ten one-row edits to a 10k-row list, as a burst of handler calls inside one
/// window: flushed after every edit, staged and flushed once, or sent as
/// per-record deltas and flushed once.
fn burst() {
    const EDITS: usize = 10;
    const ROUNDS: usize = 9;
    let mut next_id = 0i64;
    let base = build(10_000, &mut next_id);
    let mount = |base: &[Data]| {
        let mut rt = Runtime::with_id_base(1);
        let table = rt.mount(
            &Compiled::new(Template {
                nodes: vec![TNode::new("node", None)],
                blocks: vec![Block::list(0, row())],
                ..Default::default()
            }),
            0,
        );
        rt.set_list(table, 0, base.iter().map(Data::item).collect())
            .unwrap();
        rt.flush();
        (rt, table)
    };
    // The list after each edit (cumulative), built outside the timed region.
    let versions: Vec<Vec<Data>> = {
        let mut rows = base.clone();
        (0..EDITS)
            .map(|k| {
                rows[k * 100].label.push_str(" !!!");
                rows.clone()
            })
            .collect()
    };
    type Variant = (&'static str, fn(&mut Runtime, usize, &[Vec<Data>]) -> usize);
    let variants: [Variant; 3] = [
        ("flush after every edit", |rt, table, versions| {
            let mut n = 0;
            for (k, rows) in versions.iter().enumerate() {
                rt.set_list(table, 0, items(rows, |i| i == k * 100))
                    .unwrap();
                n += rt.flush().len();
            }
            n
        }),
        ("set_list x10, one flush", |rt, table, versions| {
            for (k, rows) in versions.iter().enumerate() {
                rt.set_list(table, 0, items(rows, |i| i == k * 100))
                    .unwrap();
            }
            rt.flush().len()
        }),
        ("update_item x10, one flush", |rt, table, versions| {
            for (k, rows) in versions.iter().enumerate() {
                let r = &rows[k * 100];
                rt.update_item(table, 0, &r.id.into(), r.fields()).unwrap();
            }
            rt.flush().len()
        }),
    ];
    println!("\nburst: {EDITS} one-row edits to a 10k list");
    println!("{:<28} {:>9} {:>9}", "variant", "total ms", "commands");
    for (name, run) in variants {
        let mut samples = Vec::new();
        let mut cmds = 0;
        for round in 0..ROUNDS {
            let (mut rt, table) = mount(&base);
            let t0 = Instant::now();
            cmds = run(&mut rt, table, &versions);
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            if round > 0 {
                samples.push(ms);
            }
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "{:<28} {:>9.3} {:>9}",
            name,
            samples[samples.len() / 2],
            cmds
        );
    }
}

/// Selecting a row of a 10k-row table, then moving the selection: the table's
/// `selected` state reaches the rows through a selector, so each change flips
/// the old and the new row only.
fn selection() {
    let row = Compiled::new(Template {
        nodes: vec![TNode::new("button", None), TNode::new("text", Some(0))],
        bindings: vec![
            Binding {
                target: Target::Text(1),
                expr: e::concat([e::slot(0), e::lit(" "), e::slot(1)]),
            },
            Binding {
                target: Target::Style(0, "backgroundColor"),
                expr: e::cond(e::slot(2), e::lit("#89b4fa"), e::lit(ROW_BG_A)),
            },
        ],
        slots: vec![Value::Num(0.0), Value::str(""), Value::Bool(false)],
        ..Default::default()
    });
    let table = Compiled::new(Template {
        nodes: vec![TNode::new("node", None)],
        blocks: vec![Block::list(0, row).select(0, 0, 2)],
        slots: vec![Value::Null],
        ..Default::default()
    });
    let mut next_id = 0i64;
    let rows = build(10_000, &mut next_id);
    let mut times = [Vec::new(), Vec::new(), Vec::new()];
    let mut counts = [0usize; 3];
    for round in 0..9 {
        let mut rt = Runtime::with_id_base(1);
        let t = rt.mount(&table, 0);
        rt.set_list(t, 0, rows.iter().map(|r| r.item_fields(2)).collect())
            .unwrap();
        rt.flush();
        for (i, sel) in [Value::Num(5000.0), Value::Num(7000.0), Value::Null]
            .into_iter()
            .enumerate()
        {
            let sel = if let Value::Num(n) = sel {
                Value::Num(rows[n as usize].id as f64)
            } else {
                sel
            };
            let t0 = Instant::now();
            rt.set(t, 0, sel);
            let cmds = rt.flush();
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            counts[i] = cmds.len();
            if round > 0 {
                times[i].push(ms);
            }
        }
    }
    println!("\nselection in a 10k-row table");
    println!("{:<28} {:>9} {:>9}", "step", "total ms", "commands");
    for (i, name) in ["select a row", "move the selection", "clear the selection"]
        .into_iter()
        .enumerate()
    {
        times[i].sort_by(f64::total_cmp);
        println!(
            "{:<28} {:>9.4} {:>9}",
            name,
            times[i][times[i].len() / 2],
            counts[i]
        );
    }
}
