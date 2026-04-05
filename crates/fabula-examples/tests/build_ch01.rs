use fabula_memory::MemGraph;

// #region simulation_loop
fn add_event(g: &mut MemGraph, id: &str, action: &str, actor: &str, stock: &str, tick: i64) {
    g.add_str(id, "action", action, tick);
    g.add_ref(id, "actor", actor, tick);
    g.add_ref(id, "stock", stock, tick);
    g.set_time(tick);
}

fn build_market_graph() -> MemGraph {
    let mut graph = MemGraph::new();

    // Hardcoded event schedule: (action, actor, stock)
    let schedule: Vec<Vec<(&str, &str, &str)>> = vec![
        // tick 1
        vec![
            ("insider_tip", "alice", "ACME"),
            ("trade", "bob", "ZINC"),
            ("price_change", "market", "ACME"),
        ],
        // tick 2
        vec![
            ("trade", "alice", "ACME"),
            ("trade", "charlie", "ZINC"),
            ("alert", "system", "ACME"),
        ],
        // tick 3
        vec![
            ("trade", "bob", "ACME"),
            ("trade", "bob", "ACME"),
            ("price_change", "market", "ZINC"),
        ],
        // tick 4
        vec![
            ("insider_tip", "charlie", "ZINC"),
            ("alert", "system", "ZINC"),
            ("trade", "alice", "ACME"),
        ],
        // tick 5
        vec![
            ("trade", "bob", "ACME"),
            ("trade", "charlie", "ZINC"),
            ("price_change", "market", "ACME"),
        ],
        // tick 6
        vec![
            ("trade", "bob", "ACME"),
            ("alert", "system", "ACME"),
            ("trade", "alice", "ZINC"),
        ],
        // tick 7
        vec![
            ("price_change", "market", "ZINC"),
            ("trade", "charlie", "ZINC"),
            ("trade", "charlie", "ZINC"),
        ],
        // tick 8
        vec![
            ("insider_tip", "bob", "ACME"),
            ("trade", "alice", "ZINC"),
            ("alert", "system", "ACME"),
        ],
        // tick 9
        vec![
            ("trade", "bob", "ACME"),
            ("trade", "charlie", "ACME"),
            ("price_change", "market", "ZINC"),
        ],
        // tick 10
        vec![
            ("trade", "alice", "ACME"),
            ("alert", "system", "ZINC"),
            ("trade", "bob", "ZINC"),
        ],
    ];

    let mut event_id = 0;
    for (tick_idx, tick_events) in schedule.iter().enumerate() {
        let tick = (tick_idx + 1) as i64;
        println!("--- tick {} ---", tick);

        for (action, actor, stock) in tick_events {
            let id = format!("ev{}", event_id);
            add_event(&mut graph, &id, action, actor, stock, tick);
            println!("  {} {} {} on {}", id, actor, action, stock);
            event_id += 1;
        }
    }

    println!("\ntotal edges: {}", graph.edge_count());
    graph
}
// #endregion

#[test]
fn simulation_loop() {
    let graph = build_market_graph();
    assert_eq!(graph.edge_count(), 90);
}
