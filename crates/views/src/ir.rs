//! The view IR: what the compiler emits for one component.
//!
//! A [`Template`] is the component's static tree plus its dynamic holes. Each
//! hole ([`Binding`]) is a pure [`Expr`] over the instance's state slots; the
//! slots an expression reads are known statically ([`Expr::reads`]), so the
//! runtime needs no dependency tracking at run time.

use std::sync::Arc;

use crate::compiled::Compiled;
use crate::value::Value;

/// A state slot of a component instance (`useState`, or an island prop).
pub type SlotId = u16;
/// A node's index within its template.
pub type NodeIx = u16;
/// A JS function the compiled code references (a handler, an effect).
pub type JsRef = u32;

/// One compiled component.
#[derive(Clone, Debug, Default)]
pub struct Template {
    /// Depth-first order; a node's parent always comes before it.
    pub nodes: Vec<TNode>,
    /// Every dynamic hole of the tree.
    pub bindings: Vec<Binding>,
    /// Initial slot values (`useState(initial)`); the length is the slot count.
    pub slots: Vec<Value>,
    /// Keyed lists.
    pub blocks: Vec<Block>,
    /// Conditional and nested components.
    pub children: Vec<ChildBlock>,
    /// `useEffect`s.
    pub effects: Vec<Effect>,
}

/// A `useEffect`: JS the host runs after the component mounts and after any
/// of its dependencies changed (compared by value at flush time, so a slot
/// written and restored inside one flush runs nothing). Calls come out of
/// `Runtime::take_effects` in React's order: children before parents on
/// mount, cleanup before the next run, and a cleanup at unmount.
#[derive(Clone, Debug)]
pub struct Effect {
    /// The slots whose change re-runs it; `None` = any slot of the component
    /// (an effect with no dependency array); empty = on mount only.
    pub deps: Option<Vec<SlotId>>,
    /// The JS function the host runs.
    pub run: JsRef,
    /// Does a run return a cleanup the host must call before the next run
    /// and at unmount?
    pub cleanup: bool,
}

impl Effect {
    /// Runs on mount and whenever one of `deps` changes.
    pub fn on(deps: &[SlotId], run: JsRef) -> Self {
        Effect {
            deps: Some(deps.to_vec()),
            run,
            cleanup: false,
        }
    }

    /// Runs after any change of the component's state.
    pub fn always(run: JsRef) -> Self {
        Effect {
            deps: None,
            run,
            cleanup: false,
        }
    }

    /// The host holds a cleanup for each run.
    pub fn with_cleanup(mut self) -> Self {
        self.cleanup = true;
        self
    }
}

/// A nested component, optionally conditional (`{open && <Detail name={n} />}`).
///
/// While `when` holds (always, if absent) one instance of `component` is a
/// child of node `parent`, placed before the static sibling `before` (or last).
/// Its slots are its own state, except the ones `props` bind to expressions
/// over this component's slots: those follow their expression whenever the
/// slots it reads change. The instance keeps its state while mounted, and a
/// condition that goes false and true again mounts a fresh one. `component`
/// has a single root, node 0, and `parent` is not a list's parent.
#[derive(Clone, Debug)]
pub struct ChildBlock {
    pub parent: NodeIx,
    pub before: Option<NodeIx>,
    pub when: Option<Expr>,
    pub component: Arc<Compiled>,
    /// `(child slot, expression over this component's slots)`.
    pub props: Vec<(SlotId, Expr)>,
}

impl ChildBlock {
    pub fn new(parent: NodeIx, component: Arc<Compiled>) -> Self {
        ChildBlock {
            parent,
            before: None,
            when: None,
            component,
            props: Vec::new(),
        }
    }

    /// Place the child before the static sibling `node`.
    pub fn before(mut self, node: NodeIx) -> Self {
        self.before = Some(node);
        self
    }

    /// Mount the child only while `cond` is truthy.
    pub fn when(mut self, cond: Expr) -> Self {
        self.when = Some(cond);
        self
    }

    /// Bind child slot `slot` to `expr`.
    pub fn prop(mut self, slot: SlotId, expr: Expr) -> Self {
        self.props.push((slot, expr));
        self
    }
}

