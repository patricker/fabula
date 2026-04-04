---
sidebar_position: 5
title: Scoring
---

# Scoring

`fabula::scoring` -- statistical surprise scoring for pattern matches. Operates as post-processing: the engine finds matches, the scorers rank them.

Three independent scorers:

- **`SurpriseScorer`** -- pattern-level Shannon surprise (how often does this pattern fire vs. baseline?)
- **`StuScorer`** -- property-level surprise using the StU heuristic (how rare are the properties in this match?)
- **`SequentialScorer`** -- transition surprise using bigram model (how unexpected is this pattern after the previous one?)

---

## `SurpriseScorer`

Pattern-level surprise scorer. Tracks per-pattern match counts and computes Shannon surprise relative to user-provided baseline frequencies.

Shannon surprise: `-log2(observed / baseline)` with Laplace smoothing.

```rust
use fabula::scoring::SurpriseScorer;

let mut scorer = SurpriseScorer::new();
scorer.set_baseline(0, 0.1); // expect pattern 0 to match 10% of rounds

// After evaluation:
let matches = engine.evaluate(&graph);
scorer.observe(&matches, engine.patterns());
let scored = scorer.score(&matches, engine.patterns());
// scored[i].surprise — higher = more unexpected
```

### Methods

#### `SurpriseScorer::new`

Create a new scorer with no baselines or observations.

```rust
pub fn new() -> Self
```

---

#### `set_baseline`

Set the expected match frequency for a pattern (by registration index). `baseline` is a probability in (0, 1].

```rust
pub fn set_baseline(&mut self, pattern_idx: usize, baseline: f64)
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `pattern_idx` | `usize` | Pattern index (from `engine.register()`). |
| `baseline` | `f64` | Expected match probability per round. Must be in (0, 1]. |

**Panics** if baseline is not in (0, 1].

---

#### `observe`

Record one round of observations from batch evaluation results. Call once per `evaluate()` call. Increments the round counter and counts each pattern at most once per round.

```rust
pub fn observe<N, V, T, L, VV>(&mut self, matches: &[Match<N, V, T>], patterns: &[Pattern<L, VV>])
```

---

#### `observe_events`

Record observations from incremental matching events. Only counts `Completed` events. Does NOT increment the round counter -- call `tick()` manually for incremental mode.

```rust
pub fn observe_events<N: Debug, V: Debug, L, VV>(&mut self, events: &[SiftEvent<N, V>], patterns: &[Pattern<L, VV>])
```

---

#### `tick`

Mark one observation round in incremental mode. Call once per simulation tick. Batch mode's `observe()` does this automatically.

```rust
pub fn tick(&mut self)
```

---

#### `score`

Compute surprise scores for a set of matches. Returns one `ScoredMatch` per input. Patterns without a baseline get score 0.0.

```rust
pub fn score<N, V, T, L, VV>(&self, matches: &[Match<N, V, T>], patterns: &[Pattern<L, VV>]) -> Vec<ScoredMatch<N, V, T>>
```

---

#### `surprise_for`

Get the current surprise score for a pattern (by index). Returns `None` if no baseline is set. Uses Laplace smoothing: `p = (count + 1) / (rounds + 1)`.

```rust
pub fn surprise_for(&self, pattern_idx: usize) -> Option<f64>
```

---

#### `reset_counts`

Reset all observation counts and rounds. Baselines are preserved.

```rust
pub fn reset_counts(&mut self)
```

---

#### `total_rounds`

Total observation rounds recorded.

```rust
pub fn total_rounds(&self) -> u64
```

---

#### `count_for`

Observed match count for a pattern.

```rust
pub fn count_for(&self, pattern_idx: usize) -> u64
```

---

## `ScoredMatch<N, V, T>`

A match annotated with a surprise score. `N`, `V`, `T` are the node, value, and time types from your `DataSource`.

| Field | Type | Description |
|-------|------|-------------|
| `pattern` | `String` | Pattern name. |
| `pattern_idx` | `Option<usize>` | Pattern index in the engine's pattern list. |
| `bindings` | `HashMap<String, BoundValue<N, V>>` | Variable bindings from the match. |
| `intervals` | `HashMap<String, Interval<T>>` | Intervals of matched stage anchors. |
| `metadata` | `HashMap<String, String>` | Metadata propagated from the pattern. |
| `surprise` | `f64` | Surprise score in bits. Higher = more unexpected. Negative = fires more often than baseline. |

Trait implementations: `Debug`, `Clone`.

---

## `StuAggregation`

Aggregation strategy for combining per-property frequencies into a single StU score. Each variant implements a different "theory of surprise."

```rust
pub enum StuAggregation {
    ArithmeticMean,  // default
    TfIdf,
    GeometricMean,
    Min,
}
```

| Variant | Formula | Polarity | Use when |
|---------|---------|----------|----------|
| `ArithmeticMean` | `sum(freq) / k` | Lower = more surprising | Default. Original StU heuristic. |
| `TfIdf` | `sum(-log2(freq))` | **Higher = more surprising** | You want total information content. Rare properties dominate via log weighting. |
| `GeometricMean` | `exp(sum(ln(freq)) / k)` | Lower = more surprising | A single rare property should pull the entire score down multiplicatively. |
| `Min` | `min(freq)` | Lower = more surprising | Only the single most surprising property matters. |

Implements `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Default` (default: `ArithmeticMean`).

---

## `StuScorer`

Property-level surprise scorer using the StU heuristic (Kreminski et al. 2022 ICIDS). Scores individual matches by the empirical frequency of their *properties*. Two matches of the same pattern score differently if one involves rarer attributes.

Score polarity depends on the aggregation mode — **lower = more surprising** for `ArithmeticMean`, `GeometricMean`, and `Min`; **higher = more surprising** for `TfIdf`.

The scorer applies **cold-start confidence weighting**: with few observations, scores are attenuated toward "unsurprising" (1.0 for lower-is-surprising modes, 0.0 for TfIdf). Confidence formula: `1 - 1/(total_matches + 1)`. At 1 match, confidence is 0.5. At 10 matches, ~0.91. At 100, ~0.99.

The scorer only does frequency math. Property extraction is the caller's responsibility.

```rust
use fabula::scoring::{StuScorer, StuAggregation};

