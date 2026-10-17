//! `useEffect`: what the host is told to run, and in what order.
//!
//! ```tsx
//! useEffect(() => { fetchUser(id); return () => cancel(); }, [id]);
//! useEffect(() => { log("mounted"); }, []);
//! useEffect(() => { log("rendered"); });          // no dependency array
//! ```

use std::sync::Arc;

use bevy_react_views::{
    Block, ChildBlock, Compiled, Effect, EffectCall, EffectKind, InstanceId, ListItem, Runtime,
    TNode, Template, Value,
};

const FETCH: u32 = 1;
const MOUNTED: u32 = 2;
const RENDERED: u32 = 3;

fn call(instance: InstanceId, effect: u16, js: u32, kind: EffectKind) -> EffectCall {
    EffectCall {
        instance,
        effect,
        js,
        kind,
    }
}

fn run(instance: InstanceId, effect: u16, js: u32) -> EffectCall {
    call(instance, effect, js, EffectKind::Run)
}

fn cleanup(instance: InstanceId, effect: u16, js: u32) -> EffectCall {
    call(instance, effect, js, EffectKind::Cleanup)
}

/// Slots: 0 = id, 1 = other. Effects: 0 = fetch (deps [id], cleanup),
/// 1 = mount-only, 2 = every change.
fn user() -> Arc<Compiled> {
    Compiled::new(Template {
        nodes: vec![TNode::new("node", None)],
        slots: vec![Value::Num(1.0), Value::Num(0.0)],
        effects: vec![
            Effect::on(&[0], FETCH).with_cleanup(),
            Effect::on(&[], MOUNTED),
            Effect::always(RENDERED),
        ],
        ..Default::default()
    })
}

fn mounted() -> (Runtime, InstanceId) {
    let mut rt = Runtime::with_id_base(1);
    let u = rt.mount(&user(), 0);
    rt.flush();
    rt.take_effects();
    (rt, u)
}

#[test]
fn every_effect_runs_after_mount_in_declaration_order() {
    let mut rt = Runtime::with_id_base(1);
    let u = rt.mount(&user(), 0);
    rt.flush();
    assert_eq!(
        rt.take_effects(),
        vec![run(u, 0, FETCH), run(u, 1, MOUNTED), run(u, 2, RENDERED)]
    );
    assert_eq!(rt.take_effects(), vec![], "taken once");
}

#[test]
fn a_dependency_change_cleans_up_then_runs_again() {
    let (mut rt, u) = mounted();
    rt.set(u, 0, Value::Num(2.0));
    rt.flush();
    // The mount-only effect stays quiet; the dependency effect cleans up
    // and re-runs; the no-array effect runs because state changed.
    assert_eq!(
        rt.take_effects(),
        vec![cleanup(u, 0, FETCH), run(u, 0, FETCH), run(u, 2, RENDERED)]
    );
}

#[test]
fn an_unrelated_write_runs_only_the_effect_with_no_array() {
    let (mut rt, u) = mounted();
    rt.set(u, 1, Value::Num(9.0));
    rt.flush();
    assert_eq!(rt.take_effects(), vec![run(u, 2, RENDERED)]);
}

#[test]
fn a_dependency_changed_and_restored_inside_one_flush_runs_nothing_for_it() {
    let (mut rt, u) = mounted();
    rt.set(u, 0, Value::Num(7.0));
    rt.set(u, 0, Value::Num(1.0));
    rt.flush();
    let calls = rt.take_effects();
    assert!(calls.iter().all(|c| c.effect != 0), "{calls:?}");
}

#[test]
fn a_run_the_host_never_collected_is_replaced_not_cleaned_up() {
    let mut rt = Runtime::with_id_base(1);
    let u = rt.mount(&user(), 0);
    rt.flush();
    // The host has not called take_effects: its first run is still queued.
    rt.set(u, 0, Value::Num(2.0));
    rt.flush();
    let calls = rt.take_effects();
    let fetches: Vec<_> = calls.iter().filter(|c| c.effect == 0).collect();
    assert_eq!(
        fetches,
        vec![&run(u, 0, FETCH)],
        "one run, no cleanup: {calls:?}"
    );
}

#[test]
fn unmounting_cleans_up_what_ran() {
    let (mut rt, u) = mounted();
    rt.unmount(u);
    rt.flush();
    // Only the effect declared with a cleanup has one.
    assert_eq!(rt.take_effects(), vec![cleanup(u, 0, FETCH)]);
}

