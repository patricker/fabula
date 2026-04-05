use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

#[test]
fn batch_auditing() {
    // #region batch_auditing
    let skipped_approval_pattern = PatternBuilder::<String, MemValue>::new("skipped_approval")
        .stage("e1", |s| {
            s.edge(
                "e1",
                "type".into(),
                MemValue::Str("purchase_request".into()),
            )
            .edge_bind("e1", "order".into(), "order")
            .edge_bind("e1", "requester".into(), "requester")
        })
        .stage("e2", |s| {
            s.edge("e2", "type".into(), MemValue::Str("fulfillment".into()))
                .edge_bind("e2", "order".into(), "order")
        })
        .unless_between("e1", "e2", |neg| {
            neg.edge("mid", "type".into(), MemValue::Str("approval".into()))
                .edge_bind("mid", "order".into(), "order")
        })
        .build();

    let payment_before_confirmation_pattern =
        PatternBuilder::<String, MemValue>::new("payment_before_confirmation")
            .stage("e1", |s| {
                s.edge(
                    "e1",
                    "type".into(),
                    MemValue::Str("payment_received".into()),
                )
                .edge_bind("e1", "order".into(), "order")
                .edge_bind("e1", "amount".into(), "amount")
            })
            .stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("order_confirmed".into()))
                    .edge_bind("e2", "order".into(), "order")
            })
            .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(skipped_approval_pattern);
    engine.register(payment_before_confirmation_pattern);

    // Load your event log into the graph.
    let mut graph = MemGraph::new();
    // po_100: request then fulfillment with no approval (violation).
    graph.add_str("r1", "type", "purchase_request", 1);
    graph.add_ref("r1", "order", "po_100", 1);
    graph.add_ref("r1", "requester", "alice", 1);
    graph.add_str("f1", "type", "fulfillment", 3);
    graph.add_ref("f1", "order", "po_100", 3);
    // ord_200: payment before confirmation (violation).
    graph.add_str("p1", "type", "payment_received", 2);
    graph.add_ref("p1", "order", "ord_200", 2);
    graph.add_num("p1", "amount", 150.0, 2);
    graph.add_str("c1", "type", "order_confirmed", 4);
    graph.add_ref("c1", "order", "ord_200", 4);
    graph.set_time(10);

    // Find all deviations.
    let matches = engine.evaluate(&graph);
    for m in &matches {
        println!(
            "Deviation: {} — order: {:?}",
            m.pattern,
            m.bindings.get("order")
        );
    }

    // Check near-misses for each pattern.
    for pattern in engine.patterns() {
        let gap = gap_analysis(&graph, pattern);
        let matched = gap
            .stages
            .iter()
            .filter(|s| matches!(s.status, StageStatus::Matched))
            .count();
        if matched > 0 && matched < gap.stages.len() {
            println!(
                "Near-miss: {} — {}/{} stages matched",
                pattern.name,
                matched,
                gap.stages.len()
            );
        }
    }
    // #endregion

    assert_eq!(matches.len(), 2);
    let pattern_names: Vec<&str> = matches.iter().map(|m| m.pattern.as_str()).collect();
    assert!(pattern_names.contains(&"skipped_approval"));
    assert!(pattern_names.contains(&"payment_before_confirmation"));
}