let mut stu = StuScorer::new()
    .with_aggregation(StuAggregation::TfIdf)
    .with_pmi_correction();

// Observe properties for completed matches
stu.observe_one("betrayal", &["actor_trait=ambitious", "target_role=king"]);
stu.observe_one("betrayal", &["actor_trait=loyal", "target_role=merchant"]);

// Score a new match
let freq = stu.property_frequency("betrayal", "actor_trait=ambitious");
// freq ≈ 0.6 (2 of 3 observations, with Laplace smoothing)
```

### Property extraction guidance

Properties should be **categorical attributes**, not entity identifiers. Emit `"actor_faction=rebels"` rather than `"actor=char_147"`. Entity IDs have near-uniform frequency in rich simulations, making all matches score identically (the "everything is rare" failure mode).

Good properties: traits, factions, relationship types, event categories, emotional states, location types.

### Methods

#### `StuScorer::new`

Create a new empty scorer.

```rust
pub fn new() -> Self
```

---

#### `observe_one`

Record properties for a single match. Call once per completed match. Deduplicates properties within a match (presence, not multiplicity).

```rust
pub fn observe_one(&mut self, pattern: &str, properties: &[impl AsRef<str>])
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `pattern` | `&str` | Pattern name. |
| `properties` | `&[impl AsRef<str>]` | Property strings extracted by the caller. |

---

#### `observe_batch`

Record properties for a batch of matches.

```rust
pub fn observe_batch(&mut self, observations: &[(&str, &[String])])
```

---

#### `with_aggregation`

Set the aggregation strategy. Default is `ArithmeticMean`. Returns `self` for builder chaining.

```rust
pub fn with_aggregation(self, aggregation: StuAggregation) -> Self
```

---

#### `with_pmi_correction`

Enable PMI-based correction for correlated properties. When two properties frequently co-occur (high PMI), their individual rarities would be double-counted. This correction replaces the less-rare member's frequency with its conditional frequency given the partner. Adds O(k^2) pair counting per `observe_one` call where k is the number of properties per match (typically 2-8).

```rust
pub fn with_pmi_correction(self) -> Self
```

---

#### `property_frequency`

Compute the Laplace-smoothed frequency of a property within a pattern's matches. Returns `None` if the pattern has never been observed.

Formula: `(count + 1) / (total_matches + V)` where V is the vocabulary size.

```rust
pub fn property_frequency(&self, pattern: &str, property: &str) -> Option<f64>
```

---

#### `score`

Score a set of matches given their pre-extracted properties. Score interpretation depends on the aggregation mode: **lower = more surprising** for `ArithmeticMean`, `GeometricMean`, and `Min`; **higher = more surprising** for `TfIdf`. Matches whose pattern has not been observed get `stu_score = 1.0`. Cold-start confidence weighting is applied automatically.

