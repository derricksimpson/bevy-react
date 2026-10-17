//! Evaluating an [`Expr`] against an instance's slots.

use std::fmt::Write;

use crate::ir::Expr;
use crate::value::Value;

/// Evaluate `expr` with `slots` as the instance state. Pure: same slots, same
/// value. An out-of-range slot reads as `null` (a compiler bug, not a panic).
pub fn eval(expr: &Expr, slots: &[Value]) -> Value {
    match expr {
        Expr::Const(v) => v.clone(),
        Expr::Slot(s) => slots.get(usize::from(*s)).cloned().unwrap_or(Value::Null),
        Expr::Concat(parts) => {
            let mut out = String::new();
            for p in parts {
                let _ = write!(out, "{}", eval(p, slots));
            }
            Value::str(&out)
        }
        Expr::Cond(c, then, otherwise) => {
            if eval(c, slots).truthy() {
                eval(then, slots)
            } else {
                eval(otherwise, slots)
            }
        }
        Expr::Not(a) => Value::Bool(!eval(a, slots).truthy()),
        Expr::Eq(a, b) => Value::Bool(strict_eq(&eval(a, slots), &eval(b, slots))),
        Expr::Mul(a, b) => Value::Num(eval(a, slots).num() * eval(b, slots).num()),
        Expr::Gt(a, b) => Value::Bool(eval(a, slots).num() > eval(b, slots).num()),
        Expr::Add(a, b) => match (eval(a, slots), eval(b, slots)) {
            // JS `+`: string concatenation when either side is a string.
            (l @ Value::Str(_), r) | (l, r @ Value::Str(_)) => Value::str(&format!("{l}{r}")),
            (l, r) => Value::Num(l.num() + r.num()),
        },
    }
}

/// `===` on primitives (`NaN !== NaN`).
fn strict_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => x == y,
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::e;

    #[test]
    fn template_literal_formats_numbers_like_js() {
        let slots = [Value::Num(4.0), Value::Num(1.5)];
        let v = eval(
            &e::concat([e::lit("a="), e::slot(0), e::lit(" b="), e::slot(1)]),
            &slots,
        );
        assert_eq!(v, Value::str("a=4 b=1.5"));
    }

    #[test]
    fn plus_concatenates_when_a_side_is_a_string() {
        assert_eq!(
            eval(&e::add(e::lit(1.0), e::lit(2.0)), &[]),
            Value::Num(3.0)
        );
        assert_eq!(
            eval(&e::add(e::lit("x"), e::lit(2.0)), &[]),
            Value::str("x2")
        );
    }

    #[test]
    fn strict_equality_and_truthiness() {
        assert_eq!(
            eval(&e::eq(e::lit(f64::NAN), e::lit(f64::NAN)), &[]),
            Value::Bool(false)
        );
        assert_eq!(
            eval(&e::eq(e::lit("a"), e::lit("a")), &[]),
            Value::Bool(true)
        );
        assert_eq!(eval(&e::not(e::lit("")), &[]), Value::Bool(true));
        assert_eq!(
            eval(&e::cond(e::lit(0.0), e::lit("y"), e::lit("n")), &[]),
            Value::str("n")
        );
    }

    #[test]
    fn missing_slot_reads_null() {
        assert_eq!(eval(&e::slot(9), &[]), Value::Null);
    }
}
