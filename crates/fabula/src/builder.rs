//! Ergonomic builder API for constructing patterns.
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

/// Builder for constructing a [`Pattern`].
pub struct PatternBuilder<L, V> {
    name: String,
    stages: Vec<Stage<L, V>>,
    temporal: Vec<TemporalConstraint>,
    negations: Vec<Negation<L, V>>,
}

impl<L: Clone, V: Clone> PatternBuilder<L, V> {
    /// Start building a new pattern with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            stages: Vec::new(),
            temporal: Vec::new(),
            negations: Vec::new(),
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
    pub fn edge(
        mut self,
        source: impl Into<String>,
        label: L,
        value: V,
    ) -> Self {
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

    /// Add a negated clause: the edge must NOT exist.
    pub fn not_edge(
        mut self,
        source: impl Into<String>,
        label: L,
        value: V,
    ) -> Self {
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
    pub fn edge(
        mut self,
        source: impl Into<String>,
        label: L,
        value: V,
    ) -> Self {
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
