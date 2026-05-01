//! Expression language for computed pattern bindings (`let`).
//!
//! `Expr<V>` is a small arithmetic AST evaluated against a binding map to
//! produce a derived `V`. Used by `Stage::let_bindings` to introduce variables
//! computed from already-bound clause variables and literals.

use crate::engine::BoundValue;
use std::collections::HashMap;
use std::fmt::Debug;

/// Binary arithmetic operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}

/// An expression producing a `V` from bound variables and literals.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Expr<V> {
    /// A literal value.
    Literal(V),
    /// Reference to a bound variable. Evaluates to that variable's value if
    /// it is bound to a `BoundValue::Value`. `None` if unbound or bound to a node.
    Var(String),
    /// Binary operation. Evaluates left and right, then applies `op` via
    /// `ArithmeticValue`.
    BinOp(BinOp, Box<Expr<V>>, Box<Expr<V>>),
}

/// A named expression evaluated after a stage's clauses match. The result is
/// inserted into the bindings map under `name`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ComputedBinding<V> {
    pub name: String,
    pub expr: Expr<V>,
}

/// Trait for value types that support arithmetic. Returns `None` for unsupported
/// operand combinations (e.g., string + number) or numeric failures (e.g., div
/// by zero).
///
/// # Relationship to engine bounds
///
/// `ArithmeticValue` is **not** required by fabula's engine API. It is required
/// only by [`DefaultLetEvaluator`](crate::engine::DefaultLetEvaluator), the
/// built-in evaluator that delegates to [`Expr::eval`]. Consumers whose `V`
/// type cannot implement `ArithmeticValue` (e.g., a foreign type subject to
/// the orphan rule) should use [`NoLetEvaluator`](crate::engine::NoLetEvaluator)
/// for let-free patterns, or supply their own
/// [`LetEvaluator`](crate::engine::LetEvaluator) implementation.
pub trait ArithmeticValue: Sized {
    fn try_add(&self, other: &Self) -> Option<Self>;
    fn try_sub(&self, other: &Self) -> Option<Self>;
    fn try_mul(&self, other: &Self) -> Option<Self>;
    fn try_div(&self, other: &Self) -> Option<Self>;
}

/// `String` carries no arithmetic semantics in `fabula`; this no-op impl returns
/// `None` from every operation. It exists so adapters and test fixtures that use
/// `V = String` (e.g. label-only graphs, the test suite) satisfy the per-site
/// `V: ArithmeticValue` bound on engine methods without manual boilerplate. Any
/// pattern that tries to evaluate a `let` against `V = String` will simply fail
/// the binding (and the partial match) rather than compute a value.
impl ArithmeticValue for String {
    fn try_add(&self, _: &Self) -> Option<Self> {
        None
    }
    fn try_sub(&self, _: &Self) -> Option<Self> {
        None
    }
    fn try_mul(&self, _: &Self) -> Option<Self> {
        None
    }
    fn try_div(&self, _: &Self) -> Option<Self> {
        None
    }
}

impl<V> Expr<V> {
    /// Convenience constructor for `Expr::Var`.
    pub fn var(name: impl Into<String>) -> Self {
        Expr::Var(name.into())
    }

    /// Convenience constructor for `Expr::Literal`.
    pub fn lit(v: V) -> Self {
        Expr::Literal(v)
    }

    /// Convenience constructor for binary ops.
    pub fn bin(op: BinOp, left: Expr<V>, right: Expr<V>) -> Self {
        Expr::BinOp(op, Box::new(left), Box::new(right))
    }

    /// Transform the value type.
    pub fn map<V2>(&self, f: &impl Fn(&V) -> V2) -> Expr<V2> {
        match self {
            Expr::Literal(v) => Expr::Literal(f(v)),
            Expr::Var(s) => Expr::Var(s.clone()),
            Expr::BinOp(op, l, r) => Expr::BinOp(*op, Box::new(l.map(f)), Box::new(r.map(f))),
        }
    }

