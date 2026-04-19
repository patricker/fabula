//! Pattern types -- the compiled representation of a sifting query.
//!
//! A pattern describes a subgraph template with named variables, temporal
//! ordering constraints, and negation windows.
//!
//! Based on Kreminski et al. (2019) "Felt" -- sifting patterns as Datalog-like
//! queries with logic variables and temporal ordering -- and Kreminski et al.
//! (2021) "Winnow" -- multi-stage pattern architecture with negation windows.

use crate::datasource::ValueConstraint;
use crate::interval::AllenRelation;
use std::collections::{HashMap, HashSet};
use std::fmt;

/// A named position in a pattern traversal.
///
/// Variables that appear in multiple clauses create joins -- the pattern
/// matcher ensures they bind to the same node.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Target<V> {
    /// Bind the target to a variable (for traversal / join).
    Bind(Var),
    /// The target must equal this literal value.
    Literal(V),
    /// The target must satisfy this constraint.
    Constraint(ValueConstraint<V>),
}

/// A single clause in a pattern -- one edge traversal constraint.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Clause<L, V> {
    /// The source node variable (must be bound by a prior clause or be a scan root).
    pub source: Var,
    /// The edge label to follow.
    pub label: L,
    /// What the target should match.
    pub target: Target<V>,
    /// If true, this clause is negated -- the edge must NOT exist.
    pub negated: bool,
}

/// Optional metric bound on the gap distance for a temporal constraint.
/// The "gap" meaning depends on the Allen relation (see `Interval::gap_for_relation`).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MetricGap {
    /// Minimum gap distance. `None` = no lower bound.
    pub min: Option<f64>,
    /// Maximum gap distance. `None` = no upper bound.
    pub max: Option<f64>,
}

/// A temporal ordering constraint between two event variables.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

/// A negation window -- a set of clauses that must NOT match between two events.
///
/// Corresponds to Winnow's `unless-event ... between ?start ?end`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

/// A compiled sifting pattern -- a named subgraph template with temporal
/// constraints and negation windows.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Pattern<L, V> {
    /// Pattern name (for identification in matches and gap analysis).
    pub name: String,
    /// Ordered event stages -- each stage is a group of clauses that must all
    /// match the same event/node. Stages are temporally ordered left-to-right.
    pub stages: Vec<Stage<L, V>>,
    /// Additional temporal constraints beyond implicit stage ordering.
    pub temporal: Vec<TemporalConstraint>,
    /// Negation windows -- clauses that must NOT match between events.
    pub negations: Vec<Negation<L, V>>,
    /// Optional mutual-exclusion group. When a pattern with a group completes,
    /// all other active partial matches in the same group are killed.
    /// Used by `compose::choice()` with `exclusive: true`.
    pub group: Option<String>,
    /// Arbitrary key-value metadata attached to this pattern.
    /// Propagated to Match results and SiftEvent notifications.
    pub metadata: HashMap<String, String>,
    /// Optional deadline in engine ticks. If a partial match does not
    /// complete within this many ticks of its creation, the engine
    /// emits `SiftEvent::Expired` and kills the PM.
    pub deadline_ticks: Option<u64>,
    /// If set, PMs that don't advance for this many ticks are auto-pruned in `end_tick()`.
    #[cfg_attr(feature = "serde", serde(default))]
    pub inactivity_threshold: Option<u64>,
    /// Repeat range configuration. When set, the engine loops over a segment
    /// of stages instead of completing after the last stage. Enables "at least
    /// N, up to M" matching with first/last binding bookends.
    pub repeat_range: Option<RepeatRange>,
    /// Unordered stage groups. Each inner Vec contains stage indices that may
    /// match in any order (must be consecutive indices, max stage index < 64).
    /// Stages within the same group have no implicit temporal ordering -- the
    /// engine tries all unmatched group stages against each incoming edge and
    /// advances past the group when all are matched.
    #[cfg_attr(feature = "serde", serde(default))]
    pub unordered_groups: Vec<Vec<usize>>,
    /// If true, this pattern's matches and events are suppressed from engine
    /// output. The engine still evaluates the pattern internally (for
    /// composition and exclusive group handling), but `evaluate()`,
    /// `drain_completed()`, and `on_edge_added()` filter out its results.
    #[cfg_attr(feature = "serde", serde(default))]
    pub private: bool,
    /// Relative importance weight for narrative scoring and prioritization.
    /// Defaults to 1.0. Higher values cause this pattern's matches to be
    /// weighted more heavily in composite scores.
    #[cfg_attr(feature = "serde", serde(default = "default_importance"))]
    pub importance: f64,
    /// When true, this pattern's PMs are consumed after strict-forward
    /// advancement (the original PM is marked Dead by the engine). This
    /// prevents long-tail duplication in crowded simulations where the
    /// same stage will never be matched again with the same bindings.
    /// Default: false (preserves the classic "original survives" invariant).
    ///
    /// Within-unordered-group advancements do NOT consume the original --
    /// the mask semantics require it alive for further group matching.
    #[cfg_attr(feature = "serde", serde(default))]
    pub advance_in_place: bool,
}

