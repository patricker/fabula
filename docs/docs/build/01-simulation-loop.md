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

```rust reference file=tests/build_ch01.rs#simulation_loop
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

[Next: Define Patterns ->](define-patterns)
