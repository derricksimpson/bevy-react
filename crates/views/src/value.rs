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

/// `String(v)`, as a template literal hole renders it: numbers print like
/// JS (`4`, `1.5`, `1e+21`, `1e-7`, `Infinity`, `NaN`).
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => f.write_str("null"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Num(n) => f.write_str(&js_number(*n)),
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

/// `String(n)` for a number: JS switches to exponent form at 1e21 and below
/// 1e-6, and has no `-0` or `inf`.
pub(crate) fn js_number(n: f64) -> String {
    if n.is_nan() {
        "NaN".into()
    } else if n.is_infinite() {
        if n > 0.0 { "Infinity" } else { "-Infinity" }.into()
    } else if n == 0.0 {
        "0".into()
    } else if n.abs() >= 1e21 || n.abs() < 1e-6 {
        // Rust: `1.5e-7` / `1e21`; JS wants an explicit sign on the exponent.
        let s = format!("{n:e}");
        let (mantissa, exp) = s.split_once('e').expect("exponent form");
        let sign = if exp.starts_with('-') { "" } else { "+" };
        format!("{mantissa}e{sign}{exp}")
    } else {
        format!("{n}")
    }
}

/// `n.toFixed(digits)`: round to `digits` decimals, exact ties away from
/// zero (Rust's formatter breaks exact ties to even), `String(n)` from 1e21.
pub(crate) fn to_fixed(n: f64, digits: u8) -> String {
    if !n.is_finite() || n.abs() >= 1e21 {
        return js_number(n);
    }
    let d = usize::from(digits);
    let negative = n < 0.0;
    // The exact binary value, far enough out to see whether it is a tie: the
    // digit after the last kept one is `5` and nothing follows.
    let long = format!("{:.*}", d + 140, n.abs());
    let (int, frac) = long.split_once('.').expect("a decimal point");
    let tie = frac.as_bytes()[d] == b'5' && frac[d + 1..].bytes().all(|b| b == b'0');
    let mut text = if tie {
        increment(&format!("{int}.{}", &frac[..d]), d)
    } else {
        format!("{:.*}", d, n.abs())
    };
    if d == 0 && text.ends_with('.') {
        text.pop();
    }
    // JS keeps the sign of a negative number that rounds to zero (`-0.00`).
    if negative {
        text.insert(0, '-');
    }
    text
}

/// Add one unit in the last place to a decimal string with `d` decimals.
fn increment(text: &str, d: usize) -> String {
    let mut digits: Vec<u8> = text.bytes().collect();
    let mut i = digits.len();
    loop {
        if i == 0 {
            digits.insert(0, b'1');
            break;
        }
        i -= 1;
        match digits[i] {
            b'.' => {}
            b'9' => digits[i] = b'0',
            c => {
                digits[i] = c + 1;
                break;
            }
        }
    }
    let out = String::from_utf8(digits).expect("ascii");
    if d == 0 {
        out.trim_end_matches('.').to_owned()
    } else {
        out
    }
}
