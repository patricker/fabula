# Fabula Roadmap

**Status**: Living document
**Date**: 2026-04-03 (updated)

---

## Completed Work

Phases 1–4 and most of Phase 5 are complete. See git history for
implementation details.

### Phase 1 — Polish & Parity (DONE)

| Item | Summary |
|------|---------|
| 1.1 | Strict variable/literal distinction in DSL — `?var` required for bound references, bare identifiers are literals, compile-time validation |
| 1.2 | Negated constraint validation — reject `! source.label < value` at compile time |
| 1.3 | Stage anchor / variable name collision — compile error when binding target matches stage anchor |
| 1.4 | Partial match deduplication — u64 XOR fingerprint, `HashSet<u64>` dedup set |
| 1.5 | Metric temporal constraints (STN-based) — `gap min..max` on Allen relations, per-constraint bounds |
| 1.6 | Partial match age tracking — `created_at: T` on `PartialMatch` |

### Phase 2 — Benchmarking & Performance (DONE)

| Item | Summary |
|------|---------|
| 2.1 | Engine stats counters — `EngineStats` with O(1) live operation counters |
| 2.2 | Profiling & benchmark harness — `fabula-bench` crate: divan benchmarks + dhat profiling |
| 2.3 | Fingerprint optimization — String→u64 XOR hashing, 5.8x speedup (164us→28us per edge) |
| 2.4 | Label indexing — DEFERRED (Phase 2.3 closed the gap; MemGraph is testing-only) |

### Phase 3 — Pattern Composition (DONE)

| Item | Summary |
|------|---------|
| 3.1 | Composition operators — `sequence`, `choice` (exclusive groups), `repeat` with `rename_vars` |
| 3.2 | DSL compose syntax — `>>`, `\|`, `* N`, `sharing(var)` |
| 3.3a | Pattern-level surprise scoring — Shannon surprise with Laplace smoothing |
| 3.3b | Property-level surprise scoring (StU) — per-property frequency with Laplace smoothing |

### Phase 4 — Narrative Scoring (DONE)

`fabula-narratives` crate with four modules: `ThreadTracker` (MICE-style
lifecycle), `TensionTracker` (sliding window trajectory), `PivotDetector`
(JSD shift detection), `NarrativeScorer` (composite MCTS quality function).

### Phase 5 (Completed Items) — Stack Integration

| Item | Summary |
|------|---------|
| 5.0 | Paracausality core prerequisites — `Hash`, `Sub`, `Add` on `T`; `Hash`, `PartialOrd` on `Value`; 3 new Store methods |
| 5.1 | Paracausality adapter — `ParaDataSource<S: Store>` in Paracausality repo |
| 5.2 | Pattern lifecycle — `set_pattern_enabled()`, `deregister()`, `PatternMetrics`, `stale_patterns()` |
| 5.3 | Delta reporting — `TickDelta` struct with advanced/completed/negated/stalled/active_pm_count |
| 5.4 | Fork-aware evaluation — `Clone` impl for MCTS speculative evaluation |
| 5.5 | Plant/payoff tracking — `PlantPayoffPair`, `PlantStatus`, staleness monitoring |

**Performance baseline** (post-Phase 2.3): ~28us per `on_edge_added` call
(petgraph, GM-scale workload). Petgraph well within 16ms frame budget.
~420+ tests across all crates, 61 golden scenarios x 3 adapters.

### Salience Integration (DONE — 2026-04-03)

API changes requested by the Salience storylet selection system. Three
rounds of work to unblock the downstream consumer.

