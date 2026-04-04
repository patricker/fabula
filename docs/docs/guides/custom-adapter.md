---
sidebar_position: 8
title: Custom Adapter
---

# Custom Adapter

**Learning objective:** Implement `DataSource` for a custom graph store and validate it against the golden test suite.

## Prerequisites

- A graph store that supports labeled, directed edges with time intervals
- The `fabula` crate as a dependency
- For golden suite validation: `fabula-test-suite` as a dev dependency

## Step 1: Define your value type

Your graph store needs a value type that can represent both node references and literal values (strings, numbers, booleans). The engine uses `value_as_node` to distinguish between them.

```rust
use std::fmt;

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum MyValue {
    NodeRef(String),
    Text(String),
    Number(f64),
    Flag(bool),
}

impl fmt::Display for MyValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MyValue::NodeRef(id) => write!(f, "@{}", id),
            MyValue::Text(s) => write!(f, "\"{}\"", s),
            MyValue::Number(n) => write!(f, "{}", n),
            MyValue::Flag(b) => write!(f, "{}", b),
        }
    }
}
```

## Step 2: Define your graph struct

A minimal implementation stores edges as tuples. Production implementations should index by (source, label) and (label, value) for fast lookup.

```rust
use fabula::datasource::{DataSource, Edge, ValueConstraint};
use fabula::interval::Interval;
use std::hash::Hash;

struct StoredEdge {
    source: String,
    label: String,
    target: MyValue,
    interval: Interval<i64>,
}

pub struct MyGraph {
    edges: Vec<StoredEdge>,
    current_time: i64,
}

impl MyGraph {
    pub fn new() -> Self {
        Self {
            edges: Vec::new(),
            current_time: 0,
        }
    }

    pub fn set_time(&mut self, t: i64) {
        self.current_time = t;
    }

    pub fn add_edge(&mut self, source: &str, label: &str, target: MyValue, start: i64) {
        self.edges.push(StoredEdge {
            source: source.to_string(),
            label: label.to_string(),
            target,
            interval: Interval::open(start),
        });
    }

    pub fn add_edge_bounded(
        &mut self,
        source: &str,
        label: &str,
        target: MyValue,
        start: i64,
        end: i64,
    ) {
        self.edges.push(StoredEdge {
            source: source.to_string(),
            label: label.to_string(),
            target,
            interval: Interval::new(start, end),
        });
    }
}
```

## Step 3: Implement DataSource

The trait has 4 associated types and 6 methods. See the [DataSource Reference](../reference/datasource) for full API documentation. Here is a complete implementation:

```rust
impl DataSource for MyGraph {
    type N = String;
    type L = String;
    type V = MyValue;
    type T = i64;

    fn edges_from(
        &self,
        node: &String,
        label: &String,
        at: &i64,
    ) -> Vec<Edge<String, MyValue, i64>> {
        self.edges
            .iter()
            .filter(|e| &e.source == node && &e.label == label && e.interval.covers(at))
            .map(|e| Edge {
                source: e.source.clone(),
                target: e.target.clone(),
                interval: e.interval.clone(),
            })
            .collect()
    }

    fn scan(
        &self,
        label: &String,
        constraint: &ValueConstraint<MyValue>,
        at: &i64,
    ) -> Vec<Edge<String, MyValue, i64>> {
        self.edges
            .iter()
            .filter(|e| {
                &e.label == label
                    && constraint.matches(&e.target)
                    && e.interval.covers(at)
            })
            .map(|e| Edge {
                source: e.source.clone(),
                target: e.target.clone(),
                interval: e.interval.clone(),
            })
            .collect()
    }

    fn edges_from_any_time(
        &self,
        node: &String,
        label: &String,
    ) -> Vec<Edge<String, MyValue, i64>> {
        self.edges
            .iter()
            .filter(|e| &e.source == node && &e.label == label)
            .map(|e| Edge {
                source: e.source.clone(),
                target: e.target.clone(),
                interval: e.interval.clone(),
            })
            .collect()
    }

    fn scan_any_time(
        &self,
        label: &String,
        constraint: &ValueConstraint<MyValue>,
    ) -> Vec<Edge<String, MyValue, i64>> {
        self.edges
            .iter()
            .filter(|e| &e.label == label && constraint.matches(&e.target))
            .map(|e| Edge {
                source: e.source.clone(),
                target: e.target.clone(),
                interval: e.interval.clone(),
            })
            .collect()
    }

    fn now(&self) -> i64 {
        self.current_time
    }

    fn value_as_node(&self, value: &MyValue) -> Option<String> {
        match value {
            MyValue::NodeRef(id) => Some(id.clone()),
            _ => None,
        }
    }
}
```

## Step 4: Verify with a smoke test

