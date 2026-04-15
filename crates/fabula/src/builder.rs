//! Ergonomic builder API for constructing patterns.
//!
//! Provides a fluent interface for building [`Pattern`] structs with stages,
//! temporal constraints, and negation windows. See [`crate::pattern`] for the
//! underlying types and their research lineage.
//!
//! ```rust
//! use fabula::builder::PatternBuilder;
//!
//! let pattern = PatternBuilder::<String, String>::new("betrayal_after_failure")
//!     .stage("event1", |s| s
//!         .edge("event1", "type".to_string(), "institutional_failure".to_string())
//!         .edge_bind("event1", "target".to_string(), "character"))
//!     .stage("event2", |s| s
//!         .edge("event2", "type".to_string(), "trust_violation".to_string())
//!         .edge_bind("event2", "target".to_string(), "character"))
//!     .unless_between("event1", "event2", |neg| neg
//!         .edge("recovery", "type".to_string(), "trust_restored".to_string())
//!         .edge_bind("recovery", "target".to_string(), "character"))
//!     .build();
//! ```

use crate::interval::AllenRelation;
use crate::pattern::*;
use std::collections::HashMap;

/// Builder for constructing a [`Pattern`].
pub struct PatternBuilder<L, V> {
    name: String,
    stages: Vec<Stage<L, V>>,
    temporal: Vec<TemporalConstraint>,
    negations: Vec<Negation<L, V>>,
    metadata: HashMap<String, String>,
    deadline_ticks: Option<u64>,
    unordered_groups: Vec<Vec<usize>>,
    private: bool,
    importance: f64,
}

