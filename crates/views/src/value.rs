//! The values bindings compute: the JS primitives a view reads.

use std::fmt;
use std::sync::Arc;

/// A JS primitive as a binding sees it. Strings are shared (`Arc<str>`), so
/// copying a value out of a slot never copies text.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Num(f64),
    Str(Arc<str>),
}

impl Value {
    pub fn str(s: &str) -> Self {
        Value::Str(Arc::from(s))
    }

    /// JS truthiness (`!!v`).
    pub fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Num(n) => *n != 0.0 && !n.is_nan(),
            Value::Str(s) => !s.is_empty(),
        }
    }

    /// The number a numeric operator reads (`+v`); `NaN` when there is none.
    pub fn num(&self) -> f64 {
        match self {
            Value::Null => 0.0,
            Value::Bool(b) => f64::from(u8::from(*b)),
            Value::Num(n) => *n,
            Value::Str(s) => s.trim().parse().unwrap_or(f64::NAN),
        }
    }
}

/// `String(v)`, as a template literal hole renders it. Integral numbers print
/// without a fraction (`4`, not `4.0`), like JS; exponent forms differ from JS
/// past 1e21, which views don't render.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => f.write_str("null"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Num(n) => write!(f, "{n}"),
            Value::Str(s) => f.write_str(s),
        }
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Value::Num(n)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::str(s)
    }
}
