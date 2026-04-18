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

**Example output** (for a graph where `insult_event → grudge_event → betrayal_event` sits alongside a rival-alliance branch, both converging at `betrayal_event`):

```
cleanliness=0.50 (1 hops): ["grudge_event", "betrayal_event"]
cleanliness=0.50 (2 hops): ["insult_event", "grudge_event", "betrayal_event"]
cleanliness=0.43 (3 hops): ["failed_negotiation", "insult_event", "grudge_event", "betrayal_event"]
cleanliness=0.40 (1 hops): ["rival_encounter", "betrayal_event"]
cleanliness=0.34 (2 hops): ["strained_alliance", "rival_encounter", "betrayal_event"]
cleanliness=0.31 (3 hops): ["old_debt", "strained_alliance", "rival_encounter", "betrayal_event"]
```

Paths are sorted by cleanliness — the first path is the most confident explanation. `causal_paths` returns **every depth for every branch**: the top-ranked "proximate cause" (1 hop), the mid-range explanation (2 hops), and the full root-cause trace (3 hops). Pick whichever depth suits your UI — a GM debug panel might show just `[0]`; an after-action summary might show the deepest path per branch.

The ceiling for any path is `~0.5` here because `betrayal_event` has two causal predecessors (`grudge_event` via `causes`, `rival_encounter` via `escalates`), so every path pays a divergence penalty for the branch it didn't take. Single-predecessor graphs produce cleaner scores.

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
