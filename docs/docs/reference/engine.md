---
sidebar_position: 4
title: SiftEngine
---

# SiftEngine

`fabula::engine` -- pattern registration, batch evaluation, incremental matching, gap analysis, and pattern lifecycle management.

## `SiftEngine<N, L, V, T>`

The sift engine, generic over four independent type parameters. Maintains registered patterns and partial match state. Decoupled from `DataSource` -- the engine stores patterns and partial matches using the type parameters directly. Methods that need graph access take `&impl DataSource` as a parameter.

```rust
use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

// Explicit type parameters:
let mut engine: SiftEngine<String, String, MemValue, i64> = SiftEngine::new();

// Or use the SiftEngineFor alias (extracts types from a DataSource):
let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

engine.register(
    PatternBuilder::new("example")
        .stage("e", |s| s.edge("e", "type".into(), MemValue::Str("harm".into())))
        .build(),
);
```

### Type parameters

| Parameter | Bounds (lifecycle methods) | Bounds (evaluation methods) | Description |
|-----------|-----|-----|-------------|
| `N` | `Eq + Hash + Clone + Debug` | same | Node ID type |
| `L` | `Eq + Hash + Clone + Debug` | same | Edge label type |
| `V` | `PartialEq + PartialOrd + Clone + Debug + Hash` | same | Value type |
| `T` | `Ord + Clone + Debug + Hash` | `+ Sub<Output=T> + NumericTime` | Time type |

Lifecycle methods (register, tick, enable/disable) require the lighter bounds. Evaluation methods (evaluate, on_edge_added, why_not) additionally require `T: Sub<Output=T> + NumericTime` for metric gap computation.

### `SiftEngineFor<DS>` alias

Convenience type alias that extracts type parameters from a `DataSource` implementation:

```rust
pub type SiftEngineFor<DS> = SiftEngine<
    <DS as DataSource>::N,
    <DS as DataSource>::L,
    <DS as DataSource>::V,
    <DS as DataSource>::T,
>;
```

Use this when you have a specific `DataSource` type and want terser declarations.

---

### Lifecycle methods

These methods require only the lighter type bounds (no `Sub` or `NumericTime`).

#### `SiftEngine::new`

Creates a new empty engine with no patterns and no partial matches.

```rust
pub fn new() -> Self
```

**Returns:** `SiftEngine<N, L, V, T>`

---

#### `register`

Registers a pattern. Returns its index in the internal pattern list.

```rust
pub fn register(&mut self, pattern: Pattern<L, V>) -> usize
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `pattern` | `Pattern<L, V>` | The compiled pattern to register. |

**Returns:** `usize` -- the pattern's index (used by `pattern_metrics`, `set_pattern_enabled`, etc.).

---

#### `patterns`

Returns a slice of all registered patterns.

```rust
pub fn patterns(&self) -> &[Pattern<L, V>]
```

---

#### `partial_matches`

Returns a slice of all partial matches (including completed and dead ones, until drained/cleaned).

```rust
pub fn partial_matches(&self) -> &[PartialMatch<N, V, T>]
```

---

#### `active_matches_for`

Returns active partial matches for a specific pattern (by name).

```rust
pub fn active_matches_for(&self, name: &str) -> Vec<&PartialMatch<N, V, T>>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `name` | `&str` | Pattern name to filter by. |

---

#### `drain_completed`

Removes all completed matches from internal storage and returns them.

```rust
pub fn drain_completed(&mut self) -> Vec<Match<N, V, T>>
```

**Returns:** `Vec<Match<N, V, T>>` -- completed matches removed from the engine.

---

#### `stats`

Returns a reference to cumulative operation counters.

```rust
pub fn stats(&self) -> &EngineStats
```

---

#### `reset_stats`

Zeroes all operation counters.

```rust
pub fn reset_stats(&mut self)
```

---

#### `tick`

Advance the tick counter by one. Call once per simulation step. Used for staleness detection. Does NOT produce a delta summary -- use `end_tick` for the happy path, or `tick_delta` with manually collected events.

```rust
pub fn tick(&mut self)
```

---

#### `end_tick`

End the current tick: increments the tick counter, scans for expired partial matches (deadline exceeded), builds a `TickDelta` from accumulated events, and clears the accumulators.

