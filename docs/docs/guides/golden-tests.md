---
sidebar_position: 5
title: Golden Tests
---

# Golden Tests

**Learning objective:** Add a new golden test scenario that runs against all three adapters.

## The 3-step process

### Step 1: Write the scenario function

Create a public function in `crates/fabula-test-suite/src/scenarios/`. It must be generic over `TestGraph`.

```rust
// crates/fabula-test-suite/src/scenarios/my_feature.rs

use crate::TestGraph;
use fabula::prelude::*;

/// Batch: describe what this scenario tests.
pub fn batch_my_scenario<G: TestGraph>() {
    // 1. Build the graph
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "greet", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev2", "eventType", "farewell", 3);
    g.add_ref_edge("ev2", "actor", "alice", 3);
    g.set_current_time(10);

    // 2. Build the pattern
    let pattern = PatternBuilder::new("greet_then_farewell")
        .stage("e1", |s| s
            .edge("e1", "eventType".into(), G::str_val("greet"))
            .edge_bind("e1", "actor".into(), "person"))
        .stage("e2", |s| s
            .edge("e2", "eventType".into(), G::str_val("farewell"))
            .edge_bind("e2", "actor".into(), "person"))
        .build();

    // 3. Run the engine and assert
    let mut engine: SiftEngine<G> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1, "greet then farewell by same person should match");
    assert!(G::is_node_eq(&matches[0].bindings["person"], "alice"));
}
```

Key points:

- Use `G::str_val()`, `G::node_val()`, and `G::num_val()` to create values, not adapter-specific constructors. This keeps the scenario generic.
- Use `G::is_node_eq()` to check bindings instead of comparing to a specific adapter's `BoundValue` variant.
- Add the graph edges, build the pattern, create the engine, register, evaluate, and assert -- all in one function.

If your scenario file is new, declare the module in `src/scenarios/mod.rs`:

```rust
// crates/fabula-test-suite/src/scenarios/mod.rs

mod my_feature;
pub use my_feature::*;
```

### Step 2: Register in the golden_tests! macro

Open `crates/fabula-test-suite/tests/golden.rs` and add your function name to the `golden_tests!` invocation:

```rust
golden_tests! {
    // ... existing scenarios ...

    // --- My feature ---
    batch_my_scenario,
}
```

This generates three test functions: `mem__batch_my_scenario`, `pet__batch_my_scenario`, and `grafeo__batch_my_scenario`.

### Step 3: Run the tests

```bash
cargo test -p fabula-test-suite
```

All three adapters execute your scenario. If any fails, the test name tells you which adapter has the issue (e.g., `pet__batch_my_scenario`).

## How to debug failures

**One adapter fails, others pass.** The failing adapter's `DataSource` implementation has a bug for that scenario's edge pattern. Common causes:

- `scan` or `edges_from` returns edges in a different order, causing a binding to pick a different node.
- `value_as_node` does not recognize node references in that adapter's value type.
- Bounded intervals are stored or queried differently.

**All three adapters fail.** Your scenario's assertions are wrong, or the pattern/graph has a bug. Run with `--nocapture` to see output:

```bash
cargo test -p fabula-test-suite batch_my_scenario -- --nocapture
```

**Temporal ordering issues.** If your scenario uses multiple stages, ensure the edge start times are strictly increasing across stages. Same-timestamp edges in different stages will not match due to the strict `<` ordering requirement.

## Naming conventions

Scenario function names use these prefixes:

| Prefix | Meaning |
|---|---|
| `batch_` | Tests `engine.evaluate()` (full snapshot scan) |
| `incremental_` | Tests `engine.on_edge_added()` (streaming edge-by-edge) |
| `gap_` | Tests `engine.why_not()` (gap analysis) |

Group related scenarios under a comment header in the `golden_tests!` macro:

```rust
golden_tests! {
    // --- My feature (batch) ---
    batch_my_feature_positive,
    batch_my_feature_negated,
    batch_my_feature_constraint,

    // --- My feature (incremental) ---
    incremental_my_feature_completes,
    incremental_my_feature_negation_kills,
}
```

## What makes a good scenario

**Tests one thing.** Each scenario should test a single behavior. "Negation fires when guest leaves" is good. "Full hospitality pattern with negation and value constraints and Allen relations" is too broad.

**Has a descriptive name.** The function name should describe the expected outcome: `batch_hospitality_negated_when_guest_leaves`, not `batch_test_3`.

**Includes a failure message.** Every `assert!` and `assert_eq!` should have a message explaining what went wrong:

```rust
assert_eq!(matches.len(), 0, "guest left town - negation should block");
```

**Covers both positive and negative cases.** If you add a scenario where a pattern matches, consider adding a companion scenario where it does not (and vice versa). This prevents false passes where the engine always returns matches (or never does).

**Avoids shared mutable state.** Each scenario creates its own graph, engine, and patterns from scratch. Do not use static variables or share state between scenarios.

## About the suite

The golden test suite has three layers:

1. **`TestGraph` trait** (`src/lib.rs`) -- abstracts over adapter differences. Each adapter implements it once in this crate (satisfying orphan rules since `TestGraph` is local).

2. **Scenario functions** (`src/scenarios/*.rs`) -- generic functions that build graphs, patterns, and assertions. They work with any `TestGraph` implementor.

3. **`golden_tests!` macro** (`tests/golden.rs`) -- stamps out `#[test]` functions for every (scenario, adapter) pair. Uses the `paste` crate to generate unique test names like `mem__batch_hospitality_matches`.

Currently the suite has 50+ scenarios covering batch evaluation, incremental matching, negation windows, value constraints, temporal ordering, Allen relations, gap analysis, multi-pattern interaction, and batch/incremental consistency.