| Item | Summary |
|------|---------|
| Free `evaluate_pattern(ds, pattern)` | Standalone batch evaluation without owning a SiftEngine. All stateless helpers extracted to `engine/free.rs`; engine methods delegate. |
| Free `gap_analysis(ds, pattern)` | Standalone gap analysis without registering the pattern. Returns `GapAnalysis` directly. |
| `evaluate_pattern_at` / `gap_analysis_at` | Public with explicit `at: &T` time parameter for speculative evaluation at arbitrary timestamps. |
| `evaluate_pattern_first` / `evaluate_pattern_limit` | Early-termination variants: `_first` returns `Option<Match>` (O(1) gating), `_limit` caps result set. |
| `Match<N,V>` → `Match<N,V,T>` | Added `intervals` (stage anchor → time interval) and `pattern_idx` (Some from engine, None from free functions). Breaking change. |
| `PartialEq + Eq + Hash` on Match | Manual `Hash` via order-independent XOR. Enables `HashSet<Match>` for dedup. |
| `GapAnalysis::closeness()` | Fraction of matched clauses (0.0–1.0) for near-miss ranking. |
| `Pattern::condition_count()` | Total clauses across all stages. |
| `serde` feature flag | Optional `Serialize`/`Deserialize` on all public types via `cfg_attr`. |
| Trait bound aliases | `NodeId`, `Label`, `Val` with blanket impls — `L: Label` instead of `L: Eq + Hash + Clone + Debug`. |
| `MemGraph`: `Clone` + `end_edge` + `upsert_edge` | `Clone` for Monte Carlo simulation. `end_edge` closes open intervals. `upsert_edge` = end + insert. |
| Composable DSL parser (Option B) | `Parser::parse_pattern_body()` for downstream DSLs. `pos()`, `from_tokens_at()`, `into_inner()` for resumable parsing. 13 methods made public. `PatternBody` AST type. `compile_pattern_body[_with]()`. |

### Pattern Metadata / Tags (DONE — 2026-04-03)

Arbitrary key-value metadata on patterns, propagated through the full
pipeline: `Pattern` → `Match` → `SiftEvent` → scoring types.

| Item | Summary |
|------|---------|
| `Pattern.metadata: HashMap<String, String>` | Metadata field on `Pattern<L, V>`, preserved through `map_types()`. Default empty. |
| `PatternBuilder::metadata(key, value)` | Fluent builder API, consumed-self style. |
| `Match.metadata` | Propagated from pattern at all construction sites (engine batch, incremental, free functions, `drain_completed`). Included in `Hash` impl. |
| `SiftEvent` metadata | All 3 variants (`Advanced`, `Completed`, `Negated`) carry `metadata: HashMap<String, String>` from the matched pattern. |
| `ScoredMatch` / `StuScoredMatch` metadata | Scoring types propagate metadata from `Match`. |
| DSL `meta("key", "value")` syntax | Parsed as identifier (no new keyword). Allowed before/after/between stages. AST preserves order as `Vec<(String, String)>`, compiler converts to `HashMap` (last-write-wins). |
| Composition merge semantics | `rename_vars` preserves, `sequence`/`repeat` merge (union, last-writer-wins), `choice` preserves per-alternative via Clone. |
| 10 new tests | Builder API, DSL parse/compile, duplicate-key semantics, `compile_pattern_body`, composition merge (sequence, choice, repeat). |

**Follow-up** (not yet done): `fabula-narratives` auto-configuration from
metadata keys (`thread_type`, `priority`, `narrative_role`).

### Timeout-Based Absence Detection (DONE — 2026-04-03)

Event-driven deadline expiry: when a partial match exceeds a configurable
deadline (in engine ticks) without completing, `end_tick()` emits
`SiftEvent::Expired` and kills the PM.

| Item | Summary |
|------|---------|
| `Pattern.deadline_ticks: Option<u64>` | Optional deadline field. `None` = no deadline (default). Preserved in `map_types()`. |
| `PatternBuilder::deadline(ticks)` | Fluent builder API. |
| `PartialMatch.created_at_tick: u64` | Engine tick at PM creation. Inherited (not reset) on advancement — deadline measures total lifecycle. |
| `SiftEvent::Expired` | New variant with `pattern`, `match_id`, `bindings`, `stage_reached`, `ticks_elapsed`, `metadata`. |
| `end_tick()` → `(TickDelta, Vec<SiftEvent>)` | Returns both the delta summary and full `Expired` event objects. Expiry scan runs after tick increment, before stale check. Strict `>` check (`elapsed > deadline`). |
| `TickDelta.expired: Vec<String>` | Pattern names of expired PMs this tick. |
| DSL `deadline N` syntax | Parsed as Ident (no new keyword). Compiler validates `>= 1` (rejects zero/negative). |
| Composition | `rename_vars` preserves, `sequence`/`repeat` set `None` (composed patterns set their own), `choice` preserves per-alternative via Clone. |
| 11 new tests | Expiry timing, no-expiry without deadline, completion preempts expiry, negation preempts expiry, `created_at_tick` inheritance, `SiftEvent::Expired` field validation, DSL parse/compile, zero-deadline rejection. |

