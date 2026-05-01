use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

#[test]
fn surprise_scoring() {
    // #region surprise_scoring
    use fabula::scoring::SurpriseScorer;

    // Build the broken-promise pattern.
    let broken_promise = PatternBuilder::<String, MemValue>::new("broken_promise")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), MemValue::Str("promise".into()))
                .edge_bind("e1", "actor".into(), "char")
                .edge_bind("e1", "target".into(), "recipient")
        })
        .stage("e2", |s| {
            s.edge("e2", "type".into(), MemValue::Str("betray".into()))
                .edge_bind("e2", "actor".into(), "char")
                .edge_bind("e2", "target".into(), "recipient")
        })
        .unless_between("e1", "e2", |neg| {
            neg.edge("mid", "type".into(), MemValue::Str("fulfill".into()))
                .edge_bind("mid", "actor".into(), "char")
                .edge_bind("mid", "target".into(), "recipient")
        })
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    let broken_promise_idx = engine.register(broken_promise);

    let mut scorer = SurpriseScorer::new();

    // Baseline: broken promises happen ~30% of rounds in this simulation.
    scorer.set_baseline(broken_promise_idx, 0.3);

    // Build a graph where the pattern matches.
    let mut graph = MemGraph::new();
    graph.add_str("e1", "type", "promise", 1);
    graph.add_ref("e1", "actor", "macbeth", 1);
    graph.add_ref("e1", "target", "duncan", 1);
    graph.add_str("e2", "type", "betray", 3);
    graph.add_ref("e2", "actor", "macbeth", 3);
    graph.add_ref("e2", "target", "duncan", 3);
    graph.set_time(10);

    // Observe several rounds to build up frequency data.
    for _ in 0..20 {
        let matches = engine.evaluate(&graph);
        scorer.observe(&matches, engine.patterns());
    }

    // Score the latest round's matches.
    let matches = engine.evaluate(&graph);
    let scored = scorer.score(&matches, engine.patterns());
    for m in &scored {
        println!("{}: surprise = {:.2} bits", m.pattern, m.surprise);
    }
    // #endregion

    assert_eq!(scored.len(), 1);
    assert_eq!(scored[0].pattern, "broken_promise");
    // Pattern fires every round (20/20) vs baseline 0.3 => negative surprise
    assert!(scored[0].surprise < 0.0);
}
