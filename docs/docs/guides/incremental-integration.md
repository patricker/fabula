---
sidebar_position: 2
title: Incremental Integration
---

# Incremental Integration

**Learning objective:** Wire fabula into an event-producing system and react to pattern matches as they occur.

## Prerequisites

- `fabula` and `fabula-memory` (or your adapter) in `Cargo.toml`
- Familiarity with `PatternBuilder` (see [Pattern Cookbook](./pattern-cookbook))
- A system that produces events one at a time (simulation tick loop, message queue, log tailer)

## Step 1: Create the engine and register patterns

```rust reference file=tests/guides_incremental_integration.rs#step1_register
```

Register all patterns before feeding events. You can register patterns at any time, but patterns registered after events have been processed will not retroactively match those events. See the [Engine Reference](../reference/engine) for the full `SiftEngine` API.

## Step 2: Feed events one at a time

Each simulation tick produces one or more edges. Add them to the graph and notify the engine.

```rust reference file=tests/guides_incremental_integration.rs#step2_feed_events
```

## Step 3: A full simulation loop

Here is a complete 10-event simulation that produces events and reacts to matches:

```rust reference file=tests/guides_incremental_integration.rs#step3_simulation_loop
```

Expected output (approximate):

```
[t=1] ADVANCED: violation_of_hospitality to stage 0
[t=2] ADVANCED: violation_of_hospitality to stage 1
[t=3] ADVANCED: violation_of_hospitality to stage 0
[t=4] ADVANCED: violation_of_hospitality to stage 1
[t=6] NEGATED: violation_of_hospitality (by "eventType")
[t=7] MATCH: violation_of_hospitality
       guest = Node("alice")
       host = Node("bob")

1 completed matches drained
```

Alice's hospitality violation completes at t=7. Charlie's is negated at t=6 (charlie leaves town). Dave's harm at t=8 finds no active partial match for charlie (already negated).

## When to call drain_completed

`drain_completed` removes Complete partial matches from the engine and returns them. Call it:

- **After each tick** if you process matches immediately. This keeps memory bounded.
- **Periodically** (every N ticks) if matches are batched for later processing.
- **At the end** if you only care about the final set of matches.

Completed matches are inert -- they do not participate in further matching. But they consume memory until drained.

```rust
// After a batch of edges:
let completed = engine.drain_completed();
for m in completed {
    handle_match(&m);
}
```

## Handling late-arriving edges

Fabula's incremental engine assumes edges arrive in temporal order. If an edge arrives with a timestamp earlier than previously processed edges, it can cause issues:

- The edge may not advance partial matches that have already moved past that stage (temporal ordering requires strict inequality on start times).
- Negation windows may not trigger correctly if the negating event's timestamp falls before the window's start.

If your system produces out-of-order events, consider:

1. **Buffering** events and sorting by timestamp before feeding them to the engine.
2. **Using batch evaluation** on the accumulated graph periodically, rather than relying on incremental matching alone.
3. **Using coarse time granularity** so that events within the same tick are processed together.

## Memory management

The number of active partial matches grows as events arrive and patterns partially match. In a long-running simulation:

- **Drain completed matches** regularly (see above).
- **Dead matches are cleaned automatically** -- the engine removes them at the end of each `on_edge_added` call.
- **Active partial matches that will never complete** (stale matches) are not automatically garbage-collected unless you set a `deadline_ticks` on the pattern. Use `PatternBuilder::deadline(ticks)` or the DSL `deadline N` syntax to automatically expire PMs that have been alive too long.
- **Monitor `partial_matches().len()`** to track growth. If it grows without bound, you may have patterns with very common first stages that create many partial matches. Consider adding deadlines to these patterns.

## Tick deltas and scoring

For GM-style integration (MCTS evaluation, narrative quality scoring), use `end_tick()` to get a per-tick summary and any expiry events:

```rust reference file=tests/guides_incremental_integration.rs#tick_delta
```

Feed the delta into `fabula-narratives` for composite scoring. This requires setting up the narrative trackers before the tick loop and updating them each tick:

```rust reference file=tests/guides_incremental_integration.rs#narrative_scoring
```

See the [Scoring Reference](../reference/scoring) and [Narrative Scoring Reference](../reference/narratives) for full API details.

## Performance notes

- Each `on_edge_added` call iterates over all active partial matches (for negation and advancement) and all registered patterns (for initiation). The cost scales with `partial_matches * negation_clauses + patterns * first_stage_complexity`.
- The `DataSource` implementation dominates performance for large graphs. `MemGraph` does linear scans. Use an indexed adapter (or your own indexed implementation) for production workloads.
- Patterns with rare first stages produce fewer partial matches and run faster incrementally.
- Single-stage patterns complete immediately on match, so they never accumulate partial matches.
