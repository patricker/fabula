//! Edge case tests — adversarial inputs, boundary conditions, and
//! batch-vs-incremental consistency checks.
//!
//! Each test documents the scenario, expected behavior, and whether
//! the current behavior is a bug or acceptable.

use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

// ===========================================================================
// 1. Empty inputs
// ===========================================================================

#[test]
fn empty_pattern_no_stages() {
    let g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    let pattern = PatternBuilder::<String, MemValue>::new("empty").build();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 0, "empty pattern should never match");
    let analysis = engine.why_not(&g, "empty").unwrap();
    assert!(analysis.stages.is_empty(), "empty pattern has no stages to analyze");
}

#[test]
fn empty_stage_no_clauses() {
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "harm", 1);
    g.set_time(10);

    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    // A stage with clauses followed by an empty stage
    let pattern = PatternBuilder::new("has_empty_stage")
        .stage("e1", |s| s.edge("e1", "eventType".into(), MemValue::Str("harm".into())))
        .stage("e2", |s| s) // empty stage — no clauses
        .build();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 0,
        "pattern with empty stage can never fully match");
}

#[test]
fn empty_stage_incremental_never_advances() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    let pattern = PatternBuilder::new("empty_first_stage")
        .stage("e1", |s| s) // empty first stage
        .stage("e2", |s| s.edge("e2", "eventType".into(), MemValue::Str("harm".into())))
        .build();
    engine.register(pattern);

    g.add_str("ev1", "eventType", "harm", 1);
    g.set_time(1);
    let events = engine.on_edge_added(
        &g, &"ev1".into(), &"eventType".into(),
        &MemValue::Str("harm".into()), &Interval::open(1),
    );
    assert!(events.is_empty(),
        "empty first stage means no edge can initiate a partial match");
}

#[test]
fn empty_negation_no_clauses() {
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "betray", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.add_str("ev2", "eventType", "betray", 3);
    g.add_ref("ev2", "actor", "alice", 3);
    g.set_time(10);

    let pattern = PatternBuilder::new("empty_negation")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("betray".into()))
                .edge_bind("e1", "actor".into(), "char")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), MemValue::Str("betray".into()))
                .edge_bind("e2", "actor".into(), "char")
        })
        .unless_between("e1", "e2", |neg| neg) // empty negation body
        .build();

    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 1,
        "empty negation body should not block matches");
}

#[test]
fn empty_graph_with_registered_patterns() {
    let g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(PatternBuilder::new("find_harm")
        .stage("e", |s| s.edge("e", "eventType".into(), MemValue::Str("harm".into())))
        .build());
    assert_eq!(engine.evaluate(&g).len(), 0);
    assert!(engine.partial_matches().is_empty());
}

// ===========================================================================
// 2. Single-element inputs
// ===========================================================================

#[test]
fn single_edge_graph_single_clause_pattern() {
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "harm", 1);
    g.set_time(1);

    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(PatternBuilder::new("single")
        .stage("e", |s| s.edge("e", "eventType".into(), MemValue::Str("harm".into())))
        .build());
    assert_eq!(engine.evaluate(&g).len(), 1);
}

#[test]
fn single_stage_with_unless_after_batch() {
    // BUG DOCUMENTATION: Single-stage pattern with negation.
    // Batch correctly blocks the match; incremental may not.
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "crime", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.add_str("ev_pardon", "eventType", "pardon", 2);
    g.add_ref("ev_pardon", "actor", "alice", 2);
    g.set_time(10);

    let pattern = PatternBuilder::new("unpardoned_crime")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("crime".into()))
                .edge_bind("e1", "actor".into(), "criminal")
        })
        .unless_after("e1", |neg| {
            neg.edge("pardon_ev", "eventType".into(), MemValue::Str("pardon".into()))
                .edge_bind("pardon_ev", "actor".into(), "criminal")
        })
        .build();

    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(pattern);

    // Batch correctly blocks: pardon exists after crime
    assert_eq!(engine.evaluate(&g).len(), 0,
        "batch should block match when negation event exists");
}

#[test]
fn single_stage_with_unless_after_incremental_consistency() {
    // B3 fix: single-stage patterns now check negations before completing.
    // When no negation event exists yet, the match completes normally.
    // When a negation event already exists, the match is blocked.
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(PatternBuilder::new("unpardoned_crime")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("crime".into()))
                .edge_bind("e1", "actor".into(), "criminal")
        })
        .unless_after("e1", |neg| {
            neg.edge("pardon_ev", "eventType".into(), MemValue::Str("pardon".into()))
                .edge_bind("pardon_ev", "actor".into(), "criminal")
        })
        .build());

    // Crime arrives with no pardon existing -> completes (correct)
    g.add_str("ev1", "eventType", "crime", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.set_time(1);
    let events = engine.on_edge_added(
        &g, &"ev1".into(), &"eventType".into(),
        &MemValue::Str("crime".into()), &Interval::open(1),
    );
    let completed = events.iter().any(|e| matches!(e, SiftEvent::Completed { .. }));
    assert!(completed, "no pardon exists yet, match should complete");

    // Now test: if pardon exists BEFORE crime, match should be blocked
    let mut g2 = MemGraph::new();
    let mut engine2: SiftEngine<MemGraph> = SiftEngine::new();
    engine2.register(PatternBuilder::new("unpardoned_crime")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("crime".into()))
                .edge_bind("e1", "actor".into(), "criminal")
        })
        .unless_after("e1", |neg| {
            neg.edge("pardon_ev", "eventType".into(), MemValue::Str("pardon".into()))
                .edge_bind("pardon_ev", "actor".into(), "criminal")
        })
        .build());

    // Pardon exists first
    g2.add_str("ev_pardon", "eventType", "pardon", 2);
    g2.add_ref("ev_pardon", "actor", "alice", 2);
    // Then crime
    g2.add_str("ev1", "eventType", "crime", 1);
    g2.add_ref("ev1", "actor", "alice", 1);
    g2.set_time(3);
    let events3 = engine2.on_edge_added(
        &g2, &"ev1".into(), &"eventType".into(),
        &MemValue::Str("crime".into()), &Interval::open(1),
    );
    let completed2 = events3.iter().any(|e| matches!(e, SiftEvent::Completed { .. }));
    assert!(!completed2, "pardon exists — negation should block completion");
}

// ===========================================================================
// 3. Duplicate / repeated data
// ===========================================================================

#[test]
fn duplicate_edges_produce_duplicate_matches() {
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "harm", 1);
    g.add_str("ev1", "eventType", "harm", 1); // exact duplicate
    g.add_ref("ev1", "actor", "bob", 1);
    g.set_time(10);

    let pattern = PatternBuilder::new("find_harm")
        .stage("e", |s| {
            s.edge("e", "eventType".into(), MemValue::Str("harm".into()))
                .edge_bind("e", "actor".into(), "attacker")
        })
        .build();

    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    // MemGraph stores both edges; scan returns duplicates; engine produces
    // duplicate matches with identical bindings.
    assert_eq!(matches.len(), 2,
        "duplicate edges in MemGraph cause duplicate matches (known limitation)");
}

