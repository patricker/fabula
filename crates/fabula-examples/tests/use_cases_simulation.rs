use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

#[test]
fn incremental_monitoring() {
    // #region incremental_monitoring
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    let mut graph = MemGraph::new();

    engine.register(
        PatternBuilder::new("resource_hoarding")
            .stage("e1", |s| {
                s.edge("e1", "type".into(), MemValue::Str("acquire".into()))
                    .edge_bind("e1", "agent".into(), "agent")
                    .edge_bind("e1", "resource".into(), "r1")
            })
            .stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("acquire".into()))
                    .edge_bind("e2", "agent".into(), "agent")
                    .edge_bind("e2", "resource".into(), "r2")
            })
            .unless_between("e1", "e2", |neg| {
                neg.edge("mid", "type".into(), MemValue::Str("share".into()))
                    .edge_bind("mid", "agent".into(), "agent")
            })
            .build(),
    );

    // Simulate a few ticks of an agent-based model.
    struct SimEvent {
        id: String,
        kind: String,
        agent: String,
        resource: String,
    }

    let ticks: Vec<(i64, Vec<SimEvent>)> = vec![
        (
            1,
            vec![SimEvent {
                id: "ev1".into(),
                kind: "acquire".into(),
                agent: "alpha".into(),
                resource: "food".into(),
            }],
        ),
        (
            2,
            vec![SimEvent {
                id: "ev2".into(),
                kind: "acquire".into(),
                agent: "alpha".into(),
                resource: "water".into(),
            }],
        ),
    ];

    let mut completed = Vec::new();
    for (tick, sim_events) in &ticks {
        for event in sim_events {
            graph.add_str(&event.id, "type", &event.kind, *tick);
            graph.add_ref(&event.id, "agent", &event.agent, *tick);
            graph.add_ref(&event.id, "resource", &event.resource, *tick);
            graph.set_time(*tick);

            let interval = Interval::open(*tick);

            // Notify the engine about the type edge (trigger clause).
            let mut sift_events = engine.on_edge_added(
                &graph,
                &event.id,
                &"type".to_string(),
                &MemValue::Str(event.kind.clone()),
                &interval,
            );
            // Also notify about the agent edge.
            sift_events.extend(engine.on_edge_added(
                &graph,
                &event.id,
                &"agent".to_string(),
                &MemValue::Node(event.agent.clone()),
                &interval,
            ));

            for se in &sift_events {
                if let SiftEvent::Completed {
                    pattern, bindings, ..
                } = se
                {
                    println!("[tick {}] detected: {} {:?}", tick, pattern, bindings);
                    completed.push(pattern.clone());
                }
            }
        }

        let (_delta, _expired) = engine.end_tick(50);
    }
    // #endregion

    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0], "resource_hoarding");
}
