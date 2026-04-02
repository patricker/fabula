# Fabula Roadmap

**Status**: Living document
**Date**: 2026-03-31

Research-driven expansion plan for fabula, organized in phases. Each phase
builds on the previous and can be validated independently. Paracausality
integration is deferred to Phase 5; academic contributions to Phase 6.
Performance and benchmarking are early (Phase 2) to establish a measurement
baseline before adding complexity.

---

## Phase 1 — Polish & Parity

Small, high-value improvements that complete the core library.

### 1.1 ~~Strict variable/literal distinction in DSL~~ (DONE — b06551a)

**Problem**: The DSL currently treats bare identifiers in source position
ambiguously. When you write `char.trait = "impulsive"`, the engine checks
whether `"char"` exists in the bindings map. If it was previously bound by
`-> ?char`, it follows the bound node. If not, it silently falls through
to a `scan()` — a completely different query that searches ALL nodes.

This means a typo is never caught:

```
stage e1 {
  e1.actor -> ?character        // binds "character" → alice
  char.trait = "impulsive"      // BUG: "char" not in bindings
                                // silently scans entire graph for trait=impulsive
                                // instead of checking alice's trait
}
```

**Root cause** (engine.rs `find_stage_matches`, line 485):
```rust
if let Some(bound) = existing.get(&first.source.0) {
    // Source IS in bindings → follow bound node (edges_from)
} else {
    // Source NOT in bindings → scan ALL nodes (scan)
}
```

The binding key is the bare string (`"char"`), because `?` is stripped when
`-> ?char` stores the binding. So `char.trait` happens to match — but only
by string coincidence, not by explicit reference.

**Fix**: Make the distinction explicit in the DSL:

| Syntax | Meaning | Engine behavior |
|--------|---------|-----------------|
| `alice.trait = "impulsive"` | Literal node name | `edges_from("alice", "trait", ...)` |
| `?char.trait = "impulsive"` | Bound variable reference | Look up `"char"` in bindings → `edges_from(bound_node, "trait", ...)` |
| `char.trait = "impulsive"` | **Error** (or literal, if no binding exists) | See design choice below |

**Design choice — what does bare `char` mean?**

Option A (strict): Bare identifiers are always literal node names. `?` is
required to reference a bound variable. `char.trait` means "literal node
named char", never "the variable ?char". Typos in variable names become
errors if the compiler validates that `?var` was bound.

Option B (Winnow-compatible): Bare identifiers in source position attempt
binding lookup first, literal fallback second (current behavior). `?char`
is an explicit "must be bound" assertion. This preserves backwards compat
but doesn't fix the silent typo bug.

**Recommendation**: Option A (strict). This is pre-1.0 — correct the
semantics now. The compiler can validate:
- Every `?var` in source position must have been bound by `-> ?var` in an
  earlier clause (same stage or earlier stage)
- Bare identifiers are always literal node names
- Unbound `?var` references produce a compile error with a helpful message

**Changes required**:

1. **Parser** (`fabula-dsl/src/parser.rs`): When parsing a clause source,
   check for `?` prefix. If present, emit `ClauseAst.source_is_var = true`.
   If absent, `source_is_var = false`.

2. **Compiler** (`fabula-dsl/src/compiler.rs`): Track which variables have
   been bound (by `-> ?var` targets in earlier clauses). When compiling a
   clause with `source_is_var = true`, validate the variable was bound.
   When `source_is_var = false`, emit a literal node reference.

3. **Presets** (`presets.ts`): Update all patterns that use `char.trait`
   to `?char.trait`.

4. **Tests**: Update DSL test strings. Add tests for:
   - `?unbound.trait` → compile error
   - `literal_node.trait` → literal node lookup
   - `?bound.trait` → variable follow

5. **Docs**: Update DSL reference page with clear rules.

**Files**: `fabula-dsl/src/{parser,compiler,ast}.rs`, `presets.ts`,
`docs/docs/reference/dsl.md`
**Tests**: New golden tests + updated existing tests
**Effort**: Small-medium (parser change is small; updating all DSL strings
and adding validation is the bulk)

### 1.2 ~~Negated constraint validation~~ (DONE)
The builder API has no `not_edge_constrained()` method, but the DSL parser
accepts `! e.loyalty < 0.5` without error. The negation flag is silently
ignored at compile time — the constraint compiles as positive.

