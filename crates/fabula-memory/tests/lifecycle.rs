use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

#[test]
fn disable_pattern_skips_matching() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

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
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

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
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

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
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

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
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

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
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

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
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

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
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

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
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

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
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

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
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

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
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

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
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

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
    let (delta, _) = engine.end_tick(50);
    assert!(delta.completed.contains(&"quick".to_string()), "quick should complete");
    assert!(delta.advanced.contains(&"slow".to_string()), "slow should advance");
    assert_eq!(engine.current_tick(), 1);

    // Next tick with no events — accumulators should be cleared
    let (delta2, _) = engine.end_tick(50);
    assert!(delta2.completed.is_empty(), "no events this tick");
    assert!(delta2.advanced.is_empty(), "no events this tick");
    assert_eq!(engine.current_tick(), 2);
}

#[test]
fn end_tick_detects_stale_after_many_ticks() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

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
    let _ = engine.end_tick(50); // tick 1

    // 100 empty ticks
    for _ in 0..100 {
        let _ = engine.end_tick(50);
    }

    // The 101st end_tick should report stale
    let (delta, _) = engine.end_tick(50);
    assert!(delta.stalled.contains(&"stuck".to_string()));
}

#[test]
fn stats_reset() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
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
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
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
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
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
// 5.2 Deadline-based expiration
// ===========================================================================

#[test]
fn pm_expires_after_deadline_ticks() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    engine.register(
        PatternBuilder::new("sla")
            .deadline(5)
            .stage("e1", |s| s.edge("e1", "type".into(), MemValue::Str("submit".into())))
            .stage("e2", |s| s.edge("e2", "type".into(), MemValue::Str("review".into())))
            .build(),
    );

    // Initiate a PM at tick 0 (end_tick increments to 1)
    g.add_str("ev1", "type", "submit", 1);
    g.set_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"type".into(),
        &MemValue::Str("submit".into()), &Interval::open(1));

    assert_eq!(engine.partial_matches().iter()
        .filter(|pm| pm.state == MatchState::Active).count(), 1);

    // Tick 1-5: PM is still alive
    for _ in 0..5 {
        let (delta, _) = engine.end_tick(50);
        assert!(delta.expired.is_empty(), "should not expire within deadline");
    }
    // PM was created at tick 0, now at tick 5: elapsed = 5, NOT > 5 yet
    assert_eq!(engine.partial_matches().iter()
        .filter(|pm| pm.state == MatchState::Active).count(), 1);

    // Tick 6: elapsed = 6 > 5, PM expires
    let (delta, expired_events) = engine.end_tick(50);
    assert!(delta.expired.contains(&"sla".to_string()), "should expire after deadline");
    assert_eq!(engine.partial_matches().iter()
        .filter(|pm| pm.state == MatchState::Active).count(), 0, "expired PM should be removed");

    // Verify the SiftEvent::Expired contents
    assert_eq!(expired_events.len(), 1);
    match &expired_events[0] {
        SiftEvent::Expired { pattern, stage_reached, ticks_elapsed, metadata, .. } => {
            assert_eq!(pattern, "sla");
            assert_eq!(*stage_reached, 1, "PM was at stage 1 (next_stage)");
            assert_eq!(*ticks_elapsed, 6);
            assert!(metadata.is_empty());
        }
        other => panic!("expected Expired event, got {:?}", other),
    }
}

#[test]
fn no_expiry_without_deadline() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    engine.register(
        PatternBuilder::new("no_deadline")
            .stage("e1", |s| s.edge("e1", "type".into(), MemValue::Str("start".into())))
            .stage("e2", |s| s.edge("e2", "type".into(), MemValue::Str("end".into())))
            .build(),
    );

    g.add_str("ev1", "type", "start", 1);
    g.set_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"type".into(),
        &MemValue::Str("start".into()), &Interval::open(1));

    // 200 ticks without completing — should never expire
    for _ in 0..200 {
        let (delta, _) = engine.end_tick(50);
        assert!(delta.expired.is_empty());
    }
    assert_eq!(engine.partial_matches().iter()
        .filter(|pm| pm.state == MatchState::Active).count(), 1);
}

