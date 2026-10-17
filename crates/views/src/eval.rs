//! Evaluating an [`Expr`] against an instance's slots.

use std::fmt::Write;

use crate::ir::{BinOp, Expr, UnOp};
use crate::value::{Value, to_fixed};

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
        Expr::Bin(BinOp::And, a, b) => {
            let l = eval(a, slots);
            if l.truthy() { eval(b, slots) } else { l }
        }
        Expr::Bin(BinOp::Or, a, b) => {
            let l = eval(a, slots);
            if l.truthy() { l } else { eval(b, slots) }
        }
        Expr::Bin(op, a, b) => binary(*op, &eval(a, slots), &eval(b, slots)),
        Expr::Un(op, a) => {
            let x = eval(a, slots).num();
            Value::Num(match op {
                UnOp::Neg => -x,
                UnOp::Abs => x.abs(),
                UnOp::Floor => x.floor(),
                UnOp::Ceil => x.ceil(),
                UnOp::Round => round_half_up(x),
            })
        }
        Expr::Fixed(a, digits) => Value::str(&to_fixed(eval(a, slots).num(), *digits)),
        Expr::Add(a, b) => match (eval(a, slots), eval(b, slots)) {
            // JS `+`: string concatenation when either side is a string.
            (l @ Value::Str(_), r) | (l, r @ Value::Str(_)) => Value::str(&format!("{l}{r}")),
            (l, r) => Value::Num(l.num() + r.num()),
        },
    }
}

/// The eager binary operators.
fn binary(op: BinOp, l: &Value, r: &Value) -> Value {
    let (x, y) = (l.num(), r.num());
    match op {
        BinOp::Sub => Value::Num(x - y),
        BinOp::Div => Value::Num(x / y),
        BinOp::Rem => Value::Num(x % y),
        BinOp::Ne => Value::Bool(!strict_eq(l, r)),
        // Two strings compare as strings; anything else as numbers.
        BinOp::Lt => Value::Bool(order(l, r).is_some_and(|o| o.is_lt())),
        BinOp::Le => Value::Bool(order(l, r).is_some_and(|o| o.is_le())),
        BinOp::Ge => Value::Bool(order(l, r).is_some_and(|o| o.is_ge())),
        BinOp::Min => Value::Num(if x.is_nan() || y.is_nan() {
            f64::NAN
        } else {
            x.min(y)
        }),
        BinOp::Max => Value::Num(if x.is_nan() || y.is_nan() {
            f64::NAN
        } else {
            x.max(y)
        }),
        BinOp::And | BinOp::Or => unreachable!("lazy operators are handled by the caller"),
    }
}

/// The JS relational order: strings by code unit, otherwise numbers (`None`
/// when either is `NaN`, so every comparison is false).
fn order(l: &Value, r: &Value) -> Option<std::cmp::Ordering> {
    match (l, r) {
        (Value::Str(a), Value::Str(b)) => Some(a.cmp(b)),
        _ => l.num().partial_cmp(&r.num()),
    }
}

