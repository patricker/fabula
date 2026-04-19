use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

#[test]
fn disable_pattern_skips_matching() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    let idx = engine.register(
        PatternBuilder::new("test")
            .stage("e", |s| {
                s.edge("e", "type".into(), MemValue::Str("x".into()))
            })
            .build(),
    );

    g.add_str("ev1", "type", "x", 1);
    g.set_time(1);
    let events = engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("x".into()),
        &Interval::open(1),
    );
    assert!(!events.is_empty(), "enabled pattern should match");

    engine.set_pattern_enabled(idx, false);

    g.add_str("ev2", "type", "x", 2);
    g.set_time(2);
    let events = engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"type".into(),
        &MemValue::Str("x".into()),
        &Interval::open(2),
    );
    assert!(events.is_empty(), "disabled pattern should not match");
}

#[test]
fn disable_kills_active_pms() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    let idx = engine.register(
        PatternBuilder::new("two_stage")
            .stage("e1", |s| {
                s.edge("e1", "type".into(), MemValue::Str("start".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("end".into()))
            })
            .build(),
    );

    // Initiate a PM
    g.add_str("ev1", "type", "start", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("start".into()),
        &Interval::open(1),
    );
    assert_eq!(
        engine
            .partial_matches()
            .iter()
            .filter(|pm| pm.state == MatchState::Active)
            .count(),
        1,
        "should have 1 active PM"
    );

    // Disable kills the PM
    engine.set_pattern_enabled(idx, false);
    assert_eq!(
        engine
            .partial_matches()
            .iter()
            .filter(|pm| pm.state == MatchState::Active)
            .count(),
        0,
        "disabling should kill active PMs"
    );
}

#[test]
fn reenable_allows_new_matches() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    let idx = engine.register(
        PatternBuilder::new("test")
            .stage("e", |s| {
                s.edge("e", "type".into(), MemValue::Str("x".into()))
            })
            .build(),
    );

    engine.set_pattern_enabled(idx, false);

    g.add_str("ev1", "type", "x", 1);
    g.set_time(1);
    let events = engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("x".into()),
        &Interval::open(1),
    );
    assert!(events.is_empty(), "disabled → no match");

    engine.set_pattern_enabled(idx, true);

    g.add_str("ev2", "type", "x", 2);
    g.set_time(2);
    let events = engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"type".into(),
        &MemValue::Str("x".into()),
        &Interval::open(2),
    );
    assert!(!events.is_empty(), "re-enabled → should match");
}

#[test]
fn pattern_metrics_track_events() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    let idx = engine.register(
        PatternBuilder::new("test")
            .stage("e", |s| {
                s.edge("e", "type".into(), MemValue::Str("x".into()))
            })
            .build(),
    );

    engine.tick();
    g.add_str("ev1", "type", "x", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("x".into()),
        &Interval::open(1),
    );

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
            .stage("e1", |s| {
                s.edge("e1", "type".into(), MemValue::Str("start".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("end".into()))
            })
            .build(),
    );

    // Initiate a PM at tick 1
    engine.tick();
    g.add_str("ev1", "type", "start", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("start".into()),
        &Interval::open(1),
    );

    // Advance 100 ticks without completing
    for _ in 0..100 {
        engine.tick();
    }

    let stale = engine.stale_patterns(50);
    assert_eq!(
        stale.len(),
        1,
        "pattern should be stale after 100 ticks without advancement"
    );
    assert_eq!(stale[0], 0);
}

#[test]
fn deregister_soft_deletes() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    let idx = engine.register(
        PatternBuilder::new("ephemeral")
            .stage("e", |s| {
                s.edge("e", "type".into(), MemValue::Str("x".into()))
            })
            .build(),
    );

    engine.deregister(idx);
    assert!(!engine.is_pattern_enabled(idx));

    // Pattern still in the list (index stable) but won't match
    assert_eq!(engine.patterns().len(), 1);
    g.add_str("ev1", "type", "x", 1);
    g.set_time(1);
    let events = engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("x".into()),
        &Interval::open(1),
    );
    assert!(events.is_empty());
}

