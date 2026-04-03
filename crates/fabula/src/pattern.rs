//! Pattern types — the compiled representation of a sifting query.
//!
//! A pattern describes a subgraph template with named variables, temporal
//! ordering constraints, and negation windows.
//!
//! Based on Kreminski et al. (2019) "Felt" — sifting patterns as Datalog-like
//! queries with logic variables and temporal ordering — and Kreminski et al.
//! (2021) "Winnow" — multi-stage pattern architecture with negation windows.

use crate::datasource::ValueConstraint;
use crate::interval::AllenRelation;
use std::fmt;

/// A named position in a pattern traversal.
///
/// Variables that appear in multiple clauses create joins — the pattern
/// matcher ensures they bind to the same node.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Var(pub String);

impl Var {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

impl fmt::Display for Var {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?{}", self.0)
    }
}

/// What an edge's target should match against.
#[derive(Debug, Clone, PartialEq)]
pub enum Target<V> {
    /// Bind the target to a variable (for traversal / join).
    Bind(Var),
    /// The target must equal this literal value.
    Literal(V),
    /// The target must satisfy this constraint.
    Constraint(ValueConstraint<V>),
}

/// A single clause in a pattern — one edge traversal constraint.
#[derive(Debug, Clone, PartialEq)]
pub struct Clause<L, V> {
    /// The source node variable (must be bound by a prior clause or be a scan root).
    pub source: Var,
    /// The edge label to follow.
    pub label: L,
    /// What the target should match.
    pub target: Target<V>,
    /// If true, this clause is negated — the edge must NOT exist.
    pub negated: bool,
}

/// Optional metric bound on the gap distance for a temporal constraint.
/// The "gap" meaning depends on the Allen relation (see `Interval::gap_for_relation`).
#[derive(Debug, Clone, PartialEq)]
pub struct MetricGap {
    /// Minimum gap distance. `None` = no lower bound.
    pub min: Option<f64>,
    /// Maximum gap distance. `None` = no upper bound.
    pub max: Option<f64>,
}

/// A temporal ordering constraint between two event variables.
#[derive(Debug, Clone, PartialEq)]
pub struct TemporalConstraint {
    /// The variable whose interval should come first.
    pub left: Var,
    /// The required Allen relation (typically `Before` or `Meets`).
    pub relation: AllenRelation,
    /// The variable whose interval should come second.
    pub right: Var,
    /// Optional metric bound on the gap distance (STN-style).
    pub gap: Option<MetricGap>,
}

/// A negation window — a set of clauses that must NOT match between two events.
///
/// Corresponds to Winnow's `unless-event ... between ?start ?end`.
#[derive(Debug, Clone, PartialEq)]
pub struct Negation<L, V> {
    /// Start of the negation window (must be bound).
    pub between_start: Var,
    /// End of the negation window (must be bound or open to "now").
    pub between_end: Option<Var>,
    /// The clauses that must not match within the window.
    pub clauses: Vec<Clause<L, V>>,
    /// Internal: true if this negation should span the entire pattern.
    /// Resolved to first/last stage anchors during `PatternBuilder::build()`.
    #[doc(hidden)]
    pub is_global: bool,
}

/// A compiled sifting pattern — a named subgraph template with temporal
/// constraints and negation windows.
#[derive(Debug, Clone, PartialEq)]
pub struct Pattern<L, V> {
    /// Pattern name (for identification in matches and gap analysis).
    pub name: String,
    /// Ordered event stages — each stage is a group of clauses that must all
    /// match the same event/node. Stages are temporally ordered left-to-right.
    pub stages: Vec<Stage<L, V>>,
    /// Additional temporal constraints beyond implicit stage ordering.
    pub temporal: Vec<TemporalConstraint>,
    /// Negation windows — clauses that must NOT match between events.
    pub negations: Vec<Negation<L, V>>,
    /// Optional mutual-exclusion group. When a pattern with a group completes,
    /// all other active partial matches in the same group are killed.
    /// Used by `compose::choice()` with `exclusive: true`.
    pub group: Option<String>,
}

/// A stage is a group of clauses anchored to a single event/node variable.
/// Stages are the units of incremental matching — each new event is tested
/// against the next unmatched stage.
#[derive(Debug, Clone, PartialEq)]
pub struct Stage<L, V> {
    /// The event/node variable this stage is anchored to.
    pub anchor: Var,
    /// Clauses that constrain this event/node.
    pub clauses: Vec<Clause<L, V>>,
}

impl<V> Target<V> {
    /// Transform the value type.
    pub fn map<V2>(&self, f: &impl Fn(&V) -> V2) -> Target<V2> {
        match self {
            Target::Bind(v) => Target::Bind(v.clone()),
            Target::Literal(v) => Target::Literal(f(v)),
            Target::Constraint(c) => Target::Constraint(c.map(f)),
        }
    }
}

impl<L, V> Clause<L, V> {
    /// Transform label and value types.
    pub fn map_types<L2, V2>(
        &self,
        label_fn: &impl Fn(&L) -> L2,
        value_fn: &impl Fn(&V) -> V2,
    ) -> Clause<L2, V2> {
        Clause {
            source: self.source.clone(),
            label: label_fn(&self.label),
            target: self.target.map(value_fn),
            negated: self.negated,
        }
    }
}

impl<L, V> Stage<L, V> {
    /// Transform label and value types.
    pub fn map_types<L2, V2>(
        &self,
        label_fn: &impl Fn(&L) -> L2,
        value_fn: &impl Fn(&V) -> V2,
    ) -> Stage<L2, V2> {
        Stage {
            anchor: self.anchor.clone(),
            clauses: self.clauses.iter().map(|c| c.map_types(label_fn, value_fn)).collect(),
        }
    }
}

