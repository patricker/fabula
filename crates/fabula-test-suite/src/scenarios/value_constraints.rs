//! Value constraint scenarios -- Lt, Gt, Between on numeric edges.

use crate::TestGraph;
use fabula::prelude::*;

/// Batch: Lt constraint matches when value is below threshold.
pub fn batch_value_lt_matches<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "loyalty_check", 1);
    g.add_num_edge("ev1", "loyalty", 0.3, 1);
    g.set_current_time(10);

    let pattern = PatternBuilder::new("low_loyalty")
        .stage("e", |s| {
            s.edge("e", "eventType".into(), G::str_val("loyalty_check"))
                .edge_constrained("e", "loyalty".into(), ValueConstraint::Lt(G::num_val(0.5)))
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 1, "0.3 < 0.5 should match");
}

/// Batch: Lt constraint does NOT match when value is above threshold.
pub fn batch_value_lt_no_match<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "loyalty_check", 1);
    g.add_num_edge("ev1", "loyalty", 0.8, 1);
    g.set_current_time(10);

    let pattern = PatternBuilder::new("low_loyalty")
        .stage("e", |s| {
            s.edge("e", "eventType".into(), G::str_val("loyalty_check"))
                .edge_constrained("e", "loyalty".into(), ValueConstraint::Lt(G::num_val(0.5)))
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 0, "0.8 < 0.5 should not match");
}

/// Batch: Between constraint matches when value is in range.
pub fn batch_value_between_matches<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "morale_check", 1);
    g.add_num_edge("ev1", "morale", 0.5, 1);
    g.set_current_time(10);

    let pattern = PatternBuilder::new("mid_morale")
        .stage("e", |s| {
            s.edge("e", "eventType".into(), G::str_val("morale_check"))
                .edge_constrained(
                    "e",
                    "morale".into(),
                    ValueConstraint::Between(G::num_val(0.3), G::num_val(0.7)),
                )
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        1,
        "0.5 in [0.3, 0.7] should match"
    );
}

/// Batch: Gt constraint matches when value is above threshold.
pub fn batch_value_gt_matches<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "power_check", 1);
    g.add_num_edge("ev1", "power", 0.8, 1);
    g.set_current_time(10);

    let pattern = PatternBuilder::new("high_power")
        .stage("e", |s| {
            s.edge("e", "eventType".into(), G::str_val("power_check"))
                .edge_constrained("e", "power".into(), ValueConstraint::Gt(G::num_val(0.5)))
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 1, "0.8 > 0.5 should match");

    // No match case: 0.3 > 0.5 is false
    let mut g2 = G::new_graph();
    g2.add_str_edge("ev1", "eventType", "power_check", 1);
    g2.add_num_edge("ev1", "power", 0.3, 1);
    g2.set_current_time(10);

    let pattern2 = PatternBuilder::new("high_power")
        .stage("e", |s| {
            s.edge("e", "eventType".into(), G::str_val("power_check"))
                .edge_constrained("e", "power".into(), ValueConstraint::Gt(G::num_val(0.5)))
        })
        .build();

    let mut engine2: SiftEngineFor<G> = SiftEngine::new();
    engine2.register(pattern2);
    assert_eq!(engine2.evaluate(&g2).len(), 0, "0.3 > 0.5 should not match");
}