### Documentation Update (DONE — 2026-04-03)

Two-pass audit and update of the Docusaurus documentation site to reflect
all changes from Phases 1–5 and Salience integration.

| Item | Summary |
|------|---------|
| reference/patterns.md | Added `group`, `metadata`, `deadline_ticks` fields to `Pattern`. Added `gap` field to `TemporalConstraint`. Added `.metadata()` and `.deadline()` builder methods. |
| reference/engine.md | `Match<N,V>` → `Match<N,V,T>` with `pattern_idx`, `intervals`, `metadata`. `SiftEvent::Expired` variant. All variants carry `metadata`. `end_tick()` → `(TickDelta, Vec<SiftEvent>)`. `TickDelta.expired`. `PartialMatch.created_at_tick`. Updated `drain_completed`/`evaluate` return types. |
| reference/scoring.md | `ScoredMatch`/`StuScoredMatch` → 3 type params, added `pattern_idx`, `intervals`, `metadata`. |
| reference/dsl.md | `meta("key", "value")` syntax, `deadline N` syntax. Full Composable Parser API section: `PatternBody`, `parse_pattern_body`, `from_tokens_at`, `pos`, `into_inner`, `compile_pattern_body[_with]`. |
| reference/narratives.md | Clarified `observe_delta` ignores `expired`/`negated`/`stalled` delta fields. |
| guides/incremental-integration.md | `end_tick()` tuple destructuring. `SiftEvent::Expired` in all match arms. Deadline mention in memory management. |
| guides/debugging-patterns.md | 3 new failure mode rows: immediate expiry, unexpected expiry, empty expired events. |
| concepts/how-the-engine-works.md | New "Deadline expiry" subsection explaining `end_tick()` scan, `created_at_tick` inheritance, tuple return. |
| concepts/overview.md | Mentioned metadata and deadline features in "Beyond matching" section. |
| playground/step-through.mdx | Added "expired" to event type lists. |

---

## Active Roadmap

### Phase 5 — Platform Generalization

Core engine enhancements that broaden fabula from a narrative-only tool to
a general-purpose temporal graph pattern matching platform. These items
emerged from cross-domain research (see FUTURE_PATHS.md) identifying gaps
that block adoption in security, clinical, observability, compliance,
IoT, and gaming domains.

No item in this phase requires external dependencies. All changes are in
the zero-dependency `fabula` core crate (plus DSL support in `fabula-dsl`).

### Cross-stage Value Comparison (DONE — 2026-04-03)

Compare edge targets against previously-bound variables instead of
literals. Pre-resolution approach: engine converts `GtVar("x")` →
`Gt(resolved_value)` before matching, keeping DataSource unchanged.

| Item | Summary |
|------|---------|
| `ValueConstraint::*Var` variants | `EqVar`, `LtVar`, `GtVar`, `LteVar`, `GteVar` — 5 new variants referencing bound variable names. `matches()` returns false for unresolved (fail-closed). `map()` passes through String names. `is_var()` predicate. |
| Pre-resolution in engine | `resolve_constraint()`, `resolve_target()` helpers in `engine/free.rs`. Updated 6 eval sites: `find_stage_matches`, `try_match_stage`, `target_matches_ds`, `check_negations_batch`, `check_negation_kill`. `BoundValue::Node` → resolution fails (type mismatch). |
| `StageBuilder` methods | `edge_eq_var`, `edge_lt_var`, `edge_gt_var`, `edge_lte_var`, `edge_gte_var`. |
| DSL `> ?var` syntax | All 5 comparison operators (`=`, `<`, `>`, `<=`, `>=`) check for `?` — parse as `ConstraintVar(Op, var_name)`. Compiler validates variable scope. Negated constraint-vars rejected. |
| `compose::rename_vars` | Correctly renames `*Var` variable references during pattern composition (sequence/repeat). |
| 22 new tests | DSL parse/compile (4), lifecycle batch/incremental (10), DSL roundtrip eval (3), golden scenarios (5). Boundary values, range check, negation body, type mismatch. |

