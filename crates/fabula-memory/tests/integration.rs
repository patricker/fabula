//! Integration tests — porting Winnow's test scenarios + additional coverage.

use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

// ---------------------------------------------------------------------------
// Pattern helpers
// ---------------------------------------------------------------------------

/// Winnow test: violationOfHospitality
/// Guest enters → host shows hospitality → host harms guest.
/// Unless guest leaves between entry and harm.
fn violation_of_hospitality() -> Pattern<String, MemValue> {
    PatternBuilder::new("violation_of_hospitality")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("enterTown".into()))
                .edge_bind("e1", "actor".into(), "guest")
        })
        .stage("e2", |s| {
            s.edge(
                "e2",
                "eventType".into(),
                MemValue::Str("showHospitality".into()),
            )
            .edge_bind("e2", "actor".into(), "host")
            .edge_bind("e2", "target".into(), "guest")
        })
        .stage("e3", |s| {
            s.edge("e3", "eventType".into(), MemValue::Str("harm".into()))
                .edge_bind("e3", "actor".into(), "host")
                .edge_bind("e3", "target".into(), "guest")
        })
        .unless_between("e1", "e3", |neg| {
            neg.edge(
                "eMid",
                "eventType".into(),
                MemValue::Str("leaveTown".into()),
            )
            .edge_bind("eMid", "actor".into(), "guest")
        })
        .build()
}

/// Winnow test: romanticFailureThenSuccess
/// Character has two negative romantic events then one positive.
fn romantic_arc() -> Pattern<String, MemValue> {
    PatternBuilder::new("romantic_arc")
        .stage("e1", |s| {
            s.edge("e1", "tag".into(), MemValue::Str("negative".into()))
                .edge("e1", "tag".into(), MemValue::Str("romantic".into()))
                .edge_bind("e1", "actor".into(), "char")
        })
        .stage("e2", |s| {
            s.edge("e2", "tag".into(), MemValue::Str("negative".into()))
                .edge("e2", "tag".into(), MemValue::Str("romantic".into()))
                .edge_bind("e2", "actor".into(), "char")
        })
        .stage("e3", |s| {
            s.edge("e3", "tag".into(), MemValue::Str("positive".into()))
                .edge("e3", "tag".into(), MemValue::Str("romantic".into()))
                .edge_bind("e3", "actor".into(), "char")
        })
        .build()
}

fn hospitality_graph() -> MemGraph {
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "enterTown", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.add_str("ev2", "eventType", "showHospitality", 2);
    g.add_ref("ev2", "actor", "bob", 2);
    g.add_ref("ev2", "target", "alice", 2);
    g.add_str("ev3", "eventType", "harm", 3);
    g.add_ref("ev3", "actor", "bob", 3);
    g.add_ref("ev3", "target", "alice", 3);
    g.set_time(10);
    g
}

// ---------------------------------------------------------------------------
// Batch: violation of hospitality
// ---------------------------------------------------------------------------

#[test]
fn batch_hospitality_matches() {
    let g = hospitality_graph();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(violation_of_hospitality());
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].pattern, "violation_of_hospitality");
    match &matches[0].bindings["guest"] {
        BoundValue::Node(n) => assert_eq!(n, "alice"),
        other => panic!("expected guest=alice, got {:?}", other),
    }
    match &matches[0].bindings["host"] {
        BoundValue::Node(n) => assert_eq!(n, "bob"),
        other => panic!("expected host=bob, got {:?}", other),
    }
}

#[test]
fn batch_hospitality_negated_when_guest_leaves() {
    let mut g = hospitality_graph();
    g.add_str("ev_leave", "eventType", "leaveTown", 2);
    g.add_ref("ev_leave", "actor", "alice", 2);
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(violation_of_hospitality());
    assert_eq!(engine.evaluate(&g).len(), 0);
}

#[test]
fn batch_hospitality_unrelated_leave_doesnt_negate() {
    let mut g = hospitality_graph();
    g.add_str("ev_leave", "eventType", "leaveTown", 2);
    g.add_ref("ev_leave", "actor", "charlie", 2);
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(violation_of_hospitality());
    assert_eq!(engine.evaluate(&g).len(), 1);
}

// ---------------------------------------------------------------------------
// Incremental: violation of hospitality
// ---------------------------------------------------------------------------

