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

```rust reference file=tests/guides_custom_adapter.rs#value_type
```

## Step 2: Define your graph struct

A minimal implementation stores edges as tuples. Production implementations should index by (source, label) and (label, value) for fast lookup.

```rust reference file=tests/guides_custom_adapter.rs#graph_struct
```

## Step 3: Implement DataSource

The trait has 4 associated types and 6 methods. See the [DataSource Reference](../reference/datasource) for full API documentation. Here is a complete implementation:

```rust reference file=tests/guides_custom_adapter.rs#impl_datasource
```

## Step 4: Verify with a smoke test

Before running the full golden suite, verify basic functionality:

```rust reference file=tests/guides_custom_adapter.rs#smoke_test
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

## Opting out of computed bindings

Engine methods that evaluate [`let` bindings](./computed-bindings) require `V: ArithmeticValue`. If your value type isn't numeric -- or you simply don't intend to use `let` in your patterns -- add a no-op impl that returns `None` from every method:

```rust
impl fabula::expr::ArithmeticValue for MyValue {
    fn try_add(&self, _: &Self) -> Option<Self> { None }
    fn try_sub(&self, _: &Self) -> Option<Self> { None }
    fn try_mul(&self, _: &Self) -> Option<Self> { None }
    fn try_div(&self, _: &Self) -> Option<Self> { None }
}
```

Patterns that don't use `let` work normally. Patterns that do use `let` will silently fail to match, the same as any unsatisfied clause -- the let's expression evaluates to `None` and the stage match drops.

If your value type *does* have a numeric variant you want to expose, return `Some(MyValue::Num(...))` for the `(Num, Num)` case and `None` otherwise. The in-tree `MemValue`, `PetValue`, and `GrafeoValue` impls follow this pattern -- read them as templates.

See the [`ArithmeticValue` reference](../reference/patterns#arithmeticvalue) for the trait surface and the [Computed Bindings guide](./computed-bindings) for what `let` actually does.

## Benchmarking your adapter

Once the golden tests pass, benchmark your adapter against PetGraph to catch performance regressions or surface indexing opportunities.

### Quick comparison

The easiest path is to reuse the existing `WorkloadConfig` harness from `fabula-bench`. Because `YourAdapter` must already implement `TestGraph` (from Step 5), `build_isolated_workload` will accept it directly:

```rust
use fabula_bench::{build_isolated_workload, WorkloadConfig};
use std::time::Instant;

let config = WorkloadConfig { pattern_count: 30, ..Default::default() };
let mut workload = build_isolated_workload::<YourAdapter>(&config);

// Insert all edges first so secondary clauses in multi-clause stages
// can see edges arriving in the same tick.
let edges: Vec<_> = workload.pending_edges.drain(..).collect();
for edge in &edges {
    edge.insert(&mut workload.graph);
}
let start = Instant::now();
for edge in &edges {
    edge.notify(&workload.graph, &mut workload.engine);
}
let elapsed = start.elapsed();
println!("avg per edge: {:?}", elapsed / edges.len().max(1) as u32);
```

Compare against the published baseline: **~28 microseconds per `on_edge_added` on PetGraph at GM-scale**. If you're more than 3-5x slower, your `edges_from` or `scan` implementation is probably the hot path.

### What to measure

- **Per-edge latency** (incremental): the time taken by one `on_edge_added` call averaged over a realistic stream. This is the number users care about.
- **`edges_from` latency in isolation**: benchmark the method directly against a graph of 1K, 10K, 100K edges. It should be O(degree) or better; if it's O(E), you need an index.
- **Batch throughput**: `engine.evaluate(&graph)` for one-shot queries. This calls `scan` heavily.
- **Memory**: peak active PMs. The engine's working set reflects the adapter's ability to prune stale partial matches during `end_tick()`.

### Common bottlenecks

- **Linear scans in `edges_from`.** Add a `HashMap<(Node, Label), Vec<Edge>>` index on insert.
- **Clone-heavy `Edge` construction.** Returning `Edge` by value is fine, but if your values are expensive (`String`, `HashMap`), consider interning or `Arc`.
- **Allocating `Vec` on every query.** Pre-allocate or reuse via a `&mut Vec` parameter if your hot path repeats the same query shape.

See [Performance -- Benchmarking your workload](./performance#benchmarking-your-workload) for the full benchmark suite layout.

## Verification checklist

After implementing `DataSource` and `TestGraph`, verify:

- [ ] `cargo test -p your-adapter-crate` passes all golden scenarios
- [ ] Batch and incremental modes produce the same results for consistency scenarios
- [ ] `edges_from` returns only edges active at the given time
- [ ] `scan` returns edges with the correct `source` field
- [ ] `edges_from_any_time` ignores time and returns all matching edges
- [ ] `value_as_node` correctly distinguishes node refs from literal values
- [ ] `now()` returns a meaningful current time
