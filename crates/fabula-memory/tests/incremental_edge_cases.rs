use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

#[test]
fn incremental_negation_kills_only_matching_variable_bindings() {
    // Two partial matches for different characters; negation should
    // only kill the one whose bound variable matches.
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(
        PatternBuilder::new("enter_then_harm")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("enter".into()))
                    .edge_bind("e1", "actor".into(), "guest")
            })
            .stage("e2", |s| {
                s.edge("e2", "eventType".into(), MemValue::Str("harm".into()))
                    .edge_bind("e2", "actor".into(), "host")
                    .edge_bind("e2", "target".into(), "guest")
            })
            .unless_between("e1", "e2", |neg| {
                neg.edge("mid", "eventType".into(), MemValue::Str("leave".into()))
                    .edge_bind("mid", "actor".into(), "guest")
            })
            .build(),
    );

    // Alice enters
    g.add_str("ev1", "eventType", "enter", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("enter".into()),
        &Interval::open(1),
    );

    // Bob enters
    g.add_str("ev2", "eventType", "enter", 2);
    g.add_ref("ev2", "actor", "bob", 2);
    g.set_time(2);
    engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &MemValue::Str("enter".into()),
        &Interval::open(2),
    );

    assert_eq!(engine.active_matches_for("enter_then_harm").len(), 2);

    // Alice leaves -- should kill only alice's partial match
    g.add_str("ev_leave", "eventType", "leave", 3);
    g.add_ref("ev_leave", "actor", "alice", 3);
    g.set_time(3);
    let events = engine.on_edge_added(
        &g,
        &"ev_leave".into(),
        &"eventType".into(),
        &MemValue::Str("leave".into()),
        &Interval::open(3),
    );

    let negated_count = events
        .iter()
        .filter(|e| matches!(e, SiftEvent::Negated { .. }))
        .count();
    assert_eq!(
        negated_count, 1,
        "only alice's partial match should be negated"
    );
    assert_eq!(
        engine.active_matches_for("enter_then_harm").len(),
        1,
        "bob's partial match should survive"
    );
}

// ===========================================================================
// 8. Incremental vs batch consistency
// ===========================================================================

#[test]
fn out_of_order_insertion_incremental_misses_match() {
    // BUG DOCUMENTATION: Inserting edges in reverse chronological order
    // via on_edge_added causes the incremental engine to miss valid matches
    // that batch evaluation finds.
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(
        PatternBuilder::new("enter_then_harm")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("enter".into()))
                    .edge_bind("e1", "actor".into(), "person")
            })
            .stage("e2", |s| {
                s.edge("e2", "eventType".into(), MemValue::Str("harm".into()))
                    .edge_bind("e2", "actor".into(), "person")
            })
            .build(),
    );

    // Insert stage 2's event FIRST
    g.add_str("ev2", "eventType", "harm", 5);
    g.add_ref("ev2", "actor", "alice", 5);
    g.set_time(5);
    engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &MemValue::Str("harm".into()),
        &Interval::open(5),
    );

    // Then insert stage 1's event (earlier timestamp)
    g.add_str("ev1", "eventType", "enter", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.set_time(5);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("enter".into()),
        &Interval::open(1),
    );

    // Incremental: no completed matches (out-of-order)
    let incremental_completed = engine
        .partial_matches()
        .iter()
        .filter(|pm| pm.state == MatchState::Complete)
        .count();
    assert_eq!(
        incremental_completed, 0,
        "incremental misses match when edges arrive out of chronological order"
    );

    // Batch: finds the match
    let batch_matches = engine.evaluate(&g);
    assert_eq!(
        batch_matches.len(),
        1,
        "batch correctly finds the match regardless of insertion order"
    );
}