**Fix**: Reject negated constraint clauses at compile time with an error:
`"negated constraints (! source.label < value) are not supported"`.
Alternatively, add `not_edge_constrained()` to the builder if the semantics
are well-defined.

**Files**: `fabula-dsl/src/compiler.rs` (validation), optionally `fabula/src/builder.rs`
**Effort**: Small

### 1.3 ~~Stage anchor / variable name collision~~ (DONE)
If a `-> ?var` binding produces a name identical to the enclosing stage
anchor, the engine silently constrains `?var` to equal the anchor node
(only self-loops match). This is almost never intended.

```
stage char {
  char.actor -> ?char   // BUG: ?char is now constrained to equal node "char"
}
```

**Fix**: Emit a compile warning or error when a binding target name matches
the current stage anchor.

**Files**: `fabula-dsl/src/compiler.rs`
**Effort**: Small

### 1.4 ~~Partial match deduplication~~ (DONE)
First-stage PMs accumulate unboundedly on repeated matching edges (documented
in `edge_cases.rs`). Add fingerprint-based dedup: `(pattern_idx, next_stage,
bindings_hash)`.

**Files**: `fabula/src/engine.rs`
**Tests**: Edge case test asserting bounded PM pool size
**Effort**: Small

### 1.5 ~~Metric temporal constraints (STN-based)~~ (DONE)

Allen relations are qualitative (before, during, overlaps). This adds
**quantitative bounded difference constraints** on interval endpoints,
following the **Simple Temporal Network (STN)** formalism (Dechter, Meiri,
Pearl 1991). Meiri (1996) is the canonical reference for combining
qualitative Allen relations with quantitative metric constraints.

The key insight: each Allen relation decomposes into difference constraints
between the four endpoints `(start(A), end(A), start(B), end(B))`. Metric
constraints add numeric bounds to those differences.

**Standard form** (from STN literature):
```
start(B) - end(A) in [min, max]    // gap between A's end and B's start
end(A) - start(A) in [min, max]    // duration of A
```

**DSL syntax**:
```
temporal e1 before e2 gap 3..10     // gap in [3, 10]
temporal e1 before e2 within 10     // shorthand: gap [0, 10]
temporal e1 before e2 min_gap 3     // shorthand: gap [3, infinity)
```

**Scope**: This extends fabula's existing Allen relation support with
optional metric bounds. NOT a full STN solver (no Floyd-Warshall network
propagation). Just per-constraint bounds checked during pattern evaluation.
A full STN solver would be Phase 5+ (stack integration with Paracausality).

**Goes beyond Felt/Winnow**: Neither has any metric temporal features.
Their temporal reasoning is purely qualitative ordering.

**References**:
- Dechter, Meiri, Pearl (1991). "Temporal Constraint Networks." AI 49(1-3)
- Meiri (1996). "Combining Qualitative and Quantitative Constraints." AI 87(1-2)
- Drakengren & Jonsson (1997). "Eight Maximal Tractable Subclasses of Allen's Algebra with Metric Time." JAIR 7

**Files**: `fabula/src/pattern.rs` (MetricBound type), `fabula/src/engine.rs`
(check bounds during temporal validation), `fabula-dsl/` (parse `gap`/`within`/`min_gap`)
**Tests**: Golden tests for metric constraints, boundary conditions
**Effort**: Medium

### 1.6 ~~Partial match age tracking~~ (DONE)
`created_at: T` field on `PartialMatch`. Set from the initiating edge's
interval start in Phase 2; inherited from parent on fork in Phase 3.
The engine doesn't interpret age — consumers inspect `pm.created_at`
via `partial_matches()` or `active_matches_for()` and apply their own
classification (stale plants, urgent payoffs, etc.).

**Files**: `fabula/src/engine.rs`
**Tests**: Initiation timestamp + inheritance on advance

---

## Phase 2 — Benchmarking & Performance

Establish measurement infrastructure before adding complexity. No published
performance analysis of story sifting exists — fabula can be first.

Reordered based on deep planning review: stats first (to explain benchmarks),
then profile (to find real hotspots), fix known issues, then benchmark the
fixed code. WASM benchmarks and label indexing deferred — premature without data.

