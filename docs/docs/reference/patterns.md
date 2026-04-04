---
sidebar_position: 3
title: Patterns & Builders
---

# Patterns & Builders

`fabula::pattern` -- compiled pattern types. `fabula::builder` -- ergonomic construction API.

## Pattern types

### `Var`

A named position in a pattern traversal. Variables appearing in multiple clauses create joins.

```rust
pub struct Var(pub String);
```

| Field | Type | Description |
|-------|------|-------------|
| `0` | `String` | The variable name. |

#### Methods

##### `Var::new`

```rust
pub fn new(name: impl Into<String>) -> Self
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `name` | `impl Into<String>` | yes | -- | Variable name. |

**Returns:** `Var`

#### Trait implementations

`Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `Display` (formats as `?name`).

---

### `Target<V>`

Specifies what an edge's target must match.

```rust
pub enum Target<V> {
    Bind(Var),
    Literal(V),
    Constraint(ValueConstraint<V>),
}
```

| Variant | Description |
|---------|-------------|
| `Bind(Var)` | Bind the target to a variable for traversal or join. |
| `Literal(V)` | The target must equal this exact value. |
| `Constraint(ValueConstraint<V>)` | The target must satisfy this constraint. |

#### Trait implementations

`Debug`, `Clone`.

---

### `Clause<L, V>`

A single edge traversal constraint within a stage.