/// `Math.round`: halves go toward +Infinity (`-2.5` → `-2`, `2.5` → `3`).
fn round_half_up(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let floor = x.floor();
    if x - floor >= 0.5 { floor + 1.0 } else { floor }
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

    #[test]
    fn arithmetic_and_comparisons_follow_js() {
        let n = |v: f64| e::lit(v);
        assert_eq!(eval(&e::sub(n(5.0), n(8.0)), &[]), Value::Num(-3.0));
        assert_eq!(eval(&e::div(n(1.0), n(4.0)), &[]), Value::Num(0.25));
        assert_eq!(
            eval(&e::div(n(1.0), n(0.0)), &[]),
            Value::Num(f64::INFINITY)
        );
        assert_eq!(eval(&e::rem(n(-7.0), n(3.0)), &[]), Value::Num(-1.0));
        assert_eq!(eval(&e::lt(n(1.0), n(2.0)), &[]), Value::Bool(true));
        assert_eq!(eval(&e::ge(n(2.0), n(2.0)), &[]), Value::Bool(true));
        assert_eq!(eval(&e::le(n(f64::NAN), n(2.0)), &[]), Value::Bool(false));
        assert_eq!(
            eval(&e::ne(e::lit("a"), e::lit("b")), &[]),
            Value::Bool(true)
        );
        // Strings compare as strings, not numbers.
        assert_eq!(
            eval(&e::lt(e::lit("10"), e::lit("9")), &[]),
            Value::Bool(true)
        );
        assert_eq!(eval(&e::lt(e::lit("10"), n(9.0)), &[]), Value::Bool(false));
    }

    #[test]
    fn logical_operators_return_operands_and_short_circuit() {
        let slots = [Value::Num(0.0), Value::str("x")];
        assert_eq!(
            eval(&e::and(e::slot(0), e::slot(1)), &slots),
            Value::Num(0.0)
        );
        assert_eq!(
            eval(&e::and(e::slot(1), e::lit(7.0)), &slots),
            Value::Num(7.0)
        );
        assert_eq!(
            eval(&e::or(e::slot(0), e::slot(1)), &slots),
            Value::str("x")
        );
        assert_eq!(
            eval(&e::or(e::slot(1), e::slot(0)), &slots),
            Value::str("x")
        );
    }

    #[test]
    fn min_max_clamp_round() {
        let n = |v: f64| e::lit(v);
        assert_eq!(
            eval(&e::clamp(n(150.0), n(0.0), n(100.0)), &[]),
            Value::Num(100.0)
        );
        assert_eq!(
            eval(&e::clamp(n(-5.0), n(0.0), n(100.0)), &[]),
            Value::Num(0.0)
        );
        assert!(eval(&e::min(n(f64::NAN), n(1.0)), &[]).num().is_nan());
        assert_eq!(eval(&e::round(n(2.5)), &[]), Value::Num(3.0));
        assert_eq!(eval(&e::round(n(-2.5)), &[]), Value::Num(-2.0));
        assert_eq!(eval(&e::round(n(2.4)), &[]), Value::Num(2.0));
        assert_eq!(eval(&e::floor(n(-0.5)), &[]), Value::Num(-1.0));
        assert_eq!(eval(&e::ceil(n(0.2)), &[]), Value::Num(1.0));
        assert_eq!(eval(&e::abs(n(-3.0)), &[]), Value::Num(3.0));
        assert_eq!(eval(&e::neg(n(3.0)), &[]), Value::Num(-3.0));
    }

    #[test]
    fn numbers_print_like_js() {
        let show = |v: f64| eval(&e::concat([e::lit(v)]), &[]).to_string();
        assert_eq!(show(4.0), "4");
        assert_eq!(show(-0.0), "0");
        assert_eq!(show(0.1 + 0.2), "0.30000000000000004");
        assert_eq!(show(1e21), "1e+21");
        assert_eq!(show(1.5e-7), "1.5e-7");
        assert_eq!(show(0.000001), "0.000001");
        assert_eq!(show(123456789012345680000.0), "123456789012345680000");
        assert_eq!(show(f64::INFINITY), "Infinity");
        assert_eq!(show(f64::NEG_INFINITY), "-Infinity");
        assert_eq!(show(f64::NAN), "NaN");
    }

    #[test]
    fn to_fixed_matches_js_including_ties() {
        let fx = |v: f64, d: u8| eval(&e::fixed(e::lit(v), d), &[]).to_string();
        assert_eq!(fx(123.456, 2), "123.46");
        assert_eq!(fx(123.456, 0), "123");
        assert_eq!(fx(0.0, 2), "0.00");
        // Exact ties go away from zero (Rust's own formatter goes to even).
        assert_eq!(fx(0.5, 0), "1");
        assert_eq!(fx(1.5, 0), "2");
        assert_eq!(fx(2.5, 0), "3");
        assert_eq!(fx(0.125, 2), "0.13");
        assert_eq!(fx(-1.5, 0), "-2");
        assert_eq!(fx(9.5, 0), "10");
        assert_eq!(
            fx(99.995, 2),
            "100.00",
            "99.995 is just above the tie in binary"
        );
        // Binary values just either side of a decimal tie are not ties.
        assert_eq!(fx(1.005, 2), "1.00");
        assert_eq!(fx(1.45, 1), "1.4");
        assert_eq!(fx(8.345, 2), "8.35");
        assert_eq!(fx(10.235, 2), "10.23");
        assert_eq!(fx(-0.001, 2), "-0.00");
        assert_eq!(fx(1e21, 2), "1e+21");
        assert_eq!(fx(f64::NAN, 2), "NaN");
        assert_eq!(
            fx(0.05, 1),
            "0.1",
            "0.05 is 0.05000000000000000277: above the tie"
        );
    }
}