#[test]
fn completed_before_deadline_no_expiry() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    // Single-stage pattern: completion removes the PM entirely.
    engine.register(
        PatternBuilder::new("fast")
            .deadline(10)
            .stage("e1", |s| s.edge("e1", "type".into(), MemValue::Str("done".into())))
            .build(),
    );

    // Complete at tick 0 (well within deadline)
    g.add_str("ev1", "type", "done", 1);
    g.set_time(1);
    let events = engine.on_edge_added(&g, &"ev1".into(), &"type".into(),
        &MemValue::Str("done".into()), &Interval::open(1));
    assert!(events.iter().any(|e| matches!(e, SiftEvent::Completed { .. })));

    // Drain the completed PM
    engine.drain_completed();

    // 20 more ticks — no expiry (PM already completed and drained)
    for _ in 0..20 {
        let (delta, _) = engine.end_tick(50);
        assert!(delta.expired.is_empty());
    }
}

#[test]
fn negation_kills_before_deadline() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    engine.register(
        PatternBuilder::new("negatable")
            .deadline(100)
            .stage("e1", |s| s.edge("e1", "type".into(), MemValue::Str("start".into())))
            .stage("e2", |s| s.edge("e2", "type".into(), MemValue::Str("end".into())))
            .unless_between("e1", "e2", |n|
                n.edge("mid", "type".into(), MemValue::Str("cancel".into())))
            .build(),
    );

    // Initiate
    g.add_str("ev1", "type", "start", 1);
    g.set_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"type".into(),
        &MemValue::Str("start".into()), &Interval::open(1));
    let _ = engine.end_tick(50);

    // Negate at tick 2
    g.add_str("mid1", "type", "cancel", 2);
    g.set_time(2);
    let events = engine.on_edge_added(&g, &"mid1".into(), &"type".into(),
        &MemValue::Str("cancel".into()), &Interval::open(2));
    assert!(events.iter().any(|e| matches!(e, SiftEvent::Negated { .. })));

    // PM is dead — no expiry should fire later
    for _ in 0..200 {
        let (delta, _) = engine.end_tick(50);
        assert!(delta.expired.is_empty(), "negated PM should not also expire");
    }
}

#[test]
fn deadline_created_at_tick_inherited_on_advance() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    engine.register(
        PatternBuilder::new("three_stage_deadline")
            .deadline(8)
            .stage("e1", |s| s.edge("e1", "type".into(), MemValue::Str("a".into())))
            .stage("e2", |s| s.edge("e2", "type".into(), MemValue::Str("b".into())))
            .stage("e3", |s| s.edge("e3", "type".into(), MemValue::Str("c".into())))
            .build(),
    );

    // Initiate at tick 0
    g.add_str("ev1", "type", "a", 1);
    g.set_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"type".into(),
        &MemValue::Str("a".into()), &Interval::open(1));

    // Advance to stage 2 at tick 3
    for _ in 0..3 {
        let _ = engine.end_tick(50);
    }
    g.add_str("ev2", "type", "b", 4);
    g.set_time(4);
    engine.on_edge_added(&g, &"ev2".into(), &"type".into(),
        &MemValue::Str("b".into()), &Interval::open(4));

    // 5 more ticks (total 8 from tick 3 → tick 8)
    // But created_at_tick = 0, so at tick 9: elapsed = 9 > 8, should expire
    for _ in 0..5 {
        let (delta, _) = engine.end_tick(50);
        assert!(delta.expired.is_empty());
    }
    // Now at tick 8, elapsed = 8, not > 8 yet
    // Tick 9: elapsed = 9 > 8
    let (delta, _) = engine.end_tick(50);
    assert!(delta.expired.contains(&"three_stage_deadline".to_string()),
        "should expire based on original creation tick, not advancement tick");
}

// ===========================================================================
// 5d. Metric temporal constraints
// ===========================================================================