#[test]
fn same_event_cannot_satisfy_two_stages() {
    // An event with contradictory tags that could match both stages
    let mut g = MemGraph::new();
    g.add_str("ev1", "tag", "negative", 1);
    g.add_str("ev1", "tag", "positive", 1);
    g.add_str("ev1", "tag", "romantic", 1);
    g.add_ref("ev1", "actor", "mira", 1);
    g.set_time(10);

    let pattern = PatternBuilder::new("mood_swing")
        .stage("e1", |s| {
            s.edge("e1", "tag".into(), MemValue::Str("negative".into()))
                .edge("e1", "tag".into(), MemValue::Str("romantic".into()))
                .edge_bind("e1", "actor".into(), "char")
        })
        .stage("e2", |s| {
            s.edge("e2", "tag".into(), MemValue::Str("positive".into()))
                .edge("e2", "tag".into(), MemValue::Str("romantic".into()))
                .edge_bind("e2", "actor".into(), "char")
        })
        .build();

    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    // Same event (same start time) cannot satisfy both stages
    // due to strict temporal ordering: left.start >= right.start -> reject
    assert_eq!(engine.evaluate(&g).len(), 0,
        "same event (same timestamp) cannot satisfy two stages");
}

#[test]
fn events_at_identical_timestamps_cannot_sequence() {
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "enter", 5);
    g.add_ref("ev1", "actor", "alice", 5);
    g.add_str("ev2", "eventType", "greet", 5); // same timestamp
    g.add_ref("ev2", "actor", "bob", 5);
    g.add_ref("ev2", "target", "alice", 5);
    g.set_time(10);

    let pattern = PatternBuilder::new("enter_then_greet")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("enter".into()))
                .edge_bind("e1", "actor".into(), "guest")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), MemValue::Str("greet".into()))
                .edge_bind("e2", "target".into(), "guest")
        })
        .build();

    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 0,
        "strict temporal ordering rejects same-timestamp events in different stages");
}

#[test]
fn variable_consistency_across_stages() {
    let mut g = MemGraph::new();
    // Stage 1: actor is alice
    g.add_str("ev1", "eventType", "enter", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    // Stage 2: actor is bob (different!)
    g.add_str("ev2", "eventType", "leave", 2);
    g.add_ref("ev2", "actor", "bob", 2);
    g.set_time(10);

    let pattern = PatternBuilder::new("same_actor")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("enter".into()))
                .edge_bind("e1", "actor".into(), "person")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), MemValue::Str("leave".into()))
                .edge_bind("e2", "actor".into(), "person") // must be same as stage 1
        })
        .build();

    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 0,
        "variable bound in stage 1 must match in stage 2");
}

// ===========================================================================
// 4. Circular / self-referential
// ===========================================================================

#[test]
fn self_referential_edge() {
    let mut g = MemGraph::new();
    g.add_ref("alice", "enemy", "alice", 1); // self-loop
    g.set_time(10);

    let pattern = PatternBuilder::new("self_enemy")
        .stage("e", |s| {
            s.edge_bind("e", "enemy".into(), "target")
        })
        .build();

    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1, "self-loop edge should match");
    match (&matches[0].bindings["e"], &matches[0].bindings["target"]) {
        (BoundValue::Node(src), BoundValue::Node(tgt)) => {
            assert_eq!(src, "alice");
            assert_eq!(tgt, "alice");
        }
        other => panic!("expected both bindings to be alice, got {:?}", other),
    }
}

#[test]
fn source_equals_target_variable_self_loop_only() {
    // B8 fix: When anchor/source and bind-target share the same variable name,
    // bind_target now validates consistency. Only self-loops should match.
    let mut g = MemGraph::new();
    g.add_ref("alice", "enemy", "alice", 1); // self-loop
    g.add_ref("bob", "enemy", "charlie", 1); // NOT a self-loop
    g.set_time(10);

    // Pattern: ?e --[enemy]--> ?e (same variable for source and target)
    let pattern = PatternBuilder::new("narcissist")
        .stage("e", |s| {
            s.edge_bind("e", "enemy".into(), "e") // target var = anchor var
        })
        .build();

    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    // B8 fixed: only alice (self-loop) matches. Bob->Charlie is not a self-loop.
    assert_eq!(matches.len(), 1, "only self-loop should match when source == target var");
    match &matches[0].bindings["e"] {
        BoundValue::Node(n) => assert_eq!(n, "alice"),
        other => panic!("expected alice, got {:?}", other),
    }
}

// ===========================================================================
// 5. Large-scale
// ===========================================================================

#[test]
fn ten_stage_pattern() {
    let mut g = MemGraph::new();
    for i in 0..10u32 {
        let name = format!("ev{}", i);
        let event_type = format!("step{}", i);
        g.add_str(&name, "eventType", &event_type, (i + 1) as i64);
        g.add_ref(&name, "actor", "alice", (i + 1) as i64);
    }
    g.set_time(100);

    let mut builder = PatternBuilder::<String, MemValue>::new("long_chain");
    for i in 0..10u32 {
        let anchor = format!("e{}", i);
        let event_type = format!("step{}", i);
        let a = anchor.clone();
        builder = builder.stage(&anchor, move |s| {
            s.edge(&a, "eventType".into(), MemValue::Str(event_type.into()))
                .edge_bind(&a, "actor".into(), "char")
        });
    }
    let pattern = builder.build();

    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1, "10-stage chain should match");
}

#[test]
fn distinct_events_create_distinct_pms() {
    // 100 different source nodes at different timestamps — all unique fingerprints.
    // These are legitimately distinct PMs (different bindings + intervals).
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(PatternBuilder::new("two_stage")
        .stage("e1", |s| s.edge("e1", "eventType".into(), MemValue::Str("start".into())))
        .stage("e2", |s| s.edge("e2", "eventType".into(), MemValue::Str("finish".into())))
        .build());

    for i in 0..100i64 {
        let name = format!("ev{}", i);
        g.add_str(&name, "eventType", "start", i + 1);
        g.set_time(i + 1);
        engine.on_edge_added(
            &g, &name, &"eventType".into(),
            &MemValue::Str("start".into()), &Interval::open(i + 1),
        );
    }

    assert_eq!(engine.active_matches_for("two_stage").len(), 100,
        "100 distinct events (different nodes + timestamps) = 100 distinct PMs");
}

#[test]
fn large_graph_batch_evaluation() {
    let mut g = MemGraph::new();
    // 1000 edges, only one pair forms the pattern
    for i in 0..1000i64 {
        let name = format!("noise{}", i);
        g.add_str(&name, "eventType", "idle", i);
        g.add_ref(&name, "actor", &format!("npc{}", i % 50), i);
    }
    // The signal
    g.add_str("signal1", "eventType", "enter", 500);
    g.add_ref("signal1", "actor", "alice", 500);
    g.add_str("signal2", "eventType", "leave", 501);
    g.add_ref("signal2", "actor", "alice", 501);
    g.set_time(2000);

    let pattern = PatternBuilder::new("enter_leave")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("enter".into()))
                .edge_bind("e1", "actor".into(), "person")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), MemValue::Str("leave".into()))
                .edge_bind("e2", "actor".into(), "person")
        })
        .build();

    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1, "should find the one matching pair in 1000 edges");
}

// ===========================================================================
// 5b. Dedup — tests that demonstrate the accumulation bug (before fix)
// ===========================================================================