impl<L, V> Negation<L, V> {
    /// Transform label and value types.
    pub fn map_types<L2, V2>(
        &self,
        label_fn: &impl Fn(&L) -> L2,
        value_fn: &impl Fn(&V) -> V2,
    ) -> Negation<L2, V2> {
        Negation {
            between_start: self.between_start.clone(),
            between_end: self.between_end.clone(),
            clauses: self.clauses.iter().map(|c| c.map_types(label_fn, value_fn)).collect(),
            is_global: self.is_global,
        }
    }
}

impl<L, V> Pattern<L, V> {
    /// Transform label and value types throughout the pattern.
    ///
    /// Useful for converting between type systems (e.g., `Pattern<String, MemValue>`
    /// to `Pattern<u32, Value>`). Name, group, variables, temporal constraints,
    /// and metric gaps are preserved unchanged.
    pub fn map_types<L2, V2>(
        &self,
        label_fn: impl Fn(&L) -> L2,
        value_fn: impl Fn(&V) -> V2,
    ) -> Pattern<L2, V2> {
        Pattern {
            name: self.name.clone(),
            stages: self.stages.iter().map(|s| s.map_types(&label_fn, &value_fn)).collect(),
            temporal: self.temporal.clone(),
            negations: self.negations.iter().map(|n| n.map_types(&label_fn, &value_fn)).collect(),
            group: self.group.clone(),
        }
    }

    /// All variables used in this pattern (across all stages and negations).
    pub fn all_vars(&self) -> Vec<&Var> {
        let mut vars = Vec::new();
        for stage in &self.stages {
            vars.push(&stage.anchor);
            for clause in &stage.clauses {
                vars.push(&clause.source);
                if let Target::Bind(ref v) = clause.target {
                    vars.push(v);
                }
            }
        }
        for neg in &self.negations {
            vars.push(&neg.between_start);
            if let Some(ref v) = neg.between_end {
                vars.push(v);
            }
            for clause in &neg.clauses {
                vars.push(&clause.source);
                if let Target::Bind(ref v) = clause.target {
                    vars.push(v);
                }
            }
        }
        vars.sort_by_key(|v| &v.0);
        vars.dedup();
        vars
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::PatternBuilder;
    use crate::datasource::ValueConstraint;
    use crate::interval::AllenRelation;

    #[test]
    fn map_types_transforms_labels_and_values() {
        let pattern = PatternBuilder::<String, String>::new("test")
            .stage("e1", |s| s
                .edge("e1", "eventType".into(), "betray".into())
                .edge_bind("e1", "actor".into(), "char"))
            .build();

        let mapped = pattern.map_types(
            |l| l.len() as u32,
            |v| v.len() as i64,
        );

        assert_eq!(mapped.name, "test");
        assert_eq!(mapped.stages[0].clauses[0].label, 9); // "eventType".len()
        assert_eq!(mapped.stages[0].clauses[0].target, Target::Literal(6)); // "betray".len()
        // Bind targets are unchanged (variables, not values)
        assert_eq!(mapped.stages[0].clauses[1].label, 5); // "actor".len()
        assert!(matches!(mapped.stages[0].clauses[1].target, Target::Bind(_)));
    }

    #[test]
    fn map_types_preserves_structure() {
        let pattern = PatternBuilder::<String, String>::new("arc")
            .stage("e1", |s| s.edge("e1", "type".into(), "setup".into()))
            .stage("e2", |s| s.edge("e2", "type".into(), "payoff".into()))
            .temporal("e1", AllenRelation::Before, "e2")
            .unless_between("e1", "e2", |n| n.edge("mid", "type".into(), "block".into()))
            .build();

        let mapped = pattern.map_types(|l| l.to_uppercase(), |v| v.to_uppercase());

        // Structure preserved
        assert_eq!(mapped.stages.len(), 2);
        assert_eq!(mapped.temporal.len(), 1);
        assert_eq!(mapped.negations.len(), 1);
        assert_eq!(mapped.temporal[0].relation, AllenRelation::Before);
        assert_eq!(mapped.temporal[0].left, Var::new("e1"));
        assert_eq!(mapped.temporal[0].right, Var::new("e2"));
        assert_eq!(mapped.negations[0].between_start, Var::new("e1"));
        // Values transformed
        assert_eq!(mapped.stages[0].clauses[0].label, "TYPE");
        assert_eq!(mapped.stages[0].clauses[0].target, Target::Literal("SETUP".into()));
    }

    #[test]
    fn map_types_handles_all_constraint_variants() {
        let double = |v: &i32| (*v * 2) as i64;

        assert_eq!(ValueConstraint::Eq(5).map(&double), ValueConstraint::Eq(10));
        assert_eq!(ValueConstraint::Lt(3).map(&double), ValueConstraint::Lt(6));
        assert_eq!(ValueConstraint::Gt(4).map(&double), ValueConstraint::Gt(8));
        assert_eq!(ValueConstraint::Lte(2).map(&double), ValueConstraint::Lte(4));
        assert_eq!(ValueConstraint::Gte(1).map(&double), ValueConstraint::Gte(2));
        assert_eq!(
            ValueConstraint::Between(1, 10).map(&double),
            ValueConstraint::Between(2, 20)
        );
        assert_eq!(
            ValueConstraint::<i32>::Any.map(&double),
            ValueConstraint::<i64>::Any
        );

        // Constraint inside a Target
        let clause = Clause {
            source: Var::new("e"),
            label: "score".to_string(),
            target: Target::Constraint(ValueConstraint::Gt(50)),
            negated: false,
        };
        let mapped = clause.map_types(&|l: &String| l.clone(), &double);
        assert_eq!(mapped.target, Target::Constraint(ValueConstraint::Gt(100)));
    }
}