#[test]
fn evaluate_skips_disabled() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    let idx = engine.register(
        PatternBuilder::new("batch_test")
            .stage("e", |s| {
                s.edge("e", "type".into(), MemValue::Str("x".into()))
            })
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
            .stage("e", |s| {
                s.edge("e", "type".into(), MemValue::Str("x".into()))
            })
            .build(),
    );
    engine.register(
        PatternBuilder::new("slow")
            .stage("e1", |s| {
                s.edge("e1", "type".into(), MemValue::Str("start".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("end".into()))
            })
            .build(),
    );

    // Tick 1: initiate slow, complete quick
    engine.tick();
    g.add_str("ev1", "type", "x", 1);
    g.add_str("ev2", "type", "start", 1);
    g.set_time(1);
    let mut all_events = Vec::new();
    all_events.extend(engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("x".into()),
        &Interval::open(1),
    ));
    all_events.extend(engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"type".into(),
        &MemValue::Str("start".into()),
        &Interval::open(1),
    ));

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
            .stage("e1", |s| {
                s.edge("e1", "type".into(), MemValue::Str("start".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("end".into()))
            })
            .build(),
    );

    // Initiate a PM in the original
    engine.tick();
    g.add_str("ev1", "type", "start", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("start".into()),
        &Interval::open(1),
    );

    assert_eq!(
        engine
            .partial_matches()
            .iter()
            .filter(|pm| pm.state == MatchState::Active)
            .count(),
        1
    );

    // Fork
    let mut fork = engine.clone();

    // Complete on the fork only
    g.add_str("ev2", "type", "end", 5);
    g.set_time(5);
    let fork_events = fork.on_edge_added(
        &g,
        &"ev2".into(),
        &"type".into(),
        &MemValue::Str("end".into()),
        &Interval::open(5),
    );

    let fork_completed = fork_events
        .iter()
        .filter(|e| matches!(e, SiftEvent::Completed { .. }))
        .count();
    assert_eq!(fork_completed, 1, "fork should complete");

    // Original is unaffected
    assert_eq!(
        engine
            .partial_matches()
            .iter()
            .filter(|pm| pm.state == MatchState::Active)
            .count(),
        1,
        "original should still have 1 active PM"
    );
    assert_eq!(
        engine.pattern_metrics(0).unwrap().completion_count,
        0,
        "original has no completions"
    );
    assert_eq!(
        fork.pattern_metrics(0).unwrap().completion_count,
        1,
        "fork has 1 completion"
    );
}

#[test]
fn clone_preserves_disabled_state() {
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    let idx = engine.register(
        PatternBuilder::new("test")
            .stage("e", |s| {
                s.edge("e", "type".into(), MemValue::Str("x".into()))
            })
            .build(),
    );

    engine.set_pattern_enabled(idx, false);
    let fork = engine.clone();

    assert!(
        !fork.is_pattern_enabled(idx),
        "fork should inherit disabled state"
    );
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
            .stage("e1", |s| {
                s.edge("e1", "type".into(), MemValue::Str("promise".into()))
            })
            .build(),
    );
    let payoff_idx = engine.register(
        PatternBuilder::new("fulfill")
            .stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("fulfill".into()))
            })
            .build(),
    );

    engine.register_plant_payoff(plant_idx, payoff_idx, None);

    // Plant fires
    engine.tick();
    g.add_str("ev1", "type", "promise", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("promise".into()),
        &Interval::open(1),
    );

    let status = engine.plant_status(50);
    assert_eq!(status.len(), 1);
    assert_eq!(status[0].payoff_completions, 0, "no payoff yet");
    assert!(!status[0].stale, "only 1 tick -- not stale");

    // Payoff fires
    g.add_str("ev2", "type", "fulfill", 2);
    g.set_time(2);
    engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"type".into(),
        &MemValue::Str("fulfill".into()),
        &Interval::open(2),
    );

    let status = engine.plant_status(50);
    assert_eq!(status[0].payoff_completions, 1, "payoff resolved");
}

#[test]
fn plant_payoff_stale_detection() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    let plant_idx = engine.register(
        PatternBuilder::new("setup")
            .stage("e1", |s| {
                s.edge("e1", "type".into(), MemValue::Str("setup".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("middle".into()))
            })
            .build(),
    );
    let payoff_idx = engine.register(
        PatternBuilder::new("payoff")
            .stage("e3", |s| {
                s.edge("e3", "type".into(), MemValue::Str("payoff".into()))
            })
            .build(),
    );

    engine.register_plant_payoff(plant_idx, payoff_idx, None);

    // Initiate plant (advances to stage 1, but doesn't complete)
    engine.tick();
    g.add_str("ev1", "type", "setup", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("setup".into()),
        &Interval::open(1),
    );

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
            .stage("e", |s| {
                s.edge("e", "type".into(), MemValue::Str("x".into()))
            })
            .build(),
    );
    engine.register(
        PatternBuilder::new("slow")
            .stage("e1", |s| {
                s.edge("e1", "type".into(), MemValue::Str("start".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("end".into()))
            })
            .build(),
    );

    // Add edges within a single tick
    g.add_str("ev1", "type", "x", 1);
    g.add_str("ev2", "type", "start", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("x".into()),
        &Interval::open(1),
    );
    engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"type".into(),
        &MemValue::Str("start".into()),
        &Interval::open(1),
    );

    // end_tick summarizes everything
    let (delta, _) = engine.end_tick(50);
    assert!(
        delta.completed.contains(&"quick".to_string()),
        "quick should complete"
    );
    assert!(
        delta.advanced.contains(&"slow".to_string()),
        "slow should advance"
    );
    assert_eq!(engine.current_tick(), 1);

    // Next tick with no events -- accumulators should be cleared
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
            .stage("e1", |s| {
                s.edge("e1", "type".into(), MemValue::Str("start".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("end".into()))
            })
            .build(),
    );

    // Initiate PM
    g.add_str("ev1", "type", "start", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("start".into()),
        &Interval::open(1),
    );
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
    engine.register(
        PatternBuilder::new("test")
            .stage("e", |s| {
                s.edge("e", "eventType".into(), MemValue::Str("enter".into()))
            })
            .build(),
    );

    g.add_str("ev1", "eventType", "enter", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("enter".into()),
        &Interval::open(1),
    );

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
    engine.register(
        PatternBuilder::new("two_stage")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("enter".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "eventType".into(), MemValue::Str("leave".into()))
            })
            .build(),
    );

    g.add_str("ev1", "eventType", "enter", 42);
    g.set_time(42);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("enter".into()),
        &Interval::open(42),
    );

    let pms = engine.active_matches_for("two_stage");
    assert_eq!(pms.len(), 1);
    assert_eq!(
        pms[0].created_at, 42,
        "created_at should be the initiating edge's timestamp"
    );
}