### 2.1 ~~Engine stats counters~~ (DONE)
Live operation counters incremented during matching. O(1) to read — no
iteration over PMs. These explain *why* a benchmark is slow, not just
that it is.

```rust
pub struct EngineStats {
    pub total_on_edge_added: u64,
    pub total_fingerprints: u64,
    pub total_negation_checks: u64,
    pub peak_active_pms: usize,
}
```

`engine.stats()` returns reference. `engine.reset_stats()` zeroes counters.

**Files**: `fabula/src/engine.rs`
**Effort**: Small

### 2.2 ~~Profiling & Benchmark harness~~ (DONE — merged 2.2 + 2.4)

Merged profiling investigation with permanent benchmark harness into a
single `fabula-bench` crate. Pluggable data source via `--adapter` CLI arg
(petgraph, memgraph; Paracausality later via feature flag).

**Crate structure**:
- `src/workload.rs` — `WorkloadConfig`, pattern generators (5 categories × 6),
  graph builder, `IsolatedWorkload` (for divan), `GmWorkload` (for profiling)
- `src/bin/profile.rs` — 200-tick GM workload, per-tick CSV, optional dhat
- `benches/engine.rs` — divan parameterized benchmarks (cold + warm start)

**Profiling findings** (200-tick GM workload, 30 patterns, ~3K edges):

| Metric | petgraph | memgraph |
|--------|----------|----------|
| Total time | 494ms | 724ms |
| Avg per `on_edge_added` | 164 us | 240 us |
| Peak active PMs | 350 | 350 |
| Total fingerprints | 656K | 656K |

**Key finding**: Per-tick cost grows linearly with active PM count.
Tick 1 = 80us, tick 200 = 2.7ms. The `HashSet<String>` fingerprint
rebuild is the dominant cost — 656K string allocations across 3K edges.

**Benchmark results** (divan, cold-start):

| Benchmark | petgraph | memgraph |
|-----------|----------|----------|
| 10 edges/tick (cold) | 841 us | 1.5 ms |
| 10 edges/tick (warm, 20 ticks PM accumulation) | ~3 ms | 36 ms |
| Negation 0% → 100% | 679 → 817 us | 1.1 → 1.8 ms |
| Pattern count 1 → 100 | 109 → 832 us | 108 us → 1.1 ms |
| Batch 1K edges | 414 ms | 2.7 s |

**Confirmed hotspots** (for 2.3):
1. PM fingerprint `HashSet<String>` rebuild — O(active_pms) string allocs per call
2. Binding HashMap clones in Phase 3 happen BEFORE dedup check
3. Batch mode cross-product is O(E²) — impractical at scale

**Files**: `crates/fabula-bench/`

### 2.3 ~~Fingerprint optimization~~ (DONE)
Replaced String-based `pm_fingerprint` with hash-based dedup:
- `Hash` bound added to `DataSource::V` and `DataSource::T`
- `BoundValue`, `MemValue`, `PetValue`, `GrafeoValue` all implement `Hash`
- `compute_fingerprint()` uses order-independent XOR of per-entry hashes
- `fingerprint: u64` stored on `PartialMatch`, computed once at creation
- `seen` set is `HashSet<u64>` — zero-allocation rebuild from stored hashes

**Before/after** (warm petgraph, 10 edges/tick, 20-tick PM accumulation):

| Metric | Before (String) | After (u64) | Speedup |
|--------|-----------------|-------------|---------|
| warm petgraph/10 | 21.8 ms | **3.1 ms** | **7.0x** |
| warm memgraph/10 | 35.3 ms | 16.1 ms | 2.2x |
| Profile total (200 ticks) | 494 ms | **85 ms** | **5.8x** |
| Avg per on_edge_added | 164 us | **28 us** | **5.9x** |

Petgraph now well within 16ms frame budget at GM scale.

**Files**: `fabula/src/{engine,datasource}.rs`, all three adapter crates

### 2.4 Label indexing optimization (conditional)
Only if 2.3 doesn't close the gap. Benchmarks show MemGraph is 1.5-2x
slower than petgraph due to O(n) scan. For incremental mode the gap is
modest; for batch mode MemGraph is unusable at 1K+ edges. Label indexing
would help batch mode but fingerprint optimization (2.3) is the bigger win
for incremental.

