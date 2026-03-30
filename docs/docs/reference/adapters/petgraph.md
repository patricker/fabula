---
sidebar_position: 2
title: "Adapter: Petgraph"
---

# Adapter: Petgraph

`fabula_petgraph` -- wraps `petgraph::StableGraph` with temporal edges as a fabula `DataSource`.

**Crate:** `fabula-petgraph`

## `PetTemporalGraph<N, L, V, T>`

A temporal graph backed by `petgraph::StableGraph`. Nodes are identified by `N`, edges carry `TemporalEdge<L, V, T>`. A `HashMap<N, NodeIndex>` provides reverse lookup from user IDs to petgraph's internal `NodeIndex`.

Edges are stored as self-loops on the source node. The target is stored in the edge weight's `value` field, not as a petgraph edge endpoint.

```rust
use fabula_petgraph::{PetTemporalGraph, PetValue, NodeRef};
use fabula::interval::Interval;

let mut g: PetTemporalGraph<String, String, PetValue<String>, i64>
    = PetTemporalGraph::new(0);
g.add_node("alice".into());
g.add_edge_to_node("ev1".into(), "actor".into(), "alice".into(), Interval::open(1));
g.add_edge("ev1".into(), "type".into(), PetValue::Str("harm".into()), Interval::open(1));
g.set_time(10);
```

### Type parameters

| Parameter | Bounds | Description |
|-----------|--------|-------------|
| `N` | `Eq + Hash + Clone + Debug` | Node identifier type. |
| `L` | `Eq + Hash + Clone + Debug` | Edge label type. |
| `V` | `PartialEq + PartialOrd + Clone + Debug` | Value type. Use `PetValue<N>` for the default adapter. |
| `T` | `Ord + Clone + Debug` | Time type. |

### `DataSource` type mapping (with `PetValue<N>`)

| Associated type | Concrete type |
|-----------------|---------------|
| `N` | `N` |
| `L` | `L` |
| `V` | `PetValue<N>` |
| `T` | `T` |

The `DataSource` implementation requires `V = PetValue<N>` and `N: PartialOrd`.

### Methods

#### `PetTemporalGraph::new`

Creates a new empty graph with the given initial time.

```rust
pub fn new(initial_time: T) -> Self
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `initial_time` | `T` | yes | -- | Initial current time. |

**Returns:** `PetTemporalGraph<N, L, V, T>`

---

#### `set_time`

Sets the current time.

```rust
pub fn set_time(&mut self, t: T)
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `t` | `T` | yes | -- | The new current time. |

---

#### `add_node`

Adds a node, or returns the existing `NodeIndex` if already present.

```rust
pub fn add_node(&mut self, id: N) -> NodeIndex
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `id` | `N` | yes | -- | Node identifier. |

**Returns:** `NodeIndex` -- petgraph's internal node index.

---

#### `add_edge`

Adds a temporal edge from a node with an arbitrary value and interval.

```rust
pub fn add_edge(&mut self, from: N, label: L, value: V, interval: Interval<T>)
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `from` | `N` | yes | -- | Source node ID. Created if it does not exist. |
| `label` | `L` | yes | -- | Edge label. |
| `value` | `V` | yes | -- | Target value. |
| `interval` | `Interval<T>` | yes | -- | Validity interval. |

---

#### `add_edge_to_node`

Adds a temporal edge with a node-valued target. Ensures the target node exists. Requires `V: From<NodeRef<N>>`.

```rust
pub fn add_edge_to_node(&mut self, from: N, label: L, to: N, interval: Interval<T>)
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `from` | `N` | yes | -- | Source node ID. |
| `label` | `L` | yes | -- | Edge label. |
| `to` | `N` | yes | -- | Target node ID. Created if it does not exist. |
| `interval` | `Interval<T>` | yes | -- | Validity interval. |

---

#### `add_edge_bounded`

Adds a temporal edge with a bounded interval `[start, end)`. Requires `T: Ord`.

```rust
pub fn add_edge_bounded(&mut self, from: N, label: L, value: V, start: T, end: T)
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `from` | `N` | yes | -- | Source node ID. |
| `label` | `L` | yes | -- | Edge label. |
| `value` | `V` | yes | -- | Target value. |
| `start` | `T` | yes | -- | Interval start (inclusive). |
| `end` | `T` | yes | -- | Interval end (exclusive). |

---

#### `node_count`

Returns the number of nodes.

```rust
pub fn node_count(&self) -> usize
```

**Returns:** `usize`

---

#### `edge_count`

Returns the number of edges.

```rust
pub fn edge_count(&self) -> usize
```

**Returns:** `usize`

---

## `TemporalEdge<L, V, T>`

A temporal edge weight stored on a petgraph edge.

```rust
pub struct TemporalEdge<L, V, T> {
    pub label: L,
    pub value: V,
    pub interval: Interval<T>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `label` | `L` | Edge label. |
| `value` | `V` | Target value. |
| `interval` | `Interval<T>` | Validity interval. |

### Trait implementations

`Debug`, `Clone`.

---

## `NodeRef<N>`

Wrapper to distinguish node references from other values. Used with `From<NodeRef<N>>` to convert node IDs into `PetValue::Node`.

```rust
pub struct NodeRef<N>(pub N);
```

| Field | Type | Description |
|-------|------|-------------|
| `0` | `N` | The node identifier. |

### Trait implementations

`Debug`, `Clone`.

---

## `PetValue<N>`

A value type for the petgraph adapter. Holds node references or literals.

```rust
pub enum PetValue<N: Debug + Clone + PartialOrd> {
    Node(N),
    Str(String),
    Num(f64),
    Bool(bool),
}
```

| Variant | Description |
|---------|-------------|
| `Node(N)` | Reference to another node. `value_as_node` returns `Some`. |
| `Str(String)` | String literal. |
| `Num(f64)` | Numeric value. |
| `Bool(bool)` | Boolean value. |

### Trait implementations

| Trait | Notes |
|-------|-------|
| `Debug` | Derived. |
| `Clone` | Derived. |
| `PartialEq` | Derived. |
| `PartialOrd` | Derived. |
| `From<NodeRef<N>>` | Converts `NodeRef(n)` to `PetValue::Node(n)`. |
