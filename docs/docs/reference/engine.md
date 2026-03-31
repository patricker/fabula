---
sidebar_position: 4
title: SiftEngine
---

# SiftEngine

`fabula::engine` -- pattern registration, batch evaluation, incremental matching, and gap analysis.

## `SiftEngine<DS>`

The sift engine, generic over a `DataSource` implementation. Maintains registered patterns and partial match state.

```rust
use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
engine.register(
    PatternBuilder::new("example")
        .stage("e", |s| s.edge("e", "type".into(), MemValue::Str("harm".into())))
        .build(),
);
```

### Type parameter

| Parameter | Bounds | Description |
|-----------|--------|-------------|
| `DS` | `DataSource` (with `DS::N: PartialEq`, `DS::V: PartialEq`, `DS::T: Sub<Output=T> + NumericTime`) | The backing graph store. `NumericTime` enables metric gap computation for temporal constraints. Built-in for `i64`, `i32`, `f64`, `f32`. |

### Methods

#### `SiftEngine::new`

Creates a new empty engine with no patterns and no partial matches.

```rust
pub fn new() -> Self
```

**Returns:** `SiftEngine<DS>`

---

#### `register`

Registers a pattern. Returns its index in the internal pattern list.

```rust
pub fn register(&mut self, pattern: Pattern<DS::L, DS::V>) -> usize
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `pattern` | `Pattern<DS::L, DS::V>` | yes | -- | The compiled pattern to register. |

**Returns:** `usize` -- the pattern's index.

---

#### `patterns`

Returns a slice of all registered patterns.

```rust
pub fn patterns(&self) -> &[Pattern<DS::L, DS::V>]
```

**Returns:** `&[Pattern<DS::L, DS::V>]`

---

#### `partial_matches`

Returns a slice of all partial matches (including completed and dead ones, until drained/cleaned).

```rust
pub fn partial_matches(&self) -> &[PartialMatch<DS::N, DS::V, DS::T>]
```

**Returns:** `&[PartialMatch<DS::N, DS::V, DS::T>]`

---

#### `active_matches_for`

Returns active partial matches for a specific pattern (by name).

```rust
pub fn active_matches_for(&self, name: &str) -> Vec<&PartialMatch<DS::N, DS::V, DS::T>>
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `name` | `&str` | yes | -- | Pattern name to filter by. |

**Returns:** `Vec<&PartialMatch<DS::N, DS::V, DS::T>>`

---

#### `drain_completed`

Removes all completed matches from internal storage and returns them.

```rust
pub fn drain_completed(&mut self) -> Vec<Match<DS::N, DS::V>>
```

**Returns:** `Vec<Match<DS::N, DS::V>>` -- completed matches removed from the engine.

---

#### `evaluate`

Batch evaluation: finds all complete matches in the current graph state. Does not modify engine state.

```rust
pub fn evaluate(&self, ds: &DS) -> Vec<Match<DS::N, DS::V>>
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `ds` | `&DS` | yes | -- | The data source to evaluate against. |

**Returns:** `Vec<Match<DS::N, DS::V>>` -- all complete matches found.

---

#### `on_edge_added`

Incremental evaluation: processes a newly added edge. Checks negation windows on existing partial matches, initiates new partial matches, and advances existing ones.

Executes in three phases:
1. **Negation check** -- tests existing partial matches for negation kills.
2. **Initiation** -- tries to start new partial matches (first stage).
3. **Advancement** -- tries to advance existing active partial matches.

Dead partial matches are removed after processing.

```rust
pub fn on_edge_added(
    &mut self,
    ds: &DS,
    source: &DS::N,
    label: &DS::L,
    value: &DS::V,
    interval: &Interval<DS::T>,
) -> Vec<SiftEvent<DS::N, DS::V>>
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `ds` | `&DS` | yes | -- | The data source (for secondary clause validation). |
| `source` | `&DS::N` | yes | -- | Source node of the new edge. |
| `label` | `&DS::L` | yes | -- | Label of the new edge. |
| `value` | `&DS::V` | yes | -- | Target value of the new edge. |
| `interval` | `&Interval<DS::T>` | yes | -- | Validity interval of the new edge. |

