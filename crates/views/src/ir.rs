//! The view IR: what the compiler emits for one component.
//!
//! A [`Template`] is the component's static tree plus its dynamic holes. Each
//! hole ([`Binding`]) is a pure [`Expr`] over the instance's state slots; the
//! slots an expression reads are known statically ([`Expr::reads`]), so the
//! runtime needs no dependency tracking at run time.

use std::sync::Arc;

use crate::runtime::Compiled;
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
    /// Lists and other structural blocks.
    pub blocks: Vec<Block>,
}

/// A structural block: children whose number and order are data.
#[derive(Clone, Debug)]
pub enum Block {
    /// A keyed list (`items.map(i => <Row key={i.id} … />)`): the children of
    /// node `parent` are one `item` instance per list entry, whose slots are
    /// the entry's fields. The list owns `parent`'s children (no static
    /// siblings), and `item` has a single root.
    For { parent: NodeIx, item: Arc<Compiled> },
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
    /// Handlers: wire event name (`"click"`) → the JS function to run.
    pub handlers: Vec<(&'static str, JsRef)>,
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

    pub fn on(mut self, event: &'static str, js: JsRef) -> Self {
        self.handlers.push((event, js));
        self
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
            Expr::Eq(a, b) | Expr::Gt(a, b) | Expr::Add(a, b) => {
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
}