#[cfg(feature = "serde")]
fn default_importance() -> f64 {
    1.0
}

/// Configuration for looping repeat patterns (`* N..M` or `* N..`).
///
/// The pattern's stages are laid out as `[first_... | last_...]`. The `last_`
/// segment loops: when a PM advances past `stage_end - 1`, the engine resets
/// `next_stage` to `stage_start` and increments `repetition_count`. Completion
/// is emitted once `min_reps` is reached. Looping stops at `max_reps` (or never
/// if unbounded).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RepeatRange {
    /// First stage index of the looping segment (inclusive).
    pub stage_start: usize,
    /// Last stage index of the looping segment (exclusive).
    pub stage_end: usize,
    /// Minimum repetitions before the first completion.
    pub min_reps: usize,
    /// Maximum repetitions. `None` = unlimited.
    pub max_reps: Option<usize>,
    /// Variables shared across repetitions (not prefixed, persist across loops).
    pub shared_vars: HashSet<String>,
}

/// A stage is a group of clauses anchored to a single event/node variable.
/// Stages are the units of incremental matching -- each new event is tested
/// against the next unmatched stage.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
            clauses: self
                .clauses
                .iter()
                .map(|c| c.map_types(label_fn, value_fn))
                .collect(),
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
            clauses: self
                .clauses
                .iter()
                .map(|c| c.map_types(label_fn, value_fn))
                .collect(),
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
            stages: self
                .stages
                .iter()
                .map(|s| s.map_types(&label_fn, &value_fn))
                .collect(),
            temporal: self.temporal.clone(),
            negations: self
                .negations
                .iter()
                .map(|n| n.map_types(&label_fn, &value_fn))
                .collect(),
            group: self.group.clone(),
            metadata: self.metadata.clone(),
            deadline_ticks: self.deadline_ticks,
            inactivity_threshold: self.inactivity_threshold,
            repeat_range: self.repeat_range.clone(),
            unordered_groups: self.unordered_groups.clone(),
            private: self.private,
            importance: self.importance,
            advance_in_place: self.advance_in_place,
        }
    }

    /// Return the unordered group containing this stage index, if any.
    pub fn unordered_group_for(&self, stage_idx: usize) -> Option<&Vec<usize>> {
        self.unordered_groups
            .iter()
            .find(|g| g.contains(&stage_idx))
    }

    /// Check if two stage indices are in the same unordered group.
    pub fn same_unordered_group(&self, a: usize, b: usize) -> bool {
        self.unordered_groups
            .iter()
            .any(|g| g.contains(&a) && g.contains(&b))
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

    /// Total number of clauses across all stages.
    pub fn condition_count(&self) -> usize {
        self.stages.iter().map(|s| s.clauses.len()).sum()
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
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), "betray".into()).edge_bind(
                    "e1",
                    "actor".into(),
                    "char",
                )
            })
            .build();

        let mapped = pattern.map_types(|l| l.len() as u32, |v| v.len() as i64);

        assert_eq!(mapped.name, "test");
        assert_eq!(mapped.stages[0].clauses[0].label, 9); // "eventType".len()
        assert_eq!(mapped.stages[0].clauses[0].target, Target::Literal(6)); // "betray".len()
                                                                            // Bind targets are unchanged (variables, not values)
        assert_eq!(mapped.stages[0].clauses[1].label, 5); // "actor".len()
        assert!(matches!(
            mapped.stages[0].clauses[1].target,
            Target::Bind(_)
        ));
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
        assert_eq!(
            mapped.stages[0].clauses[0].target,
            Target::Literal("SETUP".into())
        );
    }

    #[test]
    fn condition_count_sums_clauses_across_stages() {
        let pattern = PatternBuilder::<String, String>::new("test")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), "betray".into()).edge_bind(
                    "e1",
                    "actor".into(),
                    "char",
                )
            })
            .stage("e2", |s| s.edge("e2", "eventType".into(), "betray".into()))
            .build();
        assert_eq!(pattern.condition_count(), 3);

        let empty = PatternBuilder::<String, String>::new("empty").build();
        assert_eq!(empty.condition_count(), 0);

        // Negation clauses are NOT counted -- only stage clauses
        let with_negation = PatternBuilder::<String, String>::new("neg")
            .stage("e1", |s| s.edge("e1", "type".into(), "start".into()))
            .stage("e2", |s| s.edge("e2", "type".into(), "end".into()))
            .unless_between("e1", "e2", |n| n.edge("mid", "type".into(), "block".into()))
            .build();
        assert_eq!(with_negation.condition_count(), 2); // 2 stage clauses, not 3
    }

    #[test]
    fn map_types_handles_all_constraint_variants() {
        let double = |v: &i32| (*v * 2) as i64;

        assert_eq!(ValueConstraint::Eq(5).map(&double), ValueConstraint::Eq(10));
        assert_eq!(ValueConstraint::Lt(3).map(&double), ValueConstraint::Lt(6));
        assert_eq!(ValueConstraint::Gt(4).map(&double), ValueConstraint::Gt(8));
        assert_eq!(
            ValueConstraint::Lte(2).map(&double),
            ValueConstraint::Lte(4)
        );
        assert_eq!(
            ValueConstraint::Gte(1).map(&double),
            ValueConstraint::Gte(2)
        );
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

    #[test]
    fn builder_metadata_propagates() {
        let pattern = PatternBuilder::<String, String>::new("test")
            .metadata("severity", "high")
            .metadata("mitre", "T1078")
            .stage("e1", |s| s.edge("e1", "type".into(), "x".into()))
            .build();

        assert_eq!(pattern.metadata.get("severity").unwrap(), "high");
        assert_eq!(pattern.metadata.get("mitre").unwrap(), "T1078");
        assert_eq!(pattern.metadata.len(), 2);
    }

    #[test]
    fn metadata_empty_by_default() {
        let pattern = PatternBuilder::<String, String>::new("test")
            .stage("e1", |s| s.edge("e1", "type".into(), "x".into()))
            .build();
        assert!(pattern.metadata.is_empty());
    }

    #[test]
    fn private_pattern_field() {
        let pattern = PatternBuilder::<String, String>::new("helper")
            .stage("e1", |s| s.edge("e1", "type".into(), "test".into()))
            .private()
            .build();
        assert!(pattern.private);

        let public = PatternBuilder::<String, String>::new("visible")
            .stage("e1", |s| s.edge("e1", "type".into(), "test".into()))
            .build();
        assert!(!public.private);
    }

    #[test]
    fn map_types_preserves_metadata() {
        let pattern = PatternBuilder::<String, String>::new("test")
            .metadata("key", "value")
            .stage("e1", |s| s.edge("e1", "type".into(), "x".into()))
            .build();

        let mapped = pattern.map_types(|l| l.to_uppercase(), |v| v.to_uppercase());
        assert_eq!(mapped.metadata.get("key").unwrap(), "value");
    }
}