/// A structural block: children whose number and order are data.
#[derive(Clone, Debug)]
pub enum Block {
    /// A keyed list (`items.map(i => <Row key={i.id} … />)`): the children of
    /// node `parent` are one `item` instance per list entry, whose slots are
    /// the entry's fields (then one flag slot per selector). The list owns
    /// `parent`'s children (no static siblings), and `item` has a single root.
    For {
        parent: NodeIx,
        item: Arc<Compiled>,
        selectors: Vec<Selector>,
        /// Rows are placed before this static sibling of theirs (a bottom
        /// spacer), or last when absent.
        anchor: Option<NodeIx>,
        /// Mount only the rows in view (see [`Window`]).
        window: Option<Window>,
    },
}

/// A windowed (virtual) list: rows exist only for a range of the list, set
/// with `Runtime::set_window`; the list's data lives on in Rust for the rest.
/// Two slots of the owning instance hold how many rows are **not** mounted
/// above and below the range, for spacer nodes to size themselves from
/// (`height: top * ROW_HEIGHT`). A windowed list needs an `anchor`: the
/// bottom spacer, a static sibling that follows the rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    /// The slot receiving the count of unmounted rows before the range.
    pub top: SlotId,
    /// The slot receiving the count of unmounted rows after it.
    pub bottom: SlotId,
}

/// `selected === row.id` for every row, without visiting every row: the item
/// slot `flag` is true exactly for the row whose `field` equals the parent
/// slot `from`. A write to `from` flips only the old and the new row.
///
/// Flag slots are the item's last slots, in selector order (so
/// `flag == data slots + index`); rows compare by key, and `null` selects no
/// row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selector {
    /// The parent instance's slot holding the selection.
    pub from: SlotId,
    /// The item slot holding the row's key value.
    pub field: SlotId,
    /// The item slot that receives the boolean.
    pub flag: SlotId,
}

impl Block {
    /// A keyed list with no selectors.
    pub fn list(parent: NodeIx, item: Arc<Compiled>) -> Self {
        Block::For {
            parent,
            item,
            selectors: Vec::new(),
            anchor: None,
            window: None,
        }
    }

    /// Place rows before the static sibling `node` instead of last.
    pub fn before(mut self, node: NodeIx) -> Self {
        let Block::For { anchor, .. } = &mut self;
        *anchor = Some(node);
        self
    }

    /// Make the list windowed: rows before the static sibling `anchor`, the
    /// unmounted counts written to `top` and `bottom` (see [`Window`]).
    pub fn windowed(mut self, anchor: NodeIx, top: SlotId, bottom: SlotId) -> Self {
        let Block::For {
            anchor: a, window, ..
        } = &mut self;
        *a = Some(anchor);
        *window = Some(Window { top, bottom });
        self
    }

    /// Add a selector (see [`Selector`]).
    pub fn select(mut self, from: SlotId, field: SlotId, flag: SlotId) -> Self {
        let Block::For { selectors, .. } = &mut self;
        selectors.push(Selector { from, field, flag });
        self
    }
}

