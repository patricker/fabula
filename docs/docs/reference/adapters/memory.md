---
sidebar_position: 1
title: "Adapter: Memory"
---

# Adapter: Memory

`fabula_memory` -- a simple in-memory temporal graph. Linear-scan queries. Suitable for testing and small graphs.

**Crate:** `fabula-memory`

## `MemGraph`

An in-memory temporal graph storing edges as `(source, label, target, interval)` tuples.

```rust
use fabula_memory::{MemGraph, MemValue};

let mut g = MemGraph::new();
g.add_str("ev1", "type", "arrival", 1);
g.add_ref("ev1", "actor", "alice", 1);
g.add_num("ev1", "severity", 3.5, 1);
g.set_time(10);
```

### `DataSource` type mapping

| Associated type | Concrete type |
|-----------------|---------------|
| `N` | `String` |
| `L` | `String` |
| `V` | `MemValue` |
| `T` | `i64` |

### Methods

#### `MemGraph::new`

Creates a new empty graph with `current_time = 0`.

```rust
pub fn new() -> Self
```

**Returns:** `MemGraph`

---

#### `set_time`

Sets the current time.

```rust
pub fn set_time(&mut self, t: i64)
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `t` | `i64` | yes | -- | The new current time. |

---

#### `add_edge`

Adds an edge with an open-ended interval starting at `start`.

```rust
pub fn add_edge(&mut self, source: &str, label: &str, target: MemValue, start: i64)
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `source` | `&str` | yes | -- | Source node ID. |
| `label` | `&str` | yes | -- | Edge label. |
| `target` | `MemValue` | yes | -- | Target value. |
| `start` | `i64` | yes | -- | Interval start time. |

---

#### `add_edge_bounded`

Adds an edge with a bounded interval `[start, end)`.

```rust
pub fn add_edge_bounded(
    &mut self,
    source: &str,
    label: &str,
    target: MemValue,
    start: i64,
    end: i64,
)
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `source` | `&str` | yes | -- | Source node ID. |
| `label` | `&str` | yes | -- | Edge label. |
| `target` | `MemValue` | yes | -- | Target value. |
| `start` | `i64` | yes | -- | Interval start time. |
| `end` | `i64` | yes | -- | Interval end time (exclusive). |

---

#### `add_ref`

Convenience: adds a node-to-node edge with an open-ended interval.

```rust
pub fn add_ref(&mut self, source: &str, label: &str, target_node: &str, start: i64)
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `source` | `&str` | yes | -- | Source node ID. |
| `label` | `&str` | yes | -- | Edge label. |
| `target_node` | `&str` | yes | -- | Target node ID. Stored as `MemValue::Node`. |
| `start` | `i64` | yes | -- | Interval start time. |

---

#### `add_str`

Convenience: adds a node-to-string edge with an open-ended interval.

```rust
pub fn add_str(&mut self, source: &str, label: &str, value: &str, start: i64)
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `source` | `&str` | yes | -- | Source node ID. |
| `label` | `&str` | yes | -- | Edge label. |
| `value` | `&str` | yes | -- | String value. Stored as `MemValue::Str`. |
| `start` | `i64` | yes | -- | Interval start time. |

---

#### `add_num`

Convenience: adds a node-to-number edge with an open-ended interval.

```rust
pub fn add_num(&mut self, source: &str, label: &str, value: f64, start: i64)
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `source` | `&str` | yes | -- | Source node ID. |
| `label` | `&str` | yes | -- | Edge label. |
| `value` | `f64` | yes | -- | Numeric value. Stored as `MemValue::Num`. |
| `start` | `i64` | yes | -- | Interval start time. |

---

#### `edge_count`

Returns the total number of edges in the graph.

```rust
pub fn edge_count(&self) -> usize
```

**Returns:** `usize`

---

### Trait implementations

| Trait | Notes |
|-------|-------|
| `Default` | Equivalent to `MemGraph::new()`. |
| `DataSource` | Linear scan over all edges for all query methods. |

---

## `MemValue`

A value in the in-memory graph.

```rust
pub enum MemValue {
    Node(String),
    Str(String),
    Num(f64),
    Bool(bool),
}
```

| Variant | Description |
|---------|-------------|
| `Node(String)` | Reference to another node. `value_as_node` returns `Some`. |
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
| `Display` | `Node` formats as `@id`, `Str` as `"value"`, `Num` and `Bool` as their values. |