**Files**: `fabula-memory/src/lib.rs`
**Gated on**: 2.3 results
**Effort**: Small-medium

### Deferred from Phase 2

**WASM benchmark page**: Browser benchmarks are noisy (JIT, GC, V8 tiers)
and measure serde JSON overhead, not engine performance. Move to Phase 4+
when there's a pattern library to demo. Use `performance.now()` inside
WASM to exclude serialization when eventually built.

---

## Phase 3 — Pattern Composition

The research insight: "The framework is data." Narrative structure (Propp,
MICE, Save-the-Cat) should be expressible as composable pattern data, not
code. This phase builds the algebra that enables it.

### 3.1 ~~Pattern composition operators~~ (DONE)
Three operators that produce regular `Pattern` structs — the engine handles
them without modification (except group-based mutual exclusion for choice):

```rust
let arc = compose::sequence("arc", &setup, &payoff, &["char"]);
let crises = compose::choice("crisis", &[&war, &famine, &plague], true);
let escalation = compose::repeat("three_strikes", &offense, 3, &["offender"]);
```

- `rename_vars()` core utility handles all variable locations
- `group: Option<String>` on Pattern for exclusive choice
- Engine kills sibling PMs on group completion (~15 lines)
- 7 golden tests × 3 adapters = 21 tests

**Files**: `fabula/src/compose.rs`, `pattern.rs`, `engine.rs`

### 3.2 ~~DSL compose syntax~~ (DONE)
DSL syntax for composing named patterns:

```
pattern setup { stage e1 { e1.eventType = "promise"  e1.actor -> ?char } }
pattern payoff { stage e2 { e2.eventType = "fulfill"  e2.actor -> ?char } }

compose promise_kept = setup >> payoff sharing(char)
compose crisis = war | famine | plague
compose three_strikes = offense * 3 sharing(offender)
```

- `>>` sequence, `|` exclusive choice, `* N` repeat
- `sharing(var, ...)` declares cross-pattern variable bindings
- No forward references (define before compose)
- Compose chains work: `compose ab = a >> b` then `compose abc = ab >> c`
- `Document` now uses ordered `Vec<DocumentItem>` for name resolution

**Deferred to future iterations**:
- Nested expressions: `(A >> B) | C` — use named intermediates instead
- Non-exclusive choice syntax — register patterns separately
- Implicit sharing by name — use explicit `sharing(...)` clause
- `private` pattern modifier — all patterns appear in output

**Files**: `fabula-dsl/src/{ast,lexer,parser,compiler,lib}.rs`
**Tests**: 7 new DSL tests (parse, compile, roundtrip, chaining, error)

### 3.3a ~~Pattern-level surprise scoring~~ (DONE)
Standalone `SurpriseScorer` in `scoring.rs` — operates as post-processing,
no engine modification. Shannon surprise: `-log₂(observed / baseline)` with
Laplace smoothing for zero-observation cases.

```rust
let mut scorer = SurpriseScorer::new();
scorer.set_baseline(0, 0.1); // expect 10% match rate
// After evaluation:
let matches = engine.evaluate(&graph);
scorer.observe(&matches, engine.patterns());
let scored = scorer.score(&matches, engine.patterns());
// scored[i].surprise — higher = more unexpected
```

- `ScoredMatch` type (no changes to `Match` or `SiftEvent`)
- `observe()` for batch, `observe_events()` + `tick()` for incremental
- Laplace smoothing handles never-matched patterns gracefully
- 9 unit tests covering rare/common/never-matched/negative surprise

**Files**: `fabula/src/scoring.rs`, `lib.rs` prelude

### 3.3b Property-level surprise scoring (StU) — planned
Kreminski's "Select the Unexpected" (ICIDS 2022) scores individual matches
by the mean empirical frequency of their *properties* — character traits,
event types, relationships present in the bindings. Two matches of the same
pattern score differently if one involves rare entities.

Requires:
- Property extractor: `fn(match, graph) -> Vec<String>` that inspects
  bindings and queries the DataSource for entity attributes
- Per-property frequency table across all matches of a pattern
- Score = mean of per-property frequencies (lower = more surprising)
- DataSource access (the scorer needs to query the graph)