impl<L: Clone, V: Clone> PatternBuilder<L, V> {
    /// Start building a new pattern with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            stages: Vec::new(),
            temporal: Vec::new(),
            negations: Vec::new(),
            metadata: HashMap::new(),
            deadline_ticks: None,
            unordered_groups: Vec::new(),
            private: false,
            importance: 1.0,
        }
    }

    /// Add an event stage. The `anchor` names the event variable.
    /// Use the callback to add clauses.
    pub fn stage(
        mut self,
        anchor: impl Into<String>,
        build: impl FnOnce(StageBuilder<L, V>) -> StageBuilder<L, V>,
    ) -> Self {
        let builder = StageBuilder::new(anchor.into());
        let builder = build(builder);
        self.stages.push(builder.build());
        self
    }

    /// Add an explicit temporal constraint (beyond implicit stage ordering).
    pub fn temporal(
        mut self,
        left: impl Into<String>,
        relation: AllenRelation,
        right: impl Into<String>,
    ) -> Self {
        self.temporal.push(TemporalConstraint {
            left: Var::new(left),
            relation,
            right: Var::new(right),
            gap: None,
        });
        self
    }

    /// Add a temporal constraint with a metric gap bound (STN-style).
    pub fn temporal_with_gap(
        mut self,
        left: impl Into<String>,
        relation: AllenRelation,
        right: impl Into<String>,
        gap: MetricGap,
    ) -> Self {
        self.temporal.push(TemporalConstraint {
            left: Var::new(left),
            relation,
            right: Var::new(right),
            gap: Some(gap),
        });
        self
    }

    /// Add a negation window: these clauses must NOT match between two events.
    pub fn unless_between(
        mut self,
        start: impl Into<String>,
        end: impl Into<String>,
        build: impl FnOnce(NegationBuilder<L, V>) -> NegationBuilder<L, V>,
    ) -> Self {
        let builder = NegationBuilder::new(start.into(), Some(end.into()));
        let builder = build(builder);
        self.negations.push(builder.build());
        self
    }

    /// Add a negation window with an open end (up to "now").
    pub fn unless_after(
        mut self,
        start: impl Into<String>,
        build: impl FnOnce(NegationBuilder<L, V>) -> NegationBuilder<L, V>,
    ) -> Self {
        let builder = NegationBuilder::new(start.into(), None);
        let builder = build(builder);
        self.negations.push(builder.build());
        self
    }

    /// Set a deadline in engine ticks for partial match expiry.
    pub fn deadline(mut self, ticks: u64) -> Self {
        self.deadline_ticks = Some(ticks);
        self
    }

    /// Mark this pattern as private — the engine evaluates it internally but
    /// suppresses its matches and events from output.
    pub fn private(mut self) -> Self {
        self.private = true;
        self
    }

    /// Set the importance weight for this pattern. Defaults to 1.0.
    /// Higher values cause this pattern's matches to be weighted more
    /// heavily in narrative scoring.
    pub fn importance(mut self, weight: f64) -> Self {
        self.importance = weight;
        self
    }

    /// Attach a metadata key-value pair to the pattern.
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Add a negation that spans the entire pattern (first stage to last stage).
    /// Equivalent to Winnow's `(unless-event ... where ...)` with no `between`.
    pub fn unless_global(
        mut self,
        build: impl FnOnce(NegationBuilder<L, V>) -> NegationBuilder<L, V>,
    ) -> Self {
        // Will be resolved to first/last stage anchors at build time
        let builder = NegationBuilder::new(String::new(), None);
        let builder = build(builder);
        let mut neg = builder.build();
        neg.is_global = true;
        self.negations.push(neg);
        self
    }

    /// Add a group of stages that may match in any order (concurrent).
    ///
    /// Use the callback to add stages — they are appended to the pattern
    /// and their indices are recorded as an unordered group. The engine
    /// will try all unmatched stages in the group against each incoming
    /// edge and advance past the group when all are matched.
    ///
    /// ```rust,ignore
    /// let pattern = PatternBuilder::<String, String>::new("concurrent_events")
    ///     .stage("setup", |s| s.edge("setup", "type".into(), "start".into()))
    ///     .unordered_group(|b| b
    ///         .stage("a", |s| s.edge("a", "type".into(), "event_a".into()))
    ///         .stage("b", |s| s.edge("b", "type".into(), "event_b".into())))
    ///     .stage("conclusion", |s| s.edge("conclusion", "type".into(), "end".into()))
    ///     .build();
    /// ```
    pub fn unordered_group(
        mut self,
        build: impl FnOnce(UnorderedGroupBuilder<L, V>) -> UnorderedGroupBuilder<L, V>,
    ) -> Self {
        let start_idx = self.stages.len();
        let group_builder = UnorderedGroupBuilder::new();
        let group_builder = build(group_builder);
        let stages = group_builder.stages;
        let count = stages.len();
        debug_assert!(count >= 2, "unordered group should have at least 2 stages");
        self.stages.extend(stages);
        if count > 0 {
            let end_idx = start_idx + count - 1;
            debug_assert!(
                end_idx < 64,
                "unordered group stage indices must be < 64 (matched_stages is u64)"
            );
            let indices: Vec<usize> = (start_idx..start_idx + count).collect();
            self.unordered_groups.push(indices);
        }
        self
    }

    /// Build the pattern.
    pub fn build(mut self) -> Pattern<L, V> {
        // Resolve global negation bounds to first/last stage anchors
        let first_anchor = self.stages.first().map(|s| s.anchor.0.clone());
        let last_anchor = self.stages.last().map(|s| s.anchor.0.clone());
        for neg in &mut self.negations {
            if neg.is_global {
                if let Some(ref first) = first_anchor {
                    neg.between_start = Var::new(first.clone());
                    // B7 fix: if single stage, use open-ended window (None)
                    // instead of same anchor which creates zero-width window
                    neg.between_end = match &last_anchor {
                        Some(last) if last != first => Some(Var::new(last.clone())),
                        _ => None, // single stage or no stages → open-ended
                    };
                }
                neg.is_global = false; // B5b: always clear, even if no stages
            }
        }
        Pattern {
            name: self.name,
            stages: self.stages,
            temporal: self.temporal,
            negations: self.negations,
            group: None,
            metadata: self.metadata,
            deadline_ticks: self.deadline_ticks,
            repeat_range: None,
            unordered_groups: self.unordered_groups,
            private: self.private,
            importance: self.importance,
        }
    }
}

/// Builder for a single event stage within a pattern.
pub struct StageBuilder<L, V> {
    anchor: String,
    clauses: Vec<Clause<L, V>>,
}

impl<L: Clone, V: Clone> StageBuilder<L, V> {
    fn new(anchor: String) -> Self {
        Self {
            anchor,
            clauses: Vec::new(),
        }
    }

