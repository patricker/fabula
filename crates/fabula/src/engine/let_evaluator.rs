//! Pluggable let-binding evaluation.
//!
//! `Stage::let_bindings` are evaluated by a [`LetEvaluator`] supplied at
//! engine construction time (or passed to free-function eval APIs). The trait
//! sits on a *separate* evaluator type — not on `V` — so consumers with
//! foreign `V` types can plug in their own arithmetic without authoring a
//! trait impl on a type they don't own (Rust orphan rule).
//!
//! Built-ins:
//! - [`NoLetEvaluator`] — always returns `None`. For let-free patterns or
//!   silent let-failure semantics.
//! - [`DefaultLetEvaluator`] — delegates to [`Expr::eval`], requires
//!   `V: ArithmeticValue`. Use this when your V supports arithmetic.

use crate::engine::types::BoundValue;
use crate::expr::Expr;
use std::collections::HashMap;
use std::fmt::Debug;

/// Evaluates `let` expressions during stage matching.
///
/// `evaluate` returns `Some(v)` if the expression produced a value, or
/// `None` if any sub-expression failed. A `None` result causes the partial
/// match to be discarded.
pub trait LetEvaluator<N: Debug, V: Debug> {
    fn evaluate(
        &self,
        expr: &Expr<V>,
        bindings: &HashMap<String, BoundValue<N, V>>,
    ) -> Option<V>;
}

/// A let evaluator that always returns `None`. Use for let-free patterns
/// or when you want let-bearing patterns to silently fail-to-match.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoLetEvaluator;

impl<N: Debug, V: Clone + Debug> LetEvaluator<N, V> for NoLetEvaluator {
    fn evaluate(
        &self,
        _expr: &Expr<V>,
        _bindings: &HashMap<String, BoundValue<N, V>>,
    ) -> Option<V> {
        None
    }
}

/// The default let evaluator: delegates to [`Expr::eval`]. Requires
/// `V: ArithmeticValue` on its impl block — the bound lives here, not on
/// the engine API.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultLetEvaluator;

impl<N, V> LetEvaluator<N, V> for DefaultLetEvaluator
where
    N: Debug,
    V: crate::expr::ArithmeticValue + Clone + Debug,
{
    fn evaluate(
        &self,
        expr: &Expr<V>,
        bindings: &HashMap<String, BoundValue<N, V>>,
    ) -> Option<V> {
        expr.eval(bindings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::{ArithmeticValue, BinOp};

    #[derive(Debug, Clone, PartialEq)]
    struct Num(f64);

    impl ArithmeticValue for Num {
        fn try_add(&self, o: &Self) -> Option<Self> { Some(Num(self.0 + o.0)) }
        fn try_sub(&self, o: &Self) -> Option<Self> { Some(Num(self.0 - o.0)) }
        fn try_mul(&self, o: &Self) -> Option<Self> { Some(Num(self.0 * o.0)) }
        fn try_div(&self, o: &Self) -> Option<Self> {
            if o.0 == 0.0 { None } else { Some(Num(self.0 / o.0)) }
        }
    }

    #[test]
    fn no_let_evaluator_always_returns_none() {
        let bindings: HashMap<String, BoundValue<String, Num>> = HashMap::new();
        let expr: Expr<Num> = Expr::lit(Num(42.0));
        let result = <NoLetEvaluator as LetEvaluator<String, Num>>::evaluate(
            &NoLetEvaluator, &expr, &bindings,
        );
        assert_eq!(result, None);
    }

    #[test]
    fn default_let_evaluator_matches_expr_eval() {
        let mut bindings: HashMap<String, BoundValue<String, Num>> = HashMap::new();
        bindings.insert("x".into(), BoundValue::Value(Num(3.0)));
        let expr: Expr<Num> = Expr::bin(BinOp::Add, Expr::var("x"), Expr::lit(Num(5.0)));
        let direct = expr.eval(&bindings);
        let via_evaluator = DefaultLetEvaluator.evaluate(&expr, &bindings);
        assert_eq!(direct, Some(Num(8.0)));
        assert_eq!(via_evaluator, Some(Num(8.0)));
    }

    #[test]
    fn default_let_evaluator_propagates_division_by_zero() {
        let bindings: HashMap<String, BoundValue<String, Num>> = HashMap::new();
        let expr: Expr<Num> = Expr::bin(BinOp::Div, Expr::lit(Num(1.0)), Expr::lit(Num(0.0)));
        let result = DefaultLetEvaluator.evaluate(&expr, &bindings);
        assert_eq!(result, None);
    }
}