#[test]
fn batch_and_incremental_agree_on_simple_case() {
    // Baseline: when edges arrive in chronological order,
    // batch and incremental should agree.
    let pattern = PatternBuilder::new("hospitality_violation")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("enter".into()))
                .edge_bind("e1", "actor".into(), "guest")
        })
        .stage("e2", |s| {
            s.edge(
                "e2",
                "eventType".into(),
                MemValue::Str("show_hospitality".into()),
            )
            .edge_bind("e2", "actor".into(), "host")
            .edge_bind("e2", "target".into(), "guest")
        })
        .stage("e3", |s| {
            s.edge("e3", "eventType".into(), MemValue::Str("harm".into()))
                .edge_bind("e3", "actor".into(), "host")
                .edge_bind("e3", "target".into(), "guest")
        })
        .build();

    // Build graph incrementally
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern.clone());

    g.add_str("ev1", "eventType", "enter", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("enter".into()),
        &Interval::open(1),
    );

    g.add_str("ev2", "eventType", "show_hospitality", 2);
    g.add_ref("ev2", "actor", "bob", 2);
    g.add_ref("ev2", "target", "alice", 2);
    g.set_time(2);
    engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &MemValue::Str("show_hospitality".into()),
        &Interval::open(2),
    );

    g.add_str("ev3", "eventType", "harm", 3);
    g.add_ref("ev3", "actor", "bob", 3);
    g.add_ref("ev3", "target", "alice", 3);
    g.set_time(3);
    engine.on_edge_added(
        &g,
        &"ev3".into(),
        &"eventType".into(),
        &MemValue::Str("harm".into()),
        &Interval::open(3),
    );

    let incremental_completed = engine
        .partial_matches()
        .iter()
        .filter(|pm| pm.state == MatchState::Complete)
        .count();

    // Batch evaluation on the same graph
    let mut batch_engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    batch_engine.register(pattern);
    let batch_matches = batch_engine.evaluate(&g);

    assert_eq!(
        incremental_completed,
        batch_matches.len(),
        "batch and incremental should agree when edges arrive in order"
    );
    assert_eq!(batch_matches.len(), 1);
}

#[test]
fn incremental_temporal_ordering_enforced() {
    // B2 fix: on_edge_added now checks temporal ordering between stages.
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(
        PatternBuilder::new("a_then_b")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("a".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "eventType".into(), MemValue::Str("b".into()))
            })
            .build(),
    );

    // Event A at t=10
    g.add_str("ev1", "eventType", "a", 10);
    g.set_time(10);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("a".into()),
        &Interval::open(10),
    );

    // Event B at t=5 (BEFORE A -- temporal order violated)
    g.add_str("ev2", "eventType", "b", 5);
    g.set_time(10);
    let events = engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &MemValue::Str("b".into()),
        &Interval::open(5),
    );

    // B2 fixed: incremental rejects inverted temporal order
    let completed = events
        .iter()
        .any(|e| matches!(e, SiftEvent::Completed { .. }));
    assert!(
        !completed,
        "incremental should reject temporally inverted match"
    );

    // Batch also rejects it
    let batch_matches = engine.evaluate(&g);
    assert_eq!(
        batch_matches.len(),
        0,
        "batch also rejects temporally inverted match"
    );
}

// ===========================================================================
// Additional interval edge cases
// ===========================================================================

#[test]
fn interval_zero_length() {
    // Interval [5, 5) -- start == end, zero length
    let iv = Interval::new(5, 5);
    assert!(!iv.covers(&5), "[5,5) should not cover 5 (empty interval)");
    assert!(!iv.covers(&4));
}

#[test]
fn interval_open_ended_relation_always_none() {
    let a = Interval::open(1);
    let b = Interval::new(3, 5);
    assert_eq!(a.relation(&b), None);

    let c = Interval::open(1);
    let d = Interval::open(3);
    assert_eq!(c.relation(&d), None);
}

#[test]
fn interval_intersects_edge_cases() {
    // Two open-ended intervals always intersect
    let a = Interval::<i64>::open(100);
    let b = Interval::<i64>::open(200);
    assert!(
        a.intersects(&b),
        "two open-ended intervals always intersect"
    );

    // Zero-length interval [5,5) intersects [3,7) by the math:
    // self.start(5) < b_end(7) = true, other.start(3) < a_end(5) = true
    // This is arguably a quirk: the interval is empty but "intersects".
    let z = Interval::new(5, 5);
    let c = Interval::new(3, 7);
    assert!(
        z.intersects(&c),
        "zero-length [5,5) 'intersects' [3,7) due to half-open comparison (quirk)"
    );

    // Adjacent intervals don't intersect (half-open semantics)
    let d = Interval::new(1, 5);
    let e = Interval::new(5, 10);
    assert!(!d.intersects(&e), "[1,5) and [5,10) don't intersect");
}

