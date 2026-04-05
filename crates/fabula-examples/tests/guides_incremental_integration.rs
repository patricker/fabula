use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

#[test]
fn step1_register_patterns() {
    // #region step1_register
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    // Pattern: betrayal after hospitality
    engine.register(
        PatternBuilder::new("violation_of_hospitality")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("enterTown".into()))
                    .edge_bind("e1", "actor".into(), "guest")
            })
            .stage("e2", |s| {
                s.edge("e2", "eventType".into(), MemValue::Str("showHospitality".into()))
                    .edge_bind("e2", "actor".into(), "host")
                    .edge_bind("e2", "target".into(), "guest")
            })
            .stage("e3", |s| {
                s.edge("e3", "eventType".into(), MemValue::Str("harm".into()))
                    .edge_bind("e3", "actor".into(), "host")
                    .edge_bind("e3", "target".into(), "guest")
            })
            .unless_between("e1", "e3", |neg| {
                neg.edge("eMid", "eventType".into(), MemValue::Str("leaveTown".into()))
                    .edge_bind("eMid", "actor".into(), "guest")
            })
            .build(),
    );

    let graph = MemGraph::new();
    // #endregion
    let _ = (&engine, &graph);
}

#[test]
fn step2_feed_events() {
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(
        PatternBuilder::new("violation_of_hospitality")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("enterTown".into()))
                    .edge_bind("e1", "actor".into(), "guest")
            })
            .stage("e2", |s| {
                s.edge("e2", "eventType".into(), MemValue::Str("showHospitality".into()))
                    .edge_bind("e2", "actor".into(), "host")
                    .edge_bind("e2", "target".into(), "guest")
            })
            .stage("e3", |s| {
                s.edge("e3", "eventType".into(), MemValue::Str("harm".into()))
                    .edge_bind("e3", "actor".into(), "host")
                    .edge_bind("e3", "target".into(), "guest")
            })
            .unless_between("e1", "e3", |neg| {
                neg.edge("eMid", "eventType".into(), MemValue::Str("leaveTown".into()))
                    .edge_bind("eMid", "actor".into(), "guest")
            })
            .build(),
    );
    let mut graph = MemGraph::new();

    // #region step2_feed_events
    // Simulation tick 1: Alice enters town
    graph.add_str("ev1", "eventType", "enterTown", 1);
    graph.add_ref("ev1", "actor", "alice", 1);
    graph.set_time(1);

    let events = engine.on_edge_added(
        &graph,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("enterTown".into()),
        &Interval::open(1),
    );

    for event in &events {
        match event {
            SiftEvent::Advanced {
                pattern,
                match_id,
                stage_index,
                ..
            } => {
                println!(
                    "Pattern '{}' advanced to stage {} (match #{})",
                    pattern, stage_index, match_id
                );
            }
            SiftEvent::Completed {
                pattern,
                match_id,
                bindings,
                ..
            } => {
                println!("Pattern '{}' completed (match #{})", pattern, match_id);
                for (var, val) in bindings {
                    println!("  {} = {:?}", var, val);
                }
            }
            SiftEvent::Negated {
                pattern,
                match_id,
                clause_label,
                trigger_source,
                ..
            } => {
                println!(
                    "Pattern '{}' negated (match #{}) by {} from {:?}",
                    pattern, match_id, clause_label, trigger_source
                );
            }
            SiftEvent::Expired {
                pattern,
                stage_reached,
                ticks_elapsed,
                ..
            } => {
                println!(
                    "Pattern '{}' expired at stage {} after {} ticks",
                    pattern, stage_reached, ticks_elapsed
                );
            }
        }
    }
    // #endregion
}

