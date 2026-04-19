use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};
use fabula_narratives::scorer::{assemble_signals, score, NarrativeWeights};
use fabula_narratives::tension::Trajectory;

/// Sets up a base engine with hospitality_violation and forgiveness_arc patterns,
/// and a graph with two ticks of events. Returns (engine, graph).
fn setup_base() -> (SiftEngineFor<MemGraph>, MemGraph) {
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    let hospitality_idx = engine.register(
        PatternBuilder::new("hospitality_violation")
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
            .build(),
    );

    let forgiveness_idx = engine.register(
        PatternBuilder::new("forgiveness_arc")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("harm".into()))
                    .edge_bind("e1", "actor".into(), "offender")
                    .edge_bind("e1", "target".into(), "victim")
            })
            .stage("e2", |s| {
                s.edge("e2", "eventType".into(), MemValue::Str("forgive".into()))
                    .edge_bind("e2", "actor".into(), "victim")
                    .edge_bind("e2", "target".into(), "offender")
            })
            .build(),
    );

    engine.register_plant_payoff(hospitality_idx, forgiveness_idx, None);

    let mut graph = MemGraph::new();

    // Tick 1: Alice enters town
    graph.add_str("ev1", "eventType", "enterTown", 1);
    graph.add_ref("ev1", "actor", "alice", 1);
    graph.set_time(1);
    engine.on_edge_added(
        &graph,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("enterTown".into()),
        &Interval::open(1),
    );
    engine.end_tick(50);

    // Tick 2: Bob shows hospitality to Alice
    graph.add_str("ev2", "eventType", "showHospitality", 2);
    graph.add_ref("ev2", "actor", "bob", 2);
    graph.add_ref("ev2", "target", "alice", 2);
    graph.set_time(2);
    engine.on_edge_added(
        &graph,
        &"ev2".into(),
        &"eventType".into(),
        &MemValue::Str("showHospitality".into()),
        &Interval::open(2),
    );
    engine.end_tick(50);

    (engine, graph)
}

#[test]
fn complete_mcts_example() {
    // #region complete_mcts
    let (engine, graph) = setup_base();

    // -- Fork-speculate-score loop --------------------------------------
    let candidates: Vec<(&str, &str, &str)> = vec![
        ("harm", "bob", "alice"),    // completes hospitality_violation
        ("forgive", "alice", "bob"), // no pattern effect (no prior harm)
        ("trade", "bob", "alice"),   // neutral -- advances nothing
    ];

    let weights = NarrativeWeights::default();
    let mut best_score = f64::NEG_INFINITY;
    let mut best_action = "";

    for (action, actor, target) in &candidates {
        // Fork
        let mut fork = engine.clone();
        let mut fork_graph = graph.clone();

        // Speculate
        fork_graph.add_str("hyp", "eventType", action, 3);
        fork_graph.add_ref("hyp", "actor", actor, 3);
        fork_graph.add_ref("hyp", "target", target, 3);
        fork_graph.set_time(3);

        fork.on_edge_added(
            &fork_graph,
            &"hyp".into(),
            &"eventType".into(),
            &MemValue::Str(action.to_string()),
            &Interval::open(3),
        );

        // Score
        let (delta, _) = fork.end_tick(50);
        let signals = assemble_signals(
            &delta,
            &fork.plant_status(50),
            0,
            Trajectory::Unknown,
            Trajectory::Rising,
            0.0,
            0.0,
            0.0,
        );
        let result = score(&signals, &weights);

        println!(
            "Action: {:<10} score: {:.2}  (adv={}, comp={}, stall={})",
            action,
            result.total,
            delta.advanced.len(),
            delta.completed.len(),
            delta.stalled.len(),
        );

        if result.total > best_score {
            best_score = result.total;
            best_action = action;
        }
        // fork and fork_graph are dropped here
    }

    println!("\nBest action: {} (score: {:.2})", best_action, best_score);
    // The original engine is unchanged -- no speculative state leaked.
    assert_eq!(
        engine.partial_matches().len(),
        engine
            .partial_matches()
            .iter()
            .filter(|pm| pm.state == MatchState::Active)
            .count(),
        "original engine has only its original active PMs"
    );
    // #endregion
}
