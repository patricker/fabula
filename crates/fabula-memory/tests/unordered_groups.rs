//! Integration tests for unordered (concurrent) stage groups.

use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

type Engine = SiftEngine<String, String, MemValue, i64>;

#[test]
fn batch_unordered_any_order() {
    let pattern = PatternBuilder::<String, MemValue>::new("concurrent")
        .unordered_group(|g| {
            g.stage("a", |s| {
                s.edge("a", "type".into(), MemValue::Str("alpha".into()))
            })
            .stage("b", |s| {
                s.edge("b", "type".into(), MemValue::Str("beta".into()))
            })
        })
        .build();

    // alpha then beta
    let mut g = MemGraph::new();
    g.add_str("ev1", "type", "alpha", 1);
    g.add_str("ev2", "type", "beta", 2);
    g.set_time(10);

    let mut engine = Engine::new();
    engine.register(pattern.clone());
    assert_eq!(engine.evaluate(&g).len(), 1);

    // beta then alpha (reversed order, still matches)
    let mut g2 = MemGraph::new();
    g2.add_str("ev1", "type", "beta", 1);
    g2.add_str("ev2", "type", "alpha", 2);
    g2.set_time(10);

    let mut engine2 = Engine::new();
    engine2.register(pattern);
    assert_eq!(engine2.evaluate(&g2).len(), 1);
}

#[test]
fn batch_unordered_with_preceding_stage() {
    let pattern = PatternBuilder::<String, MemValue>::new("setup_then_concurrent")
        .stage("setup", |s| {
            s.edge("setup", "type".into(), MemValue::Str("start".into()))
        })
        .unordered_group(|g| {
            g.stage("a", |s| {
                s.edge("a", "type".into(), MemValue::Str("alpha".into()))
            })
            .stage("b", |s| {
                s.edge("b", "type".into(), MemValue::Str("beta".into()))
            })
        })
        .build();

    let mut g = MemGraph::new();
    g.add_str("ev0", "type", "start", 1);
    g.add_str("ev1", "type", "beta", 2);
    g.add_str("ev2", "type", "alpha", 3);
    g.set_time(10);

    let mut engine = Engine::new();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 1);
}

#[test]
fn batch_unordered_with_following_stage() {
    let pattern = PatternBuilder::<String, MemValue>::new("concurrent_then_end")
        .unordered_group(|g| {
            g.stage("a", |s| {
                s.edge("a", "type".into(), MemValue::Str("alpha".into()))
            })
            .stage("b", |s| {
                s.edge("b", "type".into(), MemValue::Str("beta".into()))
            })
        })
        .stage("end", |s| {
            s.edge("end", "type".into(), MemValue::Str("finish".into()))
        })
        .build();

    let mut g = MemGraph::new();
    g.add_str("ev1", "type", "beta", 1);
    g.add_str("ev2", "type", "alpha", 2);
    g.add_str("ev3", "type", "finish", 3);
    g.set_time(10);

    let mut engine = Engine::new();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 1);
}

#[test]
fn batch_unordered_end_before_group_no_match() {
    let pattern = PatternBuilder::<String, MemValue>::new("concurrent_then_end")
        .unordered_group(|g| {
            g.stage("a", |s| {
                s.edge("a", "type".into(), MemValue::Str("alpha".into()))
            })
            .stage("b", |s| {
                s.edge("b", "type".into(), MemValue::Str("beta".into()))
            })
        })
        .stage("end", |s| {
            s.edge("end", "type".into(), MemValue::Str("finish".into()))
        })
        .build();

    // finish at t=2, beta at t=3 -- finish before group complete
    let mut g = MemGraph::new();
    g.add_str("ev1", "type", "alpha", 1);
    g.add_str("ev2", "type", "finish", 2);
    g.add_str("ev3", "type", "beta", 3);
    g.set_time(10);

    let mut engine = Engine::new();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 0);
}