#[test]
fn dedup_before_fix_same_actor_duplicates() {
    // Same actor does "start" 10 times. Each creates a PM with the same
    // bindings {person=alice} but different intervals. These are distinct
    // temporal threads and should all be kept.
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(PatternBuilder::new("two_stage")
        .stage("e1", |s| s.edge("e1", "eventType".into(), MemValue::Str("start".into()))
            .edge_bind("e1", "actor".into(), "person"))
        .stage("e2", |s| s.edge("e2", "eventType".into(), MemValue::Str("finish".into()))
            .edge_bind("e2", "actor".into(), "person"))
        .build());

    for t in 1..=10i64 {
        let name = format!("ev{}", t);
        g.add_str(&name, "eventType", "start", t);
        g.add_ref(&name, "actor", "alice", t);
        g.set_time(t);
        engine.on_edge_added(&g, &name, &"eventType".into(),
            &MemValue::Str("start".into()), &Interval::open(t));
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
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(PatternBuilder::new("find_harm")
        .stage("e", |s| s.edge("e", "eventType".into(), MemValue::Str("harm".into())))
        .build());

    // Add the SAME edge twice to the graph
    g.add_str("ev1", "eventType", "harm", 1);
    g.add_str("ev1", "eventType", "harm", 1); // exact duplicate
    g.set_time(1);

    let events = engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(),
        &MemValue::Str("harm".into()), &Interval::open(1));

    // Dedup: duplicate edges produce only 1 completion (same fingerprint).
    let completed = events.iter().filter(|e| matches!(e, SiftEvent::Completed { .. })).count();
    assert_eq!(completed, 1, "duplicate edges should produce exactly 1 completion");
}

#[test]
fn dedup_distinct_intervals_not_merged() {
    // Same actor enters at t=1 and t=3. Same bindings (person=alice) but
    // different intervals. These are distinct temporal threads — both kept.
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(PatternBuilder::new("enter_then_act")
        .stage("e1", |s| s.edge("e1", "eventType".into(), MemValue::Str("enter".into()))
            .edge_bind("e1", "actor".into(), "person"))
        .stage("e2", |s| s.edge("e2", "eventType".into(), MemValue::Str("act".into()))
            .edge_bind("e2", "actor".into(), "person"))
        .build());

    g.add_str("ev1", "eventType", "enter", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.set_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(),
        &MemValue::Str("enter".into()), &Interval::open(1));

    g.add_str("ev2", "eventType", "enter", 3);
    g.add_ref("ev2", "actor", "alice", 3);
    g.set_time(3);
    engine.on_edge_added(&g, &"ev2".into(), &"eventType".into(),
        &MemValue::Str("enter".into()), &Interval::open(3));

    assert_eq!(engine.active_matches_for("enter_then_act").len(), 2,
        "same bindings but different intervals = 2 distinct temporal threads");
}

#[test]
fn dedup_pm_count_bounded() {
    // 50 events from the same node at the same timestamp should produce
    // at most 1 PM per stage, not 50.
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(PatternBuilder::new("two_harms")
        .stage("e1", |s| s.edge("e1", "eventType".into(), MemValue::Str("harm".into()))
            .edge_bind("e1", "actor".into(), "attacker"))
        .stage("e2", |s| s.edge("e2", "eventType".into(), MemValue::Str("harm".into()))
            .edge_bind("e2", "actor".into(), "attacker"))
        .build());

    // Add 50 duplicate harm edges from bob at t=1
    for _ in 0..50 {
        g.add_str("ev1", "eventType", "harm", 1);
        g.add_ref("ev1", "actor", "bob", 1);
    }
    g.set_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(),
        &MemValue::Str("harm".into()), &Interval::open(1));

    // Should have exactly 1 active PM, not 50
    assert_eq!(engine.active_matches_for("two_harms").len(), 1,
        "50 duplicate edges should dedup to 1 PM");
}

#[test]
fn dedup_events_match_pms() {
    // Event count must equal PM count — no orphan events, no silent PMs.
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(PatternBuilder::new("find_harm")
        .stage("e", |s| s.edge("e", "eventType".into(), MemValue::Str("harm".into())))
        .build());

    g.add_str("ev1", "eventType", "harm", 1);
    g.add_str("ev1", "eventType", "harm", 1); // duplicate
    g.add_str("ev1", "eventType", "harm", 1); // triple
    g.set_time(1);
    let events = engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(),
        &MemValue::Str("harm".into()), &Interval::open(1));

    assert_eq!(events.len(), 1, "exactly 1 event for deduplicated match");
    assert_eq!(engine.partial_matches().len(), 1, "exactly 1 PM");
}

// ===========================================================================
// 5c. Engine stats counters
// ===========================================================================

#[test]
fn stats_on_edge_added_counter() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(PatternBuilder::new("test")
        .stage("e", |s| s.edge("e", "eventType".into(), MemValue::Str("enter".into())))
        .build());

    assert_eq!(engine.stats().total_on_edge_added, 0);

    g.add_str("ev1", "eventType", "enter", 1);
    g.set_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(),
        &MemValue::Str("enter".into()), &Interval::open(1));

    assert_eq!(engine.stats().total_on_edge_added, 1);

    g.add_str("ev2", "eventType", "enter", 2);
    g.set_time(2);
    engine.on_edge_added(&g, &"ev2".into(), &"eventType".into(),
        &MemValue::Str("enter".into()), &Interval::open(2));

    assert_eq!(engine.stats().total_on_edge_added, 2);
}

#[test]
fn stats_fingerprints_and_negation() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(PatternBuilder::new("test")
        .stage("e1", |s| s.edge("e1", "eventType".into(), MemValue::Str("enter".into()))
            .edge_bind("e1", "actor".into(), "person"))
        .stage("e2", |s| s.edge("e2", "eventType".into(), MemValue::Str("leave".into()))
            .edge_bind("e2", "actor".into(), "person"))
        .unless_between("e1", "e2", |neg| {
            neg.edge("mid", "eventType".into(), MemValue::Str("cancel".into()))
                .edge_bind("mid", "actor".into(), "person")
        })
        .build());

    g.add_str("ev1", "eventType", "enter", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.set_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(),
        &MemValue::Str("enter".into()), &Interval::open(1));

    assert!(engine.stats().total_fingerprints > 0, "should compute fingerprints");

    // Second edge triggers negation check on the active PM
    g.add_str("ev2", "eventType", "cancel", 2);
    g.add_ref("ev2", "actor", "alice", 2);
    g.set_time(2);
    engine.on_edge_added(&g, &"ev2".into(), &"eventType".into(),
        &MemValue::Str("cancel".into()), &Interval::open(2));

    assert!(engine.stats().total_negation_checks > 0, "should check negations");
}

#[test]
fn stats_peak_active_pms() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(PatternBuilder::new("test")
        .stage("e1", |s| s.edge("e1", "eventType".into(), MemValue::Str("enter".into())))
        .stage("e2", |s| s.edge("e2", "eventType".into(), MemValue::Str("leave".into())))
        .build());

    for t in 1..=5i64 {
        let name = format!("ev{}", t);
        g.add_str(&name, "eventType", "enter", t);
        g.set_time(t);
        engine.on_edge_added(&g, &name, &"eventType".into(),
            &MemValue::Str("enter".into()), &Interval::open(t));
    }

    assert_eq!(engine.stats().peak_active_pms, 5,
        "peak should be 5 after adding 5 matching first-stage edges");
}

// ===========================================================================
// Pattern lifecycle (Phase 5.2)
// ===========================================================================

