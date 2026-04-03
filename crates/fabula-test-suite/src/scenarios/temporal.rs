//! Temporal ordering scenarios — verifying that stage order is enforced.

use crate::TestGraph;
use fabula::prelude::*;

/// Batch: events in wrong temporal order should NOT match a two-stage pattern.
pub fn batch_rejects_wrong_temporal_order<G: TestGraph>() {
    let mut g = G::new_graph();
    // "enter" at time 5, "leave" at time 1 — reverse order
    g.add_str_edge("ev1", "eventType", "enter", 5);
    g.add_ref_edge("ev1", "actor", "alice", 5);
    g.add_str_edge("ev2", "eventType", "leave", 1);
    g.add_ref_edge("ev2", "actor", "alice", 1);
    g.set_current_time(10);

    let pattern = PatternBuilder::new("enter_then_leave")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("enter"))
                .edge_bind("e1", "actor".into(), "person")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("leave"))
                .edge_bind("e2", "actor".into(), "person")
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "temporal order violated — should not match"
    );
}

/// Batch: two events at the same timestamp should NOT match a 2-stage pattern (strict < ordering).
pub fn temporal_same_timestamp_no_sequence<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "enter", 5);
    g.add_ref_edge("ev1", "actor", "alice", 5);
    g.add_str_edge("ev2", "eventType", "leave", 5);
    g.add_ref_edge("ev2", "actor", "alice", 5);
    g.set_current_time(10);

    let pattern = PatternBuilder::new("enter_then_leave")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("enter"))
                .edge_bind("e1", "actor".into(), "person")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("leave"))
                .edge_bind("e2", "actor".into(), "person")
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "same timestamp should not satisfy strict < ordering"
    );
}

/// Batch: events separated by a large gap still match.
pub fn temporal_large_gap_still_matches<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "enter", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev2", "eventType", "leave", 1_000_000);
    g.add_ref_edge("ev2", "actor", "alice", 1_000_000);
    g.set_current_time(2_000_000);

    let pattern = PatternBuilder::new("enter_then_leave")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("enter"))
                .edge_bind("e1", "actor".into(), "person")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("leave"))
                .edge_bind("e2", "actor".into(), "person")
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        1,
        "large temporal gap should still match"
    );
}

/// Batch: events in correct temporal order should match.
pub fn batch_correct_temporal_order<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "enter", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev2", "eventType", "leave", 5);
    g.add_ref_edge("ev2", "actor", "alice", 5);
    g.set_current_time(10);

    let pattern = PatternBuilder::new("enter_then_leave")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("enter"))
                .edge_bind("e1", "actor".into(), "person")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("leave"))
                .edge_bind("e2", "actor".into(), "person")
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        1,
        "correct temporal order — should match"
    );
}