#[test]
fn incremental_unordered_group() {
    let pattern = PatternBuilder::<String, MemValue>::new("concurrent")
        .unordered_group(|g| {
            g.stage("a", |s| {
                s.edge("a", "type".into(), MemValue::Str("alpha".into()))
            })
            .stage("b", |s| {
                s.edge("b", "type".into(), MemValue::Str("beta".into()))
            })
        })
        .build();

    let mut g = MemGraph::new();
    let mut engine = Engine::new();
    engine.register(pattern);

    // Add beta first
    g.add_str("ev1", "type", "beta", 1);
    g.set_time(1);
    let iv1 = Interval::open(1);
    let events1 = engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("beta".into()),
        &iv1,
    );
    assert!(events1
        .iter()
        .any(|e| matches!(e, SiftEvent::Advanced { .. })));

    // Add alpha
    g.add_str("ev2", "type", "alpha", 2);
    g.set_time(2);
    let iv2 = Interval::open(2);
    let events2 = engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"type".into(),
        &MemValue::Str("alpha".into()),
        &iv2,
    );
    assert!(events2
        .iter()
        .any(|e| matches!(e, SiftEvent::Completed { .. })));
}

#[test]
fn batch_unordered_shared_binding() {
    let pattern = PatternBuilder::<String, MemValue>::new("shared")
        .unordered_group(|g| {
            g.stage("a", |s| {
                s.edge("a", "type".into(), MemValue::Str("alpha".into()))
                    .edge_bind("a", "actor".into(), "person")
            })
            .stage("b", |s| {
                s.edge("b", "type".into(), MemValue::Str("beta".into()))
                    .edge_bind("b", "actor".into(), "person")
            })
        })
        .build();

    // Same actor
    let mut g = MemGraph::new();
    g.add_str("ev1", "type", "alpha", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.add_str("ev2", "type", "beta", 2);
    g.add_ref("ev2", "actor", "alice", 2);
    g.set_time(10);

    let mut engine = Engine::new();
    engine.register(pattern.clone());
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].bindings["person"],
        BoundValue::Node("alice".into())
    );

    // Different actors -> no match
    let mut g2 = MemGraph::new();
    g2.add_str("ev1", "type", "alpha", 1);
    g2.add_ref("ev1", "actor", "alice", 1);
    g2.add_str("ev2", "type", "beta", 2);
    g2.add_ref("ev2", "actor", "bob", 2);
    g2.set_time(10);

    let mut engine2 = Engine::new();
    engine2.register(pattern);
    assert_eq!(engine2.evaluate(&g2).len(), 0);
}

#[test]
fn incremental_three_stage_middle_unordered() {
    let pattern = PatternBuilder::<String, MemValue>::new("sandwich")
        .stage("start", |s| {
            s.edge("start", "type".into(), MemValue::Str("begin".into()))
        })
        .unordered_group(|g| {
            g.stage("a", |s| {
                s.edge("a", "type".into(), MemValue::Str("alpha".into()))
            })
            .stage("b", |s| {
                s.edge("b", "type".into(), MemValue::Str("beta".into()))
            })
        })
        .stage("end", |s| {
            s.edge("end", "type".into(), MemValue::Str("finish".into()))
        })
        .build();

    let mut g = MemGraph::new();
    let mut engine = Engine::new();
    engine.register(pattern);

    // Stage 1: start
    g.add_str("ev0", "type", "begin", 1);
    g.set_time(1);
    let iv0 = Interval::open(1);
    let events0 = engine.on_edge_added(
        &g,
        &"ev0".into(),
        &"type".into(),
        &MemValue::Str("begin".into()),
        &iv0,
    );
    assert!(events0
        .iter()
        .any(|e| matches!(e, SiftEvent::Advanced { .. })));

    // Stage 2b: beta (in the unordered group, arriving before alpha)
    g.add_str("ev1", "type", "beta", 2);
    g.set_time(2);
    let iv1 = Interval::open(2);
    let events1 = engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("beta".into()),
        &iv1,
    );
    assert!(events1
        .iter()
        .any(|e| matches!(e, SiftEvent::Advanced { .. })));

    // Stage 2a: alpha (completing the unordered group)
    g.add_str("ev2", "type", "alpha", 3);
    g.set_time(3);
    let iv2 = Interval::open(3);
    let events2 = engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"type".into(),
        &MemValue::Str("alpha".into()),
        &iv2,
    );
    assert!(events2
        .iter()
        .any(|e| matches!(e, SiftEvent::Advanced { .. })));

    // Stage 3: finish
    g.add_str("ev3", "type", "finish", 4);
    g.set_time(4);
    let iv3 = Interval::open(4);
    let events3 = engine.on_edge_added(
        &g,
        &"ev3".into(),
        &"type".into(),
        &MemValue::Str("finish".into()),
        &iv3,
    );
    assert!(
        events3
            .iter()
            .any(|e| matches!(e, SiftEvent::Completed { .. })),
        "should complete after all 4 stages matched"
    );
}