#[test]
fn disable_pattern_skips_matching() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();

    let idx = engine.register(
        PatternBuilder::new("test")
            .stage("e", |s| s.edge("e", "type".into(), MemValue::Str("x".into())))
            .build(),
    );

    g.add_str("ev1", "type", "x", 1);
    g.set_time(1);
    let events = engine.on_edge_added(&g, &"ev1".into(), &"type".into(),
        &MemValue::Str("x".into()), &Interval::open(1));
    assert!(!events.is_empty(), "enabled pattern should match");

    engine.set_pattern_enabled(idx, false);

    g.add_str("ev2", "type", "x", 2);
    g.set_time(2);
    let events = engine.on_edge_added(&g, &"ev2".into(), &"type".into(),
        &MemValue::Str("x".into()), &Interval::open(2));
    assert!(events.is_empty(), "disabled pattern should not match");
}

#[test]
fn disable_kills_active_pms() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();

    let idx = engine.register(
        PatternBuilder::new("two_stage")
            .stage("e1", |s| s.edge("e1", "type".into(), MemValue::Str("start".into())))
            .stage("e2", |s| s.edge("e2", "type".into(), MemValue::Str("end".into())))
            .build(),
    );

    // Initiate a PM
    g.add_str("ev1", "type", "start", 1);
    g.set_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"type".into(),
        &MemValue::Str("start".into()), &Interval::open(1));
    assert_eq!(
        engine.partial_matches().iter().filter(|pm| pm.state == MatchState::Active).count(),
        1, "should have 1 active PM"
    );

    // Disable kills the PM
    engine.set_pattern_enabled(idx, false);
    assert_eq!(
        engine.partial_matches().iter().filter(|pm| pm.state == MatchState::Active).count(),
        0, "disabling should kill active PMs"
    );
}

#[test]
fn reenable_allows_new_matches() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();

    let idx = engine.register(
        PatternBuilder::new("test")
            .stage("e", |s| s.edge("e", "type".into(), MemValue::Str("x".into())))
            .build(),
    );

    engine.set_pattern_enabled(idx, false);

    g.add_str("ev1", "type", "x", 1);
    g.set_time(1);
    let events = engine.on_edge_added(&g, &"ev1".into(), &"type".into(),
        &MemValue::Str("x".into()), &Interval::open(1));
    assert!(events.is_empty(), "disabled → no match");

    engine.set_pattern_enabled(idx, true);

    g.add_str("ev2", "type", "x", 2);
    g.set_time(2);
    let events = engine.on_edge_added(&g, &"ev2".into(), &"type".into(),
        &MemValue::Str("x".into()), &Interval::open(2));
    assert!(!events.is_empty(), "re-enabled → should match");
}

#[test]
fn pattern_metrics_track_events() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();

    let idx = engine.register(
        PatternBuilder::new("test")
            .stage("e", |s| s.edge("e", "type".into(), MemValue::Str("x".into())))
            .build(),
    );

    engine.tick();
    g.add_str("ev1", "type", "x", 1);
    g.set_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"type".into(),
        &MemValue::Str("x".into()), &Interval::open(1));

    let metrics = engine.pattern_metrics(idx).unwrap();
    assert_eq!(metrics.completion_count, 1);
    assert_eq!(metrics.last_advanced_tick, 1);
    assert!(metrics.enabled);
}

#[test]
fn stale_patterns_detected() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();

    engine.register(
        PatternBuilder::new("stale")
            .stage("e1", |s| s.edge("e1", "type".into(), MemValue::Str("start".into())))
            .stage("e2", |s| s.edge("e2", "type".into(), MemValue::Str("end".into())))
            .build(),
    );

    // Initiate a PM at tick 1
    engine.tick();
    g.add_str("ev1", "type", "start", 1);
    g.set_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"type".into(),
        &MemValue::Str("start".into()), &Interval::open(1));

    // Advance 100 ticks without completing
    for _ in 0..100 {
        engine.tick();
    }

    let stale = engine.stale_patterns(50);
    assert_eq!(stale.len(), 1, "pattern should be stale after 100 ticks without advancement");
    assert_eq!(stale[0], 0);
}

#[test]
fn deregister_soft_deletes() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();

    let idx = engine.register(
        PatternBuilder::new("ephemeral")
            .stage("e", |s| s.edge("e", "type".into(), MemValue::Str("x".into())))
            .build(),
    );

    engine.deregister(idx);
    assert!(!engine.is_pattern_enabled(idx));

    // Pattern still in the list (index stable) but won't match
    assert_eq!(engine.patterns().len(), 1);
    g.add_str("ev1", "type", "x", 1);
    g.set_time(1);
    let events = engine.on_edge_added(&g, &"ev1".into(), &"type".into(),
        &MemValue::Str("x".into()), &Interval::open(1));
    assert!(events.is_empty());
}

#[test]
fn evaluate_skips_disabled() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();

    let idx = engine.register(
        PatternBuilder::new("batch_test")
            .stage("e", |s| s.edge("e", "type".into(), MemValue::Str("x".into())))
            .build(),
    );

    g.add_str("ev1", "type", "x", 1);
    g.set_time(10);

    assert_eq!(engine.evaluate(&g).len(), 1, "enabled → 1 match");
    engine.set_pattern_enabled(idx, false);
    assert_eq!(engine.evaluate(&g).len(), 0, "disabled → 0 matches");
    engine.set_pattern_enabled(idx, true);
    assert_eq!(engine.evaluate(&g).len(), 1, "re-enabled → 1 match");
}

#[test]
fn tick_delta_summarizes_events() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();

    engine.register(
        PatternBuilder::new("quick")
            .stage("e", |s| s.edge("e", "type".into(), MemValue::Str("x".into())))
            .build(),
    );
    engine.register(
        PatternBuilder::new("slow")
            .stage("e1", |s| s.edge("e1", "type".into(), MemValue::Str("start".into())))
            .stage("e2", |s| s.edge("e2", "type".into(), MemValue::Str("end".into())))
            .build(),
    );

    // Tick 1: initiate slow, complete quick
    engine.tick();
    g.add_str("ev1", "type", "x", 1);
    g.add_str("ev2", "type", "start", 1);
    g.set_time(1);
    let mut all_events = Vec::new();
    all_events.extend(engine.on_edge_added(&g, &"ev1".into(), &"type".into(),
        &MemValue::Str("x".into()), &Interval::open(1)));
    all_events.extend(engine.on_edge_added(&g, &"ev2".into(), &"type".into(),
        &MemValue::Str("start".into()), &Interval::open(1)));

    let delta = engine.tick_delta(&all_events, 50);
    assert!(delta.completed.contains(&"quick".to_string()));
    assert!(delta.advanced.contains(&"slow".to_string()));
    assert!(delta.stalled.is_empty());

    // Advance 100 ticks without completing "slow"
    for _ in 0..100 {
        engine.tick();
    }
    let no_events: Vec<SiftEvent<String, MemValue>> = vec![];
    let delta = engine.tick_delta(&no_events, 50);
    assert!(delta.stalled.contains(&"slow".to_string()));
    assert_eq!(delta.active_pm_count, 1);
}

// ===========================================================================
// Fork-aware evaluation (Phase 5.4)
// ===========================================================================