    /// All variable names referenced in this expression.
    pub fn vars(&self) -> Vec<&str> {
        let mut out = Vec::new();
        self.collect_vars(&mut out);
        out
    }

    fn collect_vars<'a>(&'a self, out: &mut Vec<&'a str>) {
        match self {
            Expr::Literal(_) => {}
            Expr::Var(s) => out.push(s.as_str()),
            Expr::BinOp(_, l, r) => {
                l.collect_vars(out);
                r.collect_vars(out);
            }
        }
    }

    /// In-place rename of all `Var` references using the provided function.
    /// Used by `compose` to namespace variables across composed sub-patterns.
    /// The function returns `Some(new_name)` to rename or `None` to keep as-is.
    pub fn rename_vars(&mut self, f: &impl Fn(&str) -> Option<String>) {
        match self {
            Expr::Literal(_) => {}
            Expr::Var(s) => {
                if let Some(new) = f(s.as_str()) {
                    *s = new;
                }
            }
            Expr::BinOp(_, l, r) => {
                l.rename_vars(f);
                r.rename_vars(f);
            }
        }
    }
}

impl<V: ArithmeticValue + Clone + Debug> Expr<V> {
    /// Evaluate against a binding map. Returns `None` if any referenced var is
    /// unbound or bound to a node, or if any operation returns `None`.
    pub fn eval<N: Debug>(&self, bindings: &HashMap<String, BoundValue<N, V>>) -> Option<V> {
        match self {
            Expr::Literal(v) => Some(v.clone()),
            Expr::Var(name) => match bindings.get(name) {
                Some(BoundValue::Value(v)) => Some(v.clone()),
                _ => None,
            },
            Expr::BinOp(op, l, r) => {
                let lv = l.eval(bindings)?;
                let rv = r.eval(bindings)?;
                match op {
                    BinOp::Add => lv.try_add(&rv),
                    BinOp::Sub => lv.try_sub(&rv),
                    BinOp::Mul => lv.try_mul(&rv),
                    BinOp::Div => lv.try_div(&rv),
                }
            }
        }
    }
}

impl<V> ComputedBinding<V> {
    pub fn new(name: impl Into<String>, expr: Expr<V>) -> Self {
        Self {
            name: name.into(),
            expr,
        }
    }