---

### Repeat with Range (DONE — 2026-04-03)

Looping engine for "at least N, up to M" repetition with first/last
binding bookends. True unbounded support (`* N..`) — no arbitrary caps.

**Design: Looping engine (neither Option A nor B from original plan).**
The pattern stores `first_` + `last_` prefixed stages. The `last_`
segment loops: when a PM advances past it, the engine clears non-shared
bindings, resets `next_stage` to the segment start, and increments
`repetition_count`. First/last bookends give consumers the initial and
most recent match without intermediate bloat.

| Item | Summary |
|------|---------|
| `RepeatRange` on `Pattern` | `stage_start`, `stage_end`, `min_reps`, `max_reps: Option<usize>`, `shared_vars: HashSet<String>`. `None` max = unlimited. |
| `repetition_count: u32` on `PartialMatch` | Incremented each loop. Included in fingerprint via `compute_fingerprint_with_rep`. |
| `compose::repeat_range()` | Generates `first_` + `last_` stage copies. Shared vars unprefixed across both. Sets `repeat_range` on pattern. |
| Engine Phase 3 loop logic | When PM passes segment end: emit Completed (if `rep >= min`), create looping Active PM with cleared non-shared bindings (if `rep < max` or unbounded). Intervals kept for temporal ordering. |
| DSL `* N..M` and `* N..` | Parser handles `DotDot` token between numbers. `* N` (exact) unchanged — routes to old unrolled `repeat()`. Compiler validates min >= 1, max >= min. |
| Backward compatible | `* N` (exact count) still uses fully-unrolled `repeat()` with distinct `repN_` prefixes. No behavior change for existing patterns. |
| 9 new tests | Lifecycle: complete at min, continue after min, stops at max, unbounded, first/last bindings, backward compat. DSL: parse range, parse unbounded, parse exact unchanged. |

---

#### 5.5 Unordered / concurrent stage groups

Allow a group of stages that must all match but in any order. Currently
stages are strictly sequential (left-to-right temporal ordering). This
blocks multi-signal correlation patterns where ordering is irrelevant.

- Observability: "Error rate spike AND latency increase co-occur"
- Security: "Recon scan AND social engineering simultaneously"
- Clinical: "Fever AND tachycardia AND leukocytosis present concurrently"
- IoT: "Temperature high AND pressure low at the same time"
- Canary: "Errors on pod A AND errors on pod B" (spread, order irrelevant)

**Design:**

Introduce a `StageOrdering` enum on `Pattern`:

```rust
pub enum StageOrdering {
    Sequential,                     // current behavior (default)
    Unordered { indices: Vec<usize> }, // these stage indices may match in any order
}
```

A pattern can have a mix: stages 0-1 sequential, stages 2-4 unordered,
stage 5 sequential after the unordered group completes.

**Changes required:**

1. **Pattern struct** (`fabula/src/pattern.rs`): Add field
   `pub unordered_groups: Vec<Vec<usize>>`. Each inner vec is a set of
   stage indices that form an unordered group. Empty by default
   (all stages sequential). Stages in an unordered group must be
   contiguous.

2. **Engine Phase 3** (`fabula/src/engine/eval.rs`): When advancing a PM,
   check if `next_stage` falls within an unordered group. If so, try
   ALL unmatched stages in the group against the incoming edge (not just
   `next_stage`). Track which stages in the group have been matched via
   a bitmask on the PM: `pub matched_stages: u64` (supports up to 64
   stages — sufficient for all practical patterns).

   When all stages in the group are matched, advance `next_stage` past
   the group to the first sequential stage after it.

3. **Temporal constraints**: Within an unordered group, implicit
   left-to-right ordering does NOT apply. Explicit temporal constraints
   (Allen relations) between stages in the group are still respected.
   Between an unordered group and the surrounding sequential stages,
   the group as a whole must satisfy the ordering (all group stages
   must occur after the preceding sequential stage and before the
   following one).

