---
sidebar_position: 12
title: Tracing causal chains
---

# Tracing causal chains

**Problem:** An interesting event happened. What earlier events caused it? You want a list of explanations, best-first.

**Prerequisites:** A graph whose simulation layer inserts causal edges (e.g., when event B happens because of A, the simulation adds an edge `A --causes--> B`).

## 1. Mark your causal labels

Pick edge labels that encode causality in your simulation and assign each a weight in `(0.0, 1.0]`:

```rust
use std::collections::HashMap;

let mut causal_labels: HashMap<String, f64> = HashMap::new();
causal_labels.insert("causes".to_string(), 1.0);
causal_labels.insert("enables".to_string(), 0.6);
causal_labels.insert("escalates".to_string(), 0.8);
```

A weight of `1.0` is full confidence ("A directly caused B"). Lower weights capture looser relationships ("A enabled B to happen").

## 2. Query the paths

```rust
use fabula::causality::causal_paths;

let paths = causal_paths(
    &graph,
    &betrayal_event_node,
    5,               // max hops
    &causal_labels,
);

for p in paths.iter().take(3) {
    println!(
        "cleanliness={:.2} ({} hops): {:?}",
        p.cleanliness,
        p.edges.len(),
        p.nodes,
    );
}
```

**Expected output** (shape):

```
cleanliness=0.94 (2 hops): ["insult_event", "grudge_event", "betrayal_event"]
cleanliness=0.71 (4 hops): ["failed_negotiation", "insult_event", "grudge_event", "rival_encounter", "betrayal_event"]
cleanliness=0.42 (3 hops): ["old_debt", "strained_alliance", "rival_encounter", "betrayal_event"]
```

Paths are sorted by cleanliness — the first path is the most confident explanation.

## 3. Combine with narrative scoring

`causal_paths` is read-only — it doesn't modify the engine or the graph. You can call it any time:

- After a pattern completes, to explain *why* the match was possible
- During MCTS evaluation, to score narrative chains by causal depth
- In a GM debug view, to inspect why a surprising event occurred

## 4. When no paths exist

If `causal_paths` returns empty, either:

- The effect node genuinely has no causal predecessors (a world-state origin event).
- Your simulation isn't inserting causal edges with the labels you registered.
- The causes are further back than `max_hops`.

Increase `max_hops` or audit your simulation's causal edge insertion before assuming the effect is uncaused.

## See also

- [Causality reference](/reference/causality) -- full API docs
- [Why-not gap analysis](/reference/engine#why-not) -- for non-causal "why didn't this match?"