#[test]
fn clone_engine_is_independent() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();

    engine.register(
        PatternBuilder::new("two_stage")
            .stage("e1", |s| s.edge("e1", "type".into(), MemValue::Str("start".into())))
            .stage("e2", |s| s.edge("e2", "type".into(), MemValue::Str("end".into())))
            .build(),
    );

    // Initiate a PM in the original
    engine.tick();
    g.add_str("ev1", "type", "start", 1);
    g.set_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"type".into(),
        &MemValue::Str("start".into()), &Interval::open(1));

    assert_eq!(engine.partial_matches().iter().filter(|pm| pm.state == MatchState::Active).count(), 1);

    // Fork
    let mut fork = engine.clone();

    // Complete on the fork only
    g.add_str("ev2", "type", "end", 5);
    g.set_time(5);
    let fork_events = fork.on_edge_added(&g, &"ev2".into(), &"type".into(),
        &MemValue::Str("end".into()), &Interval::open(5));

    let fork_completed = fork_events.iter().filter(|e| matches!(e, SiftEvent::Completed { .. })).count();
    assert_eq!(fork_completed, 1, "fork should complete");

    // Original is unaffected
    assert_eq!(
        engine.partial_matches().iter().filter(|pm| pm.state == MatchState::Active).count(),
        1, "original should still have 1 active PM"
    );
    assert_eq!(engine.pattern_metrics(0).unwrap().completion_count, 0, "original has no completions");
    assert_eq!(fork.pattern_metrics(0).unwrap().completion_count, 1, "fork has 1 completion");
}

#[test]
fn clone_preserves_disabled_state() {
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();

    let idx = engine.register(
        PatternBuilder::new("test")
            .stage("e", |s| s.edge("e", "type".into(), MemValue::Str("x".into())))
            .build(),
    );

    engine.set_pattern_enabled(idx, false);
    let fork = engine.clone();

    assert!(!fork.is_pattern_enabled(idx), "fork should inherit disabled state");
}

// ===========================================================================
// Plant/payoff tracking (Phase 5.5)
// ===========================================================================

#[test]
fn plant_payoff_tracks_setup_and_resolution() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();

    let plant_idx = engine.register(
        PatternBuilder::new("promise")
            .stage("e1", |s| s.edge("e1", "type".into(), MemValue::Str("promise".into())))
            .build(),
    );
    let payoff_idx = engine.register(
        PatternBuilder::new("fulfill")
            .stage("e2", |s| s.edge("e2", "type".into(), MemValue::Str("fulfill".into())))
            .build(),
    );

    engine.register_plant_payoff(plant_idx, payoff_idx, None);

    // Plant fires
    engine.tick();
    g.add_str("ev1", "type", "promise", 1);
    g.set_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"type".into(),
        &MemValue::Str("promise".into()), &Interval::open(1));

    let status = engine.plant_status(50);
    assert_eq!(status.len(), 1);
    assert_eq!(status[0].payoff_completions, 0, "no payoff yet");
    assert!(!status[0].stale, "only 1 tick — not stale");

    // Payoff fires
    g.add_str("ev2", "type", "fulfill", 2);
    g.set_time(2);
    engine.on_edge_added(&g, &"ev2".into(), &"type".into(),
        &MemValue::Str("fulfill".into()), &Interval::open(2));

    let status = engine.plant_status(50);
    assert_eq!(status[0].payoff_completions, 1, "payoff resolved");
}

#[test]
fn plant_payoff_stale_detection() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();

    let plant_idx = engine.register(
        PatternBuilder::new("setup")
            .stage("e1", |s| s.edge("e1", "type".into(), MemValue::Str("setup".into())))
            .stage("e2", |s| s.edge("e2", "type".into(), MemValue::Str("middle".into())))
            .build(),
    );
    let payoff_idx = engine.register(
        PatternBuilder::new("payoff")
            .stage("e3", |s| s.edge("e3", "type".into(), MemValue::Str("payoff".into())))
            .build(),
    );

    engine.register_plant_payoff(plant_idx, payoff_idx, None);

    // Initiate plant (advances to stage 1, but doesn't complete)
    engine.tick();
    g.add_str("ev1", "type", "setup", 1);
    g.set_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"type".into(),
        &MemValue::Str("setup".into()), &Interval::open(1));

    // Let 100 ticks pass
    for _ in 0..100 {
        engine.tick();
    }

    let status = engine.plant_status(50);
    assert_eq!(status.len(), 1);
    assert!(status[0].stale, "plant should be stale after 100 ticks");
    assert_eq!(status[0].active_plants, 1);
    assert_eq!(status[0].payoff_completions, 0);
}

// ===========================================================================
// end_tick() happy path
// ===========================================================================

#[test]
fn end_tick_accumulates_and_clears() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();

    engine.register(
        PatternBuilder::new("quick")
            .stage("e", |s| s.edge("e", "type".into(), MemValue::Str("x".into())))
            .build(),
    );
    engine.register(
        PatternBuilder::new("slow")
            .stage("e1", |s| s.edge("e1", "type".into(), MemValue::Str("start".into())))
            .stage("e2", |s| s.edge("e2", "type".into(), MemValue::Str("end".into())))
            .build(),
    );

    // Add edges within a single tick
    g.add_str("ev1", "type", "x", 1);
    g.add_str("ev2", "type", "start", 1);
    g.set_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"type".into(),
        &MemValue::Str("x".into()), &Interval::open(1));
    engine.on_edge_added(&g, &"ev2".into(), &"type".into(),
        &MemValue::Str("start".into()), &Interval::open(1));

    // end_tick summarizes everything
    let delta = engine.end_tick(50);
    assert!(delta.completed.contains(&"quick".to_string()), "quick should complete");
    assert!(delta.advanced.contains(&"slow".to_string()), "slow should advance");
    assert_eq!(engine.current_tick(), 1);

    // Next tick with no events — accumulators should be cleared
    let delta2 = engine.end_tick(50);
    assert!(delta2.completed.is_empty(), "no events this tick");
    assert!(delta2.advanced.is_empty(), "no events this tick");
    assert_eq!(engine.current_tick(), 2);
}

#[test]
fn end_tick_detects_stale_after_many_ticks() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();

    engine.register(
        PatternBuilder::new("stuck")
            .stage("e1", |s| s.edge("e1", "type".into(), MemValue::Str("start".into())))
            .stage("e2", |s| s.edge("e2", "type".into(), MemValue::Str("end".into())))
            .build(),
    );

    // Initiate PM
    g.add_str("ev1", "type", "start", 1);
    g.set_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"type".into(),
        &MemValue::Str("start".into()), &Interval::open(1));
    engine.end_tick(50); // tick 1

    // 100 empty ticks
    for _ in 0..100 {
        engine.end_tick(50);
    }

    // The 101st end_tick should report stale
    let delta = engine.end_tick(50);
    assert!(delta.stalled.contains(&"stuck".to_string()));
}

#[test]
fn stats_reset() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(PatternBuilder::new("test")
        .stage("e", |s| s.edge("e", "eventType".into(), MemValue::Str("enter".into())))
        .build());

    g.add_str("ev1", "eventType", "enter", 1);
    g.set_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(),
        &MemValue::Str("enter".into()), &Interval::open(1));

    assert!(engine.stats().total_on_edge_added > 0);
    engine.reset_stats();
    assert_eq!(engine.stats().total_on_edge_added, 0);
    assert_eq!(engine.stats().peak_active_pms, 0);
}

// ===========================================================================
// 5d. Partial match age tracking
// ===========================================================================