4. **PatternBuilder** (`fabula/src/builder.rs`):
   ```rust
   .unordered_group(|g| g
       .stage("a", |s| s.edge(...))
       .stage("b", |s| s.edge(...))
       .stage("c", |s| s.edge(...))
   )
   ```

5. **DSL** (`fabula-dsl/`):
   ```
   pattern multi_signal {
     stage setup { ... }
     concurrent {
       stage error_spike { ... }
       stage latency_spike { ... }
     }
     stage cascade { ... }
   }
   ```

6. **Fingerprint**: Unordered group matching order should NOT affect
   the fingerprint. The fingerprint already uses order-independent
   XOR hashing on bindings, so this should work naturally as long as
   the same set of bindings produces the same hash regardless of
   which stage matched first.

**Files**: `fabula/src/pattern.rs`, `engine/eval.rs`, `engine/types.rs`,
`builder.rs`, `fabula-dsl/src/{ast,parser,compiler}.rs`
**Tests**: Unordered match in both orders, mixed sequential + unordered,
temporal constraints within group, fingerprint stability, golden test
**Effort**: Medium (~200-300 LoC). Most architecturally invasive item on
this list — touches the core advancement loop.

---

#### 5.6 Windowed aggregation constraints (conditional)

A stage-level constraint that aggregates over recent matches: "count of
edges matching this clause in the last N ticks >= threshold." Not a full
CEP windowing system — a focused count/sum/min/max over a sliding window.

- Security: "5+ failed logins within 10 minutes" as one stage
- Finance: "Transaction velocity > 10/hour for this account"
- IoT: "Average temperature over last 5 readings > threshold"
- Observability: "Error rate > 1% over 5-minute window"

This bridges the gap with CEP systems (Flink, Esper) where windowed
aggregation is the bread-and-butter primitive.

**Gated on 5.4**: If repeat-with-range handles sufficient counting use
cases, this may not be needed. Evaluate after 5.4 ships.

**Design sketch:**

```rust
pub struct WindowedConstraint<L, V> {
    pub clause: Clause<L, V>,
    pub window_ticks: u64,
    pub aggregation: Aggregation,
    pub threshold: f64,
}

pub enum Aggregation { Count, Sum, Min, Max, Avg }
```

Engine maintains a ring buffer per active windowed constraint, recording
matching edges within the window. On each `end_tick()`, evict expired
entries. Stage matches when aggregation over the window meets the
threshold.

**Files**: `fabula/src/pattern.rs`, `engine/eval.rs`, `engine/mod.rs`
**Tests**: Count within window, eviction on window slide, threshold met/not
**Effort**: Medium (~200-350 LoC)

---

### Phase 6 — Narrative Stack Integration

Connect fabula to the worldbuilding stack (Paracausality, WorldKernel).
These items build on Phase 5's platform generalization.

**Recommended order**: 6.1 → 6.2 → 6.3 (6.3 blocked on Paracausality).

#### 6.1 Event causality tracing

Path-finding over the temporal graph to trace causal chains:
- "This betrayal traces back to THAT institutional failure 20 ticks ago"
- Causal chain cleanliness scoring for the MCTS quality function

**Groundwork available**: `DataSource` provides graph traversal
(`edges_from`, `edges_from_any_time`). Allen algebra handles temporal
precedence validation. Multi-stage patterns already serve as individual
causal steps — this extends to multi-hop pathfinding.

**Design decision**: Explicit causal edges (stored in graph as
"causes"/"led_to" labels with weights) vs. implicit pattern-based
inference. **Recommended: explicit** — cleaner, faster, easier to
validate. Caller provides a mapping of edge labels → causality weights.

**API sketch:**

```rust
pub struct CausalPath<N, V, T> {
    pub nodes: Vec<N>,
    pub edges: Vec<(L, V, T)>,
    pub cleanliness: f64,      // chain quality score
    pub confidence: f64,       // credibility of chain
}

impl<N, L, V, T> SiftEngine<N, L, V, T> {
    pub fn causal_paths(
        &self,
        ds: &impl DataSource<N=N, L=L, V=V, T=T>,
        effect: &N,
        max_hops: usize,
        causal_labels: &HashMap<L, f64>,
    ) -> Vec<CausalPath<N, V, T>>;
}
```

