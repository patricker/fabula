//! Golden test scenarios for unordered (concurrent) stage groups.

use crate::TestGraph;
use fabula::builder::PatternBuilder;
use fabula::engine::{BoundValue, SiftEngine};

/// Batch: Two stages in an unordered group match regardless of event order.
pub fn batch_unordered_group_any_order<G: TestGraph>() {
    let pattern = PatternBuilder::<String, G::V>::new("concurrent_events")
        .unordered_group(|g| {
            g.stage("a", |s| s.edge("a", "type".into(), G::str_val("alpha")))
                .stage("b", |s| s.edge("b", "type".into(), G::str_val("beta")))
        })
        .build();

    // Order 1: alpha before beta
    let mut g1 = G::new_graph();
    g1.add_str_edge("ev1", "type", "alpha", 1);
    g1.add_str_edge("ev2", "type", "beta", 2);
    g1.set_current_time(10);

    let mut engine: SiftEngine<String, String, G::V, i64> = SiftEngine::new();
    engine.register(pattern.clone());
    let matches = engine.evaluate(&g1);
    assert_eq!(matches.len(), 1, "should match with alpha before beta");

    // Order 2: beta before alpha
    let mut g2 = G::new_graph();
    g2.add_str_edge("ev1", "type", "beta", 1);
    g2.add_str_edge("ev2", "type", "alpha", 2);
    g2.set_current_time(10);

    let mut engine2: SiftEngine<String, String, G::V, i64> = SiftEngine::new();
    engine2.register(pattern);
    let matches2 = engine2.evaluate(&g2);
    assert_eq!(matches2.len(), 1, "should match with beta before alpha");
}

/// Batch: Unordered group with a preceding ordered stage.
pub fn batch_unordered_group_after_ordered<G: TestGraph>() {
    let pattern = PatternBuilder::<String, G::V>::new("setup_then_concurrent")
        .stage("setup", |s| s.edge("setup", "type".into(), G::str_val("start")))
        .unordered_group(|g| {
            g.stage("a", |s| s.edge("a", "type".into(), G::str_val("alpha")))
                .stage("b", |s| s.edge("b", "type".into(), G::str_val("beta")))
        })
        .build();

    let mut g = G::new_graph();
    g.add_str_edge("ev0", "type", "start", 1);
    g.add_str_edge("ev1", "type", "beta", 2); // beta first
    g.add_str_edge("ev2", "type", "alpha", 3); // alpha second
    g.set_current_time(10);

    let mut engine: SiftEngine<String, String, G::V, i64> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1);
}

/// Batch: Unordered group followed by an ordered stage.
pub fn batch_unordered_group_before_ordered<G: TestGraph>() {
    let pattern = PatternBuilder::<String, G::V>::new("concurrent_then_end")
        .unordered_group(|g| {
            g.stage("a", |s| s.edge("a", "type".into(), G::str_val("alpha")))
                .stage("b", |s| s.edge("b", "type".into(), G::str_val("beta")))
        })
        .stage("end", |s| s.edge("end", "type".into(), G::str_val("finish")))
        .build();

    let mut g = G::new_graph();
    g.add_str_edge("ev1", "type", "beta", 1);
    g.add_str_edge("ev2", "type", "alpha", 2);
    g.add_str_edge("ev3", "type", "finish", 3);
    g.set_current_time(10);

    let mut engine: SiftEngine<String, String, G::V, i64> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1);
}

/// Batch: finish event before concurrent group should not match.
pub fn batch_unordered_group_ordering_with_ordered<G: TestGraph>() {
    let pattern = PatternBuilder::<String, G::V>::new("concurrent_then_end")
        .unordered_group(|g| {
            g.stage("a", |s| s.edge("a", "type".into(), G::str_val("alpha")))
                .stage("b", |s| s.edge("b", "type".into(), G::str_val("beta")))
        })
        .stage("end", |s| s.edge("end", "type".into(), G::str_val("finish")))
        .build();

    // Finish happens before one of the concurrent events -> should not match
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "type", "alpha", 1);
    g.add_str_edge("ev2", "type", "finish", 2);
    g.add_str_edge("ev3", "type", "beta", 3);
    g.set_current_time(10);

    let mut engine: SiftEngine<String, String, G::V, i64> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    // This should not match because "finish" at t=2 comes before "beta" at t=3,
    // but "finish" must come after the entire concurrent group
    assert_eq!(matches.len(), 0, "finish before group completion should not match");
}

/// Incremental: unordered group matches with events arriving in any order.
pub fn incremental_unordered_group<G: TestGraph>() {
    let pattern = PatternBuilder::<String, G::V>::new("concurrent_events")
        .unordered_group(|g| {
            g.stage("a", |s| s.edge("a", "type".into(), G::str_val("alpha")))
                .stage("b", |s| s.edge("b", "type".into(), G::str_val("beta")))
        })
        .build();

    let mut g = G::new_graph();
    let mut engine: SiftEngine<String, String, G::V, i64> = SiftEngine::new();
    engine.register(pattern);

    // Add beta first
    g.add_str_edge("ev1", "type", "beta", 1);
    g.set_current_time(1);
    let iv1 = fabula::interval::Interval::open(1);
    let events1 = engine.on_edge_added(&g, &"ev1".into(), &"type".into(), &G::str_val("beta"), &iv1);
    // Should advance (one stage of two matched)
    assert!(
        events1.iter().any(|e| matches!(e, fabula::engine::SiftEvent::Advanced { .. })),
        "should advance after first stage match"
    );

    // Add alpha
    g.add_str_edge("ev2", "type", "alpha", 2);
    g.set_current_time(2);
    let iv2 = fabula::interval::Interval::open(2);
    let events2 = engine.on_edge_added(&g, &"ev2".into(), &"type".into(), &G::str_val("alpha"), &iv2);
    // Should complete
    assert!(
        events2.iter().any(|e| matches!(e, fabula::engine::SiftEvent::Completed { .. })),
        "should complete after both stages matched"
    );
}

/// Batch: shared binding across unordered group stages.
pub fn batch_unordered_group_shared_binding<G: TestGraph>() {
    let pattern = PatternBuilder::<String, G::V>::new("shared_actor")
        .unordered_group(|g| {
            g.stage("a", |s| {
                s.edge("a", "type".into(), G::str_val("alpha"))
                    .edge_bind("a", "actor".into(), "person")
            })
            .stage("b", |s| {
                s.edge("b", "type".into(), G::str_val("beta"))
                    .edge_bind("b", "actor".into(), "person")
            })
        })
        .build();

    // Same actor for both events
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "type", "alpha", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev2", "type", "beta", 2);
    g.add_ref_edge("ev2", "actor", "alice", 2);
    g.set_current_time(10);

    let mut engine: SiftEngine<String, String, G::V, i64> = SiftEngine::new();
    engine.register(pattern.clone());
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].bindings["person"], BoundValue::Node("alice".into()));

    // Different actors — should not match
    let mut g2 = G::new_graph();
    g2.add_str_edge("ev1", "type", "alpha", 1);
    g2.add_ref_edge("ev1", "actor", "alice", 1);
    g2.add_str_edge("ev2", "type", "beta", 2);
    g2.add_ref_edge("ev2", "actor", "bob", 2);
    g2.set_current_time(10);

    let mut engine2: SiftEngine<String, String, G::V, i64> = SiftEngine::new();
    engine2.register(pattern);
    let matches2 = engine2.evaluate(&g2);
    assert_eq!(matches2.len(), 0, "different actors should not match");
}