This is the happy-path API for GM consumers. Call `on_edge_added()` for each edge in the tick (events accumulate internally), then call `end_tick()` to get the summary and any expiry events.

```rust
pub fn end_tick(&mut self, stale_threshold: u64) -> (TickDelta, Vec<SiftEvent<N, V>>)
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `stale_threshold` | `u64` | Ticks since last advancement to consider a pattern "stalled." |

**Returns:** `(TickDelta, Vec<SiftEvent<N, V>>)` -- the tick summary and any `SiftEvent::Expired` events for partial matches that exceeded their pattern's `deadline_ticks`.

The expiry scan runs before the delta is built: for each active PM whose pattern has a `deadline_ticks`, if `current_tick - pm.created_at_tick > deadline_ticks`, the PM is killed and an `Expired` event is emitted. The expired PM's pattern name is included in `TickDelta.expired`.

```rust
// Example: simulation loop
for edge in new_edges {
    engine.on_edge_added(&ds, &src, &label, &val, &interval);
}
let (delta, expired_events) = engine.end_tick(50);
if !delta.stalled.is_empty() { /* alert GM about stale plants */ }
for ev in &expired_events {
    if let SiftEvent::Expired { pattern, stage_reached, ticks_elapsed, .. } = ev {
        println!("{} expired at stage {} after {} ticks", pattern, stage_reached, ticks_elapsed);
    }
}
```

---

#### `current_tick`

Returns the current tick counter value.

```rust
pub fn current_tick(&self) -> u64
```

---

#### `set_pattern_enabled`

Enable or disable a pattern. Disabled patterns are skipped during `evaluate()` and `on_edge_added()`. When disabling, all active PMs for the pattern are killed immediately (Rete convention).

```rust
pub fn set_pattern_enabled(&mut self, idx: usize, enabled: bool)
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `idx` | `usize` | Pattern index (from `register()`). |
| `enabled` | `bool` | Whether to enable or disable. |

---

#### `is_pattern_enabled`

Check if a pattern is currently enabled.

```rust
pub fn is_pattern_enabled(&self, idx: usize) -> bool
```

---

#### `deregister`

Soft-delete a pattern. Disables it and kills all its PMs. The pattern stays in the Vec (index stability) but will never match again.

```rust
pub fn deregister(&mut self, idx: usize)
```

---

#### `pattern_metrics`

Per-pattern lifecycle metrics. Returns `None` if the index is out of bounds.

```rust
pub fn pattern_metrics(&self, idx: usize) -> Option<PatternMetrics>
```

---

#### `stale_patterns`

Find patterns that have not advanced for at least `threshold` ticks but still have active partial matches (stale plants).

```rust
pub fn stale_patterns(&self, threshold: u64) -> Vec<usize>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `threshold` | `u64` | Minimum ticks since last advancement. |

**Returns:** `Vec<usize>` -- indices of stale patterns.

---

#### `register_plant_payoff`

Register a plant/payoff pair for Chekhov's gun tracking. The plant pattern is narrative setup; the payoff pattern is the resolution. `shared_binding` optionally constrains the pair: the payoff only counts as resolving the plant if both share a binding with this variable name pointing to the same entity.

```rust
pub fn register_plant_payoff(
    &mut self,
    plant_idx: usize,
    payoff_idx: usize,
    shared_binding: Option<String>,
)
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `plant_idx` | `usize` | Pattern index of the plant (setup). |
| `payoff_idx` | `usize` | Pattern index of the payoff (resolution). |
| `shared_binding` | `Option<String>` | Variable that must match across the pair (e.g., same character). |

---

#### `plant_payoff_pairs`

Returns all registered plant/payoff pairs.

```rust
pub fn plant_payoff_pairs(&self) -> &[PlantPayoffPair]
```

---

#### `plant_status`

Status of all plant/payoff pairs. Shows which setups are unresolved, stale, or paid off.

```rust
pub fn plant_status(&self, stale_threshold: u64) -> Vec<PlantStatus>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `stale_threshold` | `u64` | Ticks threshold for staleness detection. |

**Returns:** `Vec<PlantStatus>`

---

### Evaluation methods

These methods require the full bounds: `T: Sub<Output=T> + NumericTime`. They take `&impl DataSource` as a parameter -- the engine is not coupled to any specific data source.

#### `evaluate`

Batch evaluation: finds all complete matches in the current graph state. Does not modify engine state.

```rust
pub fn evaluate(&self, ds: &(impl DataSource<N=N, L=L, V=V, T=T> + ?Sized)) -> Vec<Match<N, V, T>>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `ds` | `&impl DataSource` | The data source to evaluate against. |