```rust
pub struct Clause<L, V> {
    pub source: Var,
    pub label: L,
    pub target: Target<V>,
    pub negated: bool,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `source` | `Var` | Source node variable. Must be bound by a prior clause or be a scan root. |
| `label` | `L` | Edge label to follow. |
| `target` | `Target<V>` | What the target must match. |
| `negated` | `bool` | If `true`, the edge must NOT exist. |

#### Trait implementations

`Debug`, `Clone`.

---

### `Stage<L, V>`

A group of clauses anchored to a single event/node variable. Stages are the units of incremental matching -- each new edge tests against the next unmatched stage.

```rust
pub struct Stage<L, V> {
    pub anchor: Var,
    pub clauses: Vec<Clause<L, V>>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `anchor` | `Var` | The event/node variable this stage is anchored to. |
| `clauses` | `Vec<Clause<L, V>>` | Clauses that constrain this event/node. |

#### Trait implementations

`Debug`, `Clone`.

---

### `TemporalConstraint`

An explicit temporal ordering constraint between two event variables (beyond implicit left-to-right stage ordering).

```rust
pub struct TemporalConstraint {
    pub left: Var,
    pub relation: AllenRelation,
    pub right: Var,
    pub gap: Option<MetricGap>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `left` | `Var` | Variable whose interval should come first. |
| `relation` | `AllenRelation` | Required Allen relation (typically `Before` or `Meets`). |
| `right` | `Var` | Variable whose interval should come second. |
| `gap` | `Option<MetricGap>` | Optional metric bound (STN-style bounded difference). Set via `temporal_with_gap` on the builder or `gap min..max` in the DSL. |

#### Trait implementations

`Debug`, `Clone`.

---

### `Negation<L, V>`

A negation window -- a set of clauses that must NOT all match between two events. All clauses must match for the negation to fire (conjunctive).

```rust
pub struct Negation<L, V> {
    pub between_start: Var,
    pub between_end: Option<Var>,
    pub clauses: Vec<Clause<L, V>>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `between_start` | `Var` | Start of the negation window (must be bound). |
| `between_end` | `Option<Var>` | End of the negation window. `None` means open-ended (up to "now"). |
| `clauses` | `Vec<Clause<L, V>>` | Clauses that must NOT all match within the window. |

#### Trait implementations

`Debug`, `Clone`.

---

### `Pattern<L, V>`

A compiled sifting pattern -- a named subgraph template with temporal constraints and negation windows.

```rust
pub struct Pattern<L, V> {
    pub name: String,
    pub stages: Vec<Stage<L, V>>,
    pub temporal: Vec<TemporalConstraint>,
    pub negations: Vec<Negation<L, V>>,
    pub group: Option<String>,
    pub metadata: HashMap<String, String>,
    pub deadline_ticks: Option<u64>,
    pub repeat_range: Option<RepeatRange>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Pattern name, used in match results and gap analysis. |
| `stages` | `Vec<Stage<L, V>>` | Ordered event stages. Temporally ordered left-to-right. |
| `temporal` | `Vec<TemporalConstraint>` | Explicit temporal constraints beyond implicit ordering. |
| `negations` | `Vec<Negation<L, V>>` | Negation windows. |
| `group` | `Option<String>` | Mutual-exclusion group. When one pattern in a group completes, the engine kills active PMs for all other patterns in the same group. Set by `choice` composition. |
| `metadata` | `HashMap<String, String>` | Arbitrary key-value pairs propagated to `Match`, `SiftEvent`, and scored match types. Use for tagging patterns with narrative roles, priorities, or domain-specific attributes. |
| `deadline_ticks` | `Option<u64>` | If set, active partial matches for this pattern are expired (killed with `SiftEvent::Expired`) when they have been alive for more than this many ticks without completing. Checked during `end_tick()`. |
| `repeat_range` | `Option<RepeatRange>` | Looping repeat configuration. Set by `compose::repeat_range()` or DSL `* N..M` / `* N..`. When present, the engine loops over a segment of stages instead of completing after the last stage. See [DSL Reference — Repeat](dsl.md#repeat-). |

#### Methods

##### `all_vars`

Returns all variables used in this pattern (across all stages and negations), sorted and deduplicated.

```rust
pub fn all_vars(&self) -> Vec<&Var>
```

**Returns:** `Vec<&Var>`

#### Trait implementations

`Debug`, `Clone`.

---

### `RepeatRange`

Configuration for looping repeat patterns (DSL `* N..M` or `* N..`). The pattern's stages are laid out as `[first_... | last_...]`. The `last_` segment loops in the engine.

```rust
pub struct RepeatRange {
    pub stage_start: usize,
    pub stage_end: usize,
    pub min_reps: usize,
    pub max_reps: Option<usize>,
    pub shared_vars: HashSet<String>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `stage_start` | `usize` | First stage index of the looping segment (inclusive). |
| `stage_end` | `usize` | Last stage index of the looping segment (exclusive). |
| `min_reps` | `usize` | Minimum total occurrences before the first completion. `* 3..` means 3. |
| `max_reps` | `Option<usize>` | Maximum total occurrences. `None` = unlimited (`* N..`). |
| `shared_vars` | `HashSet<String>` | Variables shared across repetitions (not prefixed, persist across loops). |

Bindings in repeat-range matches:
- `first_*` — variables from the first sub-pattern occurrence (preserved across loops)
- `last_*` — variables from the most recent occurrence (overwritten each loop iteration)
- Shared variables — unprefixed, consistent across all iterations

Trait implementations: `Debug`, `Clone`, `PartialEq`.

---

## Builders

### `PatternBuilder<L, V>`

Ergonomic builder for constructing a `Pattern`. Requires `L: Clone, V: Clone`.

```rust
use fabula::builder::PatternBuilder;

let pattern = PatternBuilder::<String, String>::new("my_pattern")
    .stage("event1", |s| s
        .edge("event1", "type".into(), "failure".into())
        .edge_bind("event1", "actor".into(), "character"))
    .stage("event2", |s| s
        .edge("event2", "type".into(), "betrayal".into())
        .edge_bind("event2", "target".into(), "character"))
    .unless_between("event1", "event2", |neg| neg
        .edge("recovery", "type".into(), "trust_restored".into()))
    .build();
```

#### `PatternBuilder::new`

Starts building a new pattern.

```rust
pub fn new(name: impl Into<String>) -> Self
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `name` | `impl Into<String>` | yes | -- | Pattern name for identification. |

**Returns:** `PatternBuilder<L, V>`

---

#### `stage`

Adds an event stage. The `anchor` names the event variable. Use the callback to add clauses via `StageBuilder`.

```rust
pub fn stage(
    self,
    anchor: impl Into<String>,
    build: impl FnOnce(StageBuilder<L, V>) -> StageBuilder<L, V>,
) -> Self
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `anchor` | `impl Into<String>` | yes | -- | Event variable name for this stage. |
| `build` | `FnOnce(StageBuilder) -> StageBuilder` | yes | -- | Callback that adds clauses to the stage. |

**Returns:** `PatternBuilder<L, V>` (chainable)

---

#### `temporal`

Adds an explicit temporal constraint beyond implicit stage ordering.

```rust
pub fn temporal(
    self,
    left: impl Into<String>,
    relation: AllenRelation,
    right: impl Into<String>,
) -> Self
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `left` | `impl Into<String>` | yes | -- | Variable whose interval should come first. |
| `relation` | `AllenRelation` | yes | -- | Required Allen relation. |
| `right` | `impl Into<String>` | yes | -- | Variable whose interval should come second. |

**Returns:** `PatternBuilder<L, V>` (chainable)

---

#### `unless_between`

Adds a negation window: clauses that must NOT all match between two events.

```rust
pub fn unless_between(
    self,
    start: impl Into<String>,
    end: impl Into<String>,
    build: impl FnOnce(NegationBuilder<L, V>) -> NegationBuilder<L, V>,
) -> Self
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `start` | `impl Into<String>` | yes | -- | Start event variable (must be a stage anchor). |
| `end` | `impl Into<String>` | yes | -- | End event variable (must be a stage anchor). |
| `build` | `FnOnce(NegationBuilder) -> NegationBuilder` | yes | -- | Callback that adds clauses to the negation. |

**Returns:** `PatternBuilder<L, V>` (chainable)

---

#### `unless_after`

Adds a negation window with an open end (from a start event up to "now").

```rust
pub fn unless_after(
    self,
    start: impl Into<String>,
    build: impl FnOnce(NegationBuilder<L, V>) -> NegationBuilder<L, V>,
) -> Self
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `start` | `impl Into<String>` | yes | -- | Start event variable. |
| `build` | `FnOnce(NegationBuilder) -> NegationBuilder` | yes | -- | Callback that adds clauses. |

**Returns:** `PatternBuilder<L, V>` (chainable)

---

#### `unless_global`

Adds a negation that spans the entire pattern (first stage anchor to last stage anchor). For single-stage patterns, resolves to an open-ended window.

```rust
pub fn unless_global(
    self,
    build: impl FnOnce(NegationBuilder<L, V>) -> NegationBuilder<L, V>,
) -> Self
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `build` | `FnOnce(NegationBuilder) -> NegationBuilder` | yes | -- | Callback that adds clauses. |

**Returns:** `PatternBuilder<L, V>` (chainable)

---

#### `metadata`

Adds a key-value metadata pair to the pattern. Metadata is propagated to `Match`, `SiftEvent`, and scored match types. Call multiple times for multiple pairs. If the same key is set twice, the last value wins.

```rust
pub fn metadata(self, key: impl Into<String>, value: impl Into<String>) -> Self
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `key` | `impl Into<String>` | yes | -- | Metadata key. |
| `value` | `impl Into<String>` | yes | -- | Metadata value. |

**Returns:** `PatternBuilder<L, V>` (chainable)

---

#### `deadline`

Sets a deadline (in ticks) for partial match expiration. Active partial matches that have been alive for more than `ticks` ticks without completing are killed with `SiftEvent::Expired` during `end_tick()`. The deadline measures total PM lifecycle from first initiation -- when a PM advances to a new stage, `created_at_tick` is inherited, not reset.

```rust
pub fn deadline(self, ticks: u64) -> Self
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `ticks` | `u64` | yes | -- | Maximum ticks before expiry. Must be >= 1. |

**Returns:** `PatternBuilder<L, V>` (chainable)

---

#### `build`

Consumes the builder and returns the compiled pattern. Resolves global negation bounds to first/last stage anchors.

```rust
pub fn build(self) -> Pattern<L, V>
```

**Returns:** `Pattern<L, V>`

---

### `StageBuilder<L, V>`

Builder for a single event stage. Obtained from `PatternBuilder::stage`. Requires `L: Clone, V: Clone`.

#### `edge`

Adds a clause: `source --[label]--> literal_value`.

```rust
pub fn edge(
    self,
    source: impl Into<String>,
    label: L,
    value: V,
) -> Self
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `source` | `impl Into<String>` | yes | -- | Source variable name. |
| `label` | `L` | yes | -- | Edge label. |
| `value` | `V` | yes | -- | Exact target value to match. |

**Returns:** `StageBuilder<L, V>` (chainable)

---

#### `edge_bind`

Adds a clause: `source --[label]--> ?bind_var`. Traverses the edge and binds the target to a variable.

```rust
pub fn edge_bind(
    self,
    source: impl Into<String>,
    label: L,
    bind_to: impl Into<String>,
) -> Self
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `source` | `impl Into<String>` | yes | -- | Source variable name. |
| `label` | `L` | yes | -- | Edge label. |
| `bind_to` | `impl Into<String>` | yes | -- | Variable name to bind the target to. |

**Returns:** `StageBuilder<L, V>` (chainable)

---

#### `edge_constrained`

Adds a clause: `source --[label]--> (constraint)`. The target must satisfy a `ValueConstraint`.

```rust
pub fn edge_constrained(
    self,
    source: impl Into<String>,
    label: L,
    constraint: ValueConstraint<V>,
) -> Self
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `source` | `impl Into<String>` | yes | -- | Source variable name. |
| `label` | `L` | yes | -- | Edge label. |
| `constraint` | `ValueConstraint<V>` | yes | -- | Value constraint the target must satisfy. |

**Returns:** `StageBuilder<L, V>` (chainable)

---

#### `edge_gt_var` / `edge_lt_var` / `edge_eq_var` / `edge_gte_var` / `edge_lte_var`

Cross-stage value comparison: compare an edge's target against a previously-bound variable. The variable must have been bound by `edge_bind` in an earlier clause or stage. If the variable is bound to a `Node` (not a `Value`), the comparison silently fails (no match).

```rust
pub fn edge_gt_var(self, source: impl Into<String>, label: L, var_name: impl Into<String>) -> Self
pub fn edge_lt_var(self, source: impl Into<String>, label: L, var_name: impl Into<String>) -> Self
pub fn edge_eq_var(self, source: impl Into<String>, label: L, var_name: impl Into<String>) -> Self
pub fn edge_gte_var(self, source: impl Into<String>, label: L, var_name: impl Into<String>) -> Self
pub fn edge_lte_var(self, source: impl Into<String>, label: L, var_name: impl Into<String>) -> Self
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `source` | `impl Into<String>` | yes | -- | Source variable name. |
| `label` | `L` | yes | -- | Edge label. |
| `var_name` | `impl Into<String>` | yes | -- | Name of a previously-bound variable to compare against. |

**Returns:** `StageBuilder<L, V>` (chainable)

---

#### `not_edge`

Adds a negated clause: the edge must NOT exist.

```rust
pub fn not_edge(
    self,
    source: impl Into<String>,
    label: L,
    value: V,
) -> Self
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `source` | `impl Into<String>` | yes | -- | Source variable name. |
| `label` | `L` | yes | -- | Edge label. |
| `value` | `V` | yes | -- | Target value that must NOT be present. |

**Returns:** `StageBuilder<L, V>` (chainable)

---

### `NegationBuilder<L, V>`

Builder for a negation window. Obtained from `PatternBuilder::unless_between`, `unless_after`, or `unless_global`. Requires `L: Clone, V: Clone`.

#### `edge`

Adds a clause to the negation body (edge that must NOT exist in the window).

```rust
pub fn edge(
    self,
    source: impl Into<String>,
    label: L,
    value: V,
) -> Self
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `source` | `impl Into<String>` | yes | -- | Source variable name. |
| `label` | `L` | yes | -- | Edge label. |
| `value` | `V` | yes | -- | Exact target value. |

**Returns:** `NegationBuilder<L, V>` (chainable)

---

#### `edge_bind`

Adds a binding clause to the negation body.

```rust
pub fn edge_bind(
    self,
    source: impl Into<String>,
    label: L,
    bind_to: impl Into<String>,
) -> Self
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `source` | `impl Into<String>` | yes | -- | Source variable name. |
| `label` | `L` | yes | -- | Edge label. |
| `bind_to` | `impl Into<String>` | yes | -- | Variable name to bind. |

**Returns:** `NegationBuilder<L, V>` (chainable)

---

#### `edge_constrained`

Adds a constrained clause to the negation body.

```rust
pub fn edge_constrained(
    self,
    source: impl Into<String>,
    label: L,
    constraint: ValueConstraint<V>,
) -> Self
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `source` | `impl Into<String>` | yes | -- | Source variable name. |
| `label` | `L` | yes | -- | Edge label. |
| `constraint` | `ValueConstraint<V>` | yes | -- | Value constraint the target must satisfy. |

**Returns:** `NegationBuilder<L, V>` (chainable)
