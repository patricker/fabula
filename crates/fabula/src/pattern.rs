//! Pattern types — the compiled representation of a sifting query.
//!
//! A pattern describes a subgraph template with named variables, temporal
//! ordering constraints, and negation windows.

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
#[derive(Debug, Clone)]
pub enum Target<V> {
    /// Bind the target to a variable (for traversal / join).
    Bind(Var),
    /// The target must equal this literal value.
    Literal(V),
    /// The target must satisfy this constraint.
    Constraint(ValueConstraint<V>),
}

/// A single clause in a pattern — one edge traversal constraint.
#[derive(Debug, Clone)]
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

/// A temporal ordering constraint between two event variables.
#[derive(Debug, Clone)]
pub struct TemporalConstraint {
    /// The variable whose interval should come first.
    pub left: Var,
    /// The required Allen relation (typically `Before` or `Meets`).
    pub relation: AllenRelation,
    /// The variable whose interval should come second.
    pub right: Var,
}

/// A negation window — a set of clauses that must NOT match between two events.
///
/// Corresponds to Winnow's `unless-event ... between ?start ?end`.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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
}

/// A stage is a group of clauses anchored to a single event/node variable.
/// Stages are the units of incremental matching — each new event is tested
/// against the next unmatched stage.
#[derive(Debug, Clone)]
pub struct Stage<L, V> {
    /// The event/node variable this stage is anchored to.
    pub anchor: Var,
    /// Clauses that constrain this event/node.
    pub clauses: Vec<Clause<L, V>>,
}

impl<L, V> Pattern<L, V> {
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
