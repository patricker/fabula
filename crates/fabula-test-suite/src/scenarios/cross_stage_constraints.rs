//! Cross-stage value comparison scenarios — GtVar, LtVar, EqVar.

use crate::TestGraph;
use fabula::prelude::*;

/// Batch: GtVar matches when second value > first value.
pub fn batch_cross_stage_gt_matches<G: TestGraph>() {
    let mut g = G::new_graph();
    // Stage 1: order with price 100
    g.add_str_edge("ev1", "type", "order", 1);
    g.add_num_edge("ev1", "price", 100.0, 1);
    // Stage 2: order with price 150 (> 100)
    g.add_str_edge("ev2", "type", "order", 2);
    g.add_num_edge("ev2", "price", 150.0, 2);
    g.set_current_time(10);

    let pattern = PatternBuilder::new("escalating_price")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), G::str_val("order")).edge_bind(
                "e1",
                "price".into(),
                "base_price",
            )
        })
        .stage("e2", |s| {
            s.edge("e2", "type".into(), G::str_val("order"))
                .edge_gt_var("e2", "price".into(), "base_price")
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 1, "150 > 100 should match");
}

/// Batch: GtVar does NOT match when second value <= first value.
pub fn batch_cross_stage_gt_no_match<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "type", "order", 1);
    g.add_num_edge("ev1", "price", 100.0, 1);
    g.add_str_edge("ev2", "type", "order", 2);
    g.add_num_edge("ev2", "price", 80.0, 2);
    g.set_current_time(10);

    let pattern = PatternBuilder::new("escalating_price")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), G::str_val("order")).edge_bind(
                "e1",
                "price".into(),
                "base_price",
            )
        })
        .stage("e2", |s| {
            s.edge("e2", "type".into(), G::str_val("order"))
                .edge_gt_var("e2", "price".into(), "base_price")
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 0, "80 > 100 should not match");
}

/// Batch: LtVar matches when second value < first value (deterioration).
pub fn batch_cross_stage_lt_matches<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "type", "lab_result", 1);
    g.add_num_edge("ev1", "value", 7.5, 1);
    g.add_str_edge("ev2", "type", "lab_result", 2);
    g.add_num_edge("ev2", "value", 5.0, 2);
    g.set_current_time(10);

    let pattern = PatternBuilder::new("deterioration")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), G::str_val("lab_result"))
                .edge_bind("e1", "value".into(), "baseline")
        })
        .stage("e2", |s| {
            s.edge("e2", "type".into(), G::str_val("lab_result"))
                .edge_lt_var("e2", "value".into(), "baseline")
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 1, "5.0 < 7.5 should match");
}

/// Batch: EqVar matches when second value equals first value.
pub fn batch_cross_stage_eq_matches<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "type", "invoice", 1);
    g.add_num_edge("ev1", "amount", 500.0, 1);
    g.add_str_edge("ev2", "type", "payment", 2);
    g.add_num_edge("ev2", "amount", 500.0, 2);
    g.set_current_time(10);

    let pattern = PatternBuilder::new("exact_payment")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), G::str_val("invoice"))
                .edge_bind("e1", "amount".into(), "expected")
        })
        .stage("e2", |s| {
            s.edge("e2", "type".into(), G::str_val("payment"))
                .edge_eq_var("e2", "amount".into(), "expected")
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 1, "500 == 500 should match");
}

/// Incremental: GtVar works with on_edge_added.
pub fn incremental_cross_stage_gt<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngineFor<G> = SiftEngine::new();

    engine.register(
        PatternBuilder::new("escalation")
            .stage("e1", |s| {
                s.edge("e1", "type".into(), G::str_val("bid")).edge_bind(
                    "e1",
                    "price".into(),
                    "prev_price",
                )
            })
            .stage("e2", |s| {
                s.edge("e2", "type".into(), G::str_val("bid")).edge_gt_var(
                    "e2",
                    "price".into(),
                    "prev_price",
                )
            })
            .build(),
    );

    // Tick 1: bid at 100
    g.add_str_edge("ev1", "type", "bid", 1);
    g.add_num_edge("ev1", "price", 100.0, 1);
    g.set_current_time(1);
    let events = engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &G::str_val("bid"),
        &Interval::open(1),
    );
    let advanced = events
        .iter()
        .filter(|e| matches!(e, SiftEvent::Advanced { .. }))
        .count();
    assert!(advanced > 0, "first bid should initiate a PM");

    // Tick 2: bid at 150 — should complete
    g.add_str_edge("ev2", "type", "bid", 2);
    g.add_num_edge("ev2", "price", 150.0, 2);
    g.set_current_time(2);
    let events = engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"type".into(),
        &G::str_val("bid"),
        &Interval::open(2),
    );
    let completed = events
        .iter()
        .filter(|e| matches!(e, SiftEvent::Completed { .. }))
        .count();
    assert_eq!(completed, 1, "150 > 100 should complete the pattern");
}
