---
sidebar_position: 3
title: "2. Define Patterns"
---

# Chapter 2: Define Patterns

Three detection patterns, shown in both the builder API and the DSL. Building on the simulation from [Chapter 1](01-simulation-loop).

## Pattern 1: Insider Trading

Someone receives an insider tip about a stock, then trades that stock. No public disclosure (alert) in between. Two stages plus a negation window.

**Builder API:**

```rust
use fabula::prelude::*;
use fabula_memory::MemValue;

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
```

**DSL equivalent:**

```text
pattern insider_trading {
    stage tip {
        tip.action = "insider_tip"
        tip.actor -> ?suspect
        tip.stock -> ?ticker
    }
    stage trade {
        trade.action = "trade"
        trade.actor -> ?suspect
        trade.stock -> ?ticker
    }
    unless between tip trade {
        disclosure.action = "alert"
        disclosure.stock -> ?ticker
    }
}
```

Two stages: `tip` then `trade`, joined on `?suspect` (same actor) and `?ticker` (same stock). The negation window kills the match if an alert fires for that stock between the tip and the trade.

## Pattern 2: Repeated Manipulation

Same actor makes 3 or more trades on the same stock. Uses `compose::repeat` with shared variables.

**Builder API:**

```rust
use fabula::compose;
use fabula_memory::MemValue;

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
```

**DSL equivalent:**

```text
pattern single_trade {
    stage ev {
        ev.action = "trade"
        ev.actor -> ?manipulator
        ev.stock -> ?ticker
    }
}

compose repeated_manipulation = single_trade * 3 sharing(manipulator, ticker)
```

The `repeat` operator creates 3 copies of the stage with prefixed variables (`rep0_ev`, `rep1_ev`, `rep2_ev`), but `manipulator` and `ticker` stay shared across all repetitions -- forcing the same actor and stock in all three trades.

## Pattern 3: Flash Crash Signals

A price drop and an alert fire for the same stock (in either order), then a large sell follows. Uses an unordered group for the concurrent events, then a sequential stage.

**Builder API:**

```rust
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
```

**DSL equivalent:**

```text
pattern flash_crash {
    concurrent {
        stage drop {
            drop.action = "price_change"
            drop.stock -> ?ticker
        }
        stage alarm {
            alarm.action = "alert"
            alarm.stock -> ?ticker
        }
    }
    stage sell {
        sell.action = "trade"
        sell.stock -> ?ticker
    }
}
```

The `concurrent` block means `drop` and `alarm` can match in any order. Once both are satisfied, the engine advances to `sell`. All three stages share `?ticker`.

## Putting it together

Here is the complete code from Chapter 1 with all three patterns registered on an engine.

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
```

## Expected output

```text
registered 3 patterns:
  [0] insider_trading (2 stages)
  [1] repeated_manipulation (3 stages)
  [2] flash_crash (3 stages)

graph has 90 edges
engine ready for incremental matching
```

## What you learned

- `PatternBuilder` constructs patterns with stages, variable bindings, and negation windows
- `edge` matches a literal value; `edge_bind` binds the target to a variable for joins
- `unless_between` defines a negation window between two stage anchors
- `compose::repeat` creates multi-repetition patterns with shared variables
- `unordered_group` allows stages to match in any order (concurrent events)
- The DSL and builder API produce identical `Pattern` structs

[Next: Incremental Matching ->](03-incremental-matching)
