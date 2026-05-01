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

fn pump_dump_pattern() -> Pattern<String, MemValue> {
    PatternBuilder::new("pump_and_dump")
        .stage("pump", |s| {
            s.edge("pump", "action".into(), MemValue::Str("trade".into()))
                .edge_bind("pump", "actor".into(), "manipulator")
                .edge_bind("pump", "stock".into(), "ticker")
        })
        .stage("hype", |s| {
            s.edge("hype", "action".into(), MemValue::Str("promote".into()))
                .edge_bind("hype", "actor".into(), "manipulator")
                .edge_bind("hype", "stock".into(), "ticker")
        })
        .stage("dump", |s| {
            s.edge("dump", "action".into(), MemValue::Str("sell".into()))
                .edge_bind("dump", "actor".into(), "manipulator")
                .edge_bind("dump", "stock".into(), "ticker")
        })
        .build()
}

#[test]
fn speculate_with_mcts() {
    // #region speculate_with_mcts
    /// Fork the engine, add a hypothetical event, and score the result.
    /// Returns (completed pattern names, total surprise score).
    fn evaluate_hypothesis(
        engine: &SiftEngineFor<MemGraph>,
        graph: &MemGraph,
        surprise: &SurpriseScorer,
        hyp_id: &str,
        action: &str,
        actor: &str,
        stock: &str,
        tick: i64,
    ) -> (Vec<String>, f64) {
        let mut forked_engine = engine.clone();
        let mut forked_graph = graph.clone();

        add_event(&mut forked_graph, hyp_id, action, actor, stock, tick);

        let events = forked_engine.on_edge_added(
            &forked_graph,
            &hyp_id.to_string(),
            &"action".to_string(),
            &MemValue::Str(action.to_string()),
            &Interval::open(tick),
        );

        let completed: Vec<String> = events
            .iter()
            .filter_map(|e| {
                if let SiftEvent::Completed { pattern, .. } = e {
                    Some(pattern.clone())
                } else {
                    None
                }
            })
            .collect();

        let drained = forked_engine.drain_completed();
        let scored = surprise.score(&drained, forked_engine.patterns());
        let total: f64 = scored.iter().map(|s| s.surprise).sum();

        (completed, total)
    }

    let mut graph = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);

    let idx_insider = engine.register(insider_trading_pattern());
    let idx_crash = engine.register(flash_crash_pattern());
    let idx_pump = engine.register(pump_dump_pattern());

    let mut surprise = SurpriseScorer::new();
    surprise.set_baseline(idx_insider, 0.1);
    surprise.set_baseline(idx_crash, 0.05);
    surprise.set_baseline(idx_pump, 0.15);

    // -- run setup: build state up to a decision point (tick 4) --
    let setup: Vec<Vec<(&str, &str, &str)>> = vec![
        vec![("insider_tip", "alice", "ACME"), ("trade", "carol", "ACME")],
        vec![
            ("promote", "carol", "ACME"),
            ("price_change", "market", "ACME"),
        ],
        vec![("alert", "system", "ACME")],
        vec![("insider_tip", "bob", "ZINC")],
    ];

    let mut event_id = 0;
    for (tick_idx, tick_events) in setup.iter().enumerate() {
        let tick = (tick_idx + 1) as i64;
        for &(action, actor, stock) in tick_events {
            let id = format!("ev{}", event_id);
            add_event(&mut graph, &id, action, actor, stock, tick);
            let events = engine.on_edge_added(
                &graph,
                &id,
                &"action".to_string(),
                &MemValue::Str(action.to_string()),
                &Interval::open(tick),
            );
            surprise.observe_events(&events, engine.patterns());
            event_id += 1;
        }
        surprise.tick();
        engine.end_tick(50);
        engine.drain_completed();
    }

    println!("=== state at decision point (tick 4) ===");
    println!("active partial matches: {}", engine.partial_matches().len());
    println!("tick counter: {}", engine.current_tick());

    // -- define 3 hypothetical actions at tick 5 --
    let hypotheses: Vec<(&str, &str, &str, &str)> = vec![
        ("A: alice trades ACME", "trade", "alice", "ACME"),
        ("B: bob trades ZINC", "trade", "bob", "ZINC"),
        ("C: carol sells ACME", "sell", "carol", "ACME"),
    ];

    // -- fork, evaluate, score each --
    println!("\n=== speculative evaluation (tick 5) ===");
    let mut results: Vec<(&str, Vec<String>, f64)> = Vec::new();

    for (label, action, actor, stock) in &hypotheses {
        let (completed, score) =
            evaluate_hypothesis(&engine, &graph, &surprise, "hyp", action, actor, stock, 5);
        println!("{}", label);
        if completed.is_empty() {
            println!("  no completions");
        } else {
            for p in &completed {
                println!("  completed: {}", p);
            }
        }
        println!("  surprise: {:.2} bits", score);
        results.push((label, completed, score));
    }

    // -- pick the most surprising --
    results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    println!("\nbest hypothesis: {}", results[0].0);

    // -- verify original engine is untouched --
    println!("\n=== original engine (unchanged) ===");
    println!("active partial matches: {}", engine.partial_matches().len());
    println!("tick counter: {}", engine.current_tick());
    let stale = engine.drain_completed();
    println!("completed in original: {}", stale.len());
    // #endregion

    assert_eq!(
        engine.current_tick(),
        4,
        "original engine tick should be unchanged"
    );
    assert_eq!(
        stale.len(),
        0,
        "original engine should have no new completions"
    );
}