#[test]
fn pm_created_at_inherited_on_advance() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(
        PatternBuilder::new("three_stage")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("enter".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "eventType".into(), MemValue::Str("greet".into()))
            })
            .stage("e3", |s| {
                s.edge("e3", "eventType".into(), MemValue::Str("leave".into()))
            })
            .build(),
    );

    // Stage 1 at t=10
    g.add_str("ev1", "eventType", "enter", 10);
    g.set_time(10);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("enter".into()),
        &Interval::open(10),
    );

    // Stage 2 at t=50 -- PM advances but created_at stays 10
    g.add_str("ev2", "eventType", "greet", 50);
    g.set_time(50);
    engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &MemValue::Str("greet".into()),
        &Interval::open(50),
    );

    let active = engine.active_matches_for("three_stage");
    // Original PM (waiting for stage 2) + advanced PM (waiting for stage 3)
    assert_eq!(active.len(), 2);
    let advanced = active.iter().find(|pm| pm.next_stage == 2).unwrap();
    assert_eq!(
        advanced.created_at, 10,
        "advanced PM should inherit parent's created_at, not the advancing edge's timestamp"
    );
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
            .stage("e1", |s| {
                s.edge("e1", "type".into(), MemValue::Str("submit".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("review".into()))
            })
            .build(),
    );

    // Initiate a PM at tick 0 (end_tick increments to 1)
    g.add_str("ev1", "type", "submit", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("submit".into()),
        &Interval::open(1),
    );

    assert_eq!(
        engine
            .partial_matches()
            .iter()
            .filter(|pm| pm.state == MatchState::Active)
            .count(),
        1
    );

    // Tick 1-5: PM is still alive
    for _ in 0..5 {
        let (delta, _) = engine.end_tick(50);
        assert!(
            delta.expired.is_empty(),
            "should not expire within deadline"
        );
    }
    // PM was created at tick 0, now at tick 5: elapsed = 5, NOT > 5 yet
    assert_eq!(
        engine
            .partial_matches()
            .iter()
            .filter(|pm| pm.state == MatchState::Active)
            .count(),
        1
    );

    // Tick 6: elapsed = 6 > 5, PM expires
    let (delta, expired_events) = engine.end_tick(50);
    assert!(
        delta.expired.contains(&"sla".to_string()),
        "should expire after deadline"
    );
    assert_eq!(
        engine
            .partial_matches()
            .iter()
            .filter(|pm| pm.state == MatchState::Active)
            .count(),
        0,
        "expired PM should be removed"
    );

    // Verify the SiftEvent::Expired contents
    assert_eq!(expired_events.len(), 1);
    match &expired_events[0] {
        SiftEvent::Expired {
            pattern,
            stage_reached,
            ticks_elapsed,
            metadata,
            ..
        } => {
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
            .stage("e1", |s| {
                s.edge("e1", "type".into(), MemValue::Str("start".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("end".into()))
            })
            .build(),
    );

    g.add_str("ev1", "type", "start", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("start".into()),
        &Interval::open(1),
    );

    // 200 ticks without completing -- should never expire
    for _ in 0..200 {
        let (delta, _) = engine.end_tick(50);
        assert!(delta.expired.is_empty());
    }
    assert_eq!(
        engine
            .partial_matches()
            .iter()
            .filter(|pm| pm.state == MatchState::Active)
            .count(),
        1
    );
}

#[test]
fn completed_before_deadline_no_expiry() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    // Single-stage pattern: completion removes the PM entirely.
    engine.register(
        PatternBuilder::new("fast")
            .deadline(10)
            .stage("e1", |s| {
                s.edge("e1", "type".into(), MemValue::Str("done".into()))
            })
            .build(),
    );

    // Complete at tick 0 (well within deadline)
    g.add_str("ev1", "type", "done", 1);
    g.set_time(1);
    let events = engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("done".into()),
        &Interval::open(1),
    );
    assert!(events
        .iter()
        .any(|e| matches!(e, SiftEvent::Completed { .. })));

    // Drain the completed PM
    engine.drain_completed();

    // 20 more ticks -- no expiry (PM already completed and drained)
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
            .stage("e1", |s| {
                s.edge("e1", "type".into(), MemValue::Str("start".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("end".into()))
            })
            .unless_between("e1", "e2", |n| {
                n.edge("mid", "type".into(), MemValue::Str("cancel".into()))
            })
            .build(),
    );

    // Initiate
    g.add_str("ev1", "type", "start", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("start".into()),
        &Interval::open(1),
    );
    let _ = engine.end_tick(50);

    // Negate at tick 2
    g.add_str("mid1", "type", "cancel", 2);
    g.set_time(2);
    let events = engine.on_edge_added(
        &g,
        &"mid1".into(),
        &"type".into(),
        &MemValue::Str("cancel".into()),
        &Interval::open(2),
    );
    assert!(events
        .iter()
        .any(|e| matches!(e, SiftEvent::Negated { .. })));

    // PM is dead -- no expiry should fire later
    for _ in 0..200 {
        let (delta, _) = engine.end_tick(50);
        assert!(
            delta.expired.is_empty(),
            "negated PM should not also expire"
        );
    }
}