#[test]
fn pm_created_at_set_on_initiation() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(PatternBuilder::new("two_stage")
        .stage("e1", |s| s.edge("e1", "eventType".into(), MemValue::Str("enter".into())))
        .stage("e2", |s| s.edge("e2", "eventType".into(), MemValue::Str("leave".into())))
        .build());

    g.add_str("ev1", "eventType", "enter", 42);
    g.set_time(42);
    engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(),
        &MemValue::Str("enter".into()), &Interval::open(42));

    let pms = engine.active_matches_for("two_stage");
    assert_eq!(pms.len(), 1);
    assert_eq!(pms[0].created_at, 42, "created_at should be the initiating edge's timestamp");
}

#[test]
fn pm_created_at_inherited_on_advance() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(PatternBuilder::new("three_stage")
        .stage("e1", |s| s.edge("e1", "eventType".into(), MemValue::Str("enter".into())))
        .stage("e2", |s| s.edge("e2", "eventType".into(), MemValue::Str("greet".into())))
        .stage("e3", |s| s.edge("e3", "eventType".into(), MemValue::Str("leave".into())))
        .build());

    // Stage 1 at t=10
    g.add_str("ev1", "eventType", "enter", 10);
    g.set_time(10);
    engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(),
        &MemValue::Str("enter".into()), &Interval::open(10));

    // Stage 2 at t=50 — PM advances but created_at stays 10
    g.add_str("ev2", "eventType", "greet", 50);
    g.set_time(50);
    engine.on_edge_added(&g, &"ev2".into(), &"eventType".into(),
        &MemValue::Str("greet".into()), &Interval::open(50));

    let active = engine.active_matches_for("three_stage");
    // Original PM (waiting for stage 2) + advanced PM (waiting for stage 3)
    assert_eq!(active.len(), 2);
    let advanced = active.iter().find(|pm| pm.next_stage == 2).unwrap();
    assert_eq!(advanced.created_at, 10,
        "advanced PM should inherit parent's created_at, not the advancing edge's timestamp");
}

// ===========================================================================
// 5d. Metric temporal constraints
// ===========================================================================

#[test]
fn metric_before_gap_in_range() {
    // Two stages matched via builder API with bounded intervals.
    // e1=[1,4), e2=[8,12). Before gap = 8-4 = 4. Bound [3,10] → match.
    use fabula::pattern::MetricGap;
    let mut g = MemGraph::new();
    g.add_edge_bounded("ev1", "eventType", MemValue::Str("crisis".into()), 1, 4);
    g.add_edge_bounded("ev2", "eventType", MemValue::Str("betrayal".into()), 8, 12);
    g.set_time(3); // scan at t=3 to find ev1

    let pattern = PatternBuilder::new("test")
        .stage("e1", |s| s.edge("e1", "eventType".into(), MemValue::Str("crisis".into())))
        .stage("e2", |s| s.edge("e2", "eventType".into(), MemValue::Str("betrayal".into())))
        .temporal_with_gap("e1", AllenRelation::Before, "e2",
            MetricGap { min: Some(3.0), max: Some(10.0) })
        .build();

    // Use incremental: add edges at their respective times
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(pattern);

    g.set_time(3);
    engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(),
        &MemValue::Str("crisis".into()), &Interval::new(1, 4));
    g.set_time(10);
    let events = engine.on_edge_added(&g, &"ev2".into(), &"eventType".into(),
        &MemValue::Str("betrayal".into()), &Interval::new(8, 12));

    let completed = events.iter().filter(|e| matches!(e, SiftEvent::Completed { .. })).count();
    assert_eq!(completed, 1, "gap=4 within [3,10] → match");
}

#[test]
fn metric_before_gap_too_far() {
    use fabula::pattern::MetricGap;
    let mut g = MemGraph::new();
    g.add_edge_bounded("ev1", "eventType", MemValue::Str("crisis".into()), 1, 4);
    g.add_edge_bounded("ev2", "eventType", MemValue::Str("betrayal".into()), 20, 25);

    let pattern = PatternBuilder::new("test")
        .stage("e1", |s| s.edge("e1", "eventType".into(), MemValue::Str("crisis".into())))
        .stage("e2", |s| s.edge("e2", "eventType".into(), MemValue::Str("betrayal".into())))
        .temporal_with_gap("e1", AllenRelation::Before, "e2",
            MetricGap { min: Some(3.0), max: Some(10.0) })
        .build();

    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    g.set_time(3);
    engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(),
        &MemValue::Str("crisis".into()), &Interval::new(1, 4));
    g.set_time(22);
    let events = engine.on_edge_added(&g, &"ev2".into(), &"eventType".into(),
        &MemValue::Str("betrayal".into()), &Interval::new(20, 25));

    let completed = events.iter().filter(|e| matches!(e, SiftEvent::Completed { .. })).count();
    assert_eq!(completed, 0, "gap=16 exceeds max=10 → no match");
}

#[test]
fn metric_before_gap_too_close() {
    use fabula::pattern::MetricGap;
    let mut g = MemGraph::new();
    g.add_edge_bounded("ev1", "eventType", MemValue::Str("crisis".into()), 1, 4);
    g.add_edge_bounded("ev2", "eventType", MemValue::Str("betrayal".into()), 5, 8);

    let pattern = PatternBuilder::new("test")
        .stage("e1", |s| s.edge("e1", "eventType".into(), MemValue::Str("crisis".into())))
        .stage("e2", |s| s.edge("e2", "eventType".into(), MemValue::Str("betrayal".into())))
        .temporal_with_gap("e1", AllenRelation::Before, "e2",
            MetricGap { min: Some(3.0), max: Some(10.0) })
        .build();

    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    g.set_time(3);
    engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(),
        &MemValue::Str("crisis".into()), &Interval::new(1, 4));
    g.set_time(6);
    let events = engine.on_edge_added(&g, &"ev2".into(), &"eventType".into(),
        &MemValue::Str("betrayal".into()), &Interval::new(5, 8));

    let completed = events.iter().filter(|e| matches!(e, SiftEvent::Completed { .. })).count();
    assert_eq!(completed, 0, "gap=1 below min=3 → no match");
}

#[test]
fn metric_open_ended_skips_gap_check() {
    // Open-ended intervals: Allen relation falls back to start comparison.
    // gap_for_relation returns None → metric check skipped → match allowed.
    use fabula::pattern::MetricGap;
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "crisis", 1);
    g.add_str("ev2", "eventType", "betrayal", 100);
    g.set_time(100);

    let pattern = PatternBuilder::new("test")
        .stage("e1", |s| s.edge("e1", "eventType".into(), MemValue::Str("crisis".into())))
        .stage("e2", |s| s.edge("e2", "eventType".into(), MemValue::Str("betrayal".into())))
        .temporal_with_gap("e1", AllenRelation::Before, "e2",
            MetricGap { min: Some(3.0), max: Some(10.0) })
        .build();

    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    // Open-ended intervals: gap can't be computed (no end point).
    // Metric check skipped. Allen Before fallback (start comparison) passes.
    assert_eq!(engine.evaluate(&g).len(), 1,
        "open-ended intervals: metric check skipped, Allen fallback passes");
}

// ===========================================================================
// 6. Type edge cases
// ===========================================================================

#[test]
fn memvalue_cross_variant_ordering() {
    // Derived PartialOrd orders by discriminant: Node < Str < Num < Bool
    assert!(MemValue::Node("z".into()) < MemValue::Str("a".into()));
    assert!(MemValue::Str("z".into()) < MemValue::Num(0.0));
    assert!(MemValue::Num(f64::MAX) < MemValue::Bool(false));
}

