//! Batching: the flush window, coalesced writes, staged lists, and the
//! delta path. Time is a manual clock, so nothing sleeps.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use bevy_react_views::{
    Binding, Block, Clock, Command, Compiled, Key, ListError, ListItem, Runtime, TNode, Target,
    Template, Value, e,
};

/// A manual clock: `tick(ms)` advances it.
struct Time(Arc<AtomicU64>);

impl Time {
    fn new() -> (Time, Clock) {
        let micros = Arc::new(AtomicU64::new(0));
        let clock: Clock = {
            let micros = Arc::clone(&micros);
            Arc::new(move || Duration::from_micros(micros.load(Ordering::SeqCst)))
        };
        (Time(micros), clock)
    }

    fn tick(&self, ms: u64) {
        self.0.fetch_add(ms * 1000, Ordering::SeqCst);
    }
}

fn row() -> Arc<Compiled> {
    Compiled::new(Template {
        nodes: vec![TNode::new("button", None), TNode::new("text", Some(0))],
        bindings: vec![Binding {
            target: Target::Text(1),
            expr: e::concat([e::slot(0), e::lit(" "), e::slot(1)]),
        }],
        slots: vec![Value::Num(0.0), Value::str("")],
        ..Default::default()
    })
}

fn table() -> Arc<Compiled> {
    Compiled::new(Template {
        nodes: vec![TNode::new("node", None)],
        blocks: vec![Block::list(0, row())],
        ..Default::default()
    })
}

fn item(id: i64, label: &str) -> ListItem {
    ListItem::new(id, vec![Value::Num(id as f64), Value::str(label)])
}

fn same(ids: &[i64]) -> Vec<ListItem> {
    ids.iter().map(|&i| ListItem::same(i)).collect()
}

fn text(id: u32, s: &str) -> Command {
    Command::UpdateText {
        id,
        text: Arc::from(s),
    }
}

/// A table of rows 1..=n, applied and flushed, on a manual clock.
fn fixture(n: i64, window_ms: u64) -> (Runtime, Time, usize) {
    let (time, clock) = Time::new();
    let mut rt = Runtime::with_id_base(1);
    rt.set_clock(clock);
    rt.set_flush_window(Duration::from_millis(window_ms));
    let t = rt.mount(&table(), 0);
    rt.set_list(t, 0, (1..=n).map(|i| item(i, &format!("r{i}"))).collect())
        .unwrap();
    rt.flush();
    (rt, time, t)
}

fn row_text_node(rt: &Runtime, t: usize, id: i64) -> u32 {
    let row = rt.list_item(t, 0, &Key::Int(id)).unwrap();
    rt.node_id(row, 1).unwrap()
}

#[test]
fn nothing_pending_means_nothing_to_poll() {
    let (mut rt, _time, _t) = fixture(3, 10);
    assert!(!rt.has_pending());
    assert_eq!(rt.next_deadline(), None);
    assert_eq!(rt.poll(), None);
}

#[test]
fn writes_wait_for_the_window_then_flush_together() {
    let (mut rt, time, t) = fixture(3, 10);
    let r1 = rt.list_item(t, 0, &Key::Int(1)).unwrap();
    rt.set(r1, 1, Value::str("a"));
    assert!(rt.has_pending());
    assert_eq!(rt.poll(), None, "window just opened");
    time.tick(5);
    rt.set(r1, 1, Value::str("b")); // coalesces; the window does not restart
    assert_eq!(rt.poll(), None, "5 ms of 10");
    assert_eq!(rt.next_deadline(), Some(Duration::from_millis(10)));
    time.tick(5);
    assert_eq!(rt.poll(), Some(vec![text(row_text_node(&rt, t, 1), "1 b")]));
    assert!(!rt.has_pending());
    assert_eq!(rt.poll(), None);
}

#[test]
fn a_zero_window_flushes_on_the_first_poll() {
    let (mut rt, _time, t) = fixture(2, 0);
    let r2 = rt.list_item(t, 0, &Key::Int(2)).unwrap();
    rt.set(r2, 1, Value::str("x"));
    assert!(rt.poll().is_some());
}

#[test]
fn the_window_is_anchored_to_the_first_write() {
    let (mut rt, time, t) = fixture(2, 10);
    let r1 = rt.list_item(t, 0, &Key::Int(1)).unwrap();
    for i in 0..9 {
        rt.set(r1, 1, Value::str(&format!("w{i}")));
        time.tick(1);
        assert_eq!(rt.poll(), None, "tick {i}");
    }
    // A steady stream of writes cannot push the flush past the first deadline.
    rt.set(r1, 1, Value::str("last"));
    time.tick(1);
    assert!(rt.poll().is_some());
}

#[test]
fn a_forced_flush_ignores_the_window() {
    let (mut rt, _time, t) = fixture(2, 1000);
    let r1 = rt.list_item(t, 0, &Key::Int(1)).unwrap();
    rt.set(r1, 1, Value::str("now"));
    assert_eq!(rt.flush().len(), 1);
}