    /// Transform the value type.
    pub fn map<V2>(&self, f: &impl Fn(&V) -> V2) -> ComputedBinding<V2> {
        ComputedBinding {
            name: self.name.clone(),
            expr: self.expr.map(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::BoundValue;

    /// Minimal numeric V for trait testing.
    #[derive(Debug, Clone, PartialEq)]
    struct N(f64);

    impl ArithmeticValue for N {
        fn try_add(&self, o: &Self) -> Option<Self> {
            Some(N(self.0 + o.0))
        }
        fn try_sub(&self, o: &Self) -> Option<Self> {
            Some(N(self.0 - o.0))
        }
        fn try_mul(&self, o: &Self) -> Option<Self> {
            Some(N(self.0 * o.0))
        }
        fn try_div(&self, o: &Self) -> Option<Self> {
            if o.0 == 0.0 {
                None
            } else {
                Some(N(self.0 / o.0))
            }
        }
    }

    fn b(n: f64) -> BoundValue<String, N> {
        BoundValue::Value(N(n))
    }

    #[test]
    fn eval_literal_returns_value() {
        let bindings: HashMap<String, BoundValue<String, N>> = HashMap::new();
        let e: Expr<N> = Expr::lit(N(7.0));
        assert_eq!(e.eval(&bindings), Some(N(7.0)));
    }

    #[test]
    fn eval_var_resolves_through_bindings() {
        let mut bindings = HashMap::new();
        bindings.insert("x".to_string(), b(3.0));
        let e: Expr<N> = Expr::var("x");
        assert_eq!(e.eval(&bindings), Some(N(3.0)));
    }

    #[test]
    fn eval_unbound_var_returns_none() {
        let bindings: HashMap<String, BoundValue<String, N>> = HashMap::new();
        let e: Expr<N> = Expr::var("missing");
        assert_eq!(e.eval(&bindings), None);
    }

    #[test]
    fn eval_node_bound_var_returns_none() {
        let mut bindings: HashMap<String, BoundValue<String, N>> = HashMap::new();
        bindings.insert("n".to_string(), BoundValue::Node("alice".to_string()));
        let e: Expr<N> = Expr::var("n");
        assert_eq!(e.eval(&bindings), None);
    }

    #[test]
    fn eval_addition() {
        let mut bindings = HashMap::new();
        bindings.insert("x".to_string(), b(3.0));
        let e: Expr<N> = Expr::bin(BinOp::Add, Expr::var("x"), Expr::lit(N(5.0)));
        assert_eq!(e.eval(&bindings), Some(N(8.0)));
    }

    #[test]
    fn eval_precedence_is_explicit_via_tree_shape() {
        // (?x + 1) * 2 = 8 when x=3
        let mut bindings = HashMap::new();
        bindings.insert("x".to_string(), b(3.0));
        let e: Expr<N> = Expr::bin(
            BinOp::Mul,
            Expr::bin(BinOp::Add, Expr::var("x"), Expr::lit(N(1.0))),
            Expr::lit(N(2.0)),
        );
        assert_eq!(e.eval(&bindings), Some(N(8.0)));
    }

    #[test]
    fn eval_division_by_zero_returns_none() {
        let bindings: HashMap<String, BoundValue<String, N>> = HashMap::new();
        let e: Expr<N> = Expr::bin(BinOp::Div, Expr::lit(N(1.0)), Expr::lit(N(0.0)));
        assert_eq!(e.eval(&bindings), None);
    }

    #[test]
    fn vars_lists_all_var_references() {
        let e: Expr<N> = Expr::bin(
            BinOp::Add,
            Expr::var("a"),
            Expr::bin(BinOp::Sub, Expr::var("b"), Expr::lit(N(1.0))),
        );
        assert_eq!(e.vars(), vec!["a", "b"]);
    }

    #[test]
    fn map_transforms_literals_only() {
        let e: Expr<N> = Expr::bin(BinOp::Add, Expr::var("x"), Expr::lit(N(2.0)));
        let mapped: Expr<f64> = e.map(&|n: &N| n.0);
        match mapped {
            Expr::BinOp(BinOp::Add, l, r) => {
                assert!(matches!(*l, Expr::Var(ref s) if s == "x"));
                assert!(matches!(*r, Expr::Literal(v) if v == 2.0));
            }
            _ => panic!("expected BinOp"),
        }
    }

    #[test]
    fn rename_vars_renames_only_unmatched_names() {
        let mut e: Expr<N> = Expr::bin(
            BinOp::Add,
            Expr::var("anchor"),
            Expr::bin(BinOp::Sub, Expr::var("shared"), Expr::lit(N(1.0))),
        );
        let shared = std::collections::HashSet::from(["shared"]);
        e.rename_vars(&|name| {
            if shared.contains(name) {
                None
            } else {
                Some(format!("a_{name}"))
            }
        });
        // anchor → a_anchor; shared stays; literal untouched
        match e {
            Expr::BinOp(BinOp::Add, l, r) => {
                assert!(matches!(*l, Expr::Var(ref s) if s == "a_anchor"));
                match *r {
                    Expr::BinOp(BinOp::Sub, ll, _) => {
                        assert!(matches!(*ll, Expr::Var(ref s) if s == "shared"));
                    }
                    _ => panic!("expected inner Sub"),
                }
            }
            _ => panic!("expected outer Add"),
        }
    }
}
