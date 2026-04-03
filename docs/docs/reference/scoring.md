---
sidebar_position: 5
title: Scoring
---

# Scoring

`fabula::scoring` -- statistical surprise scoring for pattern matches. Operates as post-processing: the engine finds matches, the scorers rank them.

Two independent scorers:

- **`SurpriseScorer`** -- pattern-level Shannon surprise (how often does this pattern fire vs. baseline?)
- **`StuScorer`** -- property-level surprise using the StU heuristic (how rare are the properties in this match?)

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
pub fn observe<N, V, L, VV>(&mut self, matches: &[Match<N, V>], patterns: &[Pattern<L, VV>])
```

---

#### `observe_events`

Record observations from incremental matching events. Only counts `Completed` events. Does NOT increment the round counter -- call `tick()` manually for incremental mode.

```rust
pub fn observe_events<N, V, L, VV>(&mut self, events: &[SiftEvent<N, V>], patterns: &[Pattern<L, VV>])
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
pub fn score<N, V, L, VV>(&self, matches: &[Match<N, V>], patterns: &[Pattern<L, VV>]) -> Vec<ScoredMatch<N, V>>
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

## `ScoredMatch<N, V>`

A match annotated with a surprise score.

| Field | Type | Description |
|-------|------|-------------|
| `pattern` | `String` | Pattern name. |
| `bindings` | `HashMap<String, BoundValue<N, V>>` | Variable bindings from the match. |
| `surprise` | `f64` | Surprise score in bits. Higher = more unexpected. Negative = fires more often than baseline. |

Trait implementations: `Debug`, `Clone`.

---

## `StuScorer`

Property-level surprise scorer using the StU heuristic (Kreminski et al. 2022 ICIDS). Scores individual matches by the mean empirical frequency of their *properties*. Two matches of the same pattern score differently if one involves rarer attributes.

**Lower `stu_score` = more surprising.**

The scorer only does frequency math. Property extraction is the caller's responsibility.

```rust
use fabula::scoring::StuScorer;

let mut stu = StuScorer::new();

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

#### `property_frequency`

Compute the Laplace-smoothed frequency of a property within a pattern's matches. Returns `None` if the pattern has never been observed.

Formula: `(count + 1) / (total_matches + V)` where V is the vocabulary size.

```rust
pub fn property_frequency(&self, pattern: &str, property: &str) -> Option<f64>
```

---

#### `score`

Score a set of matches given their pre-extracted properties. Score = mean of per-property Laplace-smoothed frequencies. **Lower = more surprising.** Matches whose pattern has not been observed get `stu_score = 1.0`.

```rust
pub fn score<N, V>(&self, matches_with_props: &[(Match<N, V>, Vec<String>)]) -> Vec<StuScoredMatch<N, V>>
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

#### `reset`

Reset all observations.

```rust
pub fn reset(&mut self)
```

---

## `StuScoredMatch<N, V>`

A match annotated with property-level (StU) surprise score.

| Field | Type | Description |
|-------|------|-------------|
| `pattern` | `String` | Pattern name. |
| `bindings` | `HashMap<String, BoundValue<N, V>>` | Variable bindings from the match. |
| `property_frequencies` | `Vec<(String, f64)>` | Per-property frequencies, sorted ascending (rarest first). |
| `stu_score` | `f64` | Mean of property frequencies. **Lower = more surprising.** |

Trait implementations: `Debug`, `Clone`.