    /// Add a clause: `source --[label]--> literal_value`.
    pub fn edge(mut self, source: impl Into<String>, label: L, value: V) -> Self {
        self.clauses.push(Clause {
            source: Var::new(source),
            label,
            target: Target::Literal(value),
            negated: false,
        });
        self
    }

    /// Add a clause: `source --[label]--> ?bind_var` (traverse and bind).
    pub fn edge_bind(
        mut self,
        source: impl Into<String>,
        label: L,
        bind_to: impl Into<String>,
    ) -> Self {
        self.clauses.push(Clause {
            source: Var::new(source),
            label,
            target: Target::Bind(Var::new(bind_to)),
            negated: false,
        });
        self
    }

    /// Add a constrained clause: `source --[label]--> (constraint)`.
    pub fn edge_constrained(
        mut self,
        source: impl Into<String>,
        label: L,
        constraint: crate::datasource::ValueConstraint<V>,
    ) -> Self {
        self.clauses.push(Clause {
            source: Var::new(source),
            label,
            target: Target::Constraint(constraint),
            negated: false,
        });
        self
    }

    /// Add a clause comparing edge target against a bound variable: `source --[label]--> (== ?var)`.
    pub fn edge_eq_var(
        mut self,
        source: impl Into<String>,
        label: L,
        var_name: impl Into<String>,
    ) -> Self {
        self.clauses.push(Clause {
            source: Var::new(source),
            label,
            target: Target::Constraint(crate::datasource::ValueConstraint::EqVar(var_name.into())),
            negated: false,
        });
        self
    }

    /// Add a clause: `source --[label]--> (< ?var)`.
    pub fn edge_lt_var(
        mut self,
        source: impl Into<String>,
        label: L,
        var_name: impl Into<String>,
    ) -> Self {
        self.clauses.push(Clause {
            source: Var::new(source),
            label,
            target: Target::Constraint(crate::datasource::ValueConstraint::LtVar(var_name.into())),
            negated: false,
        });
        self
    }

    /// Add a clause: `source --[label]--> (> ?var)`.
    pub fn edge_gt_var(
        mut self,
        source: impl Into<String>,
        label: L,
        var_name: impl Into<String>,
    ) -> Self {
        self.clauses.push(Clause {
            source: Var::new(source),
            label,
            target: Target::Constraint(crate::datasource::ValueConstraint::GtVar(var_name.into())),
            negated: false,
        });
        self
    }

    /// Add a clause: `source --[label]--> (<= ?var)`.
    pub fn edge_lte_var(
        mut self,
        source: impl Into<String>,
        label: L,
        var_name: impl Into<String>,
    ) -> Self {
        self.clauses.push(Clause {
            source: Var::new(source),
            label,
            target: Target::Constraint(crate::datasource::ValueConstraint::LteVar(var_name.into())),
            negated: false,
        });
        self
    }

    /// Add a clause: `source --[label]--> (>= ?var)`.
    pub fn edge_gte_var(
        mut self,
        source: impl Into<String>,
        label: L,
        var_name: impl Into<String>,
    ) -> Self {
        self.clauses.push(Clause {
            source: Var::new(source),
            label,
            target: Target::Constraint(crate::datasource::ValueConstraint::GteVar(var_name.into())),
            negated: false,
        });
        self
    }

    /// Add a clause matching any of the given values: `source --[label]--> (one of values)`.
    pub fn edge_one_of(mut self, source: impl Into<String>, label: L, values: Vec<V>) -> Self {
        self.clauses.push(Clause {
            source: Var::new(source),
            label,
            target: Target::Constraint(crate::datasource::ValueConstraint::OneOf(values)),
            negated: false,
        });
        self
    }

    /// Add a negated one-of clause: the edge target must NOT be any of the given values.
    pub fn not_edge_one_of(mut self, source: impl Into<String>, label: L, values: Vec<V>) -> Self {
        self.clauses.push(Clause {
            source: Var::new(source),
            label,
            target: Target::Constraint(crate::datasource::ValueConstraint::OneOf(values)),
            negated: true,
        });
        self
    }

    /// Add a negated clause: the edge must NOT exist.
    pub fn not_edge(mut self, source: impl Into<String>, label: L, value: V) -> Self {
        self.clauses.push(Clause {
            source: Var::new(source),
            label,
            target: Target::Literal(value),
            negated: true,
        });
        self
    }

