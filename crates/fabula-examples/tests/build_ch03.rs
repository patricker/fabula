use fabula::compose;
use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

fn add_event(g: &mut MemGraph, id: &str, action: &str, actor: &str, stock: &str, tick: i64) {
    g.add_str(id, "action", action, tick);
    g.add_ref(id, "actor", actor, tick);
    g.add_ref(id, "stock", stock, tick);
    g.set_time(tick);
}

fn insider_trading_pattern() -> Pattern<String, MemValue> {
    PatternBuilder::new("insider_trading")
        .stage("tip", |s| {
            s.edge("tip", "action".into(), MemValue::Str("insider_tip".into()))
                .edge_bind("tip", "actor".into(), "suspect")
                .edge_bind("tip", "stock".into(), "ticker")
        })
        .stage("trade", |s| {
            s.edge("trade", "action".into(), MemValue::Str("trade".into()))
                .edge_bind("trade", "actor".into(), "suspect")
                .edge_bind("trade", "stock".into(), "ticker")
        })
        .unless_between("tip", "trade", |neg| {
            neg.edge("disclosure", "action".into(), MemValue::Str("alert".into()))
                .edge_bind("disclosure", "stock".into(), "ticker")
        })
        .build()
}

fn repeated_manipulation_pattern() -> Pattern<String, MemValue> {
    let single_trade = PatternBuilder::new("single_trade")
        .stage("ev", |s| {
            s.edge("ev", "action".into(), MemValue::Str("trade".into()))
                .edge_bind("ev", "actor".into(), "manipulator")
                .edge_bind("ev", "stock".into(), "ticker")
        })
        .build();

    compose::repeat(
        "repeated_manipulation",
        &single_trade,
        3,
        &["manipulator", "ticker"],
    )
}

fn flash_crash_pattern() -> Pattern<String, MemValue> {
    PatternBuilder::new("flash_crash")
        .unordered_group(|g| {
            g.stage("drop", |s| {
                s.edge(
                    "drop",
                    "action".into(),
                    MemValue::Str("price_change".into()),
                )
                .edge_bind("drop", "stock".into(), "ticker")
            })
            .stage("alarm", |s| {
                s.edge("alarm", "action".into(), MemValue::Str("alert".into()))
                    .edge_bind("alarm", "stock".into(), "ticker")
            })
        })
        .stage("sell", |s| {
            s.edge("sell", "action".into(), MemValue::Str("trade".into()))
                .edge_bind("sell", "stock".into(), "ticker")
        })
        .build()
}

#[test]
fn incremental_matching() {
    // #region incremental_matching
    let mut graph = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    engine.register(insider_trading_pattern());
    engine.register(repeated_manipulation_pattern());
    engine.register(flash_crash_pattern());

    let schedule: Vec<Vec<(&str, &str, &str)>> = vec![
        vec![
            ("insider_tip", "alice", "ACME"),
            ("trade", "bob", "ZINC"),
            ("price_change", "market", "ACME"),
        ],
        vec![
            ("trade", "alice", "ACME"),
            ("trade", "charlie", "ZINC"),
            ("alert", "system", "ACME"),
        ],
        vec![
            ("trade", "bob", "ACME"),
            ("trade", "bob", "ACME"),
            ("price_change", "market", "ZINC"),
        ],
        vec![
            ("insider_tip", "charlie", "ZINC"),
            ("alert", "system", "ZINC"),
            ("trade", "alice", "ACME"),
        ],
        vec![
            ("trade", "bob", "ACME"),
            ("trade", "charlie", "ZINC"),
            ("price_change", "market", "ACME"),
        ],
        vec![
            ("trade", "bob", "ACME"),
            ("alert", "system", "ACME"),
            ("trade", "alice", "ZINC"),
        ],
        vec![
            ("price_change", "market", "ZINC"),
            ("trade", "charlie", "ZINC"),
            ("trade", "charlie", "ZINC"),
        ],
        vec![
            ("insider_tip", "bob", "ACME"),
            ("trade", "alice", "ZINC"),
            ("alert", "system", "ACME"),
        ],
        vec![
            ("trade", "bob", "ACME"),
            ("trade", "charlie", "ACME"),
            ("price_change", "market", "ZINC"),
        ],
        vec![
            ("trade", "alice", "ACME"),
            ("alert", "system", "ZINC"),
            ("trade", "bob", "ZINC"),
        ],
    ];

    let mut event_id = 0;
    for (tick_idx, tick_events) in schedule.iter().enumerate() {
        let tick = (tick_idx + 1) as i64;
        println!("=== tick {} ===", tick);

        for (action, actor, stock) in tick_events {
            let id = format!("ev{}", event_id);

            // Add all edges to the graph first
            add_event(&mut graph, &id, action, actor, stock, tick);

            // Feed the action edge to the engine (trigger clause)
            let events = engine.on_edge_added(
                &graph,
                &id,
                &"action".to_string(),
                &MemValue::Str(action.to_string()),
                &Interval::open(tick),
            );

            // Print any events produced by this edge
            for ev in &events {
                match ev {
                    SiftEvent::Advanced {
                        pattern,
                        stage_index,
                        ..
                    } => {
                        println!("  >> {}: {} advanced stage {}", id, pattern, stage_index);
                    }
                    SiftEvent::Completed {
                        pattern, bindings, ..
                    } => {
                        let summary: Vec<String> = bindings
                            .iter()
                            .filter(|(_, v)| matches!(v, BoundValue::Node(_)))
                            .map(|(k, v)| format!("{}={:?}", k, v))
                            .collect();
                        println!(
                            "  ** {}: {} COMPLETED [{}]",
                            id,
                            pattern,
                            summary.join(", ")
                        );
                    }
                    SiftEvent::Negated {
                        pattern,
                        clause_label,
                        ..
                    } => {
                        println!("  xx {}: {} negated by {}", id, pattern, clause_label);
                    }
                    SiftEvent::Expired { pattern, .. } => {
                        println!("  !! {}: {} expired", id, pattern);
                    }
                }
            }

            event_id += 1;
        }

        // End the tick and print the delta
        let (delta, _expired) = engine.end_tick(50);
        if !delta.advanced.is_empty() {
            println!("  advanced: {:?}", delta.advanced);
        }
        if !delta.completed.is_empty() {
            println!("  completed: {:?}", delta.completed);
        }
        if !delta.negated.is_empty() {
            println!("  negated: {:?}", delta.negated);
        }
        println!("  active PMs: {}", delta.active_pm_count);
    }

    // Final summary
    let completed = engine.drain_completed();
    println!("\n--- final ---");
    println!("{} completed matches drained:", completed.len());
    for m in &completed {
        println!("  {}", m.pattern);
    }
    println!("engine stats: {:?}", engine.stats());
    // #endregion

    assert!(
        !completed.is_empty(),
        "should have at least one completed match"
    );
}
