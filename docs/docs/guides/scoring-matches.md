---
sidebar_position: 3
title: Scoring Matches
---

# Scoring Matches

**Learning objective:** Score and rank pattern matches by surprise using `SurpriseScorer`, `StuScorer`, and `SequentialScorer`.

Prerequisites: familiarity with the engine, pattern evaluation, and the `Match` type. See [Incremental Integration](./incremental-integration) if you need a refresher.

## Step 1: Set up SurpriseScorer

`SurpriseScorer` ranks patterns by how often they fire relative to a baseline expectation. Shannon surprise: `-log2(observed / baseline)`. Higher = more unexpected.

```rust
use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

let alliance_idx = engine.register(
    PatternBuilder::new("alliance")
        .stage("e1", |s| s
            .edge("e1", "eventType".into(), MemValue::Str("ally".into()))
            .edge_bind("e1", "actor".into(), "a")
            .edge_bind("e1", "target".into(), "b"))
        .build(),
);

let betrayal_idx = engine.register(
    PatternBuilder::new("betrayal")
        .stage("e1", |s| s
            .edge("e1", "eventType".into(), MemValue::Str("betray".into()))
            .edge_bind("e1", "actor".into(), "a")
            .edge_bind("e1", "target".into(), "b"))
        .build(),
);

let mut scorer = SurpriseScorer::new();
scorer.set_baseline(alliance_idx, 0.5);
scorer.set_baseline(betrayal_idx, 0.1);

let mut graph = MemGraph::new();

for round in 1..=10 {
    graph.add_str(&format!("ev_ally_{round}"), "eventType", "ally", round);
    graph.add_ref(&format!("ev_ally_{round}"), "actor", "alice", round);
    graph.add_ref(&format!("ev_ally_{round}"), "target", "bob", round);
    if round == 7 {
        graph.add_str("ev_betray_1", "eventType", "betray", round);
        graph.add_ref("ev_betray_1", "actor", "alice", round);
        graph.add_ref("ev_betray_1", "target", "bob", round);
    }
    graph.set_time(round);

    let matches = engine.evaluate(&graph);
    scorer.observe(&matches, engine.patterns());
}

let final_matches = engine.evaluate(&graph);
let scored = scorer.score(&final_matches, engine.patterns());

for sm in &scored {
    println!("{}: {:.2} bits", sm.pattern, sm.surprise);
}
```

`alliance` fires every round -- it will have negative surprise (over-represented vs. 0.5 baseline). `betrayal` fires once out of 10 rounds -- it will have positive surprise (under-represented vs. 0.1 baseline).

## Step 2: Set up StuScorer with property extraction

`StuScorer` scores individual matches by the rarity of their *properties*. Two matches of the same pattern score differently if one involves rarer attributes.

The scorer only does frequency math. You extract properties from match bindings and pass them in as strings.

```rust
use fabula::prelude::*;
use fabula_memory::MemValue;
use std::collections::HashMap;

let mut stu = StuScorer::new();

stu.observe_one("betrayal", &["actor_trait=ambitious", "target_role=king"]);
stu.observe_one("betrayal", &["actor_trait=loyal", "target_role=merchant"]);
stu.observe_one("betrayal", &["actor_trait=ambitious", "target_role=merchant"]);
stu.observe_one("betrayal", &["actor_trait=loyal", "target_role=merchant"]);
stu.observe_one("betrayal", &["actor_trait=loyal", "target_role=merchant"]);

let mut bindings_rare: HashMap<String, BoundValue<String, MemValue>> = HashMap::new();
bindings_rare.insert("trait".into(), BoundValue::Value(MemValue::Str("ambitious".into())));
bindings_rare.insert("role".into(), BoundValue::Value(MemValue::Str("king".into())));

let match_rare = Match {
    pattern: "betrayal".into(),
    pattern_idx: Some(0),
    bindings: bindings_rare,
    intervals: HashMap::new(),
    metadata: HashMap::new(),
};

fn extract_properties(m: &Match<String, MemValue, i64>) -> Vec<String> {
    let mut props = Vec::new();
    if let Some(BoundValue::Value(MemValue::Str(t))) = m.bindings.get("trait") {
        props.push(format!("actor_trait={t}"));
    }
    if let Some(BoundValue::Value(MemValue::Str(r))) = m.bindings.get("role") {
        props.push(format!("target_role={r}"));
    }
    props
}

let props_rare = extract_properties(&match_rare);
let scored = stu.score(&[(match_rare, props_rare)]);
println!("betrayal stu_score: {:.4}", scored[0].stu_score);
for (prop, freq) in &scored[0].property_frequencies {
    println!("  {prop}: {freq:.3}");
}
```

Property extraction guidance: use **categorical attributes** (traits, factions, roles, emotional states), not entity IDs. `"actor_faction=rebels"` gives useful signal. `"actor=char_147"` produces near-uniform frequencies where every match looks equally rare.

## Step 3: Try different aggregation modes

```rust
use fabula::scoring::{StuScorer, StuAggregation};

let stu_tfidf = StuScorer::new().with_aggregation(StuAggregation::TfIdf);
let stu_min = StuScorer::new().with_aggregation(StuAggregation::Min);
let stu_geo = StuScorer::new().with_aggregation(StuAggregation::GeometricMean);
```