    fn build(self) -> Stage<L, V> {
        Stage {
            anchor: Var::new(self.anchor),
            clauses: self.clauses,
        }
    }
}

/// Builder for a negation window.
pub struct NegationBuilder<L, V> {
    between_start: String,
    between_end: Option<String>,
    clauses: Vec<Clause<L, V>>,
}

impl<L: Clone, V: Clone> NegationBuilder<L, V> {
    fn new(start: String, end: Option<String>) -> Self {
        Self {
            between_start: start,
            between_end: end,
            clauses: Vec::new(),
        }
    }

    /// Add a clause to the negation (edge that must NOT exist in the window).
    pub fn edge(mut self, source: impl Into<String>, label: L, value: V) -> Self {
        self.clauses.push(Clause {
            source: Var::new(source),
            label,
            target: Target::Literal(value),
            negated: false,
        });
        self
    }

    /// Add a binding clause to the negation.
    pub fn edge_bind(
        mut self,
        source: impl Into<String>,
        label: L,
        bind_to: impl Into<String>,
    ) -> Self {
        self.clauses.push(Clause {
            source: Var::new(source),
            label,
            target: Target::Bind(Var::new(bind_to)),
            negated: false,
        });
        self
    }

    /// Add a constrained clause to the negation.
    pub fn edge_constrained(
        mut self,
        source: impl Into<String>,
        label: L,
        constraint: crate::datasource::ValueConstraint<V>,
    ) -> Self {
        self.clauses.push(Clause {
            source: Var::new(source),
            label,
            target: Target::Constraint(constraint),
            negated: false,
        });
        self
    }

    fn build(self) -> Negation<L, V> {
        Negation {
            between_start: Var::new(self.between_start),
            between_end: self.between_end.map(Var::new),
            clauses: self.clauses,
            is_global: false,
        }
    }
}

/// Builder for an unordered (concurrent) group of stages within a pattern.
pub struct UnorderedGroupBuilder<L, V> {
    stages: Vec<Stage<L, V>>,
}

impl<L: Clone, V: Clone> UnorderedGroupBuilder<L, V> {
    fn new() -> Self {
        Self { stages: Vec::new() }
    }

    /// Add a stage to the unordered group.
    pub fn stage(
        mut self,
        anchor: impl Into<String>,
        build: impl FnOnce(StageBuilder<L, V>) -> StageBuilder<L, V>,
    ) -> Self {
        let builder = StageBuilder::new(anchor.into());
        let builder = build(builder);
        self.stages.push(builder.build());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datasource::ValueConstraint;

    #[test]
    fn edge_one_of_creates_constraint_clause() {
        let pattern = PatternBuilder::<String, String>::new("test_one_of")
            .stage("e1", |s| {
                s.edge_one_of(
                    "e1",
                    "status".to_string(),
                    vec![
                        "active".to_string(),
                        "pending".to_string(),
                        "review".to_string(),
                    ],
                )
            })
            .build();

        assert_eq!(pattern.stages.len(), 1);
        let clause = &pattern.stages[0].clauses[0];
        assert!(!clause.negated);
        match &clause.target {
            Target::Constraint(ValueConstraint::OneOf(values)) => {
                assert_eq!(
                    values,
                    &vec![
                        "active".to_string(),
                        "pending".to_string(),
                        "review".to_string(),
                    ]
                );
            }
            other => panic!("Expected Target::Constraint(ValueConstraint::OneOf(..)), got {:?}", other),
        }
    }

    #[test]
    fn not_edge_one_of_creates_negated_constraint_clause() {
        let pattern = PatternBuilder::<String, String>::new("test_not_one_of")
            .stage("e1", |s| {
                s.not_edge_one_of(
                    "e1",
                    "status".to_string(),
                    vec!["closed".to_string(), "archived".to_string()],
                )
            })
            .build();

        assert_eq!(pattern.stages.len(), 1);
        let clause = &pattern.stages[0].clauses[0];
        assert!(clause.negated);
        match &clause.target {
            Target::Constraint(ValueConstraint::OneOf(values)) => {
                assert_eq!(
                    values,
                    &vec!["closed".to_string(), "archived".to_string()]
                );
            }
            other => panic!("Expected Target::Constraint(ValueConstraint::OneOf(..)), got {:?}", other),
        }
    }
}