#[test]
fn unmounting_before_the_host_ran_anything_calls_nothing() {
    let mut rt = Runtime::with_id_base(1);
    let u = rt.mount(&user(), 0);
    rt.flush();
    rt.unmount(u);
    rt.flush();
    assert_eq!(rt.take_effects(), vec![]);
}

#[test]
fn children_run_before_their_parent() {
    let child = Compiled::new(Template {
        nodes: vec![TNode::new("node", None)],
        effects: vec![Effect::on(&[], 20).with_cleanup()],
        ..Default::default()
    });
    let parent = Compiled::new(Template {
        nodes: vec![TNode::new("node", None)],
        children: vec![ChildBlock::new(0, child)],
        effects: vec![Effect::on(&[], 10).with_cleanup()],
        ..Default::default()
    });
    let mut rt = Runtime::with_id_base(1);
    let p = rt.mount(&parent, 0);
    rt.flush();
    let calls = rt.take_effects();
    let child_instance = calls[0].instance;
    assert_ne!(child_instance, p);
    assert_eq!(calls, vec![run(child_instance, 0, 20), run(p, 0, 10)]);
    // Unmount runs the parent's cleanup first, then the child's.
    rt.unmount(p);
    rt.flush();
    assert_eq!(
        rt.take_effects(),
        vec![cleanup(p, 0, 10), cleanup(child_instance, 0, 20)]
    );
}

#[test]
fn a_prop_driven_child_effect_follows_the_parents_state() {
    // `<Child id={id} />` with `useEffect(load, [id])` in the child.
    let child = Compiled::new(Template {
        nodes: vec![TNode::new("node", None)],
        slots: vec![Value::Num(0.0)],
        effects: vec![Effect::on(&[0], FETCH)],
        ..Default::default()
    });
    let parent = Compiled::new(Template {
        nodes: vec![TNode::new("node", None)],
        children: vec![ChildBlock::new(0, child).prop(0, bevy_react_views::e::slot(0))],
        slots: vec![Value::Num(5.0)],
        ..Default::default()
    });
    let mut rt = Runtime::with_id_base(1);
    let p = rt.mount(&parent, 0);
    rt.flush();
    let calls = rt.take_effects();
    assert_eq!(calls.len(), 1, "{calls:?}");
    let c = calls[0].instance;
    rt.set(p, 0, Value::Num(6.0));
    rt.flush();
    assert_eq!(rt.take_effects(), vec![run(c, 0, FETCH)]);
}

#[test]
fn list_rows_run_effects_when_they_mount_and_clean_up_when_they_go() {
    let row = Compiled::new(Template {
        nodes: vec![TNode::new("node", None)],
        slots: vec![Value::Num(0.0)],
        effects: vec![Effect::on(&[], 30).with_cleanup()],
        ..Default::default()
    });
    let table = Compiled::new(Template {
        nodes: vec![TNode::new("node", None)],
        blocks: vec![Block::list(0, row)],
        ..Default::default()
    });
    let mut rt = Runtime::with_id_base(1);
    let t = rt.mount(&table, 0);
    rt.set_list(
        t,
        0,
        (1..=3)
            .map(|i| ListItem::new(i, vec![Value::Num(i as f64)]))
            .collect(),
    )
    .unwrap();
    rt.flush();
    let calls = rt.take_effects();
    assert_eq!(calls.len(), 3);
    assert!(
        calls
            .iter()
            .all(|c| c.kind == EffectKind::Run && c.js == 30)
    );
    // Row 2 goes away: its cleanup, and only its.
    rt.set_list(t, 0, vec![ListItem::same(1), ListItem::same(3)])
        .unwrap();
    rt.flush();
    let calls = rt.take_effects();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].kind, EffectKind::Cleanup);
    // The cleanup names row 2's instance: the one missing from the live rows.
    let live: Vec<InstanceId> = [1i64, 3]
        .iter()
        .map(|i| rt.list_item(t, 0, &(*i).into()).unwrap())
        .collect();
    assert!(!live.contains(&calls[0].instance));
}

#[test]
#[should_panic(expected = "depends on slot 4")]
fn an_effect_depending_on_an_undeclared_slot_is_rejected() {
    Compiled::new(Template {
        nodes: vec![TNode::new("node", None)],
        effects: vec![Effect::on(&[4], 1)],
        ..Default::default()
    });
}