#[test]
fn incremental_hospitality_three_stages() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(violation_of_hospitality());

    // Stage 1
    g.add_str("ev1", "eventType", "enterTown", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.set_time(1);
    let ev = engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("enterTown".into()),
        &Interval::open(1),
    );
    assert!(ev.iter().any(|e| matches!(e, SiftEvent::Advanced { pattern, .. } if pattern == "violation_of_hospitality")));

    // Stage 2
    g.add_str("ev2", "eventType", "showHospitality", 2);
    g.add_ref("ev2", "actor", "bob", 2);
    g.add_ref("ev2", "target", "alice", 2);
    g.set_time(2);
    let ev = engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &MemValue::Str("showHospitality".into()),
        &Interval::open(2),
    );
    assert!(ev.iter().any(|e| matches!(e, SiftEvent::Advanced { pattern, .. } if pattern == "violation_of_hospitality")));

    // Stage 3 → complete
    g.add_str("ev3", "eventType", "harm", 3);
    g.add_ref("ev3", "actor", "bob", 3);
    g.add_ref("ev3", "target", "alice", 3);
    g.set_time(3);
    let ev = engine.on_edge_added(
        &g,
        &"ev3".into(),
        &"eventType".into(),
        &MemValue::Str("harm".into()),
        &Interval::open(3),
    );
    assert!(ev.iter().any(|e| matches!(e, SiftEvent::Completed { pattern, .. } if pattern == "violation_of_hospitality")));
}

#[test]
fn incremental_hospitality_negation_kills() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(violation_of_hospitality());

    // Stages 1 + 2
    g.add_str("ev1", "eventType", "enterTown", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("enterTown".into()),
        &Interval::open(1),
    );
    g.add_str("ev2", "eventType", "showHospitality", 2);
    g.add_ref("ev2", "actor", "bob", 2);
    g.add_ref("ev2", "target", "alice", 2);
    g.set_time(2);
    engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &MemValue::Str("showHospitality".into()),
        &Interval::open(2),
    );
    assert!(!engine
        .active_matches_for("violation_of_hospitality")
        .is_empty());

    // Guest leaves → kill
    g.add_str("ev_leave", "eventType", "leaveTown", 3);
    g.add_ref("ev_leave", "actor", "alice", 3);
    g.set_time(3);
    let ev = engine.on_edge_added(
        &g,
        &"ev_leave".into(),
        &"eventType".into(),
        &MemValue::Str("leaveTown".into()),
        &Interval::open(3),
    );
    assert!(ev.iter().any(|e| matches!(e, SiftEvent::Negated { .. })));
    assert!(engine
        .active_matches_for("violation_of_hospitality")
        .is_empty());
}

#[test]
fn incremental_hospitality_unrelated_leave_doesnt_kill() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(violation_of_hospitality());

    // Stages 1 + 2
    g.add_str("ev1", "eventType", "enterTown", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("enterTown".into()),
        &Interval::open(1),
    );
    g.add_str("ev2", "eventType", "showHospitality", 2);
    g.add_ref("ev2", "actor", "bob", 2);
    g.add_ref("ev2", "target", "alice", 2);
    g.set_time(2);
    engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &MemValue::Str("showHospitality".into()),
        &Interval::open(2),
    );

    // Charlie leaves — should NOT kill
    g.add_str("ev_leave", "eventType", "leaveTown", 3);
    g.add_ref("ev_leave", "actor", "charlie", 3);
    g.set_time(3);
    let ev = engine.on_edge_added(
        &g,
        &"ev_leave".into(),
        &"eventType".into(),
        &MemValue::Str("leaveTown".into()),
        &Interval::open(3),
    );
    assert!(!ev.iter().any(|e| matches!(e, SiftEvent::Negated { .. })));
    assert!(!engine
        .active_matches_for("violation_of_hospitality")
        .is_empty());
}

// ---------------------------------------------------------------------------
// Romantic arc (3-stage tag-based, from Winnow tests)
// ---------------------------------------------------------------------------

#[test]
fn batch_romantic_arc() {
    let mut g = MemGraph::new();
    g.add_str("r1", "tag", "negative", 1);
    g.add_str("r1", "tag", "romantic", 1);
    g.add_ref("r1", "actor", "mira", 1);
    g.add_str("r2", "tag", "negative", 2);
    g.add_str("r2", "tag", "romantic", 2);
    g.add_ref("r2", "actor", "mira", 2);
    g.add_str("r3", "tag", "positive", 3);
    g.add_str("r3", "tag", "romantic", 3);
    g.add_ref("r3", "actor", "mira", 3);
    g.set_time(10);

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(romantic_arc());
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1);
    match &matches[0].bindings["char"] {
        BoundValue::Node(n) => assert_eq!(n, "mira"),
        other => panic!("expected char=mira, got {:?}", other),
    }
}

#[test]
fn batch_romantic_arc_different_characters_no_match() {
    let mut g = MemGraph::new();
    g.add_str("r1", "tag", "negative", 1);
    g.add_str("r1", "tag", "romantic", 1);
    g.add_ref("r1", "actor", "mira", 1);
    g.add_str("r2", "tag", "negative", 2);
    g.add_str("r2", "tag", "romantic", 2);
    g.add_ref("r2", "actor", "kaelen", 2); // different character
    g.add_str("r3", "tag", "positive", 3);
    g.add_str("r3", "tag", "romantic", 3);
    g.add_ref("r3", "actor", "mira", 3);
    g.set_time(10);

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(romantic_arc());
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "different actors should not match"
    );
}

