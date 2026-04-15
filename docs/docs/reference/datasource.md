---
sidebar_position: 2
title: DataSource Trait
---

# DataSource Trait

`fabula::datasource` -- the trait a backing store implements to make its graph queryable by the sift engine.

## `DataSource`

```rust
pub trait DataSource {
    type N: Eq + Hash + Clone + Debug;
    type L: Eq + Hash + Clone + Debug;
    type V: PartialEq + PartialOrd + Clone + Debug;
    type T: Ord + Clone + Debug;

    fn edges_from(&self, node: &Self::N, label: &Self::L, at: &Self::T)
        -> Vec<Edge<Self::N, Self::V, Self::T>>;
    fn scan(&self, label: &Self::L, constraint: &ValueConstraint<Self::V>, at: &Self::T)
        -> Vec<Edge<Self::N, Self::V, Self::T>>;
    fn edges_from_any_time(&self, node: &Self::N, label: &Self::L)
        -> Vec<Edge<Self::N, Self::V, Self::T>>;
    fn scan_any_time(&self, label: &Self::L, constraint: &ValueConstraint<Self::V>)
        -> Vec<Edge<Self::N, Self::V, Self::T>>;
    fn now(&self) -> Self::T;
    fn value_as_node(&self, value: &Self::V) -> Option<Self::N>;
}
```

### Associated types

| Type | Bounds | Description |
|------|--------|-------------|
| `N` | `Eq + Hash + Clone + Debug` | Node identifier (e.g., `String`, `u64`, `EntityId`). |
| `L` | `Eq + Hash + Clone + Debug` | Edge label (e.g., `String`, `u32`, predicate enum). |
| `V` | `PartialEq + PartialOrd + Clone + Debug` | Edge value. Can represent node references, strings, numbers, booleans. Often an enum wrapping `N`. |
| `T` | `Ord + Clone + Debug` | Time type (e.g., `i64`, `chrono::NaiveDateTime`). |

### Methods

#### `edges_from`

Follows edges from a node with a given label, active at time `at`.

```rust
fn edges_from(
    &self,
    node: &Self::N,
    label: &Self::L,
    at: &Self::T,
) -> Vec<Edge<Self::N, Self::V, Self::T>>
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `node` | `&Self::N` | yes | -- | Source node to follow edges from. |
| `label` | `&Self::L` | yes | -- | Edge label to filter by. |
| `at` | `&Self::T` | yes | -- | Time point; only edges whose interval covers `at` are returned. |

**Returns:** `Vec<Edge<N, V, T>>` -- all matching edges with source, target value, and interval.

---

#### `scan`

Finds all source nodes that have an edge with `label` matching `constraint`, active at time `at`. This is the index scan -- used to find starting points for pattern matching when a clause binds a new variable.

```rust
fn scan(
    &self,
    label: &Self::L,
    constraint: &ValueConstraint<Self::V>,
    at: &Self::T,
) -> Vec<Edge<Self::N, Self::V, Self::T>>
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `label` | `&Self::L` | yes | -- | Edge label to filter by. |
| `constraint` | `&ValueConstraint<Self::V>` | yes | -- | Value constraint the target must satisfy. |
| `at` | `&Self::T` | yes | -- | Time point; only edges active at this time are returned. |

**Returns:** `Vec<Edge<N, V, T>>` -- all matching edges. The `source` field on each edge identifies the originating node.

---

#### `edges_from_any_time`

Follows edges from a node with a given label regardless of time. Used for temporal constraint checking and negation window evaluation.

```rust
fn edges_from_any_time(
    &self,
    node: &Self::N,
    label: &Self::L,
) -> Vec<Edge<Self::N, Self::V, Self::T>>
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `node` | `&Self::N` | yes | -- | Source node to follow edges from. |
| `label` | `&Self::L` | yes | -- | Edge label to filter by. |

**Returns:** `Vec<Edge<N, V, T>>` -- all edges ever valid, regardless of time.

---

#### `scan_any_time`

Scans for edges with `label` matching `constraint` at any time.

```rust
fn scan_any_time(
    &self,
    label: &Self::L,
    constraint: &ValueConstraint<Self::V>,
) -> Vec<Edge<Self::N, Self::V, Self::T>>
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `label` | `&Self::L` | yes | -- | Edge label to filter by. |
| `constraint` | `&ValueConstraint<Self::V>` | yes | -- | Value constraint the target must satisfy. |

**Returns:** `Vec<Edge<N, V, T>>` -- all matching edges ever valid.

---

#### `now`

Returns the current time in the graph's time model.

```rust
fn now(&self) -> Self::T
```

**Returns:** `Self::T` -- the current time.

---

#### `value_as_node`

Checks whether a value represents a node reference (for traversal) vs. a literal (for comparison).

```rust
fn value_as_node(&self, value: &Self::V) -> Option<Self::N>
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `value` | `&Self::V` | yes | -- | The value to inspect. |

**Returns:** `Option<Self::N>` -- `Some(node_id)` if the value is a node reference, `None` if it is a literal.

---

### When each method is called

| Method | Batch evaluation | Incremental (`on_edge_added`) | Negation check |
|--------|-----------------|-------------------------------|----------------|
| `edges_from` | Yes -- follows bound variables through stages. | Yes -- validates secondary clauses at event time. | Yes -- verifies remaining negation clauses. |
| `scan` | Yes -- finds starting nodes for unbound first clauses. | No. | No. |
| `edges_from_any_time` | No. | No. | No. |
| `scan_any_time` | No. | No. | Yes -- finds candidate entities for batch negation. |
| `now` | Yes -- passed to `edges_from`/`scan` as `at`. | Yes -- used for negation clause verification. | Yes -- implicit via batch/incremental. |
| `value_as_node` | Yes -- determines if targets are traversable. | Yes -- same. | Yes -- binding consistency checks. |

---

## `Edge<N, V, T>`

An edge returned from a `DataSource` query.

```rust
pub struct Edge<N, V, T> {
    pub target: V,
    pub interval: Interval<T>,
    pub source: N,
}
```

### Fields

| Name | Type | Description |
|------|------|-------------|
| `source` | `N` | The source node this edge originates from. |
| `target` | `V` | The target value this edge points to (node reference or literal). |
| `interval` | `Interval<T>` | The time interval during which this edge is/was valid. |

### Trait implementations

`Debug`, `Clone` (all derived, require bounds on `N`, `V`, `T`).

---

## `ValueConstraint<V>`

A constraint on edge target values. Used in pattern clauses and `scan` queries.

```rust
use fabula::datasource::ValueConstraint;

let eq  = ValueConstraint::Eq(42);
let rng = ValueConstraint::Between(10, 20);
assert!(eq.matches(&42));
assert!(rng.matches(&15));
```

### Variants

| Variant | Description |
|---------|-------------|
| `Eq(V)` | Must equal this exact value. |
| `Lt(V)` | Must be less than this value. |
| `Gt(V)` | Must be greater than this value. |
| `Lte(V)` | Must be less than or equal to this value. |
| `Gte(V)` | Must be greater than or equal to this value. |
| `Between(V, V)` | Must fall within `[low, high]` (inclusive on both ends). |
| `OneOf(Vec<V>)` | Must equal one of the listed values. |
| `Any` | Any value matches. |

### Methods

#### `matches`

Tests whether a value satisfies this constraint. Requires `V: PartialOrd + PartialEq`.

```rust
pub fn matches(&self, value: &V) -> bool
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `value` | `&V` | yes | -- | The value to test. |

**Returns:** `bool`

### Trait implementations

`Debug`, `Clone`, `PartialEq` (all derived).