#[test]
fn deadline_created_at_tick_inherited_on_advance() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    engine.register(
        PatternBuilder::new("three_stage_deadline")
            .deadline(8)
            .stage("e1", |s| {
                s.edge("e1", "type".into(), MemValue::Str("a".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("b".into()))
            })
            .stage("e3", |s| {
                s.edge("e3", "type".into(), MemValue::Str("c".into()))
            })
            .build(),
    );

    // Initiate at tick 0
    g.add_str("ev1", "type", "a", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("a".into()),
        &Interval::open(1),
    );

    // Advance to stage 2 at tick 3
    for _ in 0..3 {
        let _ = engine.end_tick(50);
    }
    g.add_str("ev2", "type", "b", 4);
    g.set_time(4);
    engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"type".into(),
        &MemValue::Str("b".into()),
        &Interval::open(4),
    );

    // 5 more ticks (total 8 from tick 3 → tick 8)
    // But created_at_tick = 0, so at tick 9: elapsed = 9 > 8, should expire
    for _ in 0..5 {
        let (delta, _) = engine.end_tick(50);
        assert!(delta.expired.is_empty());
    }
    // Now at tick 8, elapsed = 8, not > 8 yet
    // Tick 9: elapsed = 9 > 8
    let (delta, _) = engine.end_tick(50);
    assert!(
        delta.expired.contains(&"three_stage_deadline".to_string()),
        "should expire based on original creation tick, not advancement tick"
    );
}

// ===========================================================================
// 5d. Cross-stage value comparison (BoundVar)
// ===========================================================================

#[test]
fn batch_cross_stage_gt_var_matches() {
    let mut g = MemGraph::new();
    g.add_str("ev1", "type", "order", 1);
    g.add_num("ev1", "price", 100.0, 1);
    g.add_str("ev2", "type", "order", 2);
    g.add_num("ev2", "price", 150.0, 2);
    g.set_time(10);

    let pattern = PatternBuilder::new("escalation")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), MemValue::Str("order".into()))
                .edge_bind("e1", "price".into(), "base_price")
        })
        .stage("e2", |s| {
            s.edge("e2", "type".into(), MemValue::Str("order".into()))
                .edge_gt_var("e2", "price".into(), "base_price")
        })
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1, "150 > 100 should match");
}

#[test]
fn batch_cross_stage_gt_var_no_match() {
    let mut g = MemGraph::new();
    g.add_str("ev1", "type", "order", 1);
    g.add_num("ev1", "price", 100.0, 1);
    g.add_str("ev2", "type", "order", 2);
    g.add_num("ev2", "price", 80.0, 2);
    g.set_time(10);

    let pattern = PatternBuilder::new("escalation")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), MemValue::Str("order".into()))
                .edge_bind("e1", "price".into(), "base_price")
        })
        .stage("e2", |s| {
            s.edge("e2", "type".into(), MemValue::Str("order".into()))
                .edge_gt_var("e2", "price".into(), "base_price")
        })
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 0, "80 > 100 should not match");
}

