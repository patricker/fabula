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

```rust reference file=tests/build_ch03.rs#incremental_matching
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

[Next: React to Events ->](react-to-events)
