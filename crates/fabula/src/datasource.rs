//! The `DataSource` trait — how fabula queries a temporal graph.
//!
//! Any backing store implements this two-method trait to make its graph
//! queryable by fabula's pattern matcher.

use crate::interval::Interval;
use std::fmt::Debug;
use std::hash::Hash;

/// A value constraint used in pattern clauses and scans.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ValueConstraint<V> {
    /// Must equal this exact value.
    Eq(V),
    /// Must be less than this value.
    Lt(V),
    /// Must be greater than this value.
    Gt(V),
    /// Must be less than or equal to this value.
    Lte(V),
    /// Must be greater than or equal to this value.
    Gte(V),
    /// Must fall within `[low, high]`.
    Between(V, V),
    /// Any value matches.
    Any,
    /// Must equal the value bound to this variable name.
    EqVar(String),
    /// Must be less than the value bound to this variable name.
    LtVar(String),
    /// Must be greater than the value bound to this variable name.
    GtVar(String),
    /// Must be less than or equal to the value bound to this variable name.
    LteVar(String),
    /// Must be greater than or equal to the value bound to this variable name.
    GteVar(String),
}

impl<V: PartialOrd + PartialEq> ValueConstraint<V> {
    /// Check whether a value satisfies this constraint.
    pub fn matches(&self, value: &V) -> bool {
        match self {
            Self::Eq(v) => value == v,
            Self::Lt(v) => value < v,
            Self::Gt(v) => value > v,
            Self::Lte(v) => value <= v,
            Self::Gte(v) => value >= v,
            Self::Between(lo, hi) => value >= lo && value <= hi,
            Self::Any => true,
            // *Var variants must be resolved before matching. If they reach here,
            // it's a bug — fail closed (no match).
            Self::EqVar(_)
            | Self::LtVar(_)
            | Self::GtVar(_)
            | Self::LteVar(_)
            | Self::GteVar(_) => {
                debug_assert!(
                    false,
                    "BoundVar constraint reached matches() without resolution"
                );
                false
            }
        }
    }
}

impl<V> ValueConstraint<V> {
    /// Transform the value type of this constraint.
    pub fn map<V2>(&self, f: impl Fn(&V) -> V2) -> ValueConstraint<V2> {
        match self {
            Self::Eq(v) => ValueConstraint::Eq(f(v)),
            Self::Lt(v) => ValueConstraint::Lt(f(v)),
            Self::Gt(v) => ValueConstraint::Gt(f(v)),
            Self::Lte(v) => ValueConstraint::Lte(f(v)),
            Self::Gte(v) => ValueConstraint::Gte(f(v)),
            Self::Between(lo, hi) => ValueConstraint::Between(f(lo), f(hi)),
            Self::Any => ValueConstraint::Any,
            Self::EqVar(s) => ValueConstraint::EqVar(s.clone()),
            Self::LtVar(s) => ValueConstraint::LtVar(s.clone()),
            Self::GtVar(s) => ValueConstraint::GtVar(s.clone()),
            Self::LteVar(s) => ValueConstraint::LteVar(s.clone()),
            Self::GteVar(s) => ValueConstraint::GteVar(s.clone()),
        }
    }

    /// Returns `true` if this constraint references a bound variable.
    pub fn is_var(&self) -> bool {
        matches!(
            self,
            Self::EqVar(_) | Self::LtVar(_) | Self::GtVar(_) | Self::LteVar(_) | Self::GteVar(_)
        )
    }
}

// ---------------------------------------------------------------------------
// Trait bound aliases — reduce boilerplate on generic impls
// ---------------------------------------------------------------------------

/// Trait bound alias for node identifier types.
///
/// Blanket-implemented for all types satisfying the bounds.
/// Use `N: NodeId` instead of `N: Eq + Hash + Clone + Debug`.
pub trait NodeId: Eq + Hash + Clone + Debug {}
impl<T: Eq + Hash + Clone + Debug> NodeId for T {}

/// Trait bound alias for edge label types.
///
/// Blanket-implemented for all types satisfying the bounds.
/// Use `L: Label` instead of `L: Eq + Hash + Clone + Debug`.
pub trait Label: Eq + Hash + Clone + Debug {}
impl<T: Eq + Hash + Clone + Debug> Label for T {}

