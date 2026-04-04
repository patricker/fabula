//! Pattern composition operators — build complex patterns from simpler ones.
//!
//! Inspired by Kreminski et al. (2025) "Stories from the Bottom Up: Composable
//! Story Sifting Patterns" (FDG 2025). Enables authorial decomposition: write
//! atomic pattern fragments, compose them into complex narrative structures.
//!
//! Three operators that produce regular [`Pattern`] structs the engine handles
//! without modification:
//!
//! - [`sequence`] — A then B, with shared variable bindings
//! - [`choice`] — any of N alternatives (optionally exclusive)
//! - [`repeat`] — A happens N times
//!
//! All operators use [`rename_vars`] internally to prevent accidental variable
//! collisions between sub-patterns. Variables listed in `shared` are kept as-is;
//! all others are prefixed with the sub-pattern's position.
//!
//! # Examples
//!
//! ```rust
//! use fabula::prelude::*;
//! use fabula::compose;
//!
//! let setup = PatternBuilder::<String, String>::new("setup")
//!     .stage("e1", |s| s
//!         .edge("e1", "type".into(), "promise".into())
//!         .edge_bind("e1", "actor".into(), "char"))
//!     .build();
//!
//! let payoff = PatternBuilder::<String, String>::new("payoff")
//!     .stage("e2", |s| s
//!         .edge("e2", "type".into(), "fulfill".into())
//!         .edge_bind("e2", "actor".into(), "char"))
//!     .build();
//!
//! // char is shared — same character must make and fulfill the promise
//! let arc = compose::sequence("promise_kept", &setup, &payoff, &["char"]);
//! assert_eq!(arc.stages.len(), 2);
//! ```

use crate::pattern::*;
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// rename_vars — the core utility
// ---------------------------------------------------------------------------

/// Rename all variables in a pattern, prefixing with `prefix_` unless the
/// variable name is in `keep`. Returns a new pattern with renamed variables.
pub fn rename_vars<L: Clone, V: Clone>(
    pattern: &Pattern<L, V>,
    prefix: &str,
    keep: &HashSet<&str>,
) -> Pattern<L, V> {
    let rename = |var: &Var| -> Var {
        if keep.contains(var.0.as_str()) {
            var.clone()
        } else {
            Var::new(format!("{}_{}", prefix, var.0))
        }
    };

    let rename_clause = |c: &Clause<L, V>| -> Clause<L, V> {
        Clause {
            source: rename(&c.source),
            label: c.label.clone(),
            target: match &c.target {
                Target::Bind(v) => Target::Bind(rename(v)),
                Target::Literal(v) => Target::Literal(v.clone()),
                Target::Constraint(c) => Target::Constraint(c.clone()),
            },
            negated: c.negated,
        }
    };

    let stages = pattern
        .stages
        .iter()
        .map(|s| Stage {
            anchor: rename(&s.anchor),
            clauses: s.clauses.iter().map(&rename_clause).collect(),
        })
        .collect();

    let temporal = pattern
        .temporal
        .iter()
        .map(|tc| TemporalConstraint {
            left: rename(&tc.left),
            relation: tc.relation,
            right: rename(&tc.right),
            gap: tc.gap.clone(),
        })
        .collect();

    let negations = pattern
        .negations
        .iter()
        .map(|neg| Negation {
            between_start: rename(&neg.between_start),
            between_end: neg.between_end.as_ref().map(rename),
            clauses: neg.clauses.iter().map(&rename_clause).collect(),
            is_global: neg.is_global,
        })
        .collect();

    Pattern {
        name: pattern.name.clone(),
        stages,
        temporal,
        negations,
        group: pattern.group.clone(),
        metadata: pattern.metadata.clone(),
        deadline_ticks: pattern.deadline_ticks,
    }
}

// ---------------------------------------------------------------------------
// sequence
// ---------------------------------------------------------------------------

/// Compose two patterns in sequence: all of A's stages, then all of B's.
///
/// Variables in `shared` bind across both patterns (e.g., same character
/// in both setup and payoff). All other variables are prefixed to prevent
/// collisions.
///
/// The engine's implicit left-to-right temporal ordering ensures B's stages
/// come after A's.
pub fn sequence<L: Clone, V: Clone>(
    name: &str,
    a: &Pattern<L, V>,
    b: &Pattern<L, V>,
    shared: &[&str],
) -> Pattern<L, V> {
    let keep: HashSet<&str> = shared.iter().copied().collect();

    let a_renamed = rename_vars(a, "a", &keep);
    let b_renamed = rename_vars(b, "b", &keep);

    let mut stages = a_renamed.stages;
    stages.extend(b_renamed.stages);

    let mut temporal = a_renamed.temporal;
    temporal.extend(b_renamed.temporal);

    let mut negations = a_renamed.negations;
    negations.extend(b_renamed.negations);

    // Merge metadata: union with last-writer-wins on key conflicts.
    let mut metadata = a_renamed.metadata;
    metadata.extend(b_renamed.metadata);

    Pattern {
        name: name.to_string(),
        stages,
        temporal,
        negations,
        group: None,
        metadata,
        deadline_ticks: None,
    }
}