#[test]
fn lists_staged_in_one_window_diff_once_against_what_is_applied() {
    let (mut rt, time, t) = fixture(5, 10);
    // Two edits in a window: swap 1 and 5, then swap them back and drop 3.
    rt.set_list(t, 0, same(&[5, 2, 3, 4, 1])).unwrap();
    time.tick(2);
    rt.set_list(t, 0, same(&[1, 2, 4, 5])).unwrap();
    time.tick(8);
    let cmds = rt.poll().expect("due");
    // Applied -> final is one removal; the intermediate swap never happened.
    assert_eq!(cmds.len(), 1, "{cmds:?}");
    assert!(matches!(cmds[0], Command::Remove { .. }));
}

#[test]
fn a_record_changed_in_an_earlier_staging_survives_a_later_one() {
    let (mut rt, _time, t) = fixture(3, 10);
    rt.set_list(
        t,
        0,
        vec![item(1, "one!"), ListItem::same(2), ListItem::same(3)],
    )
    .unwrap();
    // The second call calls row 1 "the same record as last time".
    rt.set_list(
        t,
        0,
        vec![ListItem::same(1), item(2, "two!"), ListItem::same(3)],
    )
    .unwrap();
    let cmds = rt.flush();
    let n1 = row_text_node(&rt, t, 1);
    let n2 = row_text_node(&rt, t, 2);
    assert_eq!(cmds, vec![text(n1, "1 one!"), text(n2, "2 two!")]);
}

#[test]
fn staging_refuses_bad_lists_and_keeps_the_staged_one() {
    let (mut rt, _time, t) = fixture(3, 10);
    rt.set_list(
        t,
        0,
        vec![item(1, "kept"), ListItem::same(2), ListItem::same(3)],
    )
    .unwrap();
    assert_eq!(
        rt.set_list(t, 0, vec![ListItem::same(9)]).unwrap_err(),
        ListError::MissingFields(Key::Int(9))
    );
    assert_eq!(
        rt.set_list(t, 0, vec![ListItem::same(1), ListItem::same(1)])
            .unwrap_err(),
        ListError::DuplicateKey(Key::Int(1))
    );
    let n1 = row_text_node(&rt, t, 1);
    assert_eq!(rt.flush(), vec![text(n1, "1 kept")]);
}

#[test]
fn update_item_is_the_delta_path() {
    let (mut rt, _time, t) = fixture(1000, 10);
    rt.update_item(
        t,
        0,
        &Key::Int(500),
        vec![Value::Num(500.0), Value::str("hi")],
    )
    .unwrap();
    let n = row_text_node(&rt, t, 500);
    assert_eq!(rt.flush(), vec![text(n, "500 hi")]);
    assert_eq!(
        rt.update_item(t, 0, &Key::Int(5000), vec![Value::Null, Value::Null])
            .unwrap_err(),
        ListError::MissingFields(Key::Int(5000))
    );
    assert_eq!(
        rt.update_item(t, 0, &Key::Int(1), vec![Value::Null])
            .unwrap_err(),
        ListError::WrongFieldCount {
            key: Key::Int(1),
            expected: 2,
            got: 1
        }
    );
}

#[test]
fn update_item_after_a_staged_list_sees_its_rows() {
    let (mut rt, _time, t) = fixture(2, 10);
    let mut items = same(&[1, 2]);
    items.push(item(3, "new"));
    rt.set_list(t, 0, items).unwrap();
    rt.update_item(
        t,
        0,
        &Key::Int(3),
        vec![Value::Num(3.0), Value::str("edited")],
    )
    .unwrap();
    let cmds = rt.flush();
    let n3 = row_text_node(&rt, t, 3);
    // The row was created with its final text: no update follows the create.
    assert!(
        cmds.iter()
            .all(|c| !matches!(c, Command::UpdateText { .. }))
    );
    let created = cmds.iter().find_map(|c| match c {
        Command::Create {
            id,
            kind: "text",
            text: Some(t),
            ..
        } => Some((*id, t.to_string())),
        _ => None,
    });
    assert_eq!(created, Some((n3, "3 edited".to_owned())));
}

#[test]
fn a_staged_list_for_an_unmounted_table_is_dropped() {
    let (mut rt, _time, t) = fixture(2, 10);
    rt.set_list(t, 0, vec![]).unwrap();
    rt.unmount(t);
    let cmds = rt.flush();
    assert_eq!(cmds.len(), 1, "{cmds:?}");
    assert!(matches!(cmds[0], Command::Remove { parent: 0, .. }));
    assert_eq!(rt.live_instances(), 0);
}

#[test]
fn an_unchanged_order_list_skips_positioning() {
    let (mut rt, _time, t) = fixture(1000, 0);
    let mut items = same(&(1..=1000).collect::<Vec<_>>());
    items[10] = item(11, "changed");
    rt.set_list(t, 0, items).unwrap();
    let n = row_text_node(&rt, t, 11);
    assert_eq!(rt.flush(), vec![text(n, "11 changed")]);
}