/// Trait bound alias for value types.
///
/// Blanket-implemented for all types satisfying the bounds.
/// Use `V: Val` instead of `V: PartialEq + PartialOrd + Clone + Debug + Hash`.
pub trait Val: PartialEq + PartialOrd + Clone + Debug + Hash {}
impl<T: PartialEq + PartialOrd + Clone + Debug + Hash> Val for T {}

/// An edge returned from a [`DataSource`] query.
#[derive(Debug, Clone)]
pub struct Edge<N, V, T> {
    /// The target node or value this edge points to.
    pub target: V,
    /// The time interval during which this edge is/was valid.
    pub interval: Interval<T>,
    /// The source node (useful for scan results).
    pub source: N,
}

/// A temporal graph that fabula can query.
///
/// This is the only trait a backing store needs to implement. Fabula's
/// pattern matcher calls these methods during evaluation; everything
/// else (pattern compilation, incremental tracking, gap analysis) is
/// internal to fabula.
///
/// # Type Parameters
///
/// - `N` — Node identifier (e.g., `EntityId`, `String`, `u64`)
/// - `L` — Edge label (e.g., predicate ID `u32`, `String`, enum)
/// - `V` — Edge value (e.g., another node ID, number, string — often the same type as `N` with an enum wrapper)
/// - `T` — Time type (e.g., `i64`, `chrono::NaiveDateTime`)
pub trait DataSource {
    /// Node identifier type.
    type N: Eq + Hash + Clone + Debug;
    /// Edge label type.
    type L: Eq + Hash + Clone + Debug;
    /// Value type (edge targets — can be nodes, strings, numbers, booleans).
    type V: PartialEq + PartialOrd + Clone + Debug + Hash;
    /// Time type.
    type T: Ord + Clone + Debug + Hash;

    /// Follow edges from `node` with `label`, active at time `at`.
    ///
    /// Returns all matching edges with their target values and validity intervals.
    fn edges_from(
        &self,
        node: &Self::N,
        label: &Self::L,
        at: &Self::T,
    ) -> Vec<Edge<Self::N, Self::V, Self::T>>;

    /// Find all source nodes that have an edge with `label` matching `constraint`,
    /// active at time `at`.
    ///
    /// This is the "index scan" — used to find starting points for pattern matching
    /// when a clause binds a new variable.
    fn scan(
        &self,
        label: &Self::L,
        constraint: &ValueConstraint<Self::V>,
        at: &Self::T,
    ) -> Vec<Edge<Self::N, Self::V, Self::T>>;

    /// Find all edges from `node` with `label` that were ever valid
    /// (regardless of time). Used for temporal constraint checking.
    fn edges_from_any_time(
        &self,
        node: &Self::N,
        label: &Self::L,
    ) -> Vec<Edge<Self::N, Self::V, Self::T>>;

    /// Scan for edges with `label` matching `constraint` at any time.
    fn scan_any_time(
        &self,
        label: &Self::L,
        constraint: &ValueConstraint<Self::V>,
    ) -> Vec<Edge<Self::N, Self::V, Self::T>>;

    /// The current time in the graph's time model.
    fn now(&self) -> Self::T;

    /// Check if a value represents a node reference (for traversal)
    /// vs. a literal (for comparison). This lets the pattern matcher
    /// know whether to follow a value as a node or compare it as data.
    fn value_as_node(&self, value: &Self::V) -> Option<Self::N>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraint_eq() {
        let c = ValueConstraint::Eq(42);
        assert!(c.matches(&42));
        assert!(!c.matches(&43));
    }

    #[test]
    fn test_constraint_range() {
        let c = ValueConstraint::Between(10, 20);
        assert!(c.matches(&10));
        assert!(c.matches(&15));
        assert!(c.matches(&20));
        assert!(!c.matches(&9));
        assert!(!c.matches(&21));
    }

    #[test]
    fn test_constraint_any() {
        let c: ValueConstraint<i32> = ValueConstraint::Any;
        assert!(c.matches(&0));
        assert!(c.matches(&i32::MAX));
    }
}
