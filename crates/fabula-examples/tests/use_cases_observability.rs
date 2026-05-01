use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

#[test]
fn incremental_integration() {
    // #region incremental_integration
    let cascade_timeout = PatternBuilder::<String, MemValue>::new("cascade_timeout")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), MemValue::Str("call".into()))
                .edge_bind("e1", "caller".into(), "svc_a")
                .edge_bind("e1", "callee".into(), "svc_b")
        })
        .stage("e2", |s| {
            s.edge("e2", "type".into(), MemValue::Str("call".into()))
                .edge_bind("e2", "caller".into(), "svc_b")
                .edge_bind("e2", "callee".into(), "svc_c")
        })
        .stage("e3", |s| {
            s.edge("e3", "type".into(), MemValue::Str("timeout".into()))
                .edge_bind("e3", "service".into(), "svc_c")
        })
        .unless_after("e3", |neg| {
            neg.edge("mid", "type".into(), MemValue::Str("recovery".into()))
                .edge_bind("mid", "service".into(), "svc_c")
        })
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(cascade_timeout);

    let mut graph = MemGraph::new();
    graph.set_time(10);

    // Each span/event from your tracing system:
    let source = "e1".to_string();
    let label = "type".to_string();
    let value = MemValue::Str("call".into());
    let interval = Interval::open(1);

    graph.add_str("e1", "type", "call", 1);
    graph.add_ref("e1", "caller", "api_gateway", 1);
    graph.add_ref("e1", "callee", "auth_service", 1);

    let events = engine.on_edge_added(&graph, &source, &label, &value, &interval);
    for event in &events {
        match event {
            SiftEvent::Completed {
                pattern, bindings, ..
            } => {
                // Alert: cascade_timeout detected!
                // bindings["svc_a"], bindings["svc_b"], bindings["svc_c"]
                // contain the affected services.
                println!("Alert: {} -- {:?}", pattern, bindings);
            }
            SiftEvent::Negated { pattern, .. } => {
                // Recovery detected -- a previously active alert is resolved.
                println!("Resolved: {}", pattern);
            }
            _ => {}
        }
    }
    // #endregion
}
