//! Integration test: SiftEngine over a V type that does NOT implement
//! `ArithmeticValue`. Demonstrates the orphan-rule escape — a real
//! consumer (e.g., Salience over `paracausality_core::Value`) can use
//! their foreign V without authoring any trait impl on it.

use fabula::engine::{BoundValue, DefaultLetEvaluator, LetEvaluator, NoLetEvaluator};
use fabula::expr::{BinOp, Expr};
use fabula::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum OpaqueValue {
    Num(i64),
    Str(String),
}
// Notably: NO impl fabula::ArithmeticValue for OpaqueValue.

#[test]
fn engine_constructs_over_non_arithmetic_v_with_no_let_evaluator() {
    // The point: this code compiles. Before the refactor, it would not
    // compile because SiftEngine required V: ArithmeticValue.
    let _engine: SiftEngine<String, String, OpaqueValue, i64, NoLetEvaluator> =
        SiftEngine::new(NoLetEvaluator);
}

#[test]
fn let_free_pattern_registers_over_opaque_v() {
    let pattern: Pattern<String, OpaqueValue> = PatternBuilder::new("simple")
        .stage("e1", |s| {
            s.edge("e1", "label".to_string(), OpaqueValue::Num(1))
        })
        .build();

    let mut engine: SiftEngine<String, String, OpaqueValue, i64, NoLetEvaluator> =
        SiftEngine::new(NoLetEvaluator);
    let _idx = engine.register(pattern);
}

/// Consumer-supplied evaluator: knows how to do arithmetic over
/// OpaqueValue::Num *without* requiring fabula::ArithmeticValue to be
/// implemented for OpaqueValue. Lives in the consumer's crate (this
/// test file), legal under Rust orphan rules.
#[derive(Debug, Clone, Copy, Default)]
struct OpaqueValueLetEvaluator;

impl LetEvaluator<String, OpaqueValue> for OpaqueValueLetEvaluator {
    fn evaluate(
        &self,
        expr: &Expr<OpaqueValue>,
        bindings: &HashMap<String, BoundValue<String, OpaqueValue>>,
    ) -> Option<OpaqueValue> {
        fn extract(
            expr: &Expr<OpaqueValue>,
            bindings: &HashMap<String, BoundValue<String, OpaqueValue>>,
        ) -> Option<i64> {
            match expr {
                Expr::Literal(OpaqueValue::Num(n)) => Some(*n),
                Expr::Literal(_) => None,
                Expr::Var(name) => match bindings.get(name) {
                    Some(BoundValue::Value(OpaqueValue::Num(n))) => Some(*n),
                    _ => None,
                },
                Expr::BinOp(op, l, r) => {
                    let lv = extract(l, bindings)?;
                    let rv = extract(r, bindings)?;
                    match op {
                        BinOp::Add => Some(lv + rv),
                        BinOp::Sub => Some(lv - rv),
                        BinOp::Mul => Some(lv * rv),
                        BinOp::Div => if rv == 0 { None } else { Some(lv / rv) },
                    }
                }
            }
        }
        extract(expr, bindings).map(OpaqueValue::Num)
    }
}

#[test]
fn custom_evaluator_does_arithmetic_over_foreign_v() {
    let mut bindings: HashMap<String, BoundValue<String, OpaqueValue>> = HashMap::new();
    bindings.insert("x".into(), BoundValue::Value(OpaqueValue::Num(3)));

    let expr: Expr<OpaqueValue> = Expr::bin(
        BinOp::Add,
        Expr::var("x"),
        Expr::lit(OpaqueValue::Num(5)),
    );

    let result = OpaqueValueLetEvaluator.evaluate(&expr, &bindings);
    assert_eq!(result, Some(OpaqueValue::Num(8)));
}

#[test]
fn custom_evaluator_returns_none_for_non_numeric_bindings() {
    // Demonstrates the failure mode: when a let expression references a
    // var bound to a non-numeric variant, the custom evaluator returns
    // None, which would cause the partial match to be discarded.
    let mut bindings: HashMap<String, BoundValue<String, OpaqueValue>> = HashMap::new();
    bindings.insert(
        "x".into(),
        BoundValue::Value(OpaqueValue::Str("not a number".into())),
    );

    let expr: Expr<OpaqueValue> =
        Expr::bin(BinOp::Add, Expr::var("x"), Expr::lit(OpaqueValue::Num(5)));

    assert_eq!(OpaqueValueLetEvaluator.evaluate(&expr, &bindings), None);
}

#[test]
fn engine_uses_custom_evaluator_for_foreign_v() {
    let _engine: SiftEngine<String, String, OpaqueValue, i64, OpaqueValueLetEvaluator> =
        SiftEngine::new(OpaqueValueLetEvaluator);
}

#[test]
fn engine_with_default_evaluator_still_works_for_arithmetic_v() {
    // Sanity: existing flow continues to work over a V that DOES
    // implement ArithmeticValue.

    #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
    struct N(i64);

    impl fabula::ArithmeticValue for N {
        fn try_add(&self, o: &Self) -> Option<Self> { Some(N(self.0 + o.0)) }
        fn try_sub(&self, o: &Self) -> Option<Self> { Some(N(self.0 - o.0)) }
        fn try_mul(&self, o: &Self) -> Option<Self> { Some(N(self.0 * o.0)) }
        fn try_div(&self, o: &Self) -> Option<Self> {
            if o.0 == 0 { None } else { Some(N(self.0 / o.0)) }
        }
    }

    let _engine: SiftEngine<String, String, N, i64, DefaultLetEvaluator> =
        SiftEngine::new(DefaultLetEvaluator);
}