#[test]
fn step3_simulation_loop() {
    // #region step3_simulation_loop
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    let mut graph = MemGraph::new();

    // Register the hospitality violation pattern
    engine.register(
        PatternBuilder::new("violation_of_hospitality")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("enterTown".into()))
                    .edge_bind("e1", "actor".into(), "guest")
            })
            .stage("e2", |s| {
                s.edge("e2", "eventType".into(), MemValue::Str("showHospitality".into()))
                    .edge_bind("e2", "actor".into(), "host")
                    .edge_bind("e2", "target".into(), "guest")
            })
            .stage("e3", |s| {
                s.edge("e3", "eventType".into(), MemValue::Str("harm".into()))
                    .edge_bind("e3", "actor".into(), "host")
                    .edge_bind("e3", "target".into(), "guest")
            })
            .unless_between("e1", "e3", |neg| {
                neg.edge("eMid", "eventType".into(), MemValue::Str("leaveTown".into()))
                    .edge_bind("eMid", "actor".into(), "guest")
            })
            .build(),
    );

    // Simulated event stream
    let events = vec![
        ("ev1", "eventType", "enterTown", "actor", "alice", None, 1),
        (
            "ev2",
            "eventType",
            "showHospitality",
            "actor",
            "bob",
            Some(("target", "alice")),
            2,
        ),
        (
            "ev3",
            "eventType",
            "enterTown",
            "actor",
            "charlie",
            None,
            3,
        ),
        (
            "ev4",
            "eventType",
            "showHospitality",
            "actor",
            "dave",
            Some(("target", "charlie")),
            4,
        ),
        ("ev5", "eventType", "trade", "actor", "alice", None, 5),
        (
            "ev6",
            "eventType",
            "leaveTown",
            "actor",
            "charlie",
            None,
            6,
        ),
        (
            "ev7",
            "eventType",
            "harm",
            "actor",
            "bob",
            Some(("target", "alice")),
            7,
        ),
        (
            "ev8",
            "eventType",
            "harm",
            "actor",
            "dave",
            Some(("target", "charlie")),
            8,
        ),
        ("ev9", "eventType", "enterTown", "actor", "eve", None, 9),
        (
            "ev10",
            "eventType",
            "showHospitality",
            "actor",
            "frank",
            Some(("target", "eve")),
            10,
        ),
    ];

    for (id, label, value, actor_label, actor, extra, time) in &events {
        graph.add_str(id, label, value, *time);
        graph.add_ref(id, actor_label, actor, *time);
        if let Some((extra_label, extra_target)) = extra {
            graph.add_ref(id, extra_label, extra_target, *time);
        }
        graph.set_time(*time);

        // Notify engine of the primary edge (eventType)
        let sift_events = engine.on_edge_added(
            &graph,
            &id.to_string(),
            &label.to_string(),
            &MemValue::Str(value.to_string()),
            &Interval::open(*time),
        );

        for se in &sift_events {
            match se {
                SiftEvent::Completed {
                    pattern, bindings, ..
                } => {
                    println!("[t={}] MATCH: {}", time, pattern);
                    for (var, val) in bindings {
                        println!("       {} = {:?}", var, val);
                    }
                }
                SiftEvent::Negated {
                    pattern,
                    clause_label,
                    ..
                } => {
                    println!("[t={}] NEGATED: {} (by {})", time, pattern, clause_label);
                }
                SiftEvent::Advanced {
                    pattern,
                    stage_index,
                    ..
                } => {
                    println!("[t={}] ADVANCED: {} to stage {}", time, pattern, stage_index);
                }
                SiftEvent::Expired {
                    pattern,
                    stage_reached,
                    ticks_elapsed,
                    ..
                } => {
                    println!(
                        "[t={}] EXPIRED: {} at stage {} after {} ticks",
                        time, pattern, stage_reached, ticks_elapsed
                    );
                }
            }
        }
    }

    // Drain completed matches
    let completed = engine.drain_completed();
    println!("\n{} completed matches drained", completed.len());
    // #endregion
    assert!(!completed.is_empty());
}

#[test]
fn tick_deltas_and_scoring() {
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    let mut graph = MemGraph::new();
    engine.register(
        PatternBuilder::new("test_pattern")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("hello".into()))
            })
            .build(),
    );

    graph.add_str("ev1", "eventType", "hello", 1);
    graph.set_time(1);
    engine.on_edge_added(
        &graph,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("hello".into()),
        &Interval::open(1),
    );

    // #region tick_delta
    // After processing all edges for this tick:
    let (delta, expired_events) = engine.end_tick(50); // stale threshold = 50 ticks

    // delta.advanced — patterns that progressed this tick
    // delta.completed — patterns that fully matched
    // delta.negated — patterns that were killed
    // delta.expired — patterns that had PMs expire (deadline exceeded)
    // delta.stalled — patterns with active PMs that haven't advanced in 50+ ticks
    // delta.active_pm_count — total active partial matches

    // Handle expired partial matches
    for ev in &expired_events {
        if let SiftEvent::Expired {
            pattern,
            stage_reached,
            ticks_elapsed,
            ..
        } = ev
        {
            println!(
                "{} expired at stage {} after {} ticks",
                pattern, stage_reached, ticks_elapsed
            );
        }
    }
    // #endregion
    let _ = &delta;
}

#[test]
fn narrative_scoring_integration() {
    // #region narrative_scoring
    use fabula_narratives::pivot::PivotDetector;
    use fabula_narratives::scorer::{assemble_signals, score, NarrativeWeights};
    use fabula_narratives::tension::{TensionTracker, Trajectory};
    use fabula_narratives::thread::ThreadTracker;

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    let graph = MemGraph::new();

    // Register patterns (elided — see Step 1 above)

    // Set up narrative trackers
    let mut thread_tracker = ThreadTracker::new();
    // thread_tracker.register("investigation", open_idx, close_idx);
    let mut tension = TensionTracker::new(10); // sliding window of 10 ticks
    let mut pivot = PivotDetector::new();

    // Inside your tick loop, after feeding edges with on_edge_added():
    let (delta, _expired_events) = engine.end_tick(50);

    // Update trackers with this tick's data
    thread_tracker.observe_delta(&delta);
    tension.push(delta.completed.len() as u64, delta.active_pm_count as f64);
    for pattern_name in &delta.advanced {
        pivot.push(pattern_name);
    }
    for pattern_name in &delta.completed {
        pivot.push(pattern_name);
    }
    let pivot_magnitude = pivot.end_tick();

    // Assemble signals from all trackers
    let plant_statuses = engine.plant_status(50);
    let filo_violations = thread_tracker.check_filo().len();
    let signals = assemble_signals(
        &delta,
        &plant_statuses,
        filo_violations,
        tension.trajectory(),
        Trajectory::Rising, // desired trajectory for this story phase
        pivot_magnitude,
        0.0, // surprise score (from your own SurpriseScorer, if any)
        0.0, // sequential surprise score
    );
    let result = score(&signals, &NarrativeWeights::default());
    // result.total — composite narrative quality score
    // result.breakdown — per-signal contributions for debugging
    // #endregion
    let _ = (&result, &graph);
}