This is a significant extension that depends on a defined "property
vocabulary" for the domain. Deferred until the GM integration (Phase 5)
defines what properties matter.

**Reference**: Kreminski, Dickinson, Wardrip-Fruin, Mateas. "Select the
Unexpected: A Statistical Heuristic for Story Sifting." ICIDS 2022.
**Effort**: Medium-high

---

## Phase 4 — Narrative Pattern Library

Pre-built patterns for established narrative frameworks. Depends on Phase 3
composition. Ships as a separate `fabula-narratives` crate so the core stays
framework-agnostic.

### 4.1 Proppian function library
31 narrative function patterns (villainy, mediation, trickery, departure,
interdiction, violation, etc.). Each is a standalone pattern; composition
operators combine them into story morphologies.

**Files**: New `crates/fabula-narratives/src/propp.rs`
**Effort**: Medium

### 4.2 MICE thread patterns
Milieu/Idea/Character/Event thread detection. Opening and closing patterns
for each type. FILO order validation (threads must close in reverse order
of opening).

**Files**: `fabula-narratives/src/mice.rs`
**Effort**: Small-medium

### 4.3 Emotional arc shapes (Reagan 2016)
Six canonical arc shapes: Rags to Riches, Tragedy, Man in a Hole, Icarus,
Cinderella, Oedipus. Detect character emotional trajectories matching these
shapes by sampling valence over time windows.

**Files**: `fabula-narratives/src/arcs.rs`
**Depends on**: Emotional valence queryable as numeric edge values
**Effort**: Medium

### 4.4 Kernel/satellite classification
Patterns flagged as kernel-triggering (changes story direction, cannot be
removed) vs satellite-generating (provides texture, removable). Enables
narrative completeness checking.

**Files**: Pattern metadata in `fabula/src/pattern.rs`, classification logic
**Effort**: Small

---

### 5.0 ~~Paracausality core prerequisites~~ (DONE — in Paracausality repo)
Pre-launch core changes to enable the fabula adapter:
- `T`: added `Hash`, `Sub<Output=T>`, `Add<Output=T>`
- `Value`: added `Hash` (f64 via `to_bits()`) and `PartialOrd`
- Sync `Store` trait: 3 new methods (`active_objective_for_entity_pred`,
  `all_objective_for_entity`, `all_objective_for_predicate`)
- `InMemoryStore`: proper implementations (not just time-filter workarounds)
- `StoreEvent::InsertAssertion`: added `subject: EntityId` + `object: Value`

## Phase 5 — Stack Integration

Connect fabula to the worldbuilding stack. These items require Paracausality
and WorldKernel.

### 5.1 ~~Paracausality adapter~~ (DONE — in Paracausality repo)
`paracausality-fabula-adapter` crate in Paracausality workspace.
`ParaDataSource<S: Store>` implements `DataSource` with:
- `N=EntityId`, `L=u32`, `V=Value`, `T=T` — no wrapper types
- Feature-gated `NumericTime` impl via `paracausality-core/fabula`
- 10 unit tests + SiftEngine integration test
- `From<T> for f64` on Paracausality's T for general numeric conversion

**Files**: `Paracausality/crates/fabula-adapter/`

### 5.2 Pattern registration engine
Named pattern groups with lifecycle management:
- Register/deregister patterns at runtime
- Per-pattern metrics: last advancement tick, clause status, staleness
- Enable/disable pattern groups
- Stale-plant alerts (from 1.4)

**Files**: New module or crate `fabula-registry`
**Effort**: Medium

### 5.3 Delta reporting (match state diffs)
Engine returns "what changed since last tick" for quality scoring:
- Patterns advanced (which, by how many stages)
- Patterns completed
- Patterns negated (killed)
- Patterns stalled (no advancement for N ticks)

**Files**: `fabula/src/engine.rs` (snapshot/diff)
**Effort**: Medium

### 5.4 Fork-aware evaluation
Document and example the pattern for MCTS timeline forking:
- Fork DataSource → evaluate patterns on fork → score → commit best
- Each fork is a separate DataSource + SiftEngine instance
- Engine state cloning for speculative evaluation

**Files**: Example in docs + `SiftEngine::clone_state()` method
**Effort**: Small