// ---------------------------------------------------------------------------
// Value constraints (Lt, Gt, Between)
// ---------------------------------------------------------------------------

#[test]
fn batch_value_constraint_lt() {
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "loyalty_check", 1);
    g.add_num("ev1", "loyalty", 0.3, 1);
    g.set_time(10);

    let pattern = PatternBuilder::new("low_loyalty")
        .stage("e", |s| {
            s.edge(
                "e",
                "eventType".into(),
                MemValue::Str("loyalty_check".into()),
            )
            .edge_constrained(
                "e",
                "loyalty".into(),
                ValueConstraint::Lt(MemValue::Num(0.5)),
            )
        })
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 1);
}

#[test]
fn batch_value_constraint_lt_no_match() {
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "loyalty_check", 1);
    g.add_num("ev1", "loyalty", 0.8, 1);
    g.set_time(10);

    let pattern = PatternBuilder::new("low_loyalty")
        .stage("e", |s| {
            s.edge(
                "e",
                "eventType".into(),
                MemValue::Str("loyalty_check".into()),
            )
            .edge_constrained(
                "e",
                "loyalty".into(),
                ValueConstraint::Lt(MemValue::Num(0.5)),
            )
        })
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 0);
}

// ---------------------------------------------------------------------------
// Single-stage pattern (immediate completion)
// ---------------------------------------------------------------------------

#[test]
fn incremental_single_stage_completes_immediately() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    let pattern = PatternBuilder::new("find_harm")
        .stage("e", |s| {
            s.edge("e", "eventType".into(), MemValue::Str("harm".into()))
                .edge_bind("e", "actor".into(), "attacker")
        })
        .build();
    engine.register(pattern);

    g.add_str("ev1", "eventType", "harm", 1);
    g.add_ref("ev1", "actor", "bob", 1);
    g.set_time(1);
    let ev = engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("harm".into()),
        &Interval::open(1),
    );
    assert!(ev.iter().any(|e| matches!(e, SiftEvent::Completed { .. })));
}

// ---------------------------------------------------------------------------
// Open-ended negation (unless_after)
// ---------------------------------------------------------------------------

#[test]
fn batch_unless_after_blocks_match() {
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "promise", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.add_str("ev2", "eventType", "break_promise", 2);
    g.add_ref("ev2", "actor", "alice", 2);
    g.set_time(10);

    // "promise made, then broken, unless apologized after the promise"
    let pattern = PatternBuilder::new("broken_promise")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("promise".into()))
                .edge_bind("e1", "actor".into(), "person")
        })
        .stage("e2", |s| {
            s.edge(
                "e2",
                "eventType".into(),
                MemValue::Str("break_promise".into()),
            )
            .edge_bind("e2", "actor".into(), "person")
        })
        .unless_after("e1", |neg| {
            neg.edge(
                "apology",
                "eventType".into(),
                MemValue::Str("apologize".into()),
            )
            .edge_bind("apology", "actor".into(), "person")
        })
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);

    // No apology → should match
    assert_eq!(engine.evaluate(&g).len(), 1);

    // Add apology after promise → should not match
    g.add_str("ev_apology", "eventType", "apologize", 3);
    g.add_ref("ev_apology", "actor", "alice", 3);
    assert_eq!(engine.evaluate(&g).len(), 0);
}

// ---------------------------------------------------------------------------
// Global negation (unless_global — Winnow default)
// ---------------------------------------------------------------------------

#[test]
fn batch_unless_global() {
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "betray", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.add_str("ev2", "eventType", "betray", 3);
    g.add_ref("ev2", "actor", "alice", 3);
    g.set_time(10);

    // Two betrayals by the same character with no reconciliation anywhere in the pattern span
    let pattern = PatternBuilder::new("double_betrayal")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("betray".into()))
                .edge_bind("e1", "actor".into(), "char")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), MemValue::Str("betray".into()))
                .edge_bind("e2", "actor".into(), "char")
        })
        .unless_global(|neg| {
            neg.edge("mid", "eventType".into(), MemValue::Str("reconcile".into()))
                .edge_bind("mid", "actor".into(), "char")
        })
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 1);

    // Add reconciliation between → no match
    g.add_str("ev_rec", "eventType", "reconcile", 2);
    g.add_ref("ev_rec", "actor", "alice", 2);
    assert_eq!(engine.evaluate(&g).len(), 0);
}

// ---------------------------------------------------------------------------
// Temporal constraint violations
// ---------------------------------------------------------------------------

