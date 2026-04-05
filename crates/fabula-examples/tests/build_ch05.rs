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
                s.edge("drop", "action".into(), MemValue::Str("price_change".into()))
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
fn score_and_rank() {
    // #region score_and_rank
    /// Extract categorical properties from a match's bindings.
    /// Use roles and categories, never raw entity IDs.
    fn extract_properties(m: &Match<String, MemValue, i64>) -> Vec<String> {
        let mut props = Vec::new();
        for (var, val) in &m.bindings {
            if let BoundValue::Node(id) = val {
                if id.chars().next().is_some_and(|c| c.is_uppercase()) {
                    props.push(format!("{}_sector=equity", var));
                } else {
                    props.push(format!("{}_role=trader", var));
                }
            }
        }
        props
    }

    let mut graph = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    let idx_insider = engine.register(insider_trading_pattern());
    let idx_crash = engine.register(flash_crash_pattern());
    let idx_pump = engine.register(pump_dump_pattern());

    // -- set up scorers --
    let mut surprise = SurpriseScorer::new();
    surprise.set_baseline(idx_insider, 0.1);
    surprise.set_baseline(idx_crash, 0.05);
    surprise.set_baseline(idx_pump, 0.15);

    let mut stu_mean = StuScorer::new();
    let mut stu_tfidf = StuScorer::new().with_aggregation(StuAggregation::TfIdf);
    let mut sequential = SequentialScorer::new();

    // -- simulation: 10 ticks of market activity --
    let schedule: Vec<Vec<(&str, &str, &str)>> = vec![
        vec![("insider_tip", "alice", "ACME"), ("trade", "carol", "ACME")],
        vec![("promote", "carol", "ACME"), ("price_change", "market", "ACME")],
        vec![("trade", "alice", "ACME"), ("alert", "system", "ACME")],
        vec![("sell", "carol", "ACME"), ("price_change", "market", "ZINC")],
        vec![("insider_tip", "bob", "ZINC"), ("alert", "system", "ZINC")],
        vec![("trade", "bob", "ZINC"), ("trade", "dan", "ACME")],
        vec![("promote", "dan", "ACME"), ("insider_tip", "eve", "BETA")],
        vec![("trade", "eve", "BETA"), ("sell", "dan", "ACME")],
        vec![("price_change", "market", "BETA"), ("alert", "system", "BETA")],
        vec![("trade", "frank", "BETA")],
    ];

    let mut all_completed: Vec<Match<String, MemValue, i64>> = Vec::new();
    let mut last_pattern: Option<String> = None;
    let mut event_id = 0;

    for (tick_idx, tick_events) in schedule.iter().enumerate() {
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
        let (_delta, _expired) = engine.end_tick(50);

        let completed = engine.drain_completed();
        for m in &completed {
            let props = extract_properties(m);
            stu_mean.observe_one(&m.pattern, &props);
            stu_tfidf.observe_one(&m.pattern, &props);

            if let Some(ref prev) = last_pattern {
                sequential.observe_transition(prev, &m.pattern);
            }
            last_pattern = Some(m.pattern.clone());
        }
        all_completed.extend(completed);
    }

    // -- SurpriseScorer results --
    println!("=== Pattern-Level Surprise (SurpriseScorer) ===");
    let scored = surprise.score(&all_completed, engine.patterns());
    for sm in &scored {
        println!("  {} -> {:.2} bits", sm.pattern, sm.surprise);
    }

    // -- StuScorer: ArithmeticMean vs TfIdf side by side --
    println!("\n=== Property-Level Surprise (StuScorer) ===");
    println!("{:<20} {:>12} {:>12}", "match", "ArithMean", "TfIdf");
    println!("{}", "-".repeat(46));

    let with_props: Vec<(Match<String, MemValue, i64>, Vec<String>)> = all_completed
        .iter()
        .map(|m| (m.clone(), extract_properties(m)))
        .collect();

    let scored_mean = stu_mean.score(&with_props);
    let scored_tfidf = stu_tfidf.score(&with_props);

    for (sm, st) in scored_mean.iter().zip(scored_tfidf.iter()) {
        println!("{:<20} {:>12.4} {:>12.4}", sm.pattern, sm.stu_score, st.stu_score);
        if !sm.property_frequencies.is_empty() {
            println!("  rarest: {} ({:.3})", sm.property_frequencies[0].0, sm.property_frequencies[0].1);
        }
    }

    // -- SequentialScorer results --
    println!("\n=== Sequential Surprise ===");
    let names: Vec<&str> = all_completed.iter().map(|m| m.pattern.as_str()).collect();
    for pair in names.windows(2) {
        let score = sequential.score_transition(pair[0], pair[1]);
        println!("  {} -> {} : {:.2} bits", pair[0], pair[1], score);
    }

    println!("\ntotal completed: {}", all_completed.len());
    // #endregion

    assert!(!all_completed.is_empty(), "should have at least one completed match");
}