#[test]
fn incremental_cross_stage_gt_var() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    engine.register(
        PatternBuilder::new("escalation")
            .stage("e1", |s| {
                s.edge("e1", "type".into(), MemValue::Str("bid".into()))
                    .edge_bind("e1", "price".into(), "prev_price")
            })
            .stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("bid".into()))
                    .edge_gt_var("e2", "price".into(), "prev_price")
            })
            .build(),
    );

    // Tick 1: bid at 100
    g.add_str("ev1", "type", "bid", 1);
    g.add_num("ev1", "price", 100.0, 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("bid".into()),
        &Interval::open(1),
    );

    // Tick 2: bid at 150 -- should complete
    g.add_str("ev2", "type", "bid", 2);
    g.add_num("ev2", "price", 150.0, 2);
    g.set_time(2);
    let events = engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"type".into(),
        &MemValue::Str("bid".into()),
        &Interval::open(2),
    );

    let completed = events
        .iter()
        .filter(|e| matches!(e, SiftEvent::Completed { .. }))
        .count();
    assert_eq!(completed, 1, "150 > 100 should complete incrementally");
}

#[test]
fn cross_stage_eq_var_matches() {
    let mut g = MemGraph::new();
    g.add_str("ev1", "type", "invoice", 1);
    g.add_num("ev1", "amount", 500.0, 1);
    g.add_str("ev2", "type", "payment", 2);
    g.add_num("ev2", "amount", 500.0, 2);
    g.set_time(10);

    let pattern = PatternBuilder::new("exact_match")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), MemValue::Str("invoice".into()))
                .edge_bind("e1", "amount".into(), "expected")
        })
        .stage("e2", |s| {
            s.edge("e2", "type".into(), MemValue::Str("payment".into()))
                .edge_eq_var("e2", "amount".into(), "expected")
        })
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 1, "500 == 500 should match");
}

#[test]
fn cross_stage_var_node_type_mismatch_no_match() {
    // Variable bound to a Node, not a Value -- GtVar should fail resolution
    let mut g = MemGraph::new();
    g.add_str("ev1", "type", "action", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.add_str("ev2", "type", "action", 2);
    g.add_num("ev2", "score", 50.0, 2);
    g.set_time(10);

    // actor is bound to Node("alice"), but GtVar expects a Value
    let pattern = PatternBuilder::new("bad_comparison")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), MemValue::Str("action".into()))
                .edge_bind("e1", "actor".into(), "actor_ref")
        })
        .stage("e2", |s| {
            s.edge("e2", "type".into(), MemValue::Str("action".into()))
                .edge_gt_var("e2", "score".into(), "actor_ref")
        })
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "Node vs Value comparison should not match"
    );
}

#[test]
fn cross_stage_gt_boundary_equality_no_match() {
    // 100 > 100 should NOT match (strict >)
    let mut g = MemGraph::new();
    g.add_str("ev1", "type", "order", 1);
    g.add_num("ev1", "price", 100.0, 1);
    g.add_str("ev2", "type", "order", 2);
    g.add_num("ev2", "price", 100.0, 2);
    g.set_time(10);

    let pattern = PatternBuilder::new("escalation")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), MemValue::Str("order".into()))
                .edge_bind("e1", "price".into(), "base")
        })
        .stage("e2", |s| {
            s.edge("e2", "type".into(), MemValue::Str("order".into()))
                .edge_gt_var("e2", "price".into(), "base")
        })
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "100 > 100 should not match (strict >)"
    );
}

#[test]
fn cross_stage_gte_boundary_equality_matches() {
    // 100 >= 100 SHOULD match
    let mut g = MemGraph::new();
    g.add_str("ev1", "type", "order", 1);
    g.add_num("ev1", "price", 100.0, 1);
    g.add_str("ev2", "type", "order", 2);
    g.add_num("ev2", "price", 100.0, 2);
    g.set_time(10);

    let pattern = PatternBuilder::new("at_least")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), MemValue::Str("order".into()))
                .edge_bind("e1", "price".into(), "base")
        })
        .stage("e2", |s| {
            s.edge("e2", "type".into(), MemValue::Str("order".into()))
                .edge_gte_var("e2", "price".into(), "base")
        })
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 1, "100 >= 100 should match");
}

#[test]
fn cross_stage_lte_matches() {
    let mut g = MemGraph::new();
    g.add_str("ev1", "type", "check", 1);
    g.add_num("ev1", "limit", 50.0, 1);
    g.add_str("ev2", "type", "check", 2);
    g.add_num("ev2", "val", 30.0, 2);
    g.set_time(10);

    let pattern = PatternBuilder::new("under_limit")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), MemValue::Str("check".into()))
                .edge_bind("e1", "limit".into(), "cap")
        })
        .stage("e2", |s| {
            s.edge("e2", "type".into(), MemValue::Str("check".into()))
                .edge_lte_var("e2", "val".into(), "cap")
        })
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 1, "30 <= 50 should match");
}