// ---------------------------------------------------------------------------
// choice
// ---------------------------------------------------------------------------

/// Create a set of alternative patterns. Returns one pattern per alternative.
///
/// If `exclusive` is true, all returned patterns share a mutual-exclusion
/// group — when one completes, the engine kills active PMs for the others.
///
/// Register all returned patterns with the engine:
/// ```rust,ignore
/// for p in compose::choice("crisis", &[&war, &famine], true) {
///     engine.register(p);
/// }
/// ```
pub fn choice<L: Clone, V: Clone>(
    name: &str,
    alternatives: &[&Pattern<L, V>],
    exclusive: bool,
) -> Vec<Pattern<L, V>> {
    let group = if exclusive {
        Some(name.to_string())
    } else {
        None
    };

    debug_assert!(!alternatives.is_empty(), "choice requires at least one alternative");

    alternatives
        .iter()
        .map(|pat| {
            let mut p = (*pat).clone();
            p.name = format!("{}_{}", name, p.name);
            p.group = group.clone();
            p
        })
        .collect()
}

// ---------------------------------------------------------------------------
// repeat
// ---------------------------------------------------------------------------

/// Repeat a pattern N times in sequence.
///
/// Variables in `shared` bind across all repetitions (e.g., the same
/// offender in all three strikes). Other variables are prefixed per
/// repetition (`rep0_`, `rep1_`, etc.).
pub fn repeat<L: Clone, V: Clone>(
    name: &str,
    pattern: &Pattern<L, V>,
    count: usize,
    shared: &[&str],
) -> Pattern<L, V> {
    debug_assert!(count > 0, "repeat count must be >= 1");

    let keep: HashSet<&str> = shared.iter().copied().collect();

    let mut stages = Vec::new();
    let mut temporal = Vec::new();
    let mut negations = Vec::new();

    let mut metadata = HashMap::new();
    for i in 0..count {
        let prefix = format!("rep{}", i);
        let renamed = rename_vars(pattern, &prefix, &keep);
        stages.extend(renamed.stages);
        temporal.extend(renamed.temporal);
        negations.extend(renamed.negations);
        metadata.extend(renamed.metadata);
    }

    Pattern {
        name: name.to_string(),
        stages,
        temporal,
        negations,
        group: None,
        metadata,
        deadline_ticks: None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::PatternBuilder;

    fn make_pattern(name: &str, stage_count: usize) -> Pattern<String, String> {
        let mut builder = PatternBuilder::<String, String>::new(name);
        for i in 0..stage_count {
            let anchor = format!("e{}", i);
            let evt = format!("event_{}", i);
            builder = builder.stage(&anchor, |s| {
                s.edge(&anchor, "type".to_string(), evt)
                    .edge_bind(&anchor, "actor".to_string(), "char")
            });
        }
        builder.build()
    }

    #[test]
    fn sequence_concatenates_stages() {
        let a = make_pattern("setup", 2);
        let b = make_pattern("payoff", 2);
        let composed = sequence("arc", &a, &b, &["char"]);

        assert_eq!(composed.name, "arc");
        assert_eq!(composed.stages.len(), 4);
        assert!(composed.group.is_none());
    }

    #[test]
    fn sequence_shared_vars_not_renamed() {
        let a = make_pattern("setup", 1);
        let b = make_pattern("payoff", 1);
        let composed = sequence("arc", &a, &b, &["char"]);

        // "char" should appear without prefix in both halves
        let all_var_names: Vec<String> = composed
            .stages
            .iter()
            .flat_map(|s| {
                s.clauses.iter().filter_map(|c| {
                    if let Target::Bind(ref v) = c.target {
                        Some(v.0.clone())
                    } else {
                        None
                    }
                })
            })
            .collect();
        assert!(all_var_names.iter().all(|n| n == "char"));
    }

    #[test]
    fn sequence_non_shared_vars_prefixed() {
        let a = make_pattern("setup", 1);
        let b = make_pattern("payoff", 1);
        let composed = sequence("arc", &a, &b, &["char"]);

        // Stage anchors should be prefixed: a_e0, b_e0
        assert_eq!(composed.stages[0].anchor.0, "a_e0");
        assert_eq!(composed.stages[1].anchor.0, "b_e0");
    }

    #[test]
    fn choice_returns_multiple_patterns() {
        let war = make_pattern("war", 2);
        let famine = make_pattern("famine", 2);
        let plague = make_pattern("plague", 2);

        let crises = choice("crisis", &[&war, &famine, &plague], false);
        assert_eq!(crises.len(), 3);
        assert_eq!(crises[0].name, "crisis_war");
        assert_eq!(crises[1].name, "crisis_famine");
        assert!(crises[0].group.is_none());
    }

    #[test]
    fn choice_exclusive_sets_group() {
        let war = make_pattern("war", 2);
        let famine = make_pattern("famine", 2);

        let crises = choice("crisis", &[&war, &famine], true);
        assert_eq!(crises[0].group, Some("crisis".to_string()));
        assert_eq!(crises[1].group, Some("crisis".to_string()));
    }

    #[test]
    fn repeat_multiplies_stages() {
        let offense = make_pattern("offense", 2);
        let escalation = repeat("three_strikes", &offense, 3, &["char"]);

        assert_eq!(escalation.name, "three_strikes");
        assert_eq!(escalation.stages.len(), 6); // 2 stages × 3 reps
    }

    #[test]
    fn repeat_shared_vars_consistent() {
        let offense = make_pattern("offense", 1);
        let escalation = repeat("three_strikes", &offense, 3, &["char"]);

        // "char" binding should be unprefixed in all 3 repetitions
        let bind_names: Vec<String> = escalation
            .stages
            .iter()
            .flat_map(|s| {
                s.clauses.iter().filter_map(|c| {
                    if let Target::Bind(ref v) = c.target {
                        Some(v.0.clone())
                    } else {
                        None
                    }
                })
            })
            .collect();
        assert_eq!(bind_names, vec!["char", "char", "char"]);
    }

    #[test]
    fn repeat_non_shared_vars_prefixed_per_rep() {
        let offense = make_pattern("offense", 1);
        let escalation = repeat("three_strikes", &offense, 3, &["char"]);

        let anchors: Vec<&str> = escalation.stages.iter().map(|s| s.anchor.0.as_str()).collect();
        assert_eq!(anchors, vec!["rep0_e0", "rep1_e0", "rep2_e0"]);
    }

    #[test]
    fn rename_preserves_negations() {
        let p = PatternBuilder::<String, String>::new("test")
            .stage("e1", |s| s.edge("e1", "type".into(), "a".into()))
            .stage("e2", |s| s.edge("e2", "type".into(), "b".into()))
            .unless_between("e1", "e2", |neg| {
                neg.edge("mid", "type".into(), "cancel".into())
            })
            .build();

        let keep: HashSet<&str> = HashSet::new();
        let renamed = rename_vars(&p, "x", &keep);

        assert_eq!(renamed.negations.len(), 1);
        assert_eq!(renamed.negations[0].between_start.0, "x_e1");
        assert_eq!(
            renamed.negations[0].between_end.as_ref().unwrap().0,
            "x_e2"
        );
        assert_eq!(renamed.negations[0].clauses[0].source.0, "x_mid");
    }

    #[test]
    fn sequence_merges_negations() {
        let a = PatternBuilder::<String, String>::new("a")
            .stage("e1", |s| s.edge("e1", "type".into(), "start".into()))
            .stage("e2", |s| s.edge("e2", "type".into(), "end".into()))
            .unless_between("e1", "e2", |neg| {
                neg.edge("mid", "type".into(), "cancel".into())
            })
            .build();

        let b = make_pattern("b", 1);
        let composed = sequence("arc", &a, &b, &[]);

        // A's negation should be carried over (with renamed vars)
        assert_eq!(composed.negations.len(), 1);
        assert_eq!(composed.negations[0].between_start.0, "a_e1");
    }

    #[test]
    fn sequence_merges_metadata() {
        let a = PatternBuilder::<String, String>::new("a")
            .metadata("source", "a_val")
            .metadata("shared", "from_a")
            .stage("e1", |s| s.edge("e1", "type".into(), "x".into()))
            .build();
        let b = PatternBuilder::<String, String>::new("b")
            .metadata("target", "b_val")
            .metadata("shared", "from_b")
            .stage("e2", |s| s.edge("e2", "type".into(), "y".into()))
            .build();
        let composed = sequence("arc", &a, &b, &[]);

        assert_eq!(composed.metadata.get("source").unwrap(), "a_val");
        assert_eq!(composed.metadata.get("target").unwrap(), "b_val");
        // Last-writer-wins: b overwrites a's value
        assert_eq!(composed.metadata.get("shared").unwrap(), "from_b");
    }

    #[test]
    fn choice_preserves_metadata() {
        let a = PatternBuilder::<String, String>::new("a")
            .metadata("severity", "high")
            .stage("e1", |s| s.edge("e1", "type".into(), "x".into()))
            .build();
        let b = PatternBuilder::<String, String>::new("b")
            .metadata("severity", "low")
            .stage("e2", |s| s.edge("e2", "type".into(), "y".into()))
            .build();
        let choices = choice("crisis", &[&a, &b], true);

        assert_eq!(choices[0].metadata.get("severity").unwrap(), "high");
        assert_eq!(choices[1].metadata.get("severity").unwrap(), "low");
    }

    #[test]
    fn repeat_merges_metadata() {
        let p = PatternBuilder::<String, String>::new("strike")
            .metadata("category", "offense")
            .stage("e1", |s| s.edge("e1", "type".into(), "x".into()))
            .build();
        let rep = repeat("three_strikes", &p, 3, &[]);
        assert_eq!(rep.metadata.get("category").unwrap(), "offense");
    }
}
