use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

#[test]
fn step1_batch_check() {
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(
        PatternBuilder::new("my_pattern")
            .stage("e", |s| {
                s.edge("e", "eventType".into(), MemValue::Str("hello".into()))
            })
            .build(),
    );
    let mut graph = MemGraph::new();
    graph.add_str("ev1", "eventType", "hello", 1);
    graph.set_time(10);

    // #region step1_batch
    let matches = engine.evaluate(&graph);
    println!("Batch matches: {}", matches.len());
    // #endregion
    assert_eq!(matches.len(), 1);
}

#[test]
fn step2_why_not() {
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(
        PatternBuilder::new("my_pattern")
            .stage("e", |s| {
                s.edge("e", "eventType".into(), MemValue::Str("hello".into()))
            })
            .build(),
    );
    let graph = MemGraph::new();

    // #region step2_why_not
    if let Some(analysis) = engine.why_not(&graph, "my_pattern") {
        println!("Pattern: {}", analysis.pattern);
        for stage in &analysis.stages {
            println!("  Stage '{}': {:?}", stage.anchor, stage.status);
            for clause in &stage.clauses {
                println!(
                    "    {} => matched: {}, reason: {:?}",
                    clause.description, clause.matched, clause.reason
                );
            }
        }
    }
    // #endregion
}

#[test]
fn step4_compare_batch_incremental() {
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(
        PatternBuilder::new("my_pattern")
            .stage("e", |s| {
                s.edge("e", "eventType".into(), MemValue::Str("hello".into()))
            })
            .build(),
    );
    let mut graph = MemGraph::new();
    graph.add_str("ev1", "eventType", "hello", 1);
    graph.set_time(10);

    // #region step4_compare
    let batch_matches = engine.evaluate(&graph);
    let completed = engine.drain_completed();
    println!(
        "Batch: {}, Incremental: {}",
        batch_matches.len(),
        completed.len()
    );
    // #endregion
}

#[test]
fn step5_inspect_partial_matches() {
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(
        PatternBuilder::new("two_stage")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("start".into()))
                    .edge_bind("e1", "actor".into(), "person")
            })
            .stage("e2", |s| {
                s.edge("e2", "eventType".into(), MemValue::Str("end".into()))
                    .edge_bind("e2", "actor".into(), "person")
            })
            .build(),
    );
    let mut graph = MemGraph::new();
    graph.add_str("ev1", "eventType", "start", 1);
    graph.add_ref("ev1", "actor", "alice", 1);
    graph.set_time(1);
    engine.on_edge_added(
        &graph,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("start".into()),
        &Interval::open(1),
    );

    // #region step5_inspect
    for pm in engine.partial_matches() {
        println!(
            "Match #{}: pattern_idx={}, next_stage={}, state={:?}",
            pm.id, pm.pattern_idx, pm.next_stage, pm.state
        );
        for (var, val) in &pm.bindings {
            println!("  {} = {:?}", var, val);
        }
        for (anchor, iv) in &pm.intervals {
            println!("  {} at {}", anchor, iv);
        }
    }
    // #endregion
}