#[test]
fn cross_stage_var_range_check() {
    // Two *Var constraints: val > ?low AND val < ?high
    let mut g = MemGraph::new();
    g.add_str("ev1", "type", "bounds", 1);
    g.add_num("ev1", "low", 10.0, 1);
    g.add_num("ev1", "high", 90.0, 1);
    g.add_str("ev2", "type", "reading", 2);
    g.add_num("ev2", "val", 50.0, 2);
    g.set_time(10);

    let pattern = PatternBuilder::new("in_range")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), MemValue::Str("bounds".into()))
                .edge_bind("e1", "low".into(), "lo")
                .edge_bind("e1", "high".into(), "hi")
        })
        .stage("e2", |s| {
            s.edge("e2", "type".into(), MemValue::Str("reading".into()))
                .edge_gt_var("e2", "val".into(), "lo")
                .edge_lt_var("e2", "val".into(), "hi")
        })
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        1,
        "50 > 10 AND 50 < 90 should match"
    );
}

#[test]
fn cross_stage_var_in_negation_body() {
    // *Var inside unless_between -- negation kills if edge > ?threshold
    let mut g = MemGraph::new();
    g.add_str("ev1", "type", "set_limit", 1);
    g.add_num("ev1", "limit", 50.0, 1);
    g.add_str("ev2", "type", "violation", 2);
    g.add_num("ev2", "amount", 80.0, 2); // 80 > 50 -- triggers negation
    g.add_str("ev3", "type", "audit", 3);
    g.set_time(10);

    let pattern = PatternBuilder::new("clean_audit")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), MemValue::Str("set_limit".into()))
                .edge_bind("e1", "limit".into(), "threshold")
        })
        .stage("e3", |s| {
            s.edge("e3", "type".into(), MemValue::Str("audit".into()))
        })
        .unless_between("e1", "e3", |neg| {
            neg.edge("mid", "type".into(), MemValue::Str("violation".into()))
                .edge_constrained(
                    "mid",
                    "amount".into(),
                    ValueConstraint::GtVar("threshold".to_string()),
                )
        })
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "negation should kill -- 80 > 50"
    );
}

// ===========================================================================
// 5e. Repeat with range (min..max)
// ===========================================================================

#[test]
fn repeat_range_completes_at_min() {
    // Pattern: offense * 3..5 sharing(target)
    // min=3 means 3 total occurrences: first_ + 2 last_ loops
    let offense = PatternBuilder::new("offense")
        .stage("e", |s| {
            s.edge("e", "type".into(), MemValue::Str("offense".into()))
                .edge_bind("e", "target".into(), "target")
        })
        .build();

    let pattern = fabula::compose::repeat_range("strikes", &offense, 3, Some(5), &["target"]);

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let mut g = MemGraph::new();

    // Offense 1 (matches first_ stage)
    g.add_str("ev1", "type", "offense", 1);
    g.add_ref("ev1", "target", "alice", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("offense".into()),
        &Interval::open(1),
    );

    // Offense 2 (matches last_ stage → rep=2, min=3 → not yet)
    g.add_str("ev2", "type", "offense", 2);
    g.add_ref("ev2", "target", "alice", 2);
    g.set_time(2);
    let ev2 = engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"type".into(),
        &MemValue::Str("offense".into()),
        &Interval::open(2),
    );
    let completions: Vec<_> = ev2
        .iter()
        .filter(|e| matches!(e, SiftEvent::Completed { .. }))
        .collect();
    assert_eq!(
        completions.len(),
        0,
        "should not complete at 2 occurrences (min=3)"
    );

    // Offense 3 (last_ loop → rep=3 >= min=3 → complete!)
    g.add_str("ev3", "type", "offense", 3);
    g.add_ref("ev3", "target", "alice", 3);
    g.set_time(3);
    let ev3 = engine.on_edge_added(
        &g,
        &"ev3".into(),
        &"type".into(),
        &MemValue::Str("offense".into()),
        &Interval::open(3),
    );
    let completions: Vec<_> = ev3
        .iter()
        .filter(|e| matches!(e, SiftEvent::Completed { .. }))
        .collect();
    assert_eq!(
        completions.len(),
        1,
        "should complete at 3 occurrences (min=3)"
    );
}

#[test]
fn repeat_range_continues_after_min() {
    // Pattern: offense * 2..5 sharing(target)
    // min=2: completes after 2 total occurrences (first_ + first last_)
    // Then continues looping, producing more completions
    let offense = PatternBuilder::new("offense")
        .stage("e", |s| {
            s.edge("e", "type".into(), MemValue::Str("offense".into()))
                .edge_bind("e", "target".into(), "target")
        })
        .build();

    let pattern = fabula::compose::repeat_range("strikes", &offense, 2, Some(5), &["target"]);

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let mut g = MemGraph::new();

    // Feed 2 offenses → completes at min=2
    for i in 1..=2 {
        g.add_str(&format!("ev{}", i), "type", "offense", i);
        g.add_ref(&format!("ev{}", i), "target", "alice", i);
        g.set_time(i);
        engine.on_edge_added(
            &g,
            &format!("ev{}", i).into(),
            &"type".into(),
            &MemValue::Str("offense".into()),
            &Interval::open(i),
        );
    }

    // Offense 3 -- should produce another completion (3 >= min=2)
    g.add_str("ev3", "type", "offense", 3);
    g.add_ref("ev3", "target", "alice", 3);
    g.set_time(3);
    let ev3 = engine.on_edge_added(
        &g,
        &"ev3".into(),
        &"type".into(),
        &MemValue::Str("offense".into()),
        &Interval::open(3),
    );

    let completions: Vec<_> = ev3
        .iter()
        .filter(|e| matches!(e, SiftEvent::Completed { .. }))
        .collect();
    assert!(
        completions.len() >= 1,
        "should complete again at 3 total occurrences"
    );
}