**Returns:** `Vec<SiftEvent<DS::N, DS::V>>` -- events produced by this edge.

---

#### `why_not`

Gap analysis: clause-by-clause analysis of why a pattern has not matched. Stops at the first unmatched stage.

```rust
pub fn why_not(&self, ds: &DS, pattern_name: &str) -> Option<GapAnalysis>
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `ds` | `&DS` | yes | -- | The data source to analyze against. |
| `pattern_name` | `&str` | yes | -- | Name of the pattern to analyze. |

**Returns:** `Option<GapAnalysis>` -- `None` if no pattern with that name exists.

---

### Trait implementations

| Trait | Notes |
|-------|-------|
| `Default` | Equivalent to `SiftEngine::new()`. |

---

## `Match<N, V>`

A complete match -- all stages satisfied, temporal constraints met, negation windows clear.

```rust
pub struct Match<N: Debug, V: Debug> {
    pub pattern: String,
    pub bindings: HashMap<String, BoundValue<N, V>>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `pattern` | `String` | Name of the matched pattern. |
| `bindings` | `HashMap<String, BoundValue<N, V>>` | Variable name to bound value. |

### Trait implementations

`Debug`, `Clone`.

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

### Trait implementations

`Debug`, `Clone`.

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
| `created_at` | `T` | Timestamp when this partial match was first initiated. Set from the initiating edge's interval start in Phase 2; inherited from the parent on fork in Phase 3. Only meaningful in incremental mode. |

### Trait implementations

`Debug`, `Clone`.

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

### Trait implementations

`Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`.

---

## `SiftEvent<N, V>`

Events emitted by incremental matching via `on_edge_added`.

```rust
pub enum SiftEvent<N: Debug, V: Debug> {
    Advanced {
        pattern: String,
        match_id: usize,
        stage_index: usize,
    },
    Completed {
        pattern: String,
        match_id: usize,
        bindings: HashMap<String, BoundValue<N, V>>,
    },
    Negated {
        pattern: String,
        match_id: usize,
        clause_label: String,
        trigger_source: N,
    },
}
```

| Variant | Fields | Description |
|---------|--------|-------------|
| `Advanced` | `pattern: String`, `match_id: usize`, `stage_index: usize` | A partial match advanced (new stage satisfied). |
| `Completed` | `pattern: String`, `match_id: usize`, `bindings: HashMap<String, BoundValue<N, V>>` | A pattern fully matched. |
| `Negated` | `pattern: String`, `match_id: usize`, `clause_label: String`, `trigger_source: N` | A partial match was killed by a negation. `clause_label` is the label that triggered the kill. |

### Trait implementations

`Debug`.

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

### Trait implementations

`Debug`.

---

### `StageAnalysis`

Analysis of a single stage.

```rust
pub struct StageAnalysis {
    pub anchor: String,
    pub status: StageStatus,
    pub clauses: Vec<ClauseAnalysis>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `anchor` | `String` | Stage anchor variable name. |
| `status` | `StageStatus` | Overall status of this stage. |
| `clauses` | `Vec<ClauseAnalysis>` | Per-clause analysis. |

### Trait implementations

`Debug`.

---

### `StageStatus`

Overall status of a stage in gap analysis.

```rust
pub enum StageStatus {
    Matched,
    PartiallyMatched { matched: usize, total: usize },
    Unmatched,
}
```

| Variant | Description |
|---------|-------------|
| `Matched` | All clauses in this stage matched. |
| `PartiallyMatched { matched, total }` | Some clauses matched. `matched` out of `total`. |
| `Unmatched` | No clauses matched. |

### Trait implementations

`Debug`.

---

### `ClauseAnalysis`

Analysis of a single clause within a stage.

```rust
pub struct ClauseAnalysis {
    pub description: String,
    pub matched: bool,
    pub reason: Option<String>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `description` | `String` | Human-readable clause description (e.g., `?e1 --["type"]--> Literal("harm")`). |
| `matched` | `bool` | Whether this clause matched. |
| `reason` | `Option<String>` | Explanation of why the clause failed, or `None` if it matched. |

### Trait implementations

`Debug`.