#[test]
fn open_ended_interval_fails_non_before_temporal_constraints() {
    // BUG DOCUMENTATION: explicit temporal constraints with Allen relations
    // other than Before/Meets fail when intervals are open-ended.
    let a = Interval::open(1);
    let b = Interval::new(3, 7);

    // Conceptually, [1, inf) Contains [3, 7), but:
    assert_eq!(
        a.relation(&b),
        None,
        "open-ended interval returns None for relation()"
    );
    // The fallback in check_temporal only handles Before/Meets
}

// ===========================================================================
// Builder edge cases
// ===========================================================================

#[test]
fn unless_global_no_stages_still_resolves() {
    // B5b fix: is_global is always cleared at build time, even with no stages.
    let pattern = PatternBuilder::<String, String>::new("empty_global_neg")
        .unless_global(|neg| neg.edge("x", "type".into(), "bad".into()))
        .build();

    // is_global is cleared even with no stages
    assert!(
        !pattern.negations[0].is_global,
        "is_global should be cleared at build time"
    );
}

// ===========================================================================
// Gap analysis edge cases
// ===========================================================================

#[test]
fn why_not_nonexistent_pattern() {
    let g = MemGraph::new();
    let engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    assert!(
        engine.why_not(&g, "nonexistent").is_none(),
        "why_not for unregistered pattern should return None"
    );
}

#[test]
fn why_not_matched_pattern_shows_all_matched() {
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "harm", 1);
    g.add_ref("ev1", "actor", "bob", 1);
    g.set_time(10);

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(
        PatternBuilder::new("find_harm")
            .stage("e", |s| {
                s.edge("e", "eventType".into(), MemValue::Str("harm".into()))
                    .edge_bind("e", "actor".into(), "attacker")
            })
            .build(),
    );

    let analysis = engine.why_not(&g, "find_harm").unwrap();
    // Note: why_not doesn't propagate bindings between stages, so it may
    // report stages as matched based on partial evaluation. For a single-stage
    // pattern, this should work.
    assert_eq!(analysis.stages.len(), 1);
    // The first stage should show as matched since the edge exists
}

#[test]
fn why_not_stops_at_first_unmatched_stage() {
    let g = MemGraph::new(); // empty graph
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(
        PatternBuilder::new("three_stages")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("a".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "eventType".into(), MemValue::Str("b".into()))
            })
            .stage("e3", |s| {
                s.edge("e3", "eventType".into(), MemValue::Str("c".into()))
            })
            .build(),
    );

    let analysis = engine.why_not(&g, "three_stages").unwrap();
    assert_eq!(
        analysis.stages.len(),
        1,
        "why_not should stop at first unmatched stage, not report all three"
    );
    matches!(analysis.stages[0].status, StageStatus::Unmatched);
}

// ===========================================================================
// drain_completed edge cases
// ===========================================================================

#[test]
fn drain_completed_on_empty_engine() {
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    let drained = engine.drain_completed();
    assert!(drained.is_empty());
}

#[test]
fn drain_completed_preserves_active_matches() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    // Register two patterns
    engine.register(
        PatternBuilder::new("single_stage")
            .stage("e", |s| {
                s.edge("e", "eventType".into(), MemValue::Str("harm".into()))
            })
            .build(),
    );
    engine.register(
        PatternBuilder::new("two_stage")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("harm".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "eventType".into(), MemValue::Str("heal".into()))
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

    // single_stage completes, two_stage has a partial match
    let drained = engine.drain_completed();
    assert_eq!(drained.len(), 1);
    assert_eq!(drained[0].pattern, "single_stage");

    // Active partial match for two_stage should survive
    assert_eq!(engine.active_matches_for("two_stage").len(), 1);
}