#[test]
fn batch_rejects_wrong_temporal_order() {
    let mut g = MemGraph::new();
    // Events in reverse chronological order
    g.add_str("ev1", "eventType", "enter", 5);
    g.add_ref("ev1", "actor", "alice", 5);
    g.add_str("ev2", "eventType", "leave", 1); // earlier than enter!
    g.add_ref("ev2", "actor", "alice", 1);
    g.set_time(10);

    let pattern = PatternBuilder::new("enter_then_leave")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("enter".into()))
                .edge_bind("e1", "actor".into(), "person")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), MemValue::Str("leave".into()))
                .edge_bind("e2", "actor".into(), "person")
        })
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "temporal order violated — should not match"
    );
}

// ---------------------------------------------------------------------------
// Gap analysis (why_not)
// ---------------------------------------------------------------------------

#[test]
fn why_not_empty_graph() {
    let g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(violation_of_hospitality());
    let analysis = engine.why_not(&g, "violation_of_hospitality").unwrap();
    assert_eq!(
        analysis.stages.len(),
        1,
        "should stop at first unmatched stage"
    );
    match analysis.stages[0].status {
        StageStatus::Unmatched => {}
        ref other => panic!("expected Unmatched, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// drain_completed
// ---------------------------------------------------------------------------

#[test]
fn drain_completed_removes_matches() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(
        PatternBuilder::new("find_harm")
            .stage("e", |s| {
                s.edge("e", "eventType".into(), MemValue::Str("harm".into()))
            })
            .build(),
    );

    g.add_str("ev1", "eventType", "harm", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("harm".into()),
        &Interval::open(1),
    );

    let completed = engine.drain_completed();
    assert_eq!(completed.len(), 1);
    assert!(engine
        .partial_matches()
        .iter()
        .all(|pm| pm.state != MatchState::Complete));
}

// ---------------------------------------------------------------------------
// Irrelevant edges
// ---------------------------------------------------------------------------

#[test]
fn irrelevant_edges_produce_no_events() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(violation_of_hospitality());

    // Match stage 1
    g.add_str("ev1", "eventType", "enterTown", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("enterTown".into()),
        &Interval::open(1),
    );

    // Unrelated edge
    g.add_str("noise", "eventType", "tradeMerchant", 2);
    g.set_time(2);
    let ev = engine.on_edge_added(
        &g,
        &"noise".into(),
        &"eventType".into(),
        &MemValue::Str("tradeMerchant".into()),
        &Interval::open(2),
    );
    assert!(ev.is_empty());
    assert!(!engine
        .active_matches_for("violation_of_hospitality")
        .is_empty());
}

// ---------------------------------------------------------------------------
// Multiple patterns simultaneously
// ---------------------------------------------------------------------------

#[test]
fn multiple_patterns_fire_independently() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(violation_of_hospitality());
    engine.register(
        PatternBuilder::new("any_enter")
            .stage("e", |s| {
                s.edge("e", "eventType".into(), MemValue::Str("enterTown".into()))
            })
            .build(),
    );

    g.add_str("ev1", "eventType", "enterTown", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.set_time(1);
    let ev = engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("enterTown".into()),
        &Interval::open(1),
    );
    assert!(ev
        .iter()
        .any(|e| matches!(e, SiftEvent::Completed { pattern, .. } if pattern == "any_enter")));
    assert!(ev.iter().any(|e| matches!(e, SiftEvent::Advanced { pattern, .. } if pattern == "violation_of_hospitality")));
}

// ---------------------------------------------------------------------------
// Death details in negation events
// ---------------------------------------------------------------------------

#[test]
fn negation_event_includes_details() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(violation_of_hospitality());

    // Set up stages 1+2
    g.add_str("ev1", "eventType", "enterTown", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("enterTown".into()),
        &Interval::open(1),
    );
    g.add_str("ev2", "eventType", "showHospitality", 2);
    g.add_ref("ev2", "actor", "bob", 2);
    g.add_ref("ev2", "target", "alice", 2);
    g.set_time(2);
    engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &MemValue::Str("showHospitality".into()),
        &Interval::open(2),
    );

    // Kill it
    g.add_str("ev_leave", "eventType", "leaveTown", 3);
    g.add_ref("ev_leave", "actor", "alice", 3);
    g.set_time(3);
    let ev = engine.on_edge_added(
        &g,
        &"ev_leave".into(),
        &"eventType".into(),
        &MemValue::Str("leaveTown".into()),
        &Interval::open(3),
    );

    let negated = ev.iter().find(|e| matches!(e, SiftEvent::Negated { .. }));
    assert!(negated.is_some(), "should emit Negated event");
    if let Some(SiftEvent::Negated {
        clause_label,
        trigger_source,
        ..
    }) = negated
    {
        assert_eq!(trigger_source, "ev_leave");
        assert!(
            clause_label.contains("eventType"),
            "clause_label should reference the matching label"
        );
    }
}