#[test]
fn repeat_range_stops_at_max() {
    // Pattern: offense * 2..4 sharing(target)
    // Verify no PM loops beyond max=4 total occurrences.
    let offense = PatternBuilder::new("offense")
        .stage("e", |s| {
            s.edge("e", "type".into(), MemValue::Str("offense".into()))
                .edge_bind("e", "target".into(), "target")
        })
        .build();

    let pattern = fabula::compose::repeat_range("strikes", &offense, 2, Some(4), &["target"]);

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let mut g = MemGraph::new();

    // Use a "setup" event type for first_ stage, and "offense" for last_ -- but
    // actually both first_ and last_ match the same sub-pattern ("offense").
    // With multiple events, multiple starting points create multiple PMs.
    // Just verify that completions happen but no PM has rep_count > max.
    for i in 1..=5 {
        g.add_str(&format!("ev{}", i), "type", "offense", i);
        g.add_ref(&format!("ev{}", i), "target", "alice", i);
        g.set_time(i);
        engine.on_edge_added(
            &g,
            &format!("ev{}", i).into(),
            &"type".into(),
            &MemValue::Str("offense".into()),
            &Interval::open(i),
        );
    }

    let completed = engine.drain_completed();
    assert!(!completed.is_empty(), "should have completions");

    // Verify no PM loops beyond max=4 total occurrences.
    let active = engine.active_matches_for("strikes");
    for pm in &active {
        assert!(
            pm.repetition_count <= 4,
            "no PM should loop beyond max=4, got rep {}",
            pm.repetition_count
        );
    }
}

#[test]
fn repeat_range_unbounded_keeps_matching() {
    // Pattern: offense * 2.. sharing(target) -- no max
    // min=2 means 2 total occurrences → first completion after 2 events
    let offense = PatternBuilder::new("offense")
        .stage("e", |s| {
            s.edge("e", "type".into(), MemValue::Str("offense".into()))
                .edge_bind("e", "target".into(), "target")
        })
        .build();

    let pattern = fabula::compose::repeat_range("strikes", &offense, 2, None, &["target"]);

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let mut g = MemGraph::new();

    // Feed 6 offenses
    for i in 1..=6 {
        g.add_str(&format!("ev{}", i), "type", "offense", i);
        g.add_ref(&format!("ev{}", i), "target", "alice", i);
        g.set_time(i);
        engine.on_edge_added(
            &g,
            &format!("ev{}", i).into(),
            &"type".into(),
            &MemValue::Str("offense".into()),
            &Interval::open(i),
        );
    }

    // Completions at 2, 3, 4, 5, 6 total occurrences = 5 from the first starting PM
    // (plus more from PMs starting at later events)
    let completed = engine.drain_completed();
    assert!(
        completed.len() >= 5,
        "unbounded should produce completions at each occurrence >= min (got {})",
        completed.len()
    );
}

#[test]
fn repeat_range_first_last_bindings() {
    // Verify first_ and last_ binding bookends
    let offense = PatternBuilder::new("offense")
        .stage("e", |s| {
            s.edge("e", "type".into(), MemValue::Str("offense".into()))
                .edge_bind("e", "actor".into(), "actor")
                .edge_bind("e", "target".into(), "target")
        })
        .build();

    // min=2 means 2 total occurrences → completes after first_ + first last_
    let pattern = fabula::compose::repeat_range("strikes", &offense, 2, Some(4), &["target"]);

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let mut g = MemGraph::new();

    // Offense 1 by bob (matches first_)
    g.add_str("ev1", "type", "offense", 1);
    g.add_ref("ev1", "actor", "bob", 1);
    g.add_ref("ev1", "target", "alice", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("offense".into()),
        &Interval::open(1),
    );

    // Offense 2 by charlie (matches last_ → rep=2 >= min=2 → completes)
    g.add_str("ev2", "type", "offense", 2);
    g.add_ref("ev2", "actor", "charlie", 2);
    g.add_ref("ev2", "target", "alice", 2);
    g.set_time(2);
    engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"type".into(),
        &MemValue::Str("offense".into()),
        &Interval::open(2),
    );

    let completed = engine.drain_completed();
    assert!(
        !completed.is_empty(),
        "should have completions at 2 total occurrences"
    );

    let first_match = &completed[0];
    // Should have first_ and last_ bindings plus shared target
    assert!(
        first_match.bindings.contains_key("target"),
        "shared var should be present"
    );
    assert!(
        first_match.bindings.contains_key("first_actor"),
        "first_ binding should exist"
    );
    assert!(
        first_match.bindings.contains_key("last_actor"),
        "last_ binding should exist"
    );

    // first_actor should be bob (first offense)
    assert_eq!(
        first_match.bindings.get("first_actor"),
        Some(&BoundValue::Node("bob".into())),
        "first_actor should be bob"
    );

    // last_actor should be charlie (second offense = first last_ iteration)
    assert_eq!(
        first_match.bindings.get("last_actor"),
        Some(&BoundValue::Node("charlie".into())),
        "last_actor should be charlie"
    );
}