```rust
pub fn score<N, V, T>(&self, matches_with_props: &[(Match<N, V, T>, Vec<String>)]) -> Vec<StuScoredMatch<N, V, T>>
```

---

#### `match_count`

Total matches observed for a pattern.

```rust
pub fn match_count(&self, pattern: &str) -> u64
```

---

#### `vocabulary_size`

Number of distinct properties seen for a pattern.

```rust
pub fn vocabulary_size(&self, pattern: &str) -> usize
```

---

#### `pmi_for`

Pointwise Mutual Information between two properties for a pattern. `PMI(pi, pj) = log2(P(pi,pj) / (P(pi) * P(pj)))`. High PMI means the properties co-occur more than expected by chance. Returns `None` if the pattern has never been observed. Returns `Some(0.0)` if the properties never co-occurred (including when PMI correction is disabled, since pair counts are not tracked).

```rust
pub fn pmi_for(&self, pattern: &str, pi: &str, pj: &str) -> Option<f64>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `pattern` | `&str` | Pattern name. |
| `pi` | `&str` | First property string. |
| `pj` | `&str` | Second property string. Order does not matter (canonical sorted internally). |

---

#### `reset`

Reset all observations.

```rust
pub fn reset(&mut self)
```

---

## `StuScoredMatch<N, V, T>`

A match annotated with property-level (StU) surprise score. `N`, `V`, `T` are the node, value, and time types from your `DataSource`.

| Field | Type | Description |
|-------|------|-------------|
| `pattern` | `String` | Pattern name. |
| `pattern_idx` | `Option<usize>` | Pattern index in the engine's pattern list. |
| `bindings` | `HashMap<String, BoundValue<N, V>>` | Variable bindings from the match. |
| `intervals` | `HashMap<String, Interval<T>>` | Intervals of matched stage anchors. |
| `metadata` | `HashMap<String, String>` | Metadata propagated from the pattern. |
| `property_frequencies` | `Vec<(String, f64)>` | Per-property frequencies, sorted ascending (rarest first). |
| `stu_score` | `f64` | Aggregated property frequencies. Interpretation depends on the aggregation mode: **lower = more surprising** for `ArithmeticMean`, `GeometricMean`, `Min`; **higher = more surprising** for `TfIdf`. Includes cold-start confidence weighting. |

Trait implementations: `Debug`, `Clone`.

---

## `SequentialScorer`

Sequential surprise scorer using bigram pattern transitions. Tracks which pattern completed after which, and scores transitions by their conditional surprise: `-log2(P(current | previous))`.

A common betrayal after a rare alliance is surprising; a common betrayal after another common betrayal is not.

```rust
use fabula::scoring::SequentialScorer;

let mut seq = SequentialScorer::new();
seq.observe_transition("alliance", "betrayal");
seq.observe_transition("alliance", "betrayal");
seq.observe_transition("alliance", "trade");

// betrayal after alliance: common (2/3)
let common = seq.score_transition("alliance", "betrayal");
// trade after alliance: rarer (1/3)
let rare = seq.score_transition("alliance", "trade");
assert!(rare > common, "rarer transition should be more surprising");
```

### Methods

#### `SequentialScorer::new`

Create a new empty scorer.

```rust
pub fn new() -> Self
```

---

#### `observe_transition`

Record a transition: `prev` pattern completed, then `current` completed.

```rust
pub fn observe_transition(&mut self, prev: &str, current: &str)
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `prev` | `&str` | Pattern that completed previously. |
| `current` | `&str` | Pattern that completed now. |

---

#### `transition_probability`

Laplace-smoothed transition probability `P(current | prev)`. Returns `None` if `prev` has never been observed as a predecessor. Uses Laplace smoothing: `(count + 1) / (total + V)` where V is the number of distinct successors seen after `prev`.

```rust
pub fn transition_probability(&self, prev: &str, current: &str) -> Option<f64>
```

---

#### `score_transition`

Sequential surprise in bits: `-log2(P(current | prev))`. **Higher = more surprising.** Returns `0.0` if `prev` has never been observed (no data to judge surprise).

```rust
pub fn score_transition(&self, prev: &str, current: &str) -> f64
```

---

#### `total_transitions_from`

Total transitions observed from a predecessor.

```rust
pub fn total_transitions_from(&self, prev: &str) -> u64
```

---

#### `vocabulary_size`

Number of distinct successors observed after a predecessor.

```rust
pub fn vocabulary_size(&self, prev: &str) -> usize
```

---

#### `reset`

Reset all observations.

```rust
pub fn reset(&mut self)
```
