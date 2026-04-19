use fabula::prelude::*;
use fabula::scoring::{SequentialScorer, StuAggregation, StuScorer, SurpriseScorer};
use fabula_memory::{MemGraph, MemValue};

#[test]
fn surprise_scorer_usage() {
    // #region surprise_scorer
    let mut scorer = SurpriseScorer::new();
    scorer.set_baseline(0, 0.1); // expect pattern 0 to match 10% of rounds

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(
        PatternBuilder::new("betrayal")
            .stage("e", |s| {
                s.edge("e", "type".into(), MemValue::Str("betray".into()))
            })
            .build(),
    );

    let mut graph = MemGraph::new();
    graph.add_str("e1", "type", "betray", 1);
    graph.set_time(10);

    // After evaluation:
    let matches = engine.evaluate(&graph);
    scorer.observe(&matches, engine.patterns());
    let scored = scorer.score(&matches, engine.patterns());
    // scored[i].surprise -- higher = more unexpected
    // #endregion

    assert_eq!(scored.len(), 1);
}

#[test]
fn stu_scorer_usage() {
    // #region stu_scorer
    let mut stu = StuScorer::new()
        .with_aggregation(StuAggregation::TfIdf)
        .with_pmi_correction();

    // Observe properties for completed matches
    stu.observe_one("betrayal", &["actor_trait=ambitious", "target_role=king"]);
    stu.observe_one("betrayal", &["actor_trait=loyal", "target_role=merchant"]);

    // Score a new match
    let freq = stu.property_frequency("betrayal", "actor_trait=ambitious");
    // freq ≈ 0.6 (2 of 3 observations, with Laplace smoothing)
    // #endregion

    assert!(freq.is_some());
}

#[test]
fn sequential_scorer_usage() {
    // #region sequential_scorer
    let mut seq = SequentialScorer::new();
    seq.observe_transition("alliance", "betrayal");
    seq.observe_transition("alliance", "betrayal");
    seq.observe_transition("alliance", "trade");

    // betrayal after alliance: common (2/3)
    let common = seq.score_transition("alliance", "betrayal");
    // trade after alliance: rarer (1/3)
    let rare = seq.score_transition("alliance", "trade");
    assert!(rare > common, "rarer transition should be more surprising");
    // #endregion
}