**Returns:** `Vec<Match<N, V, T>>` -- all complete matches found.

---

#### `on_edge_added`

Incremental evaluation: processes a newly added edge. Executes in four phases:

1. **Negation check** -- tests existing partial matches for negation kills.
2. **Initiation** -- tries to start new partial matches (first stage).
3. **Advancement** -- tries to advance existing active partial matches.
4. **Cleanup** -- removes dead partial matches.

```rust
pub fn on_edge_added(
    &mut self,
    ds: &(impl DataSource<N=N, L=L, V=V, T=T> + ?Sized),
    source: &N,
    label: &L,
    value: &V,
    interval: &Interval<T>,
) -> Vec<SiftEvent<N, V>>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `ds` | `&impl DataSource` | The data source (for secondary clause validation). |
| `source` | `&N` | Source node of the new edge. |
| `label` | `&L` | Label of the new edge. |
| `value` | `&V` | Target value of the new edge. |
| `interval` | `&Interval<T>` | Validity interval of the new edge. |

**Returns:** `Vec<SiftEvent<N, V>>` -- events produced by this edge.

---

#### `why_not`

Gap analysis: clause-by-clause analysis of why a pattern has not matched. Stops at the first unmatched stage.

```rust
pub fn why_not(
    &self,
    ds: &(impl DataSource<N=N, L=L, V=V, T=T> + ?Sized),
    pattern_name: &str,
) -> Option<GapAnalysis>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `ds` | `&impl DataSource` | The data source to analyze against. |
| `pattern_name` | `&str` | Name of the pattern to analyze. |

**Returns:** `Option<GapAnalysis>` -- `None` if no pattern with that name exists.

---

#### `tick_delta`

Compute a delta summary from externally collected events. Pass the events returned by `on_edge_added()` and a staleness threshold.