### 5.5 Plant/payoff tracking
Application-level plant/payoff classification built on partial match age
(from 1.6) and pattern registration (from 5.2). The GM classifies active
PMs as "plants" (setup waiting for payoff) and completed PMs as "payoffs"
(setup resolved). Cross-pattern plant/payoff pairs (Pattern A plants,
Pattern B pays off) require pattern composition (Phase 3).

- Explicit plant/payoff pair registration via pattern metadata
- Stale-plant alerts ("this Chekhov's gun has waited 50 ticks")
- Cross-pattern resolution tracking (requires shared variable bindings)

**Depends on**: 1.6 (age tracking), 5.2 (pattern registration), 3.1 (composition for cross-pattern)
**Effort**: Medium

### 5.6 Character appraisal patterns
Sifting-based emotional appraisal from GM architecture:
- Event → relevance check (does event affect character's goals?)
- Misbelief arc monitoring (has contradicting evidence appeared?)
- Emotional trajectory patterns (mood declining over time window)

**Depends on**: 5.1 (Paracausality adapter)
**Effort**: Medium

### 5.7 Knowledge propagation patterns
Track what characters know and when they learn it:
- "A witnessed X, B doesn't know yet"
- "C heard about X through D (gossip chain)"
- Information distortion modeling

**Relates to**: Kreminski 2023 gossip paper
**Depends on**: 5.1 (Paracausality adapter)
**Effort**: Medium-high

### 5.8 Event causality tracing
Path-finding over temporal graph to trace causal chains:
- "This betrayal traces back to THAT institutional failure 20 ticks ago"
- Causal chain cleanliness scoring for quality function

**Depends on**: Causal links stored as edges in the graph
**Effort**: Medium-high

---

## Phase 6 — Research & Publication

Academic contributions that fill gaps in the literature.

### 6.1 Formal semantics paper
Formal specification of incremental temporal graph pattern matching:
- Partial match state machine (Active → Complete | Dead)
- Update semantics for `on_edge_added`
- Negation window semantics (exclusive boundaries, multi-clause bodies)
- Correctness proof: batch ≡ incremental for well-ordered streams

### 6.2 Scalability analysis
First published performance analysis of story sifting at scale.
Uses benchmarks from Phase 2.

### 6.3 Expressiveness hierarchy
Classification of pattern languages by what narrative phenomena they
can express. Single-stage < multi-stage < negation < temporal < compositional.
Maps to Chomsky-like hierarchy for narrative pattern grammars.

---

## Summary

| Phase | Theme | Key Deliverables |
|-------|-------|------------------|
| **1** | Polish & Parity | Variable distinction, negation validation, dedup, metric temporal, age tracking |
| **2** | Benchmarking | Stats counters, profiling + divan harness, fingerprint optimization, conditional label indexing |
| **3** | Composition | Pattern algebra, DSL compose syntax, surprise scoring |
| **4** | Pattern Library | Propp functions, MICE threads, emotional arcs, kernel/satellite |
| **5** | Stack Integration | Paracausality adapter, registry, deltas, MCTS, plant/payoff, appraisal, gossip |
| **6** | Research | Formal semantics, scalability paper, expressiveness hierarchy |

---

## References

- Kreminski et al. (2019). **Felt: A Simple Story Sifter.** ICIDS 2019.
- Kreminski et al. (2021). **Winnow: A Domain-Specific Language for Incremental Story Sifting.** AIIDE 2021.
- Kreminski et al. (2022). **Select the Unexpected: A Statistical Heuristic for Story Sifting.** ICIDS 2022.
- Kreminski et al. (2025). **Stories from the Bottom Up: Composable Story Sifting Patterns.** FDG 2025.
- Reagan et al. (2016). **The Emotional Arcs of Stories.** EPJ Data Science.
- Chatman, S. (1978). **Story and Discourse.** Cornell University Press.
- Propp, V. (1928/1968). **Morphology of the Folktale.**
- Nelson & Mateas (2005). **Search-Based Drama Management.** AIIDE 2005.
- Allen, J.F. (1983). **Maintaining Knowledge about Temporal Intervals.** CACM 26(11).
- Short, E. (2016). **Quality-Based Narrative.**
- Kreminski et al. (2023). **Knowledge Propagation in Interactive Narrative.**
