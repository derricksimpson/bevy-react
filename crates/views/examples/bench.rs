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
                blocks: vec![Block::For {
                    parent: 0,
                    item: row(),
                }],
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
}
