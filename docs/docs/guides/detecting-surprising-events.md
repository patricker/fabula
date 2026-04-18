---
sidebar_position: 13
title: Detecting surprising events
---

# Detecting surprising events

**Problem:** An event fired in your simulation. You want to know: was this a logical consequence of recent events, or did it come out of nowhere? A sudden alliance when nothing led to it, a betrayal with no grudge behind it, an unlock with no collected keys — these are the moments that signal either a bug, an exploit, or a genuine twist.

**Prerequisites:** Your simulation layer inserts *causal* edges for events it considers explanatory — e.g. when a `betrayal` event fires, a `grudge --causes--> betrayal` edge is also written to the graph.

## 1. Mark your causal labels

Same setup as the [causal tracing guide](./tracing-causal-chains):

```rust
use std::collections::HashMap;

let mut causal_labels: HashMap<String, f64> = HashMap::new();
causal_labels.insert("causes".to_string(), 1.0);
causal_labels.insert("enables".to_string(), 0.6);
causal_labels.insert("escalates".to_string(), 0.8);
```

## 2. Score a single event

```rust
use fabula::causality::event_causal_surprise;

let surprise = event_causal_surprise(
    &graph,
    &new_betrayal_event,
    5,             // max hops to walk back
    &causal_labels,
);

if surprise > 0.75 {
    println!("⚠ contextually surprising betrayal (score {:.2}) — check the simulation", surprise);
} else if surprise < 0.25 {
    println!("expected betrayal (score {:.2}) — grudge chain explains it", surprise);
}
```

Scores in `[0.0, 1.0]`. `1.0` means *nothing* in the graph caused this event; `0.0` means a clean chain of full-weight causes leads directly to it.

## 3. Score many events at once

When you have a list of event nodes to rate — all pattern completions from the current tick, every event in a narrative window, a speculative rollout's trace — the batch helper makes intent explicit:

```rust
use fabula::causality::event_causal_surprise_batch;

let event_nodes: Vec<String> = /* your collected event node IDs */;

let scores = event_causal_surprise_batch(
    &graph,
    &event_nodes,
    5,
    &causal_labels,
);

let anomalies: Vec<_> = event_nodes
    .iter()
    .zip(scores.iter())
    .filter(|(_, s)| **s > 0.75)
    .collect();
```

Batch scoring is semantically equivalent to looping individually — no hidden caching — but it keeps call sites tidy and signals "here's a tick's worth of events" to future readers.

Extracting event nodes from the engine's `TickDelta` is application-specific; consult the [engine reference](../reference/engine) for the exact `SiftEvent` fields.

## 4. Combine with statistical surprise

Fabula already ships statistical surprise scorers (`fabula::scoring::SurpriseScorer`, `StuScorer`, `SequentialScorer`). Those answer "how rare is this event by historical frequency?" `event_causal_surprise` answers a different question: "given the recent causal chain, was this predictable?" The two are independent:

```rust
let statistical = statistical_scorer.score(&pattern, &match_data);
let contextual = event_causal_surprise(&graph, &event_node, 5, &causal_labels);

// Flag an event if *either* score exceeds a threshold, or blend them:
let combined = 0.5 * statistical + 0.5 * contextual;
```

The [scoring-and-surprise concept page](../concepts/scoring-and-surprise) has a 2×2 matrix of outcomes for different statistical/contextual combinations.

## 5. Use as an MCTS signal

Rollout evaluators benefit from penalizing "unearned" narrative beats. During a speculative rollout, compute `event_causal_surprise` for each invented event and subtract from the rollout's score — high-surprise events are usually low-quality narrative moments.

```rust
let rollout_penalty: f64 = rollout_events
    .iter()
    .map(|e| event_causal_surprise(&forked_graph, e, 5, &causal_labels))
    .sum::<f64>() / rollout_events.len().max(1) as f64;
```

(This assumes you're already forking the engine and graph per rollout; see [Forking for MCTS](./forking-for-mcts).)

## 6. Interpreting the number

Scores fall into three broad buckets. The cause of a middling score is always either a weak causal weight (the label you registered has weight < 1.0) or divergence (the event has more than one candidate predecessor at some hop along the path). Temporal gap does *not* raise surprise on its own — a direct immediate cause always wins the ranking.

| Score | What it means | Action |
|---|---|---|
| 0.0 – 0.3 | A full-weight direct cause exists. | Accept as routine. |
| 0.3 – 0.7 | The best causal chain uses a weaker label or passes through a branching point. | Log; usually fine — interpret in context. |
| 0.7 – 1.0 | Either no chain at all, or only very weak / heavily branched chains. | Flag — possible exploit, bug, or a genuine narrative twist. |

Thresholds are domain-dependent. Start with `0.7` as an anomaly threshold and tune.

**Worked examples** (reproduced by the test suite in `crates/fabula-examples/tests/guides_detecting_surprise.rs`):

- A single `causes` edge (weight 1.0): cleanliness = 1.0, surprise = **0.0**.
- A single `enables` edge (weight 0.6): cleanliness = 0.6, surprise = **0.4**.
- Two causes for one event (both weight 1.0): the divergence factor kicks in — cleanliness = 0.5, surprise = **0.5**.
- No causal edge at all: surprise = **1.0**.

## See also

- [Causality reference](../reference/causality) -- full API
- [Tracing causal chains](./tracing-causal-chains) -- companion guide: given an event, explain it
- [Scoring and surprise](../concepts/scoring-and-surprise) -- how contextual surprise fits with the other three surprise axes