| Mode | Formula | Polarity | When to use |
|------|---------|----------|-------------|
| `ArithmeticMean` | `sum(freq) / k` | Lower = more surprising | Default. Balanced sensitivity across all properties. |
| `TfIdf` | `sum(-log2(freq))` | Higher = more surprising | You want total information content. Rare properties dominate via log weighting. |
| `GeometricMean` | `exp(sum(ln(freq)) / k)` | Lower = more surprising | One rare property should pull the entire score down multiplicatively. |
| `Min` | `min(freq)` | Lower = more surprising | Only the single rarest property matters. |

## Step 4: Enable PMI correction

When two properties are correlated (e.g., `faction=rebels` and `location=hideout` always co-occur), their individual rarities get double-counted. PMI correction detects this and discounts the redundant member.

```rust
use fabula::scoring::StuScorer;

let mut stu = StuScorer::new().with_pmi_correction();

for _ in 0..20 {
    stu.observe_one("raid", &["faction=rebels", "location=hideout"]);
}
for _ in 0..80 {
    stu.observe_one("raid", &["faction=crown", "location=castle"]);
}

let pmi = stu.pmi_for("raid", "faction=rebels", "location=hideout");
println!("PMI(rebels, hideout): {:.2}", pmi.unwrap_or(0.0));
```

Use PMI correction when your properties have known correlations (faction/location, trait/role). Skip it when properties are independent or you have fewer than ~20 observations per pattern -- the pair counts need enough data to be meaningful.

## Step 5: Add sequential surprise

`SequentialScorer` tracks which pattern completed after which, and scores transitions by conditional surprise: `-log2(P(current | previous))`. Higher = more surprising.

```rust
use fabula::scoring::SequentialScorer;

let mut seq = SequentialScorer::new();

seq.observe_transition("alliance", "trade");
seq.observe_transition("alliance", "trade");
seq.observe_transition("alliance", "trade");
seq.observe_transition("alliance", "betrayal");
seq.observe_transition("trade", "trade");
seq.observe_transition("trade", "alliance");

let surprise = seq.score_transition("alliance", "betrayal");
let boring = seq.score_transition("alliance", "trade");
println!("alliance -> betrayal: {surprise:.2} bits");
println!("alliance -> trade:    {boring:.2} bits");
```

To integrate with the engine, track the last completed pattern as matches arrive:

```rust
use fabula::prelude::*;
use fabula::scoring::SequentialScorer;

let mut seq = SequentialScorer::new();
let mut last_completed: Option<String> = None;

// Inside your evaluation loop, after engine.evaluate() or drain_completed():
let completed_matches: Vec<Match<String, String, i64>> = vec![];
for m in &completed_matches {
    if let Some(prev) = &last_completed {
        seq.observe_transition(prev, &m.pattern);
    }
    last_completed = Some(m.pattern.clone());
}
```

## Step 6: Combine scores for ranking

The three scorers measure different axes: pattern frequency, property rarity, and transition surprise. Combine them with a weighted sum.

```rust
use fabula::prelude::*;
use fabula_memory::MemValue;

struct RankedMatch {
    pattern: String,
    combined_score: f64,
    pattern_surprise: f64,
    stu_score: f64,
    sequential_surprise: f64,
}

fn rank_matches(
    pattern_scored: &[ScoredMatch<String, MemValue, i64>],
    stu_scored: &[StuScoredMatch<String, MemValue, i64>],
    seq: &SequentialScorer,
    last_pattern: Option<&str>,
    w_pattern: f64,
    w_stu: f64,
    w_seq: f64,
) -> Vec<RankedMatch> {
    let mut ranked: Vec<RankedMatch> = pattern_scored
        .iter()
        .zip(stu_scored.iter())
        .map(|(ps, ss)| {
            let seq_surprise = match last_pattern {
                Some(prev) => seq.score_transition(prev, &ps.pattern),
                None => 0.0,
            };
            let stu_inverted = 1.0 - ss.stu_score;
            let combined = w_pattern * ps.surprise
                + w_stu * stu_inverted
                + w_seq * seq_surprise;
            RankedMatch {
                pattern: ps.pattern.clone(),
                combined_score: combined,
                pattern_surprise: ps.surprise,
                stu_score: ss.stu_score,
                sequential_surprise: seq_surprise,
            }
        })
        .collect();
    ranked.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
    ranked
}
```

The `stu_inverted` term flips the default polarity (lower = more surprising) so that all three components share "higher = more surprising" directionality before summing. If you use `StuAggregation::TfIdf`, skip the inversion and use `ss.stu_score` directly.

Print the ranked list:

```rust
for (i, r) in ranked.iter().enumerate() {
    println!(
        "#{}: {} (combined={:.2}, pattern={:.2}, stu={:.4}, seq={:.2})",
        i + 1, r.pattern, r.combined_score, r.pattern_surprise, r.stu_score, r.sequential_surprise
    );
}
```

## Next steps

- [Scoring Reference](../reference/scoring) -- full API details for all three scorers.
- [Narrative Scoring Reference](../reference/narratives) -- thread tracking, tension arcs, and MCTS quality scoring.
