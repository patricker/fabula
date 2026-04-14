# Fabula

Incremental pattern matching over temporal graphs.

Fabula finds patterns in graphs where edges have validity intervals. Define a pattern ("character whose loyalty dropped after an institutional failure, with no trust recovery in between"), register it with the engine, and it tracks partial matches incrementally as new edges arrive.

```rust
use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

// Build a graph
let mut g = MemGraph::new();
g.add_str("ev1", "eventType", "enterTown", 1);
g.add_ref("ev1", "actor", "alice", 1);
g.add_str("ev2", "eventType", "showHospitality", 2);
g.add_ref("ev2", "actor", "bob", 2);
g.add_ref("ev2", "target", "alice", 2);
g.add_str("ev3", "eventType", "harm", 3);
g.add_ref("ev3", "actor", "bob", 3);
g.add_ref("ev3", "target", "alice", 3);
g.set_time(10);

// Define a pattern: host harms guest after showing hospitality
let pattern = PatternBuilder::new("violation_of_hospitality")
    .stage("e1", |s| s
        .edge("e1", "eventType".into(), MemValue::Str("enterTown".into()))
        .edge_bind("e1", "actor".into(), "guest"))
    .stage("e2", |s| s
        .edge("e2", "eventType".into(), MemValue::Str("showHospitality".into()))
        .edge_bind("e2", "actor".into(), "host")
        .edge_bind("e2", "target".into(), "guest"))
    .stage("e3", |s| s
        .edge("e3", "eventType".into(), MemValue::Str("harm".into()))
        .edge_bind("e3", "actor".into(), "host")
        .edge_bind("e3", "target".into(), "guest"))
    .unless_between("e1", "e3", |neg| neg
        .edge("eMid", "eventType".into(), MemValue::Str("leaveTown".into()))
        .edge_bind("eMid", "actor".into(), "guest"))
    .build();

// Evaluate
let mut engine = SiftEngine::new();
engine.register(pattern);
let matches = engine.evaluate(&g);
assert_eq!(matches.len(), 1);
```

## Features

- **Batch evaluation** -- run patterns against a graph snapshot, get all matches
- **Incremental evaluation** -- register patterns, feed edges one at a time, get notified as patterns advance or complete
- **Partial match tracking** -- know how close each pattern is to completion at any moment
- **Gap analysis (`why_not`)** -- clause-by-clause breakdown of why a pattern hasn't matched
- **Allen interval algebra** -- 13 temporal relations (before, after, meets, overlaps, during, contains, starts, finishes, equals, and inverses)
- **Metric temporal constraints** -- STN-style bounded gap constraints on Allen relations (`gap 3..10`)
- **Negation windows** -- "no event of type X between events A and B"
- **Value constraints** -- Eq, Lt, Gt, Lte, Gte, Between, Any
- **Pattern composition** -- sequence (`>>`), exclusive choice (`|`), repeat (`*`) with variable sharing
- **Text DSL** -- human-readable pattern syntax with strict variable scoping and compose operators
- **Surprise scoring** -- Shannon surprise + StU (Select the Unexpected) property-level scoring
- **Narrative scoring** -- thread tracking, tension arcs, pivot detection, composite MCTS quality function
- **Pattern lifecycle** -- enable/disable/deregister, per-pattern metrics, staleness detection
- **Plant/payoff tracking** -- Chekhov's gun monitoring with staleness alerts
- **MCTS forking** -- clone engine state for speculative evaluation
- **TypeMapper** -- compile DSL patterns to arbitrary type systems (e.g., Paracausality `u32` predicates)
- **Zero dependencies** in the core crate

## Crates