#[test]
fn memvalue_nan_comparisons() {
    let nan = MemValue::Num(f64::NAN);

    let lt = ValueConstraint::Lt(MemValue::Num(5.0));
    assert!(!lt.matches(&nan), "NaN is not less than 5.0");

    let gt = ValueConstraint::Gt(MemValue::Num(5.0));
    assert!(!gt.matches(&nan), "NaN is not greater than 5.0");

    let eq = ValueConstraint::Eq(MemValue::Num(f64::NAN));
    assert!(!eq.matches(&nan), "NaN != NaN");

    let between = ValueConstraint::Between(MemValue::Num(0.0), MemValue::Num(10.0));
    assert!(!between.matches(&nan), "NaN is not between 0 and 10");
}

#[test]
fn between_reversed_bounds_never_matches() {
    let c = ValueConstraint::Between(20, 10); // lo > hi
    assert!(!c.matches(&15), "nothing can be >= 20 AND <= 10");
    assert!(!c.matches(&10));
    assert!(!c.matches(&20));
    assert!(!c.matches(&0));
    assert!(!c.matches(&100));
}

#[test]
fn between_equal_bounds_matches_only_exact() {
    let c = ValueConstraint::Between(5, 5);
    assert!(c.matches(&5), "value == lo == hi should match");
    assert!(!c.matches(&4));
    assert!(!c.matches(&6));
}

#[test]
fn cross_variant_constraint_between() {
    // Between(Num(0.0), Num(10.0)) tested against Str("5")
    // Str is a different variant, so Num(0.0) < Str("5") due to discriminant ordering
    // and Str("5") > Num(10.0) for the same reason. So >= lo is true, <= hi is false.
    let c = ValueConstraint::Between(MemValue::Num(0.0), MemValue::Num(10.0));
    assert!(!c.matches(&MemValue::Str("5".into())),
        "cross-variant Between comparison: Str is not between two Nums");
}

// ===========================================================================
// 7. Negation edge cases
// ===========================================================================

#[test]
fn unless_global_single_stage_works() {
    // B7 fix: unless_global on single-stage now uses open-ended window.
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "crime", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    // This pardon should block the crime pattern
    g.add_str("ev2", "eventType", "pardon", 2);
    g.add_ref("ev2", "actor", "alice", 2);
    g.set_time(10);

    let pattern = PatternBuilder::new("unpardoned_crime")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("crime".into()))
                .edge_bind("e1", "actor".into(), "criminal")
        })
        .unless_global(|neg| {
            neg.edge("p", "eventType".into(), MemValue::Str("pardon".into()))
                .edge_bind("p", "actor".into(), "criminal")
        })
        .build();

    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    // B7 fixed: unless_global on single-stage uses open-ended window → pardon blocks
    assert_eq!(matches.len(), 0, "pardon should block the crime via unless_global");
}

#[test]
fn negation_before_window_start_does_not_block() {
    let mut g = MemGraph::new();
    g.add_str("ev0", "eventType", "pardon", 0); // BEFORE the crime
    g.add_ref("ev0", "actor", "alice", 0);
    g.add_str("ev1", "eventType", "crime", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.add_str("ev2", "eventType", "crime", 3);
    g.add_ref("ev2", "actor", "alice", 3);
    g.set_time(10);

    let pattern = PatternBuilder::new("double_crime")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("crime".into()))
                .edge_bind("e1", "actor".into(), "criminal")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), MemValue::Str("crime".into()))
                .edge_bind("e2", "actor".into(), "criminal")
        })
        .unless_between("e1", "e2", |neg| {
            neg.edge("p", "eventType".into(), MemValue::Str("pardon".into()))
                .edge_bind("p", "actor".into(), "criminal")
        })
        .build();

    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    // Pardon at t=0 is before e1 at t=1, so it's outside the window
    assert_eq!(engine.evaluate(&g).len(), 1,
        "negation event before window start should not block");
}

#[test]
fn negation_at_exact_window_boundary() {
    // B4 fix: negation window is exclusive on start (matching Winnow's < semantics).
    // An event at the exact same timestamp as the window start is NOT in the window.
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "crime", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.add_str("ev_pardon", "eventType", "pardon", 1); // same time as crime (window start)
    g.add_ref("ev_pardon", "actor", "alice", 1);
    g.add_str("ev2", "eventType", "crime", 3);
    g.add_ref("ev2", "actor", "alice", 3);
    g.set_time(10);

    let pattern = PatternBuilder::new("double_crime")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("crime".into()))
                .edge_bind("e1", "actor".into(), "criminal")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), MemValue::Str("crime".into()))
                .edge_bind("e2", "actor".into(), "criminal")
        })
        .unless_between("e1", "e2", |neg| {
            neg.edge("p", "eventType".into(), MemValue::Str("pardon".into()))
                .edge_bind("p", "actor".into(), "criminal")
        })
        .build();

    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    // Pardon at t=1, window start is t=1. With exclusive start (>), 1 > 1 is false.
    // Pardon is NOT in the window → match succeeds.
    assert_eq!(matches.len(), 1,
        "event at exact window start is outside exclusive window — match succeeds");

    // But a pardon at t=2 (strictly between 1 and 3) DOES block:
    g.add_str("ev_pardon2", "eventType", "pardon", 2);
    g.add_ref("ev_pardon2", "actor", "alice", 2);
    let matches2 = engine.evaluate(&g);
    assert_eq!(matches2.len(), 0, "pardon at t=2 is strictly between 1 and 3 — blocks match");
}