/// One host element of a template.
#[derive(Clone, Debug)]
pub struct TNode {
    /// The element kind (`"node"`, `"text"`, `"button"`, …).
    pub kind: &'static str,
    /// `None` = a root of the template (attached to the mount parent).
    pub parent: Option<NodeIx>,
    /// Static props, wire names (`name`, `tabIndex`, …).
    pub props: Vec<(&'static str, Value)>,
    /// Static style fields, wire names.
    pub style: Vec<(&'static str, Value)>,
    /// Static inline text (a `<text>` whose only child is a string).
    pub text: Option<&'static str>,
    /// Handlers: wire event name (`"click"`) → what to run.
    pub handlers: Vec<(&'static str, Handler)>,
}

impl TNode {
    pub fn new(kind: &'static str, parent: Option<NodeIx>) -> Self {
        TNode {
            kind,
            parent,
            props: Vec::new(),
            style: Vec::new(),
            text: None,
            handlers: Vec::new(),
        }
    }

    pub fn style(mut self, key: &'static str, value: impl Into<Value>) -> Self {
        self.style.push((key, value.into()));
        self
    }

    pub fn text(mut self, text: &'static str) -> Self {
        self.text = Some(text);
        self
    }

    /// A JS handler: the host runs function `js`.
    pub fn on(mut self, event: &'static str, js: JsRef) -> Self {
        self.handlers.push((event, Handler::Js(js)));
        self
    }

    /// A handler Rust runs itself, with no JS call.
    pub fn on_do(mut self, event: &'static str, actions: Vec<Action>) -> Self {
        self.handlers.push((
            event,
            Handler::Do {
                actions,
                then: None,
            },
        ));
        self
    }

    /// Rust runs `actions`, then the host runs `js` (it sees their effect).
    pub fn on_do_then(mut self, event: &'static str, actions: Vec<Action>, js: JsRef) -> Self {
        self.handlers.push((
            event,
            Handler::Do {
                actions,
                then: Some(js),
            },
        ));
        self
    }
}

/// What an event handler runs.
#[derive(Clone, Debug)]
pub enum Handler {
    /// A JS function the host runs.
    Js(JsRef),
    /// Actions Rust runs at once, then optionally a JS function. A click that
    /// only flips state never leaves Rust.
    Do {
        actions: Vec<Action>,
        then: Option<JsRef>,
    },
}

/// One statement of a Rust handler.
#[derive(Clone, Debug)]
pub enum Action {
    /// `setX(expr)`: write `slot` of the instance `up` owners above the
    /// handler's own (0 = its own slots; 1 = the table a row belongs to, the
    /// component that mounted a child). `expr` reads the handler's own slots.
    /// Every action of a handler reads the state from before the handler ran,
    /// like the closures of one React render.
    Set { up: u8, slot: SlotId, expr: Expr },
}

impl Action {
    /// `setX(expr)` on the handler's own state.
    pub fn set(slot: SlotId, expr: Expr) -> Self {
        Action::Set { up: 0, slot, expr }
    }

    /// `setX(expr)` on the instance that owns this one.
    pub fn set_owner(slot: SlotId, expr: Expr) -> Self {
        Action::Set { up: 1, slot, expr }
    }
}

/// Where a hole's value goes.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// The inline text of a `<text>` node.
    Text(NodeIx),
    /// A top-level prop.
    Prop(NodeIx, &'static str),
    /// One style field.
    Style(NodeIx, &'static str),
}

/// One dynamic hole.
#[derive(Clone, Debug)]
pub struct Binding {
    pub target: Target,
    pub expr: Expr,
}

/// A pure expression over the instance's slots.
#[derive(Clone, Debug)]
pub enum Expr {
    Const(Value),
    Slot(SlotId),
    /// A template literal: the parts' `String()`s joined.
    Concat(Vec<Expr>),
    /// `cond ? then : otherwise`.
    Cond(Box<Expr>, Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    /// `===` on primitives.
    Eq(Box<Expr>, Box<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
}

impl Expr {
    /// Every slot this expression reads, in first-read order, no repeats.
    pub fn reads(&self) -> Vec<SlotId> {
        let mut out = Vec::new();
        self.collect_reads(&mut out);
        out
    }

    fn collect_reads(&self, out: &mut Vec<SlotId>) {
        match self {
            Expr::Const(_) => {}
            Expr::Slot(s) => {
                if !out.contains(s) {
                    out.push(*s);
                }
            }
            Expr::Concat(parts) => parts.iter().for_each(|p| p.collect_reads(out)),
            Expr::Cond(a, b, c) => [a, b, c].iter().for_each(|e| e.collect_reads(out)),
            Expr::Not(a) => a.collect_reads(out),
            Expr::Eq(a, b) | Expr::Gt(a, b) | Expr::Add(a, b) | Expr::Mul(a, b) => {
                a.collect_reads(out);
                b.collect_reads(out);
            }
        }
    }
}

/// Short constructors, so hand-written IR (tests, the spike) stays readable.
pub mod e {
    use super::{Expr, SlotId};
    use crate::value::Value;

    pub fn lit(v: impl Into<Value>) -> Expr {
        Expr::Const(v.into())
    }
    pub fn slot(s: SlotId) -> Expr {
        Expr::Slot(s)
    }
    pub fn concat(parts: impl IntoIterator<Item = Expr>) -> Expr {
        Expr::Concat(parts.into_iter().collect())
    }
    pub fn cond(c: Expr, then: Expr, otherwise: Expr) -> Expr {
        Expr::Cond(Box::new(c), Box::new(then), Box::new(otherwise))
    }
    pub fn not(a: Expr) -> Expr {
        Expr::Not(Box::new(a))
    }
    pub fn eq(a: Expr, b: Expr) -> Expr {
        Expr::Eq(Box::new(a), Box::new(b))
    }
    pub fn gt(a: Expr, b: Expr) -> Expr {
        Expr::Gt(Box::new(a), Box::new(b))
    }
    pub fn add(a: Expr, b: Expr) -> Expr {
        Expr::Add(Box::new(a), Box::new(b))
    }
    pub fn mul(a: Expr, b: Expr) -> Expr {
        Expr::Mul(Box::new(a), Box::new(b))
    }
}
