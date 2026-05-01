//! Edge case tests -- adversarial inputs, boundary conditions, and
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
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    let pattern = PatternBuilder::<String, MemValue>::new("empty").build();
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "empty pattern should never match"
    );
    let analysis = engine.why_not(&g, "empty").unwrap();
    assert!(
        analysis.stages.is_empty(),
        "empty pattern has no stages to analyze"
    );
}

#[test]
fn empty_stage_no_clauses() {
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "harm", 1);
    g.set_time(10);

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    // A stage with clauses followed by an empty stage
    let pattern = PatternBuilder::new("has_empty_stage")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("harm".into()))
        })
        .stage("e2", |s| s) // empty stage -- no clauses
        .build();
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "pattern with empty stage can never fully match"
    );
}

#[test]
fn empty_stage_incremental_never_advances() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    let pattern = PatternBuilder::new("empty_first_stage")
        .stage("e1", |s| s) // empty first stage
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), MemValue::Str("harm".into()))
        })
        .build();
    engine.register(pattern);

    g.add_str("ev1", "eventType", "harm", 1);
    g.set_time(1);
    let events = engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("harm".into()),
        &Interval::open(1),
    );
    assert!(
        events.is_empty(),
        "empty first stage means no edge can initiate a partial match"
    );
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

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        1,
        "empty negation body should not block matches"
    );
}

#[test]
fn empty_graph_with_registered_patterns() {
    let g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(
        PatternBuilder::new("find_harm")
            .stage("e", |s| {
                s.edge("e", "eventType".into(), MemValue::Str("harm".into()))
            })
            .build(),
    );
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

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(
        PatternBuilder::new("single")
            .stage("e", |s| {
                s.edge("e", "eventType".into(), MemValue::Str("harm".into()))
            })
            .build(),
    );
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
            neg.edge(
                "pardon_ev",
                "eventType".into(),
                MemValue::Str("pardon".into()),
            )
            .edge_bind("pardon_ev", "actor".into(), "criminal")
        })
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);

    // Batch correctly blocks: pardon exists after crime
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "batch should block match when negation event exists"
    );
}

#[test]
fn single_stage_with_unless_after_incremental_consistency() {
    // B3 fix: single-stage patterns now check negations before completing.
    // When no negation event exists yet, the match completes normally.
    // When a negation event already exists, the match is blocked.
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(
        PatternBuilder::new("unpardoned_crime")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("crime".into()))
                    .edge_bind("e1", "actor".into(), "criminal")
            })
            .unless_after("e1", |neg| {
                neg.edge(
                    "pardon_ev",
                    "eventType".into(),
                    MemValue::Str("pardon".into()),
                )
                .edge_bind("pardon_ev", "actor".into(), "criminal")
            })
            .build(),
    );

    // Crime arrives with no pardon existing -> completes (correct)
    g.add_str("ev1", "eventType", "crime", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.set_time(1);
    let events = engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("crime".into()),
        &Interval::open(1),
    );
    let completed = events
        .iter()
        .any(|e| matches!(e, SiftEvent::Completed { .. }));
    assert!(completed, "no pardon exists yet, match should complete");

    // Now test: if pardon exists BEFORE crime, match should be blocked
    let mut g2 = MemGraph::new();
    let mut engine2: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine2.register(
        PatternBuilder::new("unpardoned_crime")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("crime".into()))
                    .edge_bind("e1", "actor".into(), "criminal")
            })
            .unless_after("e1", |neg| {
                neg.edge(
                    "pardon_ev",
                    "eventType".into(),
                    MemValue::Str("pardon".into()),
                )
                .edge_bind("pardon_ev", "actor".into(), "criminal")
            })
            .build(),
    );

    // Pardon exists first
    g2.add_str("ev_pardon", "eventType", "pardon", 2);
    g2.add_ref("ev_pardon", "actor", "alice", 2);
    // Then crime
    g2.add_str("ev1", "eventType", "crime", 1);
    g2.add_ref("ev1", "actor", "alice", 1);
    g2.set_time(3);
    let events3 = engine2.on_edge_added(
        &g2,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("crime".into()),
        &Interval::open(1),
    );
    let completed2 = events3
        .iter()
        .any(|e| matches!(e, SiftEvent::Completed { .. }));
    assert!(
        !completed2,
        "pardon exists -- negation should block completion"
    );
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

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    // MemGraph stores both edges; scan returns duplicates; engine produces
    // duplicate matches with identical bindings.
    assert_eq!(
        matches.len(),
        2,
        "duplicate edges in MemGraph cause duplicate matches (known limitation)"
    );
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

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    // Same event (same start time) cannot satisfy both stages
    // due to strict temporal ordering: left.start >= right.start -> reject
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "same event (same timestamp) cannot satisfy two stages"
    );
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

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "strict temporal ordering rejects same-timestamp events in different stages"
    );
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

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "variable bound in stage 1 must match in stage 2"
    );
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
        .stage("e", |s| s.edge_bind("e", "enemy".into(), "target"))
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
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

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    // B8 fixed: only alice (self-loop) matches. Bob->Charlie is not a self-loop.
    assert_eq!(
        matches.len(),
        1,
        "only self-loop should match when source == target var"
    );
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

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1, "10-stage chain should match");
}

#[test]
fn distinct_events_create_distinct_pms() {
    // 100 different source nodes at different timestamps -- all unique fingerprints.
    // These are legitimately distinct PMs (different bindings + intervals).
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(
        PatternBuilder::new("two_stage")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("start".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "eventType".into(), MemValue::Str("finish".into()))
            })
            .build(),
    );

    for i in 0..100i64 {
        let name = format!("ev{}", i);
        g.add_str(&name, "eventType", "start", i + 1);
        g.set_time(i + 1);
        engine.on_edge_added(
            &g,
            &name,
            &"eventType".into(),
            &MemValue::Str("start".into()),
            &Interval::open(i + 1),
        );
    }

    assert_eq!(
        engine.active_matches_for("two_stage").len(),
        100,
        "100 distinct events (different nodes + timestamps) = 100 distinct PMs"
    );
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

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(
        matches.len(),
        1,
        "should find the one matching pair in 1000 edges"
    );
}

// ===========================================================================
// 5b. Dedup -- tests that demonstrate the accumulation bug (before fix)