```rust
pub fn tick_delta(
    &self,
    events: &[SiftEvent<N, V>],
    stale_threshold: u64,
) -> TickDelta
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `events` | `&[SiftEvent<N, V>]` | Events from `on_edge_added()` calls this tick. |
| `stale_threshold` | `u64` | Ticks threshold for staleness detection. |

**Returns:** `TickDelta`

---

### Trait implementations

| Trait | Notes |
|-------|-------|
| `Default` | Equivalent to `SiftEngine::new()`. |
| `Clone` | Manual impl. Creates an independent copy of all state. Tick accumulators are intentionally empty in the clone (forked engine starts fresh). |

---

## `Match<N, V, T>`

A complete match -- all stages satisfied, temporal constraints met, negation windows clear.

```rust
pub struct Match<N: Debug, V: Debug, T> {
    pub pattern: String,
    pub pattern_idx: Option<usize>,
    pub bindings: HashMap<String, BoundValue<N, V>>,
    pub intervals: HashMap<String, Interval<T>>,
    pub metadata: HashMap<String, String>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `pattern` | `String` | Name of the matched pattern. |
| `pattern_idx` | `Option<usize>` | Index of the pattern in the engine's pattern list. `Some` for engine-produced matches, `None` for manually constructed ones. |
| `bindings` | `HashMap<String, BoundValue<N, V>>` | Variable name to bound value. |
| `intervals` | `HashMap<String, Interval<T>>` | Intervals of matched stage anchors (keyed by anchor variable name). Same as `PartialMatch::intervals`. |
| `metadata` | `HashMap<String, String>` | Metadata copied from the pattern at match time. Lets consumers inspect pattern tags without looking up the pattern by name. |

---

## `BoundValue<N, V>`

A value bound to a variable in a match.

```rust
pub enum BoundValue<N: Debug, V: Debug> {
    Node(N),
    Value(V),
}
```

| Variant | Description |
|---------|-------------|
| `Node(N)` | A graph node (traversable as a source in subsequent clauses). |
| `Value(V)` | A data value (string, number, boolean -- not traversable). |

Trait implementations: `Debug`, `Clone`, `Hash`.

---

## `PartialMatch<N, V, T>`

A partial match -- some stages satisfied, waiting for more events.

```rust
pub struct PartialMatch<N: Debug + Clone, V: Debug + Clone, T: Clone> {
    pub pattern_idx: usize,
    pub bindings: HashMap<String, BoundValue<N, V>>,
    pub intervals: HashMap<String, Interval<T>>,
    pub next_stage: usize,
    pub state: MatchState,
    pub id: usize,
    pub created_at: T,
    pub created_at_tick: u64,
    pub fingerprint: u64,
    pub repetition_count: u32,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `pattern_idx` | `usize` | Index of the pattern in the engine's pattern list. |
| `bindings` | `HashMap<String, BoundValue<N, V>>` | Variables bound so far. |
| `intervals` | `HashMap<String, Interval<T>>` | Intervals of matched stage anchors (keyed by anchor variable name). |
| `next_stage` | `usize` | Index of the next stage to match (0-indexed). |
| `state` | `MatchState` | Current state of this partial match. |
| `id` | `usize` | Unique identifier for tracking. |
| `created_at` | `T` | Timestamp when this partial match was first initiated. Set from the initiating edge's interval start; inherited from parent on fork. Only meaningful in incremental mode. |
| `created_at_tick` | `u64` | Engine tick when this partial match was first initiated. Inherited from parent on advancement (not reset). Used by `end_tick()` for deadline expiry checks: `current_tick - created_at_tick > deadline_ticks`. |
| `fingerprint` | `u64` | Precomputed dedup hash of `(pattern_idx, next_stage, bindings, intervals, repetition_count)`. Computed once at creation using order-independent XOR hashing. |
| `repetition_count` | `u32` | Number of completed sub-pattern occurrences for repeat-range patterns. Incremented each time the PM loops back to the repeat segment start. Zero for non-repeating patterns. |

---

## `MatchState`

State of a partial match.

```rust
pub enum MatchState {
    Active,
    Complete,
    Dead,
}
```

| Variant | Description |
|---------|-------------|
| `Active` | Waiting for the next stage to match. |
| `Complete` | All stages matched. |
| `Dead` | Killed by a negation window. |

Trait implementations: `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`.

---

## `SiftEvent<N, V>`

Events emitted by incremental matching via `on_edge_added` and deadline expiry via `end_tick`.

```rust
pub enum SiftEvent<N: Debug, V: Debug> {
    Advanced {
        pattern: String,
        match_id: usize,
        stage_index: usize,
        metadata: HashMap<String, String>,
    },
    Completed {
        pattern: String,
        match_id: usize,
        bindings: HashMap<String, BoundValue<N, V>>,
        metadata: HashMap<String, String>,
    },
    Negated {
        pattern: String,
        match_id: usize,
        clause_label: String,
        trigger_source: N,
        metadata: HashMap<String, String>,
    },
    Expired {
        pattern: String,
        match_id: usize,
        bindings: HashMap<String, BoundValue<N, V>>,
        stage_reached: usize,
        ticks_elapsed: u64,
        metadata: HashMap<String, String>,
    },
}
```

| Variant | Fields | Description |
|---------|--------|-------------|
| `Advanced` | `pattern`, `match_id`, `stage_index`, `metadata` | A partial match advanced (new stage satisfied). |
| `Completed` | `pattern`, `match_id`, `bindings`, `metadata` | A pattern fully matched. |
| `Negated` | `pattern`, `match_id`, `clause_label`, `trigger_source`, `metadata` | A partial match was killed by a negation. |
| `Expired` | `pattern`, `match_id`, `bindings`, `stage_reached`, `ticks_elapsed`, `metadata` | A partial match exceeded its pattern's `deadline_ticks`. Emitted by `end_tick()`, not `on_edge_added()`. `stage_reached` is the index of the last matched stage. `ticks_elapsed` is `current_tick - created_at_tick`. |

All variants carry `metadata` copied from the pattern at event creation time.

---

## `EngineStats`

Cumulative operation counters for performance analysis. Incremented during `on_edge_added()`. Read with `engine.stats()`, reset with `engine.reset_stats()`.

| Field | Type | Description |
|-------|------|-------------|
| `total_on_edge_added` | `u64` | Number of `on_edge_added()` calls. |
| `total_fingerprints` | `u64` | Fingerprint work: initial dedup set builds + per-candidate checks. |
| `total_negation_checks` | `u64` | Negation checks attempted (once per active PM per call). |
| `peak_active_pms` | `usize` | High-water mark of active partial matches. |

Trait implementations: `Debug`, `Clone`, `Default`.

---

## `PatternMetrics`

Per-pattern lifecycle metrics. Returned by `pattern_metrics()`.

| Field | Type | Description |
|-------|------|-------------|
| `enabled` | `bool` | Whether the pattern is enabled for matching. |
| `last_advanced_tick` | `u64` | Last tick at which any PM for this pattern advanced or completed. |
| `completion_count` | `u64` | Total completions (cumulative). |
| `advancement_count` | `u64` | Total stage advancements (cumulative). |
| `negation_count` | `u64` | Total negation kills (cumulative). |
| `active_pm_count` | `usize` | Number of currently active partial matches. |

Trait implementations: `Debug`, `Clone`, `Default`.

---

## `TickDelta`

Summary of what changed in one tick. Returned by `end_tick()` or `tick_delta()`. The GM uses this to assess narrative progress: which patterns are advancing, completing, dying, or stalling.

| Field | Type | Description |
|-------|------|-------------|
| `advanced` | `Vec<String>` | Patterns that had at least one PM advance this tick. |
| `completed` | `Vec<String>` | Patterns that completed this tick. |
| `negated` | `Vec<String>` | Patterns that had PMs negated this tick. |
| `expired` | `Vec<String>` | Patterns that had PMs expire (deadline exceeded) this tick. |
| `stalled` | `Vec<String>` | Patterns with active PMs that haven't advanced for `stale_threshold` ticks. |
| `active_pm_count` | `usize` | Total active PM count across all patterns. |

Trait implementations: `Debug`, `Clone`, `Default`.

---

## `PlantPayoffPair`

A registered plant/payoff pair for Chekhov's gun tracking. The plant pattern is narrative setup; the payoff pattern is the resolution.

| Field | Type | Description |
|-------|------|-------------|
| `plant_idx` | `usize` | Pattern index of the plant (setup). |
| `payoff_idx` | `usize` | Pattern index of the payoff (resolution). |
| `shared_binding` | `Option<String>` | Variable that must match across the pair (e.g., same character). |

Trait implementations: `Debug`, `Clone`.

---

## `PlantStatus`

Status of a single plant from `plant_status()`.

| Field | Type | Description |
|-------|------|-------------|
| `plant_pattern` | `String` | Plant pattern name. |
| `payoff_pattern` | `String` | Payoff pattern name. |
| `active_plants` | `usize` | Number of active plant PMs (unresolved setups). |
| `payoff_completions` | `u64` | Number of payoff completions (resolved setups). |
| `ticks_since_plant_advanced` | `u64` | Ticks since the plant pattern last advanced. High = Chekhov's gun gathering dust. |
| `stale` | `bool` | Whether the plant is stale (no advancement + active PMs). |

Trait implementations: `Debug`, `Clone`.

---

## Gap analysis types

### `GapAnalysis`

Result of `why_not` -- clause-by-clause analysis of why a pattern did not match.

```rust
pub struct GapAnalysis {
    pub pattern: String,
    pub stages: Vec<StageAnalysis>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `pattern` | `String` | Name of the analyzed pattern. |
| `stages` | `Vec<StageAnalysis>` | Per-stage analysis. Stops at the first unmatched stage. |

---

### `StageAnalysis`

Analysis of a single stage.

| Field | Type | Description |
|-------|------|-------------|
| `anchor` | `String` | Stage anchor variable name. |
| `status` | `StageStatus` | Overall status of this stage. |
| `clauses` | `Vec<ClauseAnalysis>` | Per-clause analysis. |

---

### `StageStatus`

Overall status of a stage in gap analysis.

| Variant | Description |
|---------|-------------|
| `Matched` | All clauses in this stage matched. |
| `PartiallyMatched { matched, total }` | Some clauses matched. `matched` out of `total`. |
| `Unmatched` | No clauses matched. |

---

### `ClauseAnalysis`

Analysis of a single clause within a stage.

| Field | Type | Description |
|-------|------|-------------|
| `description` | `String` | Human-readable clause description. |
| `matched` | `bool` | Whether this clause matched. |
| `reason` | `Option<String>` | Explanation of why the clause failed, or `None` if it matched. |
