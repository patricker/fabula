use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

#[test]
fn engine_creation() {
    // #region engine_creation
    // Explicit type parameters:
    let _engine: SiftEngine<String, String, MemValue, i64, DefaultLetEvaluator> = SiftEngine::new(DefaultLetEvaluator);

    // Or use the SiftEngineFor alias (extracts types from a DataSource):
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);

    engine.register(
        PatternBuilder::new("example")
            .stage("e", |s| {
                s.edge("e", "type".into(), MemValue::Str("harm".into()))
            })
            .build(),
    );
    // #endregion

    assert_eq!(engine.patterns().len(), 1);
}

#[test]
fn end_tick_usage() {
    // #region end_tick_usage
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(
        PatternBuilder::new("offer_accept")
            .stage("e1", |s| {
                s.edge("e1", "type".into(), MemValue::Str("offer".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("accept".into()))
            })
            .deadline(10)
            .build(),
    );

    let mut graph = MemGraph::new();
    graph.add_str("evt1", "type", "offer", 1);
    graph.set_time(20);

    // Feed edges and end the tick
    let interval = Interval::open(1);
    engine.on_edge_added(
        &graph,
        &"evt1".to_string(),
        &"type".to_string(),
        &MemValue::Str("offer".into()),
        &interval,
    );
    let (delta, expired_events) = engine.end_tick(50);
    if !delta.stalled.is_empty() { /* alert GM about stale plants */ }
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

    // First tick -- no expiry yet
    assert!(expired_events.is_empty());
}