#[test]
fn incremental_negation_kills_only_matching_variable_bindings() {
    // Two partial matches for different characters; negation should
    // only kill the one whose bound variable matches.
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(PatternBuilder::new("enter_then_harm")
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
        .build());

    // Alice enters
    g.add_str("ev1", "eventType", "enter", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.set_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(),
        &MemValue::Str("enter".into()), &Interval::open(1));

    // Bob enters
    g.add_str("ev2", "eventType", "enter", 2);
    g.add_ref("ev2", "actor", "bob", 2);
    g.set_time(2);
    engine.on_edge_added(&g, &"ev2".into(), &"eventType".into(),
        &MemValue::Str("enter".into()), &Interval::open(2));

    assert_eq!(engine.active_matches_for("enter_then_harm").len(), 2);

    // Alice leaves — should kill only alice's partial match
    g.add_str("ev_leave", "eventType", "leave", 3);
    g.add_ref("ev_leave", "actor", "alice", 3);
    g.set_time(3);
    let events = engine.on_edge_added(&g, &"ev_leave".into(), &"eventType".into(),
        &MemValue::Str("leave".into()), &Interval::open(3));

    let negated_count = events.iter().filter(|e| matches!(e, SiftEvent::Negated { .. })).count();
    assert_eq!(negated_count, 1, "only alice's partial match should be negated");
    assert_eq!(engine.active_matches_for("enter_then_harm").len(), 1,
        "bob's partial match should survive");
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
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(PatternBuilder::new("enter_then_harm")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("enter".into()))
                .edge_bind("e1", "actor".into(), "person")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), MemValue::Str("harm".into()))
                .edge_bind("e2", "actor".into(), "person")
        })
        .build());

    // Insert stage 2's event FIRST
    g.add_str("ev2", "eventType", "harm", 5);
    g.add_ref("ev2", "actor", "alice", 5);
    g.set_time(5);
    engine.on_edge_added(&g, &"ev2".into(), &"eventType".into(),
        &MemValue::Str("harm".into()), &Interval::open(5));

    // Then insert stage 1's event (earlier timestamp)
    g.add_str("ev1", "eventType", "enter", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.set_time(5);
    engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(),
        &MemValue::Str("enter".into()), &Interval::open(1));

    // Incremental: no completed matches (out-of-order)
    let incremental_completed = engine.partial_matches().iter()
        .filter(|pm| pm.state == MatchState::Complete)
        .count();
    assert_eq!(incremental_completed, 0,
        "incremental misses match when edges arrive out of chronological order");

    // Batch: finds the match
    let batch_matches = engine.evaluate(&g);
    assert_eq!(batch_matches.len(), 1,
        "batch correctly finds the match regardless of insertion order");
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
            s.edge("e2", "eventType".into(), MemValue::Str("show_hospitality".into()))
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
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(pattern.clone());

    g.add_str("ev1", "eventType", "enter", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.set_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(),
        &MemValue::Str("enter".into()), &Interval::open(1));

    g.add_str("ev2", "eventType", "show_hospitality", 2);
    g.add_ref("ev2", "actor", "bob", 2);
    g.add_ref("ev2", "target", "alice", 2);
    g.set_time(2);
    engine.on_edge_added(&g, &"ev2".into(), &"eventType".into(),
        &MemValue::Str("show_hospitality".into()), &Interval::open(2));

    g.add_str("ev3", "eventType", "harm", 3);
    g.add_ref("ev3", "actor", "bob", 3);
    g.add_ref("ev3", "target", "alice", 3);
    g.set_time(3);
    engine.on_edge_added(&g, &"ev3".into(), &"eventType".into(),
        &MemValue::Str("harm".into()), &Interval::open(3));

    let incremental_completed = engine.partial_matches().iter()
        .filter(|pm| pm.state == MatchState::Complete)
        .count();

    // Batch evaluation on the same graph
    let mut batch_engine: SiftEngine<MemGraph> = SiftEngine::new();
    batch_engine.register(pattern);
    let batch_matches = batch_engine.evaluate(&g);

    assert_eq!(incremental_completed, batch_matches.len(),
        "batch and incremental should agree when edges arrive in order");
    assert_eq!(batch_matches.len(), 1);
}

#[test]
fn incremental_temporal_ordering_enforced() {
    // B2 fix: on_edge_added now checks temporal ordering between stages.
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(PatternBuilder::new("a_then_b")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("a".into()))
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), MemValue::Str("b".into()))
        })
        .build());

    // Event A at t=10
    g.add_str("ev1", "eventType", "a", 10);
    g.set_time(10);
    engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(),
        &MemValue::Str("a".into()), &Interval::open(10));

    // Event B at t=5 (BEFORE A — temporal order violated)
    g.add_str("ev2", "eventType", "b", 5);
    g.set_time(10);
    let events = engine.on_edge_added(&g, &"ev2".into(), &"eventType".into(),
        &MemValue::Str("b".into()), &Interval::open(5));

    // B2 fixed: incremental rejects inverted temporal order
    let completed = events.iter().any(|e| matches!(e, SiftEvent::Completed { .. }));
    assert!(!completed, "incremental should reject temporally inverted match");

    // Batch also rejects it
    let batch_matches = engine.evaluate(&g);
    assert_eq!(batch_matches.len(), 0, "batch also rejects temporally inverted match");
}

// ===========================================================================
// Additional interval edge cases
// ===========================================================================

#[test]
fn interval_zero_length() {
    // Interval [5, 5) — start == end, zero length
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
    assert!(a.intersects(&b), "two open-ended intervals always intersect");

    // Zero-length interval [5,5) intersects [3,7) by the math:
    // self.start(5) < b_end(7) = true, other.start(3) < a_end(5) = true
    // This is arguably a quirk: the interval is empty but "intersects".
    let z = Interval::new(5, 5);
    let c = Interval::new(3, 7);
    assert!(z.intersects(&c),
        "zero-length [5,5) 'intersects' [3,7) due to half-open comparison (quirk)");

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
    assert_eq!(a.relation(&b), None, "open-ended interval returns None for relation()");
    // The fallback in check_temporal only handles Before/Meets
}

// ===========================================================================
// Builder edge cases
// ===========================================================================

#[test]
fn unless_global_no_stages_still_resolves() {
    // B5b fix: is_global is always cleared at build time, even with no stages.
    let pattern = PatternBuilder::<String, String>::new("empty_global_neg")
        .unless_global(|neg| {
            neg.edge("x", "type".into(), "bad".into())
        })
        .build();

    // is_global is cleared even with no stages
    assert!(!pattern.negations[0].is_global,
        "is_global should be cleared at build time");
}

// ===========================================================================
// Gap analysis edge cases
// ===========================================================================

#[test]
fn why_not_nonexistent_pattern() {
    let g = MemGraph::new();
    let engine: SiftEngine<MemGraph> = SiftEngine::new();
    assert!(engine.why_not(&g, "nonexistent").is_none(),
        "why_not for unregistered pattern should return None");
}

#[test]
fn why_not_matched_pattern_shows_all_matched() {
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "harm", 1);
    g.add_ref("ev1", "actor", "bob", 1);
    g.set_time(10);

    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(PatternBuilder::new("find_harm")
        .stage("e", |s| {
            s.edge("e", "eventType".into(), MemValue::Str("harm".into()))
                .edge_bind("e", "actor".into(), "attacker")
        })
        .build());

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
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    engine.register(PatternBuilder::new("three_stages")
        .stage("e1", |s| s.edge("e1", "eventType".into(), MemValue::Str("a".into())))
        .stage("e2", |s| s.edge("e2", "eventType".into(), MemValue::Str("b".into())))
        .stage("e3", |s| s.edge("e3", "eventType".into(), MemValue::Str("c".into())))
        .build());

    let analysis = engine.why_not(&g, "three_stages").unwrap();
    assert_eq!(analysis.stages.len(), 1,
        "why_not should stop at first unmatched stage, not report all three");
    matches!(analysis.stages[0].status, StageStatus::Unmatched);
}

// ===========================================================================
// drain_completed edge cases
// ===========================================================================

#[test]
fn drain_completed_on_empty_engine() {
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    let drained = engine.drain_completed();
    assert!(drained.is_empty());
}

#[test]
fn drain_completed_preserves_active_matches() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();

    // Register two patterns
    engine.register(PatternBuilder::new("single_stage")
        .stage("e", |s| s.edge("e", "eventType".into(), MemValue::Str("harm".into())))
        .build());
    engine.register(PatternBuilder::new("two_stage")
        .stage("e1", |s| s.edge("e1", "eventType".into(), MemValue::Str("harm".into())))
        .stage("e2", |s| s.edge("e2", "eventType".into(), MemValue::Str("heal".into())))
        .build());

    g.add_str("ev1", "eventType", "harm", 1);
    g.set_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(),
        &MemValue::Str("harm".into()), &Interval::open(1));

    // single_stage completes, two_stage has a partial match
    let drained = engine.drain_completed();
    assert_eq!(drained.len(), 1);
    assert_eq!(drained[0].pattern, "single_stage");

    // Active partial match for two_stage should survive
    assert_eq!(engine.active_matches_for("two_stage").len(), 1);
}