| Crate | Dependencies | Purpose |
|---|---|---|
| [`fabula`](crates/fabula) | None | Core library: pattern types, `DataSource` trait, `SiftEngine`, Allen algebra, scoring |
| [`fabula-memory`](crates/fabula-memory) | `fabula` | `MemGraph` -- simple in-memory `DataSource` for testing and prototyping |
| [`fabula-petgraph`](crates/fabula-petgraph) | `fabula`, `petgraph` | `DataSource` adapter wrapping `petgraph::StableGraph` with temporal edges |
| [`fabula-grafeo`](crates/fabula-grafeo) | `fabula`, `grafeo` | `DataSource` adapter for the [Grafeo](https://github.com/GrafeoDB/grafeo) graph database |
| [`fabula-dsl`](crates/fabula-dsl) | `fabula`, `fabula-memory` | Text DSL parser: pattern syntax, graph syntax, compose operators, TypeMapper |
| [`fabula-narratives`](crates/fabula-narratives) | `fabula` | Narrative scoring: thread tracking, tension arcs, pivot detection, MCTS quality function |
| [`fabula-wasm`](crates/fabula-wasm) | `fabula`, `fabula-dsl`, `fabula-memory` | WebAssembly bindings for DSL parsing and evaluation |
| [`fabula-test-suite`](crates/fabula-test-suite) | all adapters | Golden test suite: 61 scenarios running against all 3 adapters (183 tests) |
| [`fabula-bench`](crates/fabula-bench) | `fabula`, adapters | Benchmark harness: divan parameterized benchmarks + profiling binary |

## The DataSource Trait

Fabula queries any temporal graph through one trait:

```rust
pub trait DataSource {
    type N: Eq + Hash + Clone + Debug;    // Node ID
    type L: Eq + Hash + Clone + Debug;    // Edge label
    type V: PartialEq + PartialOrd + Clone + Debug;  // Value
    type T: Ord + Clone + Debug;          // Time

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

Implement this for your graph store and fabula handles the rest.

## Patterns

Patterns are ordered sequences of **stages**. Each stage is a group of edge constraints anchored to a single node. Stages must match in temporal order. Variables that appear in multiple stages create joins.

```rust
// Two betrayals by the same character, no reconciliation between
let pattern = PatternBuilder::new("double_betrayal")
    .stage("e1", |s| s
        .edge("e1", "eventType".into(), val("betray"))
        .edge_bind("e1", "actor".into(), "char")
        .edge("char", "trait".into(), val("impulsive")))
    .stage("e2", |s| s
        .edge("e2", "eventType".into(), val("betray"))
        .edge_bind("e2", "actor".into(), "char"))
    .unless_global(|neg| neg
        .edge("mid", "eventType".into(), val("reconcile"))
        .edge_bind("mid", "actor".into(), "char"))
    .build();
```

### Negation

Three forms:

- **`unless_between("e1", "e3", ...)`** -- no matching event between two stages
- **`unless_after("e1", ...)`** -- no matching event after a stage (open-ended)
- **`unless_global(...)`** -- no matching event anywhere in the pattern's span

### Incremental Matching

Feed edges one at a time. The engine tracks partial matches and emits events:

```rust
let events = engine.on_edge_added(&graph, &source, &label, &value, &interval);
for event in events {
    match event {
        SiftEvent::Advanced { pattern, stage_index, .. } => { /* stage matched */ }
        SiftEvent::Completed { pattern, bindings, .. } => { /* full match */ }
        SiftEvent::Negated { pattern, clause_label, .. } => { /* killed by negation */ }
    }
}
```

### Gap Analysis

When a pattern hasn't matched, find out why:

```rust
if let Some(analysis) = engine.why_not(&graph, "my_pattern") {
    for stage in &analysis.stages {
        match &stage.status {
            StageStatus::Matched => { /* this stage is fine */ }
            StageStatus::Unmatched => { /* this is where it fails */ }
            StageStatus::PartiallyMatched { matched, total } => { /* some clauses matched */ }
        }
    }
}
```

## Bringing Your Own Graph

Implement `DataSource` for your graph store. See the adapter crates for examples:

- [`fabula-memory/src/lib.rs`](crates/fabula-memory/src/lib.rs) -- simplest (~200 LOC, Vec-backed linear scan)
- [`fabula-petgraph/src/lib.rs`](crates/fabula-petgraph/src/lib.rs) -- wraps petgraph's StableGraph (~220 LOC)
- [`fabula-grafeo/src/lib.rs`](crates/fabula-grafeo/src/lib.rs) -- wraps Grafeo's programmatic API (~330 LOC)

## Testing

```bash
cargo test --workspace           # 422+ tests across all crates
cargo test -p fabula-test-suite  # 183 golden tests (61 scenarios x 3 adapters)
cargo clippy --workspace -- -D warnings
```

The golden test suite uses a `TestGraph` trait to run every scenario against all three adapters. Adding a test:

1. Write `pub fn my_scenario<G: TestGraph>()` in `crates/fabula-test-suite/src/scenarios/`
2. Add `my_scenario,` to the `golden_tests!` macro in `crates/fabula-test-suite/tests/golden.rs`
3. It now runs against MemGraph, petgraph, and Grafeo automatically.

## Research Lineage

Fabula is a Rust port and extension of:

- **[Felt](https://github.com/mkremins/felt)** (Kreminski et al., ICIDS 2019) -- story sifting over EAV databases
- **[Winnow](https://github.com/mkremins/winnow)** (Kreminski et al., AIIDE 2021) -- incremental story sifting DSL with partial match tracking

Extensions beyond Felt/Winnow:

- **Allen interval algebra** instead of entity-ID ordering for temporal constraints
- **Metric temporal constraints** (STN gap bounds -- Dechter/Meiri/Pearl 1991)
- **Generic `DataSource` trait** instead of DataScript coupling
- **Decoupled `SiftEngine<N,L,V,T>`** -- engine outlives any particular DataSource
- **Multiple adapter crates** for real-world graph backends
- **Gap analysis (`why_not`)** for debugging unmatched patterns
- **Pattern composition** (Kreminski et al. 2025 FDG) -- sequence, choice, repeat
- **Text DSL** with strict variable scoping, compose operators, and TypeMapper
- **Surprise scoring** -- Shannon surprise + StU (Kreminski et al. 2022 ICIDS)
- **Narrative scoring** -- thread tracking (Kowal MICE), tension arcs (Booth 2009), pivot detection (Schulz et al. 2024), composite MCTS quality function (Nelson & Mateas 2005)

See [DESIGN.md](DESIGN.md) for the full feature mapping to the reference implementations.

## License

MIT
