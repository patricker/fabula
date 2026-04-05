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

fn rushed_insider_pattern() -> Pattern<String, MemValue> {
    PatternBuilder::new("rushed_insider")
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
        .deadline(5)
        .build()
}

#[test]
fn react_to_events() {
    // #region react_to_events
    fn handle_events(events: &[SiftEvent<String, MemValue>]) {
        for event in events {
            match event {
                SiftEvent::Advanced {
                    pattern,
                    match_id,
                    stage_index,
                    ..
                } => {
                    println!(
                        "  [advance] {} (match {}) reached stage {}",
                        pattern, match_id, stage_index
                    );
                }
                SiftEvent::Completed {
                    pattern,
                    match_id,
                    bindings,
                    ..
                } => {
                    println!("  [COMPLETE] {} (match {})", pattern, match_id);
                    for (var, val) in bindings {
                        println!("    {} = {:?}", var, val);
                    }
                }
                SiftEvent::Negated {
                    pattern,
                    match_id,
                    clause_label,
                    trigger_source,
                    ..
                } => {
                    println!(
                        "  [negated] {} (match {}): killed by '{}' from {}",
                        pattern, match_id, clause_label, trigger_source
                    );
                }
                SiftEvent::Expired {
                    pattern,
                    match_id,
                    stage_reached,
                    ticks_elapsed,
                    ..
                } => {
                    println!(
                        "  [expired] {} (match {}): stuck at stage {} after {} ticks",
                        pattern, match_id, stage_reached, ticks_elapsed
                    );
                }
            }
        }
    }

    let mut graph = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    engine.register(insider_trading_pattern());
    engine.register(flash_crash_pattern());
    engine.register(pump_dump_pattern());
    engine.register(rushed_insider_pattern());

    let schedule: Vec<Vec<(&str, &str, &str)>> = vec![
        // tick 1: alice gets tipped on ACME, carol buys ACME
        vec![("insider_tip", "alice", "ACME"), ("trade", "carol", "ACME")],
        // tick 2: carol promotes ACME, price change on ACME
        vec![
            ("promote", "carol", "ACME"),
            ("price_change", "market", "ACME"),
        ],
        // tick 3: alert on ACME, alice trades ACME
        vec![("alert", "system", "ACME"), ("trade", "alice", "ACME")],
        // tick 4: carol sells ACME
        vec![("sell", "carol", "ACME")],
        // tick 5: bob gets tipped on ZINC (rushed_insider starts here)
        vec![("insider_tip", "bob", "ZINC")],
        // ticks 6-10: bob never trades ZINC -- the rushed_insider will expire
        vec![("trade", "dan", "BETA")],
        vec![("trade", "dan", "BETA")],
        vec![("trade", "dan", "BETA")],
        vec![("trade", "dan", "BETA")],
        vec![("trade", "dan", "BETA")],
        // tick 11: a late ZINC trade (after deadline)
        vec![("trade", "bob", "ZINC")],
    ];

    let mut all_completed: Vec<Match<String, MemValue, i64>> = Vec::new();
    let mut event_id = 0;

    for (tick_idx, tick_events) in schedule.iter().enumerate() {
        let tick = (tick_idx + 1) as i64;
        println!("=== tick {} ===", tick);

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
            handle_events(&events);
            event_id += 1;
        }

        let (delta, expired_events) = engine.end_tick(50);
        handle_events(&expired_events);

        if !delta.expired.is_empty() {
            println!("  tick expired: {:?}", delta.expired);
        }

        let before = engine.partial_matches().len();
        let completed = engine.drain_completed();
        let after = engine.partial_matches().len();
        if !completed.is_empty() {
            println!(
                "  drained {} match(es) ({} -> {} PMs)",
                completed.len(),
                before,
                after
            );
        }
        all_completed.extend(completed);

        println!("  active PMs: {}", delta.active_pm_count);
    }

    // Gap analysis on patterns that never completed
    println!("\n--- gap analysis ---");
    let completed_names: Vec<&str> = all_completed.iter().map(|m| m.pattern.as_str()).collect();
    for pat in engine.patterns() {
        if !completed_names.contains(&pat.name.as_str()) {
            if let Some(gap) = engine.why_not(&graph, &pat.name) {
                println!(
                    "'{}' closeness: {:.0}%",
                    gap.pattern,
                    gap.closeness() * 100.0
                );
                for stage in &gap.stages {
                    println!("  stage '{}': {:?}", stage.anchor, stage.status);
                    for clause in &stage.clauses {
                        let mark = if clause.matched { "ok" } else { "MISS" };
                        let reason = clause.reason.as_deref().unwrap_or("");
                        println!("    [{}] {} {}", mark, clause.description, reason);
                    }
                }
            }
        }
    }

    println!("\n--- summary ---");
    println!("total completed: {}", all_completed.len());
    for m in &all_completed {
        println!("  {}", m.pattern);
    }
    // #endregion

    assert!(
        !all_completed.is_empty(),
        "should have at least one completed match"
    );
}
