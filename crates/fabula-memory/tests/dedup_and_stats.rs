use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

// ===========================================================================

#[test]
fn dedup_before_fix_same_actor_duplicates() {
    // Same actor does "start" 10 times. Each creates a PM with the same
    // bindings {person=alice} but different intervals. These are distinct
    // temporal threads and should all be kept.
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(
        PatternBuilder::new("two_stage")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("start".into()))
                    .edge_bind("e1", "actor".into(), "person")
            })
            .stage("e2", |s| {
                s.edge("e2", "eventType".into(), MemValue::Str("finish".into()))
                    .edge_bind("e2", "actor".into(), "person")
            })
            .build(),
    );

    for t in 1..=10i64 {
        let name = format!("ev{}", t);
        g.add_str(&name, "eventType", "start", t);
        g.add_ref(&name, "actor", "alice", t);
        g.set_time(t);
        engine.on_edge_added(
            &g,
            &name,
            &"eventType".into(),
            &MemValue::Str("start".into()),
            &Interval::open(t),
        );
    }

    // These are 10 DISTINCT temporal threads (different intervals),
    // so all 10 should be kept.
    assert_eq!(engine.active_matches_for("two_stage").len(), 10);
}

#[test]
fn dedup_before_fix_duplicate_events_produce_multiple() {
    // Single-stage pattern with duplicate edges in the graph.
    // Today: duplicate edges produce duplicate completed matches.
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(
        PatternBuilder::new("find_harm")
            .stage("e", |s| {
                s.edge("e", "eventType".into(), MemValue::Str("harm".into()))
            })
            .build(),
    );

    // Add the SAME edge twice to the graph
    g.add_str("ev1", "eventType", "harm", 1);
    g.add_str("ev1", "eventType", "harm", 1); // exact duplicate
    g.set_time(1);

    let events = engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("harm".into()),
        &Interval::open(1),
    );

    // Dedup: duplicate edges produce only 1 completion (same fingerprint).
    let completed = events
        .iter()
        .filter(|e| matches!(e, SiftEvent::Completed { .. }))
        .count();
    assert_eq!(
        completed, 1,
        "duplicate edges should produce exactly 1 completion"
    );
}

#[test]
fn dedup_distinct_intervals_not_merged() {
    // Same actor enters at t=1 and t=3. Same bindings (person=alice) but
    // different intervals. These are distinct temporal threads -- both kept.
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(
        PatternBuilder::new("enter_then_act")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("enter".into()))
                    .edge_bind("e1", "actor".into(), "person")
            })
            .stage("e2", |s| {
                s.edge("e2", "eventType".into(), MemValue::Str("act".into()))
                    .edge_bind("e2", "actor".into(), "person")
            })
            .build(),
    );

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

    g.add_str("ev2", "eventType", "enter", 3);
    g.add_ref("ev2", "actor", "alice", 3);
    g.set_time(3);
    engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &MemValue::Str("enter".into()),
        &Interval::open(3),
    );

    assert_eq!(
        engine.active_matches_for("enter_then_act").len(),
        2,
        "same bindings but different intervals = 2 distinct temporal threads"
    );
}

#[test]
fn dedup_pm_count_bounded() {
    // 50 events from the same node at the same timestamp should produce
    // at most 1 PM per stage, not 50.
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(
        PatternBuilder::new("two_harms")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("harm".into()))
                    .edge_bind("e1", "actor".into(), "attacker")
            })
            .stage("e2", |s| {
                s.edge("e2", "eventType".into(), MemValue::Str("harm".into()))
                    .edge_bind("e2", "actor".into(), "attacker")
            })
            .build(),
    );

    // Add 50 duplicate harm edges from bob at t=1
    for _ in 0..50 {
        g.add_str("ev1", "eventType", "harm", 1);
        g.add_ref("ev1", "actor", "bob", 1);
    }
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("harm".into()),
        &Interval::open(1),
    );

    // Should have exactly 1 active PM, not 50
    assert_eq!(
        engine.active_matches_for("two_harms").len(),
        1,
        "50 duplicate edges should dedup to 1 PM"
    );
}

#[test]
fn dedup_events_match_pms() {
    // Event count must equal PM count -- no orphan events, no silent PMs.
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
    g.add_str("ev1", "eventType", "harm", 1); // duplicate
    g.add_str("ev1", "eventType", "harm", 1); // triple
    g.set_time(1);
    let events = engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("harm".into()),
        &Interval::open(1),
    );

    assert_eq!(events.len(), 1, "exactly 1 event for deduplicated match");
    assert_eq!(engine.partial_matches().len(), 1, "exactly 1 PM");
}

// ===========================================================================
// 5c. Engine stats counters
// ===========================================================================

#[test]
fn stats_on_edge_added_counter() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(
        PatternBuilder::new("test")
            .stage("e", |s| {
                s.edge("e", "eventType".into(), MemValue::Str("enter".into()))
            })
            .build(),
    );

    assert_eq!(engine.stats().total_on_edge_added, 0);

    g.add_str("ev1", "eventType", "enter", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("enter".into()),
        &Interval::open(1),
    );

    assert_eq!(engine.stats().total_on_edge_added, 1);

    g.add_str("ev2", "eventType", "enter", 2);
    g.set_time(2);
    engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &MemValue::Str("enter".into()),
        &Interval::open(2),
    );

    assert_eq!(engine.stats().total_on_edge_added, 2);
}

#[test]
fn stats_fingerprints_and_negation() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(
        PatternBuilder::new("test")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("enter".into()))
                    .edge_bind("e1", "actor".into(), "person")
            })
            .stage("e2", |s| {
                s.edge("e2", "eventType".into(), MemValue::Str("leave".into()))
                    .edge_bind("e2", "actor".into(), "person")
            })
            .unless_between("e1", "e2", |neg| {
                neg.edge("mid", "eventType".into(), MemValue::Str("cancel".into()))
                    .edge_bind("mid", "actor".into(), "person")
            })
            .build(),
    );

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

    assert!(
        engine.stats().total_fingerprints > 0,
        "should compute fingerprints"
    );

    // Second edge triggers negation check on the active PM
    g.add_str("ev2", "eventType", "cancel", 2);
    g.add_ref("ev2", "actor", "alice", 2);
    g.set_time(2);
    engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &MemValue::Str("cancel".into()),
        &Interval::open(2),
    );

    assert!(
        engine.stats().total_negation_checks > 0,
        "should check negations"
    );
}

#[test]
fn stats_peak_active_pms() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(
        PatternBuilder::new("test")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("enter".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "eventType".into(), MemValue::Str("leave".into()))
            })
            .build(),
    );

    for t in 1..=5i64 {
        let name = format!("ev{}", t);
        g.add_str(&name, "eventType", "enter", t);
        g.set_time(t);
        engine.on_edge_added(
            &g,
            &name,
            &"eventType".into(),
            &MemValue::Str("enter".into()),
            &Interval::open(t),
        );
    }

    assert_eq!(
        engine.stats().peak_active_pms,
        5,
        "peak should be 5 after adding 5 matching first-stage edges"
    );
}

// ===========================================================================
// Pattern lifecycle (Phase 5.2)
// ===========================================================================
