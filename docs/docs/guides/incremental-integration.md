---
sidebar_position: 2
title: Incremental Integration
---

# Incremental Integration

**Learning objective:** Wire fabula into an event-producing system and react to pattern matches as they occur.

## Prerequisites

- `fabula` and `fabula-memory` (or your adapter) in `Cargo.toml`
- Familiarity with `PatternBuilder` (see [Pattern Cookbook](./pattern-cookbook.md))
- A system that produces events one at a time (simulation tick loop, message queue, log tailer)

## Step 1: Create the engine and register patterns

```rust
use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

let mut engine: SiftEngine<MemGraph> = SiftEngine::new();

// Pattern: betrayal after hospitality
engine.register(
    PatternBuilder::new("violation_of_hospitality")
        .stage("e1", |s| s
            .edge("e1", "eventType".into(), MemValue::Str("enterTown".into()))
            .edge_bind("e1", "actor".into(), "guest"))
        .stage("e2", |s| s
            .edge("e2", "eventType".into(), MemValue::Str("showHospitality".into()))
            .edge_bind("e2", "actor".into(), "host")
            .edge_bind("e2", "target".into(), "guest"))
        .stage("e3", |s| s
            .edge("e3", "eventType".into(), MemValue::Str("harm".into()))
            .edge_bind("e3", "actor".into(), "host")
            .edge_bind("e3", "target".into(), "guest"))
        .unless_between("e1", "e3", |neg| neg
            .edge("eMid", "eventType".into(), MemValue::Str("leaveTown".into()))
            .edge_bind("eMid", "actor".into(), "guest"))
        .build(),
);

let mut graph = MemGraph::new();
```

Register all patterns before feeding events. You can register patterns at any time, but patterns registered after events have been processed will not retroactively match those events.

## Step 2: Feed events one at a time

Each simulation tick produces one or more edges. Add them to the graph and notify the engine.

```rust
// Simulation tick 1: Alice enters town
graph.add_str("ev1", "eventType", "enterTown", 1);
graph.add_ref("ev1", "actor", "alice", 1);
graph.set_time(1);

let events = engine.on_edge_added(
    &graph,
    &"ev1".into(),
    &"eventType".into(),
    &MemValue::Str("enterTown".into()),
    &Interval::open(1),
);

for event in &events {
    match event {
        SiftEvent::Advanced { pattern, match_id, stage_index } => {
            println!("Pattern '{}' advanced to stage {} (match #{})",
                     pattern, stage_index, match_id);
        }
        SiftEvent::Completed { pattern, match_id, bindings } => {
            println!("Pattern '{}' completed (match #{})", pattern, match_id);
            for (var, val) in bindings {
                println!("  {} = {:?}", var, val);
            }
        }
        SiftEvent::Negated { pattern, match_id, clause_label, trigger_source } => {
            println!("Pattern '{}' negated (match #{}) by {} from {:?}",
                     pattern, match_id, clause_label, trigger_source);
        }
    }
}
```

## Step 3: A full simulation loop

Here is a complete 10-event simulation that produces events and reacts to matches:

```rust
use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

fn main() {
    let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
    let mut graph = MemGraph::new();

    // Register the hospitality violation pattern
    engine.register(
        PatternBuilder::new("violation_of_hospitality")
            .stage("e1", |s| s
                .edge("e1", "eventType".into(), MemValue::Str("enterTown".into()))
                .edge_bind("e1", "actor".into(), "guest"))
            .stage("e2", |s| s
                .edge("e2", "eventType".into(), MemValue::Str("showHospitality".into()))
                .edge_bind("e2", "actor".into(), "host")
                .edge_bind("e2", "target".into(), "guest"))
            .stage("e3", |s| s
                .edge("e3", "eventType".into(), MemValue::Str("harm".into()))
                .edge_bind("e3", "actor".into(), "host")
                .edge_bind("e3", "target".into(), "guest"))
            .unless_between("e1", "e3", |neg| neg
                .edge("eMid", "eventType".into(), MemValue::Str("leaveTown".into()))
                .edge_bind("eMid", "actor".into(), "guest"))
            .build(),
    );

    // Simulated event stream
    let events = vec![
        ("ev1",  "eventType", "enterTown",       "actor", "alice",   None,                 1),
        ("ev2",  "eventType", "showHospitality",  "actor", "bob",     Some(("target", "alice")), 2),
        ("ev3",  "eventType", "enterTown",        "actor", "charlie", None,                 3),
        ("ev4",  "eventType", "showHospitality",  "actor", "dave",    Some(("target", "charlie")), 4),
        ("ev5",  "eventType", "trade",            "actor", "alice",   None,                 5),
        ("ev6",  "eventType", "leaveTown",        "actor", "charlie", None,                 6),
        ("ev7",  "eventType", "harm",             "actor", "bob",     Some(("target", "alice")), 7),
        ("ev8",  "eventType", "harm",             "actor", "dave",    Some(("target", "charlie")), 8),
        ("ev9",  "eventType", "enterTown",        "actor", "eve",     None,                 9),
        ("ev10", "eventType", "showHospitality",  "actor", "frank",   Some(("target", "eve")), 10),
    ];

    for (id, label, value, actor_label, actor, extra, time) in &events {
        graph.add_str(id, label, value, *time);
        graph.add_ref(id, actor_label, actor, *time);
        if let Some((extra_label, extra_target)) = extra {
            graph.add_ref(id, extra_label, extra_target, *time);
        }
        graph.set_time(*time);

        // Notify engine of the primary edge (eventType)
        let sift_events = engine.on_edge_added(
            &graph,
            &id.to_string(),
            &label.to_string(),
            &MemValue::Str(value.to_string()),
            &Interval::open(*time),
        );

        for se in &sift_events {
            match se {
                SiftEvent::Completed { pattern, bindings, .. } => {
                    println!("[t={}] MATCH: {}", time, pattern);
                    for (var, val) in bindings {
                        println!("       {} = {:?}", var, val);
                    }
                }
                SiftEvent::Negated { pattern, clause_label, .. } => {
                    println!("[t={}] NEGATED: {} (by {})", time, pattern, clause_label);
                }
                SiftEvent::Advanced { pattern, stage_index, .. } => {
                    println!("[t={}] ADVANCED: {} to stage {}", time, pattern, stage_index);
                }
            }
        }
    }

    // Drain completed matches
    let completed = engine.drain_completed();
    println!("\n{} completed matches drained", completed.len());
}
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
- **Active partial matches that will never complete** (stale matches) are not automatically garbage-collected. If you know a partial match can never complete (e.g., the simulation has moved far past the temporal window), there is currently no API to prune them.
- **Monitor `partial_matches().len()`** to track growth. If it grows without bound, you may have patterns with very common first stages that create many partial matches.

## Performance notes

- Each `on_edge_added` call iterates over all active partial matches (for negation and advancement) and all registered patterns (for initiation). The cost scales with `partial_matches * negation_clauses + patterns * first_stage_complexity`.
- The `DataSource` implementation dominates performance for large graphs. `MemGraph` does linear scans. Use an indexed adapter (or your own indexed implementation) for production workloads.
- Patterns with rare first stages produce fewer partial matches and run faster incrementally.
- Single-stage patterns complete immediately on match, so they never accumulate partial matches.
