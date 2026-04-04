---
sidebar_position: 5
title: Forking for MCTS
---

# Forking for MCTS

**Learning objective:** Clone the engine for speculative evaluation, score hypothetical outcomes, and select the best action.

## Prerequisites

- Working incremental evaluation loop (see [Incremental Integration](./incremental-integration))
- Narrative scoring basics (see [Incremental Integration: Tick deltas and scoring](./incremental-integration#tick-deltas-and-scoring))
- `fabula`, `fabula-memory`, and `fabula-narratives` in your `Cargo.toml`

## Step 1: Clone the engine

`SiftEngine` implements a manual `Clone` that copies all matching state -- patterns, partial matches, enabled flags, stats, tick counter -- but intentionally resets tick accumulators to empty. The forked engine starts with a clean slate for its first tick.

```rust
let mut fork = engine.clone();
```

The clone is fully independent. Advancing or mutating the clone has no effect on the original.

## Step 2: Fork the data source

Create a fresh graph (or clone one) for the speculative branch. The forked graph receives hypothetical events that may never happen in the real simulation.

```rust
let mut fork_graph = graph.clone();
```

`MemGraph` derives `Clone`, so this copies all existing edges. If your data source does not support clone, construct a new one and replay the relevant edges.

## Step 3: Speculate

Add hypothetical edges to the forked graph and feed them to the cloned engine.

```rust
fork_graph.add_str("hyp1", "eventType", "betray", 100);
fork_graph.add_ref("hyp1", "actor", "bob", 100);
fork_graph.add_ref("hyp1", "target", "alice", 100);
fork_graph.set_time(100);

let events = fork.on_edge_added(
    &fork_graph,
    &"hyp1".into(),
    &"eventType".into(),
    &MemValue::Str("betray".into()),
    &Interval::open(100),
);
```

The returned `Vec<SiftEvent>` tells you what happened: which patterns advanced, completed, or were negated by this hypothetical edge.

## Step 4: Score

Call `end_tick()` on the fork to get a `TickDelta`, then assemble signals and score.

```rust
let (delta, _expired) = fork.end_tick(50);

let signals = assemble_signals(
    &delta,
    &fork.plant_status(50),
    0,                    // filo violations
    Trajectory::Unknown,  // tension trajectory
    Trajectory::Rising,   // desired trajectory
    0.0,                  // pivot magnitude
    0.0,                  // surprise
    0.0,                  // sequential surprise
);
let result = score(&signals, &NarrativeWeights::default());
// result.total is the composite narrative quality score
```

## Step 5: Compare candidates

Evaluate multiple hypothetical actions, each on its own fork, and pick the best.

```rust
let candidates = vec![
    ("betray",   "bob",   "alice"),
    ("forgive",  "bob",   "alice"),
    ("conspire", "bob",   "charlie"),
];

let mut best_score = f64::NEG_INFINITY;
let mut best_action = "";

for (action, actor, target) in &candidates {
    let mut fork = engine.clone();
    let mut fork_graph = graph.clone();

    fork_graph.add_str("hyp", "eventType", action, 100);
    fork_graph.add_ref("hyp", "actor", actor, 100);
    fork_graph.add_ref("hyp", "target", target, 100);
    fork_graph.set_time(100);

    fork.on_edge_added(
        &fork_graph,
        &"hyp".into(),
        &"eventType".into(),
        &MemValue::Str(action.to_string()),
        &Interval::open(100),
    );

    let (delta, _) = fork.end_tick(50);
    let signals = assemble_signals(
        &delta,
        &fork.plant_status(50),
        0, Trajectory::Unknown, Trajectory::Rising, 0.0, 0.0, 0.0,
    );
    let result = score(&signals, &NarrativeWeights::default());

    if result.total > best_score {
        best_score = result.total;
        best_action = action;
    }
}
// best_action is the narratively strongest candidate
```

## Step 6: Discard

When a fork goes out of scope, Rust drops it. No cleanup, no deregistration. The original engine and graph are untouched.

```rust
{
    let fork = engine.clone();
    let fork_graph = graph.clone();
    // ... speculate, score ...
} // fork and fork_graph dropped here
```

## Complete example

A single `fn main()` that sets up two patterns, runs a short simulation, then evaluates three candidate actions and selects the best.

```rust
use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};
use fabula_narratives::scorer::{assemble_signals, score, NarrativeWeights};
use fabula_narratives::tension::Trajectory;

fn main() {
    // -- Setup patterns --------------------------------------------------
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    // Pattern: betrayal after hospitality (3 stages)
    let hospitality_idx = engine.register(
        PatternBuilder::new("hospitality_violation")
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
            .build(),
    );

    // Pattern: forgiveness arc (2 stages)
    let forgiveness_idx = engine.register(
        PatternBuilder::new("forgiveness_arc")
            .stage("e1", |s| s
                .edge("e1", "eventType".into(), MemValue::Str("harm".into()))
                .edge_bind("e1", "actor".into(), "offender")
                .edge_bind("e1", "target".into(), "victim"))
            .stage("e2", |s| s
                .edge("e2", "eventType".into(), MemValue::Str("forgive".into()))
                .edge_bind("e2", "actor".into(), "victim")
                .edge_bind("e2", "target".into(), "offender"))
            .build(),
    );

    // Plant/payoff: hospitality setup, forgiveness resolution
    engine.register_plant_payoff(hospitality_idx, forgiveness_idx, None);

    // -- Simulate a few ticks -------------------------------------------
    let mut graph = MemGraph::new();

    // Tick 1: Alice enters town
    graph.add_str("ev1", "eventType", "enterTown", 1);
    graph.add_ref("ev1", "actor", "alice", 1);
    graph.set_time(1);
    engine.on_edge_added(
        &graph, &"ev1".into(), &"eventType".into(),
        &MemValue::Str("enterTown".into()), &Interval::open(1),
    );
    engine.end_tick(50);

    // Tick 2: Bob shows hospitality to Alice
    graph.add_str("ev2", "eventType", "showHospitality", 2);
    graph.add_ref("ev2", "actor", "bob", 2);
    graph.add_ref("ev2", "target", "alice", 2);
    graph.set_time(2);
    engine.on_edge_added(
        &graph, &"ev2".into(), &"eventType".into(),
        &MemValue::Str("showHospitality".into()), &Interval::open(2),
    );
    engine.end_tick(50);

    // At this point, hospitality_violation has an active PM at stage 2,
    // waiting for a "harm" event from bob targeting alice.

    // -- Fork-speculate-score loop --------------------------------------
    let candidates: Vec<(&str, &str, &str)> = vec![
        ("harm",     "bob", "alice"),   // completes hospitality_violation
        ("forgive",  "alice", "bob"),   // no pattern effect (no prior harm)
        ("trade",    "bob", "alice"),   // neutral — advances nothing
    ];

    let weights = NarrativeWeights::default();
    let mut best_score = f64::NEG_INFINITY;
    let mut best_action = "";

    for (action, actor, target) in &candidates {
        // Fork
        let mut fork = engine.clone();
        let mut fork_graph = graph.clone();

        // Speculate
        fork_graph.add_str("hyp", "eventType", action, 3);
        fork_graph.add_ref("hyp", "actor", actor, 3);
        fork_graph.add_ref("hyp", "target", target, 3);
        fork_graph.set_time(3);

        fork.on_edge_added(
            &fork_graph,
            &"hyp".into(),
            &"eventType".into(),
            &MemValue::Str(action.to_string()),
            &Interval::open(3),
        );

        // Score
        let (delta, _) = fork.end_tick(50);
        let signals = assemble_signals(
            &delta,
            &fork.plant_status(50),
            0,
            Trajectory::Unknown,
            Trajectory::Rising,
            0.0,
            0.0,
            0.0,
        );
        let result = score(&signals, &weights);

        println!(
            "Action: {:<10} score: {:.2}  (adv={}, comp={}, stall={})",
            action,
            result.total,
            delta.advanced.len(),
            delta.completed.len(),
            delta.stalled.len(),
        );

        if result.total > best_score {
            best_score = result.total;
            best_action = action;
        }
        // fork and fork_graph are dropped here
    }

    println!("\nBest action: {} (score: {:.2})", best_action, best_score);
    // The original engine is unchanged — no speculative state leaked.
    assert_eq!(engine.partial_matches().len(),
               engine.partial_matches().iter()
                   .filter(|pm| pm.state == MatchState::Active)
                   .count(),
               "original engine has only its original active PMs");
}
```

## Notes

**Memory.** Each `engine.clone()` copies every partial match. If the engine has 10,000 active PMs and you evaluate 50 candidates, that is 500,000 PM copies per decision point. Profile with `dhat` (see `fabula-bench`) if this becomes a bottleneck.

**Performance.** The fork-speculate-score loop is embarrassingly parallel. Each fork is independent, so you can evaluate candidates across threads with no synchronization. The engine and graph are both `Send`.

**Shallow speculation.** The example above looks one action ahead. For deeper MCTS trees, nest the fork: clone the clone, speculate further, score, backpropagate. The same pattern applies at every depth.

**Weights tuning.** `NarrativeWeights::default()` is a starting point. Adjust weights per story phase -- for example, increase `completion` weight during a climax and `progress` weight during rising action.

## See also

- [Narrative Quality](../concepts/narrative-quality) -- how scoring signals map to narrative theory
- [Engine Reference](../reference/engine) -- full `SiftEngine` API including `clone`, `end_tick`, `plant_status`
