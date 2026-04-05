use fabula::prelude::*;
use fabula::compose;
use fabula_memory::{MemGraph, MemValue};

fn add_event(g: &mut MemGraph, id: &str, action: &str, actor: &str, stock: &str, tick: i64) {
    g.add_str(id, "action", action, tick);
    g.add_ref(id, "actor", actor, tick);
    g.add_ref(id, "stock", stock, tick);
    g.set_time(tick);
}

// #region insider_trading
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
// #endregion

// #region repeated_manipulation
fn repeated_manipulation_pattern() -> Pattern<String, MemValue> {
    let single_trade = PatternBuilder::new("single_trade")
        .stage("ev", |s| {
            s.edge("ev", "action".into(), MemValue::Str("trade".into()))
                .edge_bind("ev", "actor".into(), "manipulator")
                .edge_bind("ev", "stock".into(), "ticker")
        })
        .build();

    compose::repeat("repeated_manipulation", &single_trade, 3, &["manipulator", "ticker"])
}
// #endregion

// #region flash_crash
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
// #endregion

// #region putting_it_together
fn run_all_patterns() {
    let mut graph = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    engine.register(insider_trading_pattern());
    engine.register(repeated_manipulation_pattern());
    engine.register(flash_crash_pattern());

    println!("registered {} patterns:", engine.patterns().len());
    for (i, p) in engine.patterns().iter().enumerate() {
        println!("  [{}] {} ({} stages)", i, p.name, p.stages.len());
    }

    let schedule: Vec<Vec<(&str, &str, &str)>> = vec![
        vec![("insider_tip", "alice", "ACME"), ("trade", "bob", "ZINC"), ("price_change", "market", "ACME")],
        vec![("trade", "alice", "ACME"), ("trade", "charlie", "ZINC"), ("alert", "system", "ACME")],
        vec![("trade", "bob", "ACME"), ("trade", "bob", "ACME"), ("price_change", "market", "ZINC")],
        vec![("insider_tip", "charlie", "ZINC"), ("alert", "system", "ZINC"), ("trade", "alice", "ACME")],
        vec![("trade", "bob", "ACME"), ("trade", "charlie", "ZINC"), ("price_change", "market", "ACME")],
        vec![("trade", "bob", "ACME"), ("alert", "system", "ACME"), ("trade", "alice", "ZINC")],
        vec![("price_change", "market", "ZINC"), ("trade", "charlie", "ZINC"), ("trade", "charlie", "ZINC")],
        vec![("insider_tip", "bob", "ACME"), ("trade", "alice", "ZINC"), ("alert", "system", "ACME")],
        vec![("trade", "bob", "ACME"), ("trade", "charlie", "ACME"), ("price_change", "market", "ZINC")],
        vec![("trade", "alice", "ACME"), ("alert", "system", "ZINC"), ("trade", "bob", "ZINC")],
    ];

    let mut event_id = 0;
    for (tick_idx, tick_events) in schedule.iter().enumerate() {
        let tick = (tick_idx + 1) as i64;
        for (action, actor, stock) in tick_events {
            let id = format!("ev{}", event_id);
            add_event(&mut graph, &id, action, actor, stock, tick);
            event_id += 1;
        }
    }

    println!("\ngraph has {} edges", graph.edge_count());
    println!("engine ready for incremental matching");
}
// #endregion

#[test]
fn insider_trading() {
    let p = insider_trading_pattern();
    assert_eq!(p.name, "insider_trading");
    assert_eq!(p.stages.len(), 2);
}

#[test]
fn repeated_manipulation() {
    let p = repeated_manipulation_pattern();
    assert_eq!(p.name, "repeated_manipulation");
    assert_eq!(p.stages.len(), 3);
}

#[test]
fn flash_crash() {
    let p = flash_crash_pattern();
    assert_eq!(p.name, "flash_crash");
    assert_eq!(p.stages.len(), 3);
}

#[test]
fn putting_it_together() {
    run_all_patterns();
}
