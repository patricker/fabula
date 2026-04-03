---
sidebar_position: 4
title: Design Decisions
---

# Design Decisions

Fabula makes five deliberate design choices that distinguish it from its predecessors (Felt, Winnow) and from general-purpose graph query engines. Each choice has trade-offs.

## 1. Sifting only, no action system

Felt combines pattern matching with an action system: `registerAction`, `possibleActions`, `realizeEvent`, `processEffect`. Actions use sifting patterns as preconditions -- "find all situations where a character *could* betray someone" -- and then select and execute an action.

Fabula implements only the sifting side. It detects patterns; it does not generate events.

**Why.** The action system couples the sift engine to a specific simulation architecture. Felt actions assume a particular event model (effects with handlers, intent tokens, reflection cycles). Fabula aims to work with *any* simulation that produces temporal graph data, regardless of how it models actions.

**What you lose.** You cannot ask fabula "what could happen next?" You write that logic in your simulation layer, potentially using fabula matches as input.

**What you gain.** A smaller, more composable library. Your simulation architecture stays independent. You can use fabula for monitoring and analysis of existing logs, not just live simulations.

## 2. Generic DataSource, not DataScript

Felt and Winnow are built on DataScript, an immutable in-memory Datalog database. All state lives in DataScript's entity-attribute-value (EAV) store. Patterns compile to DataScript query strings.

Fabula defines a generic `DataSource` trait with four associated types (node, label, value, time) and six methods. Any graph store can implement it.

**Why.** DataScript is JavaScript-only and imposes a specific data model (EAV with monotonic entity IDs). Fabula is Rust-native and should work with whatever graph store the host application uses -- petgraph, a custom ECS, an in-memory adjacency list, a SQLite-backed temporal store.

**What you lose.** DataScript's immutable snapshots make time-travel trivial (keep old database values). DataScript's Datalog engine supports recursive rules. Fabula has neither.

**What you gain.** Zero forced dependencies. The core `fabula` crate has no external dependencies at all. Adapter crates (`fabula-memory`, `fabula-petgraph`, `fabula-grafeo`) are optional. You can implement `DataSource` for your own store in ~100 lines.

**Performance implications.** Fabula's performance depends entirely on the `DataSource` implementation. The in-memory `MemGraph` adapter does linear scans -- fine for tests, not for production. A production adapter should index edges by (source, label) and (label, value) for fast lookup. The engine calls `edges_from` and `scan` frequently during matching.

## 3. Allen intervals, not entity IDs

Winnow orders events by comparing DataScript entity IDs: lower ID means earlier event. Fabula attaches time intervals to edges and uses Allen's interval algebra for temporal reasoning.

**Why.** Entity-ID ordering only supports "before" and "after." Interval algebra supports 13 relations (before, during, overlaps, contains, etc.), enabling richer temporal patterns. It also decouples the engine from any particular ID assignment strategy.

**What you lose.** Events at the same timestamp cannot be sequenced -- the engine uses strict inequality (`start < start`) for implicit ordering. In Winnow, same-tick events have different entity IDs and can still be ordered.

**What you gain.** Duration-aware matching. You can find events that happen *during* other events, or that *overlap* with them. This is impossible with ID-based ordering.

See [Temporal Model](./temporal-model.md) for the full analysis.

## 4. Direct graph traversal, not Datalog

Felt patterns compile to Datalog queries executed by DataScript's query engine. Winnow adds incremental tracking on top of Datalog results. Fabula evaluates patterns by directly traversing the graph through the `DataSource` trait.

**Why.** Datalog provides powerful features (recursive rules, automatic join optimization) but requires a Datalog engine. Fabula's direct traversal is simpler to implement, debug, and reason about. Each stage in a pattern translates to a sequence of `edges_from` and `scan` calls with explicit binding propagation.

**What you lose.** Recursive rules. In Felt, you can define `eventSequence` as a recursive Datalog rule and use it in patterns. Fabula cannot express "transitive closure" or "friend-of-a-friend" patterns. You also lose Datalog's automatic join ordering -- fabula always evaluates clauses left to right within a stage.

**What you gain.** No Datalog dependency. Predictable evaluation order. Easier to debug (you can trace exactly which `edges_from` call returned what). The engine is under 1,000 lines of Rust.

## 5. Zero-dep core

The `fabula` crate has zero external dependencies. Not even `serde`, `log`, or `thiserror`.

**Why.** Fabula is designed to embed into game engines, simulation runtimes, and WASM targets. Every dependency increases compile time, binary size, and the risk of version conflicts with the host application.

**How adapters work.** Graph store integrations live in separate crates that depend on both `fabula` and the store's crate:

- `fabula-memory` -- in-memory graph (no external deps beyond `fabula`)
- `fabula-petgraph` -- petgraph adapter (depends on `petgraph`)
- `fabula-grafeo` -- grafeo adapter (depends on `grafeo`)

Your application depends on `fabula` plus whichever adapter crate matches your store. If none do, you implement `DataSource` yourself. See [Custom Adapter](../guides/custom-adapter.md).

## 6. Scoring as post-processing, not engine modification

Surprise scoring (Shannon surprise, StU property-level scoring) and narrative scoring (thread tracking, tension arcs, pivot detection) are implemented as separate modules and crates that operate on engine output. They do not modify the engine's matching logic.

**Why.** The engine has one job: find matches. Scoring is inherently domain-specific -- what counts as "surprising" or "good narrative quality" varies by application. Keeping scoring separate means the engine stays small and general, while scoring modules can be composed, replaced, or omitted entirely.

**What you lose.** Scoring cannot influence matching (e.g., "only advance patterns with surprise > 2.0"). If you need scoring-gated matching, implement it in your simulation loop by checking scores before feeding edges.

**What you gain.** The engine stays simple and fast. Scoring modules are independently testable. You can use pattern matching without any scoring, or use scoring without the narrative quality function.

## Crate layout

```
fabula/
  crates/
    fabula/              Core library: engine, patterns, intervals, DataSource trait,
                         scoring, composition. Zero external dependencies.

    fabula-memory/       MemGraph: in-memory graph with linear scan.
                         Depends on: fabula

    fabula-petgraph/     PetTemporalGraph: petgraph-backed temporal graph.
                         Depends on: fabula, petgraph

    fabula-grafeo/       GrafeoGraph: grafeo-backed temporal graph.
                         Depends on: fabula, grafeo

    fabula-dsl/          Text DSL parser: patterns, graphs, compose operators, TypeMapper.
                         Depends on: fabula, fabula-memory

    fabula-narratives/   Narrative scoring: threads, tension, pivots, MCTS quality function.
                         Depends on: fabula

    fabula-wasm/         WebAssembly bindings for DSL parsing and evaluation.
                         Depends on: fabula, fabula-dsl, fabula-memory

    fabula-bench/        Benchmark harness (divan) + profiling binary.
                         Depends on: fabula, fabula-memory, fabula-petgraph

    fabula-test-suite/   Golden tests: TestGraph trait + scenarios + macro.
                         Depends on: fabula, fabula-memory, fabula-petgraph,
                                     fabula-grafeo, paste
```

The dependency graph flows strictly downward: `fabula-test-suite` depends on all adapter crates, each adapter crate depends on `fabula`, and `fabula` depends on nothing.

## Module overview for contributors

The core `fabula` crate has seven modules:

- **`interval`** -- `Interval<T>`, `AllenRelation` enum, relation classification, helper methods (`covers`, `intersects`, `before`, `meets`).
- **`datasource`** -- `DataSource` trait (6 methods, 4 associated types), `Edge` struct, `ValueConstraint` enum (7 variants).
- **`pattern`** -- `Pattern`, `Stage`, `Clause`, `Var`, `Target`, `TemporalConstraint`, `Negation`. Data types with `map_types()` for type conversion.
- **`builder`** -- `PatternBuilder`, `StageBuilder`, `NegationBuilder`. Ergonomic API for constructing patterns.
- **`engine/`** -- `SiftEngine<N,L,V,T>`, lifecycle management (register, tick, enable/disable, metrics, plant/payoff), evaluation (batch, incremental, gap analysis), fork support (`Clone`).
- **`compose`** -- Pattern composition operators: `sequence`, `choice`, `repeat` with variable renaming.
- **`scoring/`** -- `SurpriseScorer` (Shannon surprise) and `StuScorer` (StU property-level scoring).

## Research lineage

Fabula is a Rust port and extension of two research systems:

- **Felt** (Kreminski et al., ICIDS 2019) -- 290 lines of JavaScript. A combined story sifter and action-selection framework built on DataScript. Introduced sifting patterns as Datalog queries over a simulation's EAV store, with negation-as-failure via `not-join`.

- **Winnow** (Kreminski et al., AIIDE 2021) -- ~500 lines of JavaScript. A higher-level DSL that compiles to Felt/DataScript queries and adds incremental matching via partial match tracking. Introduced the `tryAdvance` algorithm: check negation first, then try to advance, then keep the original.

Fabula preserves Winnow's incremental matching semantics (the 4-phase algorithm, forking, negation priority) while replacing the DataScript/Datalog substrate with a generic graph trait and Allen interval algebra.

For the full feature-by-feature mapping between Felt, Winnow, and fabula, see [DESIGN.md](https://github.com/your-repo/fabula/blob/main/DESIGN.md) in the repository root.
