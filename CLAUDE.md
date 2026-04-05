# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is Fabula

Incremental pattern matching over temporal graphs. A Rust port/extension of Felt (Kreminski et al., ICIDS 2019) and Winnow (AIIDE 2021) for story sifting — finding narratively compelling event sequences in simulation output. Fabula is a **sifting library only** — Felt's action/effect system (registerAction, possibleActions, realizeEvent, addEvent) is explicitly out of scope. Actions and effects belong to the simulation layer that feeds edges into fabula.

## Build & Test Commands

```bash
cargo build --workspace                    # build everything
cargo test --workspace                     # all tests (~650+)
cargo clippy --workspace -- -D warnings    # lint (CI enforces -D warnings)
cargo test -p fabula-test-suite            # golden tests only (81 scenarios x 3 adapters)
cargo test -p fabula -- test_name          # single test in a crate
cargo test -p fabula-test-suite mem__batch_hospitality_matches  # single golden test
cargo bench -p fabula-bench                # divan benchmarks
wasm-pack build --target web crates/fabula-wasm  # WASM build (needs wasm32-unknown-unknown target)
```

**CI enforces all four gates**: clippy with `-D warnings`, full test suite, wasm-pack build, on every push to main and all PRs. No warnings can be merged.

## Workspace Layout (11 crates)

| Crate | Purpose |
|---|---|
| `fabula` | Core library (**zero dependencies**): patterns, `DataSource` trait, `SiftEngine`, Allen algebra, scoring |
| `fabula-memory` | `MemGraph` — Vec-backed in-memory `DataSource` adapter |
| `fabula-petgraph` | `DataSource` adapter wrapping `petgraph::StableGraph` |
| `fabula-grafeo` | `DataSource` adapter for Grafeo graph database |
| `fabula-dsl` | Text DSL: lexer → parser → compiler, with `TypeMapper` for custom type systems |
| `fabula-narratives` | Narrative scoring: thread tracking (MICE), tension arcs, pivot detection, MCTS quality |
| `fabula-wasm` | WebAssembly bindings via wasm-bindgen |
| `fabula-test-suite` | Golden tests: scenarios generic over `TestGraph`, run against all 3 adapters |
| `fabula-discovery` | Pattern discovery: generate-evaluate framework, MINERful generator, DSL emission |
| `fabula-examples` | Doc code examples: 20 test files, 48 DSL files, glob-based validation |
| `fabula-bench` | Benchmarks (divan) and heap profiling (dhat) |

## Architecture

### Core abstractions

**`DataSource` trait** (`fabula/src/datasource.rs`): The integration point for any graph backend. Has 4 associated types (`N`ode, `L`abel, `V`alue, `T`ime) and 6 methods (`edges_from`, `scan`, `edges_from_any_time`, `scan_any_time`, `now`, `value_as_node`). All adapters (memory, petgraph, grafeo) implement this trait.

**`Pattern<L, V>`** (`fabula/src/pattern.rs`): Ordered sequence of `Stage`s with temporal constraints and negation windows. Each stage has `Clause`s that match edges. Variables appearing in multiple stages create joins. Patterns may be marked `private: bool` — private patterns participate in matching (including exclusive choice groups) but their events are filtered from all engine output (`on_edge_added`, `evaluate`, `drain_completed`, `end_tick`, `tick_delta`).

**`SiftEngine<N, L, V, T>`** (`fabula/src/engine/`): The matching engine, parameterized independently from `DataSource`. Key design: engine can outlive any particular data source, enabling MCTS forking (clone engine + fork DataSource, speculate, discard).

### Two evaluation modes

**Batch** — `engine.evaluate(&graph)`: Scans the full graph for all matches in one pass. Used for one-shot queries.

**Incremental** — `engine.on_edge_added(&graph, ...)` + `engine.end_tick()`: Feed edges one at a time. The engine tracks partial matches and emits `SiftEvent`s (Advanced, Completed, Negated). Call `end_tick()` after each batch of edges to finalize the tick — this clears tick accumulators (`tick_advanced`, `tick_completed`, `tick_negated`), increments `tick_counter`, and produces `TickDelta` for the narratives crate.

### Engine evaluation (4-phase algorithm, in `engine/eval.rs`)

Each `on_edge_added()` call runs these phases in order:

1. **Negation check** — kill active partial matches whose negation window is violated by the new edge
2. **Initiation** — try to match the new edge against stage 0 of all registered patterns
3. **Advancement** — try to advance existing partial matches to their next stage
4. **Cleanup** — fingerprint-based dedup (XOR hash, covers Active+Complete+Dead PMs to prevent re-creating a just-negated PM) + filter dead matches + exclusive choice group handling

### Key implementation details

- **First clause is the trigger**: In `try_match_stage()`, the first clause must match the incoming edge. Remaining clauses are verified by querying the data source.
- **Temporal consistency**: Secondary clauses use `event_time` from the first clause's interval, not `ds.now()`. This ensures all clauses in a stage are evaluated at the same temporal point.
- **Negation window boundary**: Exclusive on start (`strict >`, not `>=`) to match Winnow semantics.
- **MCTS forking via Clone**: `engine.clone()` copies patterns, partial matches, enabled flags, and stats, but resets tick accumulators to empty. The forked engine starts with a clean tick.

### Negation forms

- **`unless_between("e1", "e3", ...)`** — no matching event between two stages
- **`unless_after("e1", ...)`** — no matching event after a stage (open-ended)
- **`unless_global(...)`** — no matching event anywhere in the pattern's span

### Key patterns

- **`PatternBuilder`** (`builder.rs`): Fluent API for constructing patterns
- **Composition operators** (`compose.rs`): `sequence`, `choice`, `repeat` with shared variables. `choice` accepts an `exclusive: bool` parameter — exclusive (default) creates a choice group where only one branch can match; non-exclusive allows all branches to match independently. These work by renaming all variables in sub-patterns (except shared ones) with prefixes (`a_`, `b_`, `rep0_`), then merging into a single pattern.
- **`SiftEngineFor<DS>`**: Type alias that extracts `N,L,V,T` from a `DataSource` impl — use this when you have a concrete data source type
- **Allen interval algebra** (`interval.rs`): 13 temporal relations + metric gap constraints (STN-style bounded distances). Returns `Option<AllenRelation>` — `None` for open-ended intervals.
- **`why_not` gap analysis**: Clause-by-clause breakdown of why a pattern hasn't matched
- **Plant/payoff tracking**: Chekhov's gun monitoring via `plant_payoff_pairs` on the engine

### DSL pipeline (`fabula-dsl`)

Lexer → Parser → Compiler. The lexer/parser are type-agnostic; the compiler uses a `TypeMapper` trait to convert DSL literals to target types. Default `MemMapper` produces `Pattern<String, MemValue>`. Custom `TypeMapper` implementations support arbitrary label/value type systems (e.g., `u32` predicates) by implementing `label()`, `string_value()`, `num_value()`, `bool_value()`, `node_ref()` — each returns `Result<T, String>` for fallible mappings. DSL keywords: `private pattern name { }` marks a pattern as private; `compose x = a | b nonexclusive` produces a non-exclusive choice.

### Narrative scoring (`fabula-narratives`)

DataSource-agnostic — works on engine output (`TickDelta`, `SiftEvent`), not graph queries. The scorer is a pure function: `(signals, weights) → NarrativeScore`. Tracking components (`ThreadTracker`, `TensionTracker`, `PivotDetector`) observe engine state independently; the scorer combines them into a composite score with explainable breakdown.

## Golden Test Pattern

Tests in `fabula-test-suite` are generic over the `TestGraph` trait and auto-expanded by the `golden_tests!` macro (using `paste::paste!`) to run against MemGraph, PetGraph, and GrafeoGraph.

**Adding a golden test:**
1. Write `pub fn my_scenario<G: TestGraph>()` in `crates/fabula-test-suite/src/scenarios/`
2. Re-export from `scenarios/mod.rs`
3. Add `my_scenario,` to `golden_tests!` in `crates/fabula-test-suite/tests/golden.rs`

Generated test names: `mem__my_scenario`, `pet__my_scenario`, `grafeo__my_scenario`

## Design Constraints

- **`fabula` core must remain zero-dependency.** New features that need external crates go in adapter or extension crates.
- **Engine is decoupled from DataSource.** `SiftEngine<N,L,V,T>` takes `&impl DataSource` in method calls, not as a field. This is intentional for MCTS forking.
- **`fabula-narratives` is DataSource-agnostic.** It works on engine output (`TickDelta`, `SiftEvent`), not graph queries.
- **Variable scoping is strict.** All `?var` references in DSL / `edge_bind` calls must be bound by an earlier clause. The DSL compiler validates this at compile time.
- **Sifting only, no action system.** Fabula deliberately excludes Felt's action/effect machinery. The simulation layer is upstream.