BFS from effect backward through causal predecessors, validating temporal
ordering at each hop. Return paths sorted by cleanliness score.
Cleanliness: `mean(edge_weights) * (1 - gap_penalty) * divergence_factor`.

**Depends on**: Causal link representation in the graph
**Blocked on**: Nothing — can prototype with explicit causal edges
**Files**: New `fabula/src/causality.rs` + engine integration
**Effort**: Medium-high (~350-450 LoC)

---

#### 6.2 Character appraisal patterns

Sifting-based emotional appraisal from the GM architecture:
- Event → relevance check (does event affect character's goals?)
- Misbelief arc monitoring (has contradicting evidence appeared?)
- Emotional trajectory patterns (mood declining over time window)

**Groundwork available**: `TensionTracker` (sliding window, linear
regression, trajectory classification) can be repurposed for emotion
trajectories. Pattern composition operators provide structure for
multi-step appraisal logic. Paracausality Store already exposes
`all_objective_for_entity()`.

**Depends on**: Paracausality adapter (DONE)
**Blocked on**: Nothing — ready to start
**Effort**: Medium (~250-350 LoC)

---

#### 6.3 Knowledge propagation patterns

Track what characters know and when they learn it:
- "A witnessed X, B doesn't know yet"
- "C heard about X through D (gossip chain)"
- Information distortion modeling

**Groundwork available**: Pattern composition (sequence operator) can
model communication chains. Negation windows express "B doesn't know
yet". Value constraints support distortion tracking via numeric accuracy
fields.

**Relates to**: Kreminski 2023 gossip paper
**Depends on**: Paracausality adapter (DONE)
**Blocked on**: Paracausality Store epistemic layer — `query_knowledge()`,
`record_knowledge()` methods not yet implemented. Needs coordinated work
in the Paracausality repo before this can proceed.
**Effort**: Medium-high (~450 LoC fabula, ~100-150 LoC Paracausality)

---

### Phase 7 — Scoring & DSL Refinements

Lower-priority enhancements to existing subsystems. These refine rather
than extend core capabilities.

#### 7.1 StU aggregation alternatives

Replace arithmetic mean with information-theoretic aggregation:
- **TF-IDF style**: `sum(-log(freq))` — rare properties dominate via log
  weighting. Natural generalization of StU. ~5 LoC change.
- **Geometric mean**: `(∏ freq(pi))^(1/k)` — single rare property pulls
  entire score down. Needs Laplace smoothing (already have it). ~5 LoC.
- **Minimum**: `min(freq(pi))` — most surprising property dominates.
  Loses signal from multiple rare properties. ~3 LoC.

Recommend: make aggregation configurable on `StuScorer` with an enum.
Default remains arithmetic mean for backwards compatibility.

**Files**: `fabula/src/scoring/stu.rs`
**Effort**: Small (~30 LoC)

#### 7.2 StU confidence weighting (cold start)

`final = stu_score * (1 - 1/(total_matches + 1))`. At 1 match,
confidence = 0.5. At 10, confidence = 0.91. At 100, confidence = 0.99.
Gently attenuates noisy scores from sparse data.

**Files**: `fabula/src/scoring/stu.rs`
**Effort**: Trivial (~5 LoC)

#### 7.3 Correlated-unlikelihood correction (property-pair PMI)

`PMI(pi, pj) = log(P(pi,pj) / P(pi)P(pj))`. High PMI means properties
co-occur more than expected → don't double-count their rarity. Requires
O(V²) pair counting per pattern.

**Reference**: StU's known weakness per Kreminski et al.
**Files**: `fabula/src/scoring/stu.rs`
**Effort**: Small-medium (~50 LoC)

#### 7.4 Sequential surprise (bigram transitions)

`P(event_B | event_A)` from observed sequences. Sequential surprise =
`-log P(current_event | previous_event)`. Simpler alternative to Schulz
et al.'s full 5-measure framework, which requires a predictive model of
next-state distributions.

**Files**: New `fabula/src/scoring/sequential.rs`
**Effort**: Small-medium (~80 LoC)

#### 7.5 DSL nested compose expressions

Currently the parser is flat-only: `compose ab = a >> b` works but
`compose abc = (a >> b) | c` does not. Named intermediates are the
workaround. Nested expressions would require recursive expression parsing
in the parser.

**Files**: `fabula-dsl/src/{parser,ast,compiler}.rs`
**Effort**: Medium

#### 7.6 DSL non-exclusive choice

Currently all `|` creates mutual exclusion groups. Add syntax for
non-exclusive choice where multiple alternatives can match independently.

**Files**: `fabula-dsl/src/{parser,compiler}.rs`, `fabula/src/compose.rs`
**Effort**: Small

#### 7.7 DSL `private` pattern modifier

Mark patterns that should not appear in output — they exist only as
building blocks for composition.

**Files**: `fabula-dsl/src/{ast,parser,compiler}.rs`, `fabula/src/pattern.rs`
**Effort**: Small

#### 7.8 Kernel/satellite metadata

Optional metadata field on Pattern indicating whether it represents a
kernel (turning point) or satellite (elaboration) event per Chatman's
Story and Discourse. Trivial addition — the `group` field provides the
precedent.

**Files**: `fabula/src/pattern.rs`
**Effort**: Trivial (2 lines — subsumed by 5.1 pattern metadata)

---

### Phase 8 — Research & Publication

Academic contributions that fill gaps in the literature.

#### 8.1 Formal semantics paper (~30% publication-ready)

Formal specification of incremental temporal graph pattern matching:
- Partial match state machine (Active → Complete | Dead → Expired)
- Update semantics for `on_edge_added`
- Negation window semantics (exclusive boundaries, multi-clause bodies)
- Correctness proof: batch ≡ incremental for well-ordered streams

**Raw material**: `eval.rs` has clear 4-phase algorithm documentation,
`MatchState` enum with explicit transitions, Allen algebra with original
paper references. Negation window semantics (exclusive boundary,
multi-clause conjunction) are well-commented.

**Gaps**: No formal state machine specification (needs LTS/FSA definition
with pre/postconditions). No batch≡incremental correctness proof (needs
induction on edge count + fingerprint dedup correctness argument). No
complexity analysis (time: O(P × (A + N × C²)), space: O(A × V)).

#### 8.2 Scalability analysis (~60% publication-ready)

First published performance analysis of story sifting at scale.

**Raw material**: Divan parameterized benchmark suite with GM-profile and
scaling sweeps. Profiling harness with per-tick CSV and dhat integration.
Workload generators with configurable stages, clauses, and negation
fractions. Before/after data from fingerprint optimization.

**Gaps**: Needs repeated runs with error bars (3-5x, mean ± std-dev).
Missing ablation studies (fingerprint on/off, negation on/off, stage
count impact). No comparative baseline (vs. Winnow or batch break-even).
Needs experimental methodology section.

**Nearest to publication.** Benchmark infrastructure is complete; primary
work is experimental methodology and writeup.

#### 8.3 Expressiveness hierarchy (~20% publication-ready)

Classification of pattern languages by what narrative phenomena they
can express. Maps to Chomsky-like hierarchy for narrative pattern grammars.

Single-stage < multi-stage < negation < temporal < compositional

**Raw material**: DESIGN.md has complete Felt→Winnow→Fabula feature
mapping. Pattern language constructs inventoried.

**Gaps**: No formal grammar classification. No impossibility proofs (what
patterns CANNOT be expressed — transitive closure, unbounded repetition,
recursive definitions). No comparative analysis vs. Felt/Winnow/Datalog
on real narrative patterns. Needs foundational theory building — deepest
research effort of the three.

---

### Deferred

Items explicitly deferred with conditions for reconsideration.

| Item | Reason | Reconsider When |
|------|--------|-----------------|
| MemGraph label indexing (old 2.4) | Phase 2.3 closed incremental gap to 28us/edge; MemGraph is testing-only | MemGraph elevated to production, or batch performance critical |
| WASM benchmark page (old Phase 2) | Browser benchmarks noisy; measures serde overhead, not engine | Pattern library exists for demos (Phase 7+) |
| Computed value expressions | Slippery slope toward full expression evaluator; closure approach breaks DSL | Cross-stage comparison (5.3) proves insufficient for real use cases |
| Generic event-stream DataSource adapter | Useful but premature without a target domain integration | Specific domain integration prioritized (e.g., `fabula-otel`, `fabula-json`) |
| Propp functions pattern library | Academic pattern library, needs vocabulary mapping layer | Phase 2 of fabula-narratives when needed |
| Reagan emotional arcs / DTW | Retrospective time-series analysis, different computational model | Need for offline arc classification emerges |
| Implicit sharing by name (DSL) | Partial groundwork in binding validation; explicit `sharing()` works | Users frequently request it |
| Windowed aggregation (5.6) | May be subsumed by repeat-with-range (5.4) | Repeat-with-range proves insufficient for counting use cases |

---

## Summary

| Phase | Theme | Status | Key Deliverables |
|-------|-------|--------|------------------|
| 1 | Polish & Parity | **DONE** | Variable distinction, negation validation, dedup, metric temporal, age tracking |
| 2 | Benchmarking | **DONE** | Stats counters, profiling + divan harness, fingerprint optimization (5.8x) |
| 3 | Composition | **DONE** | Pattern algebra, DSL compose syntax, surprise scoring (Shannon + StU) |
| 4 | Narrative Scoring | **DONE** | Thread tracker, tension tracker, pivot detector, MCTS scorer |
| — | Salience Integration | **DONE** | Free functions, Match intervals, early termination, serde, Eq+Hash, composable parser, MemGraph utilities |
| **5** | **Platform Generalization** | **NEXT** | Metadata ✓, timeout events ✓, cross-stage comparison ✓, repeat range ✓, unordered stages |
| **6** | **Narrative Stack** | PLANNED | Causality tracing, character appraisal, knowledge propagation |
| **7** | **Scoring & DSL** | PLANNED | StU refinements, nested compose, non-exclusive choice, private patterns |
| **8** | **Research** | FUTURE | Formal semantics (30%), scalability paper (60%), expressiveness hierarchy (20%) |

**Recommended execution for Phase 5:**
Sprint 1 (quick wins): 5.1 (metadata) ✓ → 5.2 (timeout) ✓ → 5.3 (cross-stage comparison) ✓
Sprint 2 (thresholds): 5.4 (repeat range) ✓
Sprint 3 (concurrency): 5.5 (unordered stages)
Evaluate: 5.6 (windowed aggregation) after 5.4

---

## References

- Kreminski et al. (2019). **Felt: A Simple Story Sifter.** ICIDS 2019.
- Kreminski et al. (2021). **Winnow: A Domain-Specific Language for Incremental Story Sifting.** AIIDE 2021.
- Kreminski et al. (2022). **Select the Unexpected: A Statistical Heuristic for Story Sifting.** ICIDS 2022.
- Kreminski et al. (2025). **Stories from the Bottom Up: Composable Story Sifting Patterns.** FDG 2025.
- Kreminski et al. (2023). **Knowledge Propagation in Interactive Narrative.**
- Schulz et al. (2024). **Narrative Information Theory.**
- Nelson & Mateas (2005). **Search-Based Drama Management.** AIIDE 2005.
- Allen, J.F. (1983). **Maintaining Knowledge about Temporal Intervals.** CACM 26(11).
- Dechter, Meiri, Pearl (1991). **Temporal Constraint Networks.** AI 49(1-3).
- Meiri (1996). **Combining Qualitative and Quantitative Constraints.** AI 87(1-2).
- Reagan et al. (2016). **The Emotional Arcs of Stories.** EPJ Data Science.
- Chatman, S. (1978). **Story and Discourse.** Cornell University Press.
- Propp, V. (1928/1968). **Morphology of the Folktale.**
- Short, E. (2016). **Quality-Based Narrative.**
- Ehmes et al. (2020). **GrapeL: Combining Graph Pattern Matching and CEP.** Springer.
