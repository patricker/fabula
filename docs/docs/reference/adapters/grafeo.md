---
sidebar_position: 3
title: "Adapter: Grafeo"
---

# Adapter: Grafeo

`fabula_grafeo` -- wraps a `grafeo::GrafeoDB` in-memory instance as a fabula `DataSource`.

**Crate:** `fabula-grafeo`

### Storage conventions

- Temporal intervals are stored as edge properties: `_valid_from` (required, `Int64`), `_valid_to` (optional, `Int64`).
- Edge values are stored under the `_value` edge property.
- Node references are distinguished from string literals by the `_is_node_ref` boolean edge property.
- Edge labels map to Grafeo relationship types.

## `GrafeoGraph`

A temporal graph backed by Grafeo's in-memory graph database.

```rust
use fabula_grafeo::{GrafeoGraph, GrafeoValue};

let mut g = GrafeoGraph::new();
g.add_str("ev1", "eventType", "arrival", 1);
g.add_ref("ev1", "actor", "alice", 1);
g.add_num("ev1", "severity", 3.5, 1);
g.add_edge_bounded("ev2", "eventType",
    GrafeoValue::Str("departure".into()), 5, 10);
g.set_time(10);
```

### `DataSource` type mapping

| Associated type | Concrete type |
|-----------------|---------------|
| `N` | `String` |
| `L` | `String` |
| `V` | `GrafeoValue` |
| `T` | `i64` |

### Methods

#### `GrafeoGraph::new`

Creates a new in-memory Grafeo-backed graph with `current_time = 0`.

```rust
pub fn new() -> Self
```

**Returns:** `GrafeoGraph`

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

#### `ensure_node`

Ensures a node exists in the Grafeo database. Returns its `NodeId`. Creates the node with a `_id` property if it does not exist.

```rust
pub fn ensure_node(&mut self, id: &str) -> NodeId
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `id` | `&str` | yes | -- | Node identifier string. |

**Returns:** `grafeo::NodeId`

---

#### `add_edge`

Adds a temporal edge with an open-ended interval starting at `start`. For `GrafeoValue::Node` targets, ensures the target node exists.

```rust
pub fn add_edge(&mut self, from: &str, label: &str, value: GrafeoValue, start: i64)
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `from` | `&str` | yes | -- | Source node ID. Created if it does not exist. |
| `label` | `&str` | yes | -- | Edge label (Grafeo relationship type). |
| `value` | `GrafeoValue` | yes | -- | Target value. |
| `start` | `i64` | yes | -- | Interval start time. |

---

#### `add_edge_bounded`

Adds a temporal edge with a bounded interval `[start, end)`. For `GrafeoValue::Node` targets, ensures the target node exists.

```rust
pub fn add_edge_bounded(
    &mut self,
    from: &str,
    label: &str,
    value: GrafeoValue,
    start: i64,
    end: i64,
)
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `from` | `&str` | yes | -- | Source node ID. |
| `label` | `&str` | yes | -- | Edge label. |
| `value` | `GrafeoValue` | yes | -- | Target value. |
| `start` | `i64` | yes | -- | Interval start time. |
| `end` | `i64` | yes | -- | Interval end time (exclusive). |

---

#### `add_ref`

Convenience: adds a node-to-node edge with an open-ended interval.

```rust
pub fn add_ref(&mut self, from: &str, label: &str, to: &str, start: i64)
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `from` | `&str` | yes | -- | Source node ID. |
| `label` | `&str` | yes | -- | Edge label. |
| `to` | `&str` | yes | -- | Target node ID. Stored as `GrafeoValue::Node`. |
| `start` | `i64` | yes | -- | Interval start time. |

---

#### `add_str`

Convenience: adds a string-valued edge with an open-ended interval.

```rust
pub fn add_str(&mut self, from: &str, label: &str, value: &str, start: i64)
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `from` | `&str` | yes | -- | Source node ID. |
| `label` | `&str` | yes | -- | Edge label. |
| `value` | `&str` | yes | -- | String value. Stored as `GrafeoValue::Str`. |
| `start` | `i64` | yes | -- | Interval start time. |

---

#### `add_num`

Convenience: adds a numeric-valued edge with an open-ended interval.

```rust
pub fn add_num(&mut self, from: &str, label: &str, value: f64, start: i64)
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `from` | `&str` | yes | -- | Source node ID. |
| `label` | `&str` | yes | -- | Edge label. |
| `value` | `f64` | yes | -- | Numeric value. Stored as `GrafeoValue::Num`. |
| `start` | `i64` | yes | -- | Interval start time. |

---

### Trait implementations

| Trait | Notes |
|-------|-------|
| `Default` | Equivalent to `GrafeoGraph::new()`. |
| `DataSource` | Queries via Grafeo's `get_neighbors_outgoing_by_type`. |

---

## `GrafeoValue`

A value in the Grafeo adapter.

```rust
pub enum GrafeoValue {
    Node(String),
    Str(String),
    Num(f64),
    Bool(bool),
}
```

| Variant | Description |
|---------|-------------|
| `Node(String)` | Reference to another node by string ID. `value_as_node` returns `Some`. |
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