/// Batch: Eq constraint on a string value.
pub fn batch_value_eq_string<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "tag_check", 1);
    g.add_str_edge("ev1", "priority", "important", 1);
    g.set_current_time(10);

    let pattern = PatternBuilder::new("important_check")
        .stage("e", |s| {
            s.edge("e", "eventType".into(), G::str_val("tag_check"))
                .edge_constrained(
                    "e",
                    "priority".into(),
                    ValueConstraint::Eq(G::str_val("important")),
                )
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        1,
        "priority=important should match Eq"
    );

    // No match case
    let mut g2 = G::new_graph();
    g2.add_str_edge("ev1", "eventType", "tag_check", 1);
    g2.add_str_edge("ev1", "priority", "trivial", 1);
    g2.set_current_time(10);

    let pattern2 = PatternBuilder::new("important_check")
        .stage("e", |s| {
            s.edge("e", "eventType".into(), G::str_val("tag_check"))
                .edge_constrained(
                    "e",
                    "priority".into(),
                    ValueConstraint::Eq(G::str_val("important")),
                )
        })
        .build();

    let mut engine2: SiftEngineFor<G> = SiftEngine::new();
    engine2.register(pattern2);
    assert_eq!(
        engine2.evaluate(&g2).len(),
        0,
        "priority=trivial should not match Eq(important)"
    );
}

/// Batch: value constraint in negation -- negation fires when loyalty < 0.5.
pub fn batch_value_constraint_in_negation<G: TestGraph>() {
    // Pattern: promise -> fulfill, unless low loyalty (< 0.5) event between
    let pattern = PatternBuilder::new("loyal_promise")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("promise"))
                .edge_bind("e1", "actor".into(), "person")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("fulfill"))
                .edge_bind("e2", "actor".into(), "person")
        })
        .unless_between("e1", "e2", |neg| {
            neg.edge("mid", "eventType".into(), G::str_val("loyalty_check"))
                .edge_constrained(
                    "mid",
                    "loyalty".into(),
                    ValueConstraint::Lt(G::num_val(0.5)),
                )
        })
        .build();

    // Case 1: loyalty=0.3 (< 0.5) -> negation blocks
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "promise", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev_check", "eventType", "loyalty_check", 2);
    g.add_num_edge("ev_check", "loyalty", 0.3, 2);
    g.add_str_edge("ev2", "eventType", "fulfill", 3);
    g.add_ref_edge("ev2", "actor", "alice", 3);
    g.set_current_time(10);

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "loyalty=0.3 < 0.5 should trigger negation and block"
    );

    // Case 2: loyalty=0.8 (>= 0.5) -> negation doesn't fire -> match
    let pattern2 = PatternBuilder::new("loyal_promise")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("promise"))
                .edge_bind("e1", "actor".into(), "person")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("fulfill"))
                .edge_bind("e2", "actor".into(), "person")
        })
        .unless_between("e1", "e2", |neg| {
            neg.edge("mid", "eventType".into(), G::str_val("loyalty_check"))
                .edge_constrained(
                    "mid",
                    "loyalty".into(),
                    ValueConstraint::Lt(G::num_val(0.5)),
                )
        })
        .build();

    let mut g2 = G::new_graph();
    g2.add_str_edge("ev1", "eventType", "promise", 1);
    g2.add_ref_edge("ev1", "actor", "alice", 1);
    g2.add_str_edge("ev_check", "eventType", "loyalty_check", 2);
    g2.add_num_edge("ev_check", "loyalty", 0.8, 2);
    g2.add_str_edge("ev2", "eventType", "fulfill", 3);
    g2.add_ref_edge("ev2", "actor", "alice", 3);
    g2.set_current_time(10);

    let mut engine2: SiftEngineFor<G> = SiftEngine::new();
    engine2.register(pattern2);
    assert_eq!(
        engine2.evaluate(&g2).len(),
        1,
        "loyalty=0.8 >= 0.5 should not trigger negation -> match"
    );
}

/// Batch: Between constraint does NOT match when value is outside range.
pub fn batch_value_between_no_match<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "morale_check", 1);
    g.add_num_edge("ev1", "morale", 0.9, 1);
    g.set_current_time(10);

    let pattern = PatternBuilder::new("mid_morale")
        .stage("e", |s| {
            s.edge("e", "eventType".into(), G::str_val("morale_check"))
                .edge_constrained(
                    "e",
                    "morale".into(),
                    ValueConstraint::Between(G::num_val(0.3), G::num_val(0.7)),
                )
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 0, "0.9 not in [0.3, 0.7]");
}
