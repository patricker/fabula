---
sidebar_position: 2
title: "1. Simulation Loop"
---

# Chapter 1: Simulation Loop

Build a trading simulation that generates timestamped events into a `MemGraph`. No pattern matching yet -- just a deterministic event stream.

## The graph model

Each event is a node with edges describing it:

- `ev.action = "trade"` -- what happened
- `ev.actor -> alice` -- who did it
- `ev.stock -> ACME` -- what it involved

We use `MemGraph` as the backing store. Each edge gets an open-ended interval starting at the tick it was created.

## The simulation

Ten ticks. Three events per tick. Three actors (alice, bob, charlie) performing four actions (trade, price_change, alert, insider_tip). All hardcoded for reproducibility.

```rust
use fabula_memory::MemGraph;

fn add_event(g: &mut MemGraph, id: &str, action: &str, actor: &str, stock: &str, tick: i64) {
    g.add_str(id, "action", action, tick);
    g.add_ref(id, "actor", actor, tick);
    g.add_ref(id, "stock", stock, tick);
    g.set_time(tick);
}

fn main() {
    let mut graph = MemGraph::new();

    // Hardcoded event schedule: (action, actor, stock)
    let schedule: Vec<Vec<(&str, &str, &str)>> = vec![
        // tick 1
        vec![("insider_tip", "alice", "ACME"), ("trade", "bob", "ZINC"), ("price_change", "market", "ACME")],
        // tick 2
        vec![("trade", "alice", "ACME"), ("trade", "charlie", "ZINC"), ("alert", "system", "ACME")],
        // tick 3
        vec![("trade", "bob", "ACME"), ("trade", "bob", "ACME"), ("price_change", "market", "ZINC")],
        // tick 4
        vec![("insider_tip", "charlie", "ZINC"), ("alert", "system", "ZINC"), ("trade", "alice", "ACME")],
        // tick 5
        vec![("trade", "bob", "ACME"), ("trade", "charlie", "ZINC"), ("price_change", "market", "ACME")],
        // tick 6
        vec![("trade", "bob", "ACME"), ("alert", "system", "ACME"), ("trade", "alice", "ZINC")],
        // tick 7
        vec![("price_change", "market", "ZINC"), ("trade", "charlie", "ZINC"), ("trade", "charlie", "ZINC")],
        // tick 8
        vec![("insider_tip", "bob", "ACME"), ("trade", "alice", "ZINC"), ("alert", "system", "ACME")],
        // tick 9
        vec![("trade", "bob", "ACME"), ("trade", "charlie", "ACME"), ("price_change", "market", "ZINC")],
        // tick 10
        vec![("trade", "alice", "ACME"), ("alert", "system", "ZINC"), ("trade", "bob", "ZINC")],
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
}
```

## Expected output

```text
--- tick 1 ---
  ev0 alice insider_tip on ACME
  ev1 bob trade on ZINC
  ev2 market price_change on ACME
--- tick 2 ---
  ev3 alice trade on ACME
  ev4 charlie trade on ZINC
  ev5 system alert on ACME
--- tick 3 ---
  ev6 bob trade on ACME
  ev7 bob trade on ACME
  ev8 market price_change on ZINC
--- tick 4 ---
  ev9 charlie insider_tip on ZINC
  ev10 system alert on ZINC
  ev11 alice trade on ACME
--- tick 5 ---
  ev12 bob trade on ACME
  ev13 charlie trade on ZINC
  ev14 market price_change on ACME
--- tick 6 ---
  ev15 bob trade on ACME
  ev16 system alert on ACME
  ev17 alice trade on ZINC
--- tick 7 ---
  ev18 market price_change on ZINC
  ev19 charlie trade on ZINC
  ev20 charlie trade on ZINC
--- tick 8 ---
  ev21 bob insider_tip on ACME
  ev22 alice trade on ZINC
  ev23 system alert on ACME
--- tick 9 ---
  ev24 bob trade on ACME
  ev25 charlie trade on ACME
  ev26 market price_change on ZINC
--- tick 10 ---
  ev27 alice trade on ACME
  ev28 system alert on ZINC
  ev29 bob trade on ZINC

total edges: 90
```

## What you learned

- `MemGraph` stores edges as `(source, label, target, interval)` tuples
- `add_str`, `add_ref` are convenience methods for string and node-reference edges
- `set_time` advances the graph's clock (used by temporal queries later)
- Each event node gets 3 edges (action, actor, stock), so 30 events = 90 edges

[Next: Define Patterns ->](02-define-patterns)
