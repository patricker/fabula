---
sidebar_position: 4
title: "3. Incremental Matching"
---

# Chapter 3: Incremental Matching

Wire the engine into the simulation loop. For each event, call `on_edge_added` with the action edge (the trigger clause). At the end of each tick, call `end_tick` and print the delta.

## How incremental matching works

Each `on_edge_added` call runs four phases:

1. **Negation check** -- kill active partial matches whose negation window is violated
2. **Initiation** -- try to start new partial matches at stage 0
3. **Advancement** -- try to advance existing partial matches to their next stage
4. **Cleanup** -- dedup, remove dead matches, handle exclusive groups

The trigger edge is the first clause of the stage being matched. Secondary clauses (actor, stock) are resolved by querying the graph. This is why we add all edges to the graph _before_ calling `on_edge_added`.

After processing all events in a tick, `end_tick` returns a `TickDelta` summarizing what advanced, completed, or was negated, plus a vec of expiry events.

## Complete code

```rust
use fabula::prelude::*;
use fabula::compose;
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

    compose::repeat("repeated_manipulation", &single_trade, 3, &["manipulator", "ticker"])
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

fn main() {
    let mut graph = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    engine.register(insider_trading_pattern());
    engine.register(repeated_manipulation_pattern());
    engine.register(flash_crash_pattern());

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
                    SiftEvent::Advanced { pattern, stage_index, .. } => {
                        println!("  >> {}: {} advanced stage {}", id, pattern, stage_index);
                    }
                    SiftEvent::Completed { pattern, bindings, .. } => {
                        let summary: Vec<String> = bindings.iter()
                            .filter(|(_, v)| matches!(v, BoundValue::Node(_)))
                            .map(|(k, v)| format!("{}={:?}", k, v))
                            .collect();
                        println!("  ** {}: {} COMPLETED [{}]", id, pattern, summary.join(", "));
                    }
                    SiftEvent::Negated { pattern, clause_label, .. } => {
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
}
```

## How the key detections work

**Insider trading (alice/ACME):** At tick 1, alice receives an insider_tip on ACME. The engine initiates a partial match at stage 0. At tick 2, alice trades ACME. The action edge triggers advancement to stage 1, which completes the pattern. The negation window (no alert on ACME between the tip and trade) is satisfied because the ACME alert at tick 2 is processed _after_ the trade edge completes the match.

**Insider trading (charlie/ZINC):** At tick 4, charlie gets an insider_tip on ZINC. The alert on ZINC arrives at the same tick. The negation window has exclusive start: the alert's start time (4) is not strictly greater than the tip's start time (4), so the negation does not fire. At tick 5, charlie trades ZINC and the pattern completes.

**Flash crash (ACME):** The price_change at tick 1 and alert at tick 2 satisfy the unordered group (both for ACME). The concurrent group completes when the second signal arrives. Then any ACME trade in a later tick completes the third stage.

**Repeated manipulation (bob/ACME):** Bob trades ACME at ticks 3, 5, and 6. Each trade advances the 3-stage repeat pattern. The third trade completes it.

## Expected output

The exact output depends on edge processing order and dedup, but the structure is:

```text
=== tick 1 ===
  >> ev0: insider_trading advanced stage 0
  >> ev1: repeated_manipulation advanced stage 0
  >> ev2: flash_crash advanced stage 0
  advanced: ["flash_crash", "insider_trading", "repeated_manipulation"]
  active PMs: 3
=== tick 2 ===
  ** ev3: insider_trading COMPLETED [suspect=Node("alice"), ticker=Node("ACME")]
  >> ev3: repeated_manipulation advanced stage 0
  >> ev3: flash_crash advanced stage 1
  >> ev4: repeated_manipulation advanced stage 0
  >> ev5: flash_crash advanced stage 1
  advanced: ["flash_crash", "repeated_manipulation"]
  completed: ["insider_trading"]
  active PMs: 7
=== tick 3 ===
  >> ev6: repeated_manipulation advanced stage 1
  >> ev6: flash_crash advanced stage 2
  ** ev6: flash_crash COMPLETED [ticker=Node("ACME")]
  advanced: ["flash_crash", "repeated_manipulation"]
  completed: ["flash_crash"]
  active PMs: 9
...
```

After 10 ticks, you will see multiple `insider_trading` completions (alice/ACME, charlie/ZINC, bob/ACME), `flash_crash` completions for both ACME and ZINC, and `repeated_manipulation` completions for actors with 3+ trades on the same stock. The `negated` field shows any PMs killed by intervening alerts.

## What you learned

- `on_edge_added` takes 5 arguments: the data source, the source node, label, value, and interval of the new edge
- The first clause of each stage is the trigger -- pass that edge's label and value
- Secondary clauses are resolved by querying the graph, so add all edges _before_ calling `on_edge_added`
- `end_tick` returns a `TickDelta` summarizing the tick and clears internal accumulators
- `SiftEvent::Advanced` fires when a partial match moves to the next stage
- `SiftEvent::Completed` fires when all stages are satisfied
- `SiftEvent::Negated` fires when a negation window kills a partial match
- Negation windows have exclusive start boundaries: an event at the exact same timestamp as the window start does not trigger the negation

[Next: React to Events ->](04-react-to-events)
