---
sidebar_position: 5
title: "4. React to Events"
---

# 4. React to Events

:::caution Different event schedule
This chapter uses a different event schedule from chapters 1-3. The patterns and graph events below are new — don't expect continuity with the trading simulation from earlier chapters.
:::

Chapter 3 wired the engine into the simulation loop. You saw events scroll by as they happened. Now you will handle them properly: dispatch on each `SiftEvent` variant, drain completed matches to manage memory, add a deadline pattern that expires, and run gap analysis to diagnose near-misses.

## Handle each SiftEvent variant

Each variant carries different diagnostic data. `Completed` gives you bindings. `Negated` tells you which clause killed the match and where the trigger came from. `Expired` reports how far the match got and how many ticks elapsed.

## Drain completed matches

After processing each tick, call `drain_completed()` to move finished matches out of the engine. This bounds memory by removing `MatchState::Complete` entries from the internal partial-match list. The returned `Vec<Match>` is yours to keep for scoring in chapter 5.

## Gap analysis

After the simulation, call `why_not()` on any pattern that never completed. It returns a clause-by-clause breakdown: which clauses matched, which missed, and why. `closeness()` gives a 0.0--1.0 score for how close the pattern came.

## Deadline expiry

Add a fourth pattern with `.deadline(5)`. If a partial match does not complete within 5 ticks of its creation, `end_tick()` emits `SiftEvent::Expired` and kills the PM.

## Complete code

```rust reference file=tests/build_ch04.rs#react_to_events
```

## Expected output

The exact match IDs and PM counts will vary. The key results:

```text
=== tick 1 ===
  [advance] insider_trading (match 0) reached stage 0
  [advance] pump_and_dump (match ...) reached stage 0
  [advance] rushed_insider (match ...) reached stage 0
  ...
=== tick 3 ===
  [negated] insider_trading (match 0): killed by 'action' from ev4
  ...
=== tick 4 ===
  [COMPLETE] pump_and_dump (match ...)
    manipulator = Node("carol")
    ticker = Node("ACME")
  drained 1 match(es) (... -> ... PMs)
  ...
=== tick 5 ===
  [advance] rushed_insider (match ...) reached stage 0
  ...
=== tick 10 ===
  [expired] rushed_insider (match ...): stuck at stage 1 after 6 ticks
  tick expired: ["rushed_insider"]
  ...
=== tick 11 ===
  ...

--- gap analysis ---
'...' closeness: ...%
  stage '...': ...

--- summary ---
total completed: ...
  pump_and_dump
  ...
```

Key outcomes:
- **insider_trading** is negated when the ACME alert fires between the tip and trade
- **pump_and_dump** completes when carol buys, promotes, then sells ACME
- **rushed_insider** starts at tick 5 for bob/ZINC, then expires at tick 10 (5 ticks elapsed, deadline exceeded)
- **Gap analysis** shows clause-by-clause breakdown for patterns that never completed

## What you learned

- **SiftEvent dispatch** -- each variant carries different diagnostic data. `Completed` gives you bindings; `Negated` tells you which clause killed it; `Expired` reports progress and elapsed time.
- **drain_completed()** -- removes finished matches from the engine and returns them. Call each tick to bound memory.
- **end_tick(threshold)** -- finalizes the tick, checks deadlines, and returns `(TickDelta, Vec<SiftEvent::Expired>)`.
- **Deadline patterns** -- `.deadline(N)` on a pattern causes partial matches to expire after N ticks of `end_tick()` calls.
- **Gap analysis** -- `why_not(&graph, name)` returns a clause-by-clause breakdown. `closeness()` quantifies how close the pattern came (0.0 to 1.0).

[Next: Score and Rank ->](score-and-rank)