Before running the full golden suite, verify basic functionality:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use fabula::prelude::*;

    #[test]
    fn smoke_test() {
        let mut g = MyGraph::new();
        g.add_edge("ev1", "eventType", MyValue::Text("hello".into()), 1);
        g.add_edge("ev1", "actor", MyValue::NodeRef("alice".into()), 1);
        g.set_time(10);

        let pattern = PatternBuilder::new("greeting")
            .stage("e", |s| s
                .edge("e", "eventType".into(), MyValue::Text("hello".into()))
                .edge_bind("e", "actor".into(), "who"))
            .build();

        let mut engine: SiftEngine<MyGraph> = SiftEngine::new();
        engine.register(pattern);
        let matches = engine.evaluate(&g);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].bindings["who"], BoundValue::Node("alice".into()));
    }
}
```

## Step 5: Integrate with the golden test suite

The golden test suite defines a `TestGraph` trait that abstracts over adapter differences. Implement it for your graph type, then the `golden_tests!` macro stamps out tests for all scenarios automatically.

Add dependencies to your adapter crate's `Cargo.toml`:

```toml
[dev-dependencies]
fabula-test-suite = { path = "../fabula-test-suite" }
paste = "1"
```

Implement `TestGraph`:

```rust
use fabula_test_suite::TestGraph;

impl TestGraph for MyGraph {
    fn new_graph() -> Self {
        MyGraph::new()
    }

    fn set_current_time(&mut self, t: i64) {
        self.set_time(t);
    }

    fn add_str_edge(&mut self, from: &str, label: &str, value: &str, start: i64) {
        self.add_edge(from, label, MyValue::Text(value.to_string()), start);
    }

    fn add_ref_edge(&mut self, from: &str, label: &str, to: &str, start: i64) {
        self.add_edge(from, label, MyValue::NodeRef(to.to_string()), start);
    }

    fn add_num_edge(&mut self, from: &str, label: &str, value: f64, start: i64) {
        self.add_edge(from, label, MyValue::Number(value), start);
    }

    fn add_str_edge_bounded(
        &mut self, from: &str, label: &str, value: &str, start: i64, end: i64,
    ) {
        self.add_edge_bounded(from, label, MyValue::Text(value.to_string()), start, end);
    }

    fn add_ref_edge_bounded(
        &mut self, from: &str, label: &str, to: &str, start: i64, end: i64,
    ) {
        self.add_edge_bounded(from, label, MyValue::NodeRef(to.to_string()), start, end);
    }

    fn str_val(s: &str) -> MyValue {
        MyValue::Text(s.to_string())
    }

    fn node_val(s: &str) -> MyValue {
        MyValue::NodeRef(s.to_string())
    }

    fn num_val(n: f64) -> MyValue {
        MyValue::Number(n)
    }
}
```

Then run the golden tests against your adapter. See [Golden Tests](./golden-tests) for details. For reference implementations, see the built-in adapters: [MemGraph](../reference/adapters/memory), [PetGraph](../reference/adapters/petgraph), and [Grafeo](../reference/adapters/grafeo).

## Pitfalls

**`scan` returning target instead of source.** The `Edge` struct returned from `scan` must have its `source` field set to the *source node* of the edge, not the target. The engine uses `source` to bind the stage's anchor variable. If you return the target, all variable bindings will be wrong.

**`edges_from_any_time` returning empty.** This method is used for negation window checking. If it returns empty when edges exist, negations will never fire. Make sure it ignores the time parameter and returns all edges regardless of when they are active.

**`now()` returning 0.** If your `now()` always returns the default value, `edges_from` and `scan` will only see edges whose intervals cover time 0. Set your graph's current time appropriately before each evaluation.

**`value_as_node` always returning `None`.** If `value_as_node` never identifies node references, the engine cannot traverse from one node to another. Multi-stage patterns with variable joins will fail because bound variables will be `BoundValue::Value` instead of `BoundValue::Node`, and subsequent stages cannot follow them as source nodes.

**`PartialOrd` implementation for your value type.** `ValueConstraint::Lt`, `Gt`, `Lte`, `Gte`, and `Between` use `PartialOrd`. If your value type's ordering is not meaningful for numeric comparisons (e.g., comparing a node ref to a string), these constraints may produce unexpected results. Consider implementing `PartialOrd` to return `None` for incomparable types.

## Verification checklist

After implementing `DataSource` and `TestGraph`, verify:

- [ ] `cargo test -p your-adapter-crate` passes all golden scenarios
- [ ] Batch and incremental modes produce the same results for consistency scenarios
- [ ] `edges_from` returns only edges active at the given time
- [ ] `scan` returns edges with the correct `source` field
- [ ] `edges_from_any_time` ignores time and returns all matching edges
- [ ] `value_as_node` correctly distinguishes node refs from literal values
- [ ] `now()` returns a meaningful current time