#[test]
fn repeat_range_exact_is_backward_compatible() {
    // * N (exact) should produce same behavior as old repeat()
    let offense = PatternBuilder::<String, MemValue>::new("offense")
        .stage("e", |s| {
            s.edge("e", "type".into(), MemValue::Str("offense".into()))
                .edge_bind("e", "target".into(), "target")
        })
        .build();

    let exact = fabula::compose::repeat("three", &offense, 3, &["target"]);
    // repeat_range with min=max should also work via the old path
    assert!(
        exact.repeat_range.is_none(),
        "exact repeat should not have repeat_range"
    );
    assert_eq!(exact.stages.len(), 3, "exact repeat should unroll 3 copies");
}

// ===========================================================================
// 5f. Metric temporal constraints
// ===========================================================================

// ===========================================================================
// Inactivity-based pruning
// ===========================================================================

#[test]
fn inactivity_pruning_kills_stale_pms() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    engine.register(
        PatternBuilder::new("two_stage")
            .stage("e1", |s| {
                s.edge("e1", "type".into(), MemValue::Str("start".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("end".into()))
            })
            .inactivity_threshold(3)
            .build(),
    );

    // Feed stage 1 before any end_tick (tick_counter = 0, PM gets last_advanced_tick = 0)
    g.add_str("ev1", "type", "start", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("start".into()),
        &Interval::open(1),
    );

    // end_tick #1: tick_counter becomes 1, diff = 1 < 3 => survives
    let (_, _) = engine.end_tick(100);
    assert_eq!(
        engine
            .partial_matches()
            .iter()
            .filter(|pm| pm.state == MatchState::Active)
            .count(),
        1,
        "PM should survive after 1 idle tick"
    );

    // end_tick #2: tick_counter becomes 2, diff = 2 < 3 => survives
    let (_, _) = engine.end_tick(100);
    assert_eq!(
        engine
            .partial_matches()
            .iter()
            .filter(|pm| pm.state == MatchState::Active)
            .count(),
        1,
        "PM should survive after 2 idle ticks"
    );

    // end_tick #3: tick_counter becomes 3, diff = 3 >= 3 => pruned
    let (_, _) = engine.end_tick(100);
    assert_eq!(
        engine
            .partial_matches()
            .iter()
            .filter(|pm| pm.state == MatchState::Active)
            .count(),
        0,
        "PM should be pruned after 3 idle ticks"
    );
}

#[test]
fn inactivity_pruning_spares_active_pms() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    engine.register(
        PatternBuilder::new("two_stage")
            .stage("e1", |s| {
                s.edge("e1", "type".into(), MemValue::Str("start".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("end".into()))
            })
            .inactivity_threshold(3)
            .build(),
    );

    // Tick 1: initiate a PM
    g.add_str("ev1", "type", "start", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("start".into()),
        &Interval::open(1),
    );
    let (_, _) = engine.end_tick(100); // tick 1

    // Tick 2: idle
    let (_, _) = engine.end_tick(100);

    // Tick 3: advance (complete) the PM before threshold
    g.add_str("ev2", "type", "end", 3);
    g.set_time(3);
    let events = engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"type".into(),
        &MemValue::Str("end".into()),
        &Interval::open(3),
    );
    let completed = events
        .iter()
        .filter(|e| matches!(e, SiftEvent::Completed { .. }))
        .count();
    assert_eq!(completed, 1, "PM should complete before inactivity kills it");
}

#[test]
fn inactivity_pruning_none_means_no_pruning() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    engine.register(
        PatternBuilder::new("two_stage")
            .stage("e1", |s| {
                s.edge("e1", "type".into(), MemValue::Str("start".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("end".into()))
            })
            .build(), // no inactivity_threshold
    );

    // Tick 1: initiate a PM
    g.add_str("ev1", "type", "start", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"type".into(),
        &MemValue::Str("start".into()),
        &Interval::open(1),
    );
    let (_, _) = engine.end_tick(10000); // tick 1

    // Idle for 100 ticks
    for _ in 0..100 {
        let (_, _) = engine.end_tick(10000);
    }

    assert_eq!(
        engine
            .partial_matches()
            .iter()
            .filter(|pm| pm.state == MatchState::Active)
            .count(),
        1,
        "PM should survive indefinitely without inactivity_threshold"
    );
}
