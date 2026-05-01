# Fabula — Design Document

**Status**: Living document
**Date**: 2026-04-02

Fabula is a Rust library for incremental pattern matching over temporal graphs. It is a port and extension of [Felt](https://github.com/mkremins/felt) (Kreminski et al., ICIDS 2019) and [Winnow](https://github.com/mkremins/winnow) (Kreminski et al., AIIDE 2021), which are JavaScript/ClojureScript libraries for **story sifting** — extracting narratively compelling event sequences from simulation output.

This document defines what fabula does, feature by feature, mapped to the original implementations and research papers.

---

## What Felt Does

Felt (290 lines of JavaScript) is both a **story sifter** and a **simulation action engine** built on [DataScript](https://github.com/tonsky/datascript), an immutable in-memory Datalog database. All state — characters, events, relationships, intent tokens — lives in a flat entity-attribute-value (EAV) store. Felt provides:

### Sifting (pattern detection)

| Felt Function | What It Does |
|---|---|
| `registerSiftingPattern(name, clauses)` | Register a named sifting pattern (array of Datalog clause strings) |
| `runSiftingPattern(db, pattern)` | Execute one pattern against the DB, return all matches ("nuggets") |
| `runSiftingPatterns(db)` | Execute all registered patterns, return all matches |
| `setQueryRules(rules)` | Register reusable Datalog rules (e.g., `eventSequence`, `causalRelationship`) |

A **sifting pattern** is a set of Datalog clauses that bind logic variables across entities and events:

```clojure
;; Two betrayals by the same impulsive character with no actions between
(eventSequence ?eventA ?eventB)
[?eventA "eventType" "betray"] [?eventA "actor" ?char]
[?eventB "eventType" "betray"] [?eventB "actor" ?char]
[?char "trait" "impulsive"]
(not-join [?char ?eventA ?eventB]
  (eventSequence ?eventA ?eventMid ?eventB)
  [?eventMid "actor" ?char])
```

A **nugget** (match) is a set of variable bindings: `{char: 5, eventA: 17, eventB: 31}`.

**Key design decisions in Felt:**
- Patterns desugar to DataScript Datalog queries (the `parseSiftingPattern` function compiles clause arrays into `:find ... :in $ % :where ...` query strings)
- Logic variables start with `?` and are collected automatically
- `not-join` provides negation (DataScript's closed-world negation-as-failure)
- Query rules (`eventSequence`, etc.) are registered globally and available to all patterns
- The EAV store is immutable — every "update" produces a new database value, enabling snapshot/rollback
- `whyNot` debugging: tests pattern clauses individually to identify which clause fails

### Action system (simulation integration)

| Felt Function | What It Does |
|---|---|
| `registerAction(name, spec)` | Register an action with `where` (sifting pattern as precondition), `event` (constructor function), optional `find` and weight |
| `possibleActions(db)` | Find all action/binding pairs whose preconditions are currently satisfied |
| `possibleActionsByType(db)` | Same, grouped by action name |
| `realizeEvent(action, bindings)` | Instantiate an action into a concrete event object |
| `addEvent(db, event)` | Commit event to DB + process its effects |
| `registerEffectHandler(name, handler)` | Register a side-effect processor (e.g., `setIntention`, `changeRelationship`) |
| `processEffect(db, effect)` | Apply one effect via its registered handler |
| `checkEffectKeys(effect, keys)` | Validate effect structure |

**Key insight: sifting patterns and action preconditions use the same query language.** An action's `where` clause IS a sifting pattern. Finding "what interesting things have happened" and "what can happen next" are the same operation.

Felt distinguishes two action flavors by convention:
- **Internal (reflection) actions**: a character sifts their own history and produces an intent token (e.g., "Alice reflects on Bob's betrayal → forms intent to seek revenge")
- **External actions**: a character consumes an intent token and changes the world (e.g., "Alice confronts Bob")

This creates causal chains: event → reflection → intent → action → new event.

---

## What Winnow Adds

Winnow (~500 lines of JavaScript: parser + compiler + runner) is a higher-level DSL that compiles to Felt/DataScript queries and adds **incremental matching** — tracking partial pattern matches as events stream in, rather than re-evaluating from scratch.

### The DSL

Winnow provides a readable pattern syntax that compiles to DataScript Datalog:

```clojure
(pattern violationOfHospitality
  (event ?e1 where
    eventType: enterTown,
    actor: ?guest)
  (event ?e2 where
    eventType: showHospitality,
    actor: ?host,
    target: ?guest,
    ?host.value: communalism)    ;; dotted access: check host's value attribute
  (event ?e3 where
    tag: harm,
    actor: ?host,
    target: ?guest)
  (unless-event ?eMid between ?e1 ?e3 where
    eventType: leaveTown,
    actor: ?guest))
```

**Compilation pipeline**: Winnow text → S-expression parser → AST → DataScript Datalog query string.

Key Winnow syntax features:
- `(event ?name where attr: val, ...)` — event stages with attribute constraints
- `?var.attr: val` — dotted lvar access (check a property of a bound entity)
- `(unless-event ... between ?start ?end where ...)` — negation window
- `(not attr val)` — inline negation within an event stage
- `(ruleName ?args)` — invoke user-defined Datalog rules
- Semicolon comments
- Commas and colons treated as whitespace

### Incremental matching (the core innovation)

Felt evaluates patterns in batch: query the DB, get all matches. Winnow adds a **runner** that tracks partial matches and advances them one event at a time.

**Data structure: PartialMatch**
```javascript
{
  pattern,           // compiled pattern
  bindings: {},      // variable → entity ID (so far)
  lastStep,          // "accept" | "complete" | "pass" | "die"
  parent,            // previous partial match (for history)
  deathDetails       // {eventID, constraint} if killed by negation
}
```

**Core algorithm: `tryAdvance(partialMatch, db, rules, latestEventID)`**

For each new event added to the simulation:

1. **Check negation constraints first.** For each `unless-event` constraint where `betweenStart` is bound but `betweenEnd` is not yet bound (the negation window is open): query whether the latest event matches the negation clause. If yes → **kill** the partial match (`lastStep = "die"`, record `deathDetails`). Return immediately.

2. **Try to advance.** Find the first unbound event clause in the pattern. Run a focused DataScript query: does the latest event satisfy this clause, given the existing bindings? For each successful binding → spawn a **new** partial match with the additional bindings and `lastStep = "accept"` (or `"complete"` if it was the last clause).

3. **Keep the original.** The original partial match is returned alongside any new matches, with `lastStep = "pass"`. This is critical: the same clause might match a different future event with different bindings.

**Driver: `getAllMatches(patterns, db, rules, events)`**

```javascript
partialMatches = patterns.map(p => ({pattern: p, bindings: {}}))
for (event of events) {
  db = addEvent(db, event)
  latestEID = newestEID(db)
  partialMatches = flatMap(partialMatches, pm => tryAdvance(pm, db, rules, latestEID))
  partialMatches = partialMatches.filter(pm => pm.lastStep !== "die")
}
return partialMatches  // includes complete matches
```

### Temporal ordering

Winnow enforces temporal ordering through DataScript entity ID comparison:
- Events are added sequentially → lower entity IDs = earlier events
- The compiler emits `[(< ?e1 ?e2 ?e3)]` to enforce stage ordering
- `unless-event ... between ?start ?end` emits `[(< ?start ?eMid ?end)]`
- If no `between` is specified, defaults to `between ?firstEvent ?lastEvent`

### Negation semantics

Two forms:
- **`unless-event ... between ?start ?end where ...`**: No event matching the constraint exists in the temporal window. Compiled to DataScript `not-join` with ordering predicates. In the runner, checked eagerly on each new event.
- **`(not attr val)`**: Inline negation — an event must NOT have this attribute/value. Compiled to `(not [?e "attr" "val"])`.

---

## What Fabula Is

Fabula is a Rust implementation that provides the **sifting and incremental matching** capabilities of Felt and Winnow, with three key extensions:

1. **Allen interval algebra** instead of entity ID ordering. Felt/Winnow use integer comparison for temporal ordering. Fabula uses proper temporal intervals with all 13 Allen relations (before, after, meets, overlaps, during, contains, starts, finishes, equals, and their inverses). This enables richer temporal constraints: "event A overlaps with event B" or "event A is contained within event B."

2. **Generic `DataSource` trait** instead of DataScript. Felt is tightly coupled to DataScript. Fabula queries any temporal graph through a trait with associated types for node IDs, edge labels, values, and time. A `MemGraph` reference implementation is included for testing.

3. **No Datalog dependency.** Felt compiles patterns to Datalog queries and uses DataScript's query engine. Fabula implements pattern matching directly as graph traversal with variable binding.

### Feature mapping

| Feature | Felt | Winnow | Fabula |
|---|---|---|---|
| **Sifting patterns** | `registerSiftingPattern` + `runSiftingPatterns` | Compiled from DSL | `SiftEngine::register` + `SiftEngine::evaluate` |
| **Incremental matching** | No | `tryAdvance` + `PartialMatch` | `SiftEngine::on_edge_added` + `PartialMatch` |
| **Negation (unless-event)** | `not-join` in Datalog | `unless-event ... between` + runtime check | `Negation` + `check_negation_kill` |
| **Inline negation** | `(not ...)` in Datalog | `(not attr val)` in DSL | `Clause { negated: true }` |
| **Variable binding** | Datalog unification | Datalog unification | Graph traversal + `BoundValue<N, V>` |
| **Temporal ordering** | Entity ID comparison | `[(< ?e1 ?e2)]` | `Interval.start` comparison + Allen relations |
| **Gap analysis (whyNot)** | Clause-by-clause DB query | N/A | `SiftEngine::why_not` → `GapAnalysis` |
| **Dotted lvar access** | N/A | `?host.value: communalism` | Property clauses within stages |
| **Default negation bounds** | N/A | First/last event in pattern | `unless_after` (open end) + explicit bounds |
| **Death details** | N/A | `{eventID, constraint}` | `SiftEvent::Negated { negation_label }` |
| **Query rules** | `setQueryRules` | Inherited | Not applicable (no Datalog) |
| **Text DSL** | N/A | S-expression parser + compiler | `fabula-dsl` crate: lexer + parser + compiler + TypeMapper |
| **Pattern composition** | N/A | N/A | `compose` module: sequence (`>>`), choice (`\|`), repeat (`*`) |
| **Surprise scoring** | N/A | N/A | `scoring` module: Shannon surprise + StU (Kreminski 2022) |
| **Narrative scoring** | N/A | N/A | `fabula-narratives` crate: threads, tension, pivots, MCTS scorer |
| **Metric temporal** | N/A | N/A | STN-style gap bounds on Allen relations (Dechter/Meiri/Pearl 1991) |
| **Pattern lifecycle** | N/A | N/A | Enable/disable/deregister, per-pattern metrics, staleness |
| **Plant/payoff tracking** | N/A | N/A | Chekhov's gun monitoring + staleness detection |
| **MCTS forking** | N/A | N/A | `Clone` impl for speculative evaluation |
| **Action system** | `registerAction` + `possibleActions` + `addEvent` + effects | Inherited | **Not in scope** — see below |
| **Partial match survival** | N/A | Original PM returned with `lastStep: "pass"` | Original PM remains in `partial_matches` vec |

### What fabula intentionally omits

**Felt's action/effect system.** Fabula is a sifting library, not a simulation engine. It detects patterns in a graph; it does not generate events, select actions, or process effects. The action system (`registerAction`, `possibleActions`, `realizeEvent`, `addEvent`, `registerEffectHandler`, `processEffect`) belongs to the simulation layer that feeds edges into fabula.

This is a deliberate scope boundary. Felt combines sifting and simulation in one library because it targets small research prototypes. Fabula is designed for integration into larger systems where the simulation is a separate concern.

**Datalog query rules.** Felt and Winnow compile patterns to DataScript Datalog, which supports user-defined rules (`eventSequence`, `causalRelationship`, etc.). Fabula replaces Datalog with direct graph traversal, so there is no rule system. Complex multi-hop queries that would use recursive Datalog rules in Felt must be expressed as multi-clause patterns in fabula, or handled by the consuming application.

---

## Architecture

### Concepts (mapped to Felt/Winnow)

| Fabula Concept | Felt/Winnow Equivalent |
|---|---|
| **Edge** `(source, label, target, interval)` | DataScript EAV triple `[entity attribute value]` |
| **Pattern** with ordered **Stages** | Winnow `(event ...)` clauses in sequence |
| **Stage** = anchor var + clauses | Winnow `(event ?e where attr: val, ...)` |
| **Clause** = `source --[label]--> target` | Felt clause `[?e "attr" val]` |
| **Target::Bind(var)** | Datalog logic variable in value position |
| **Target::Literal(val)** | Quoted string/number in Datalog |
| **Target::Constraint(vc)** | Datalog function call `[(< ?val threshold)]` |
| **TemporalConstraint** | Winnow's implicit `[(< ?e1 ?e2)]` |
| **Negation** | Winnow's `(unless-event ... between ...)` |
| **PartialMatch** | Winnow's `{pattern, bindings, lastStep}` |
| **SiftEvent::Completed** | Winnow's `lastStep: "complete"` |
| **SiftEvent::Negated** | Winnow's `lastStep: "die"` with `deathDetails` |
| **SiftEvent::Advanced** | Winnow's `lastStep: "accept"` |
| **MemGraph** | DataScript's in-memory EAV database |
| **DataSource trait** | DataScript's query interface (implicitly coupled in Felt) |

### The DataSource trait

The single integration point. Any temporal graph implements this:

```rust
pub trait DataSource {
    type N: Eq + Hash + Clone + Debug;    // Node ID
    type L: Eq + Hash + Clone + Debug;    // Edge label
    type V: PartialEq + PartialOrd + Clone + Debug;  // Value
    type T: Ord + Clone + Debug;          // Time

    /// Follow edges from a node. (Felt: DataScript query with bound entity)
    fn edges_from(&self, node: &Self::N, label: &Self::L, at: &Self::T)
        -> Vec<Edge<Self::N, Self::V, Self::T>>;

    /// Find edges matching constraints. (Felt: DataScript query with unbound entity)
    fn scan(&self, label: &Self::L, constraint: &ValueConstraint<Self::V>, at: &Self::T)
        -> Vec<Edge<Self::N, Self::V, Self::T>>;

    /// Same as edges_from but ignoring time. (For negation window checks.)
    fn edges_from_any_time(&self, node: &Self::N, label: &Self::L)
        -> Vec<Edge<Self::N, Self::V, Self::T>>;

    /// Same as scan but ignoring time. (For batch negation checks.)
    fn scan_any_time(&self, label: &Self::L, constraint: &ValueConstraint<Self::V>)
        -> Vec<Edge<Self::N, Self::V, Self::T>>;

    /// Current time in the graph's model.
    fn now(&self) -> Self::T;

    /// Can this value be followed as a node? (Felt: `:db.type/ref` attributes)
    fn value_as_node(&self, value: &Self::V) -> Option<Self::N>;
}
```

`edges_from` and `scan` are the two fundamental operations — following a known edge vs. finding edges by constraint. The `_any_time` variants exist because negation window checks need to scan across all time, not just the current snapshot. `value_as_node` distinguishes node references (traversable) from literal values (comparable), equivalent to DataScript's `:db.type/ref` declaration.

### Incremental algorithm (ported from Winnow)

Fabula's `on_edge_added` implements Winnow's `tryAdvance` algorithm:

**Phase 1: Negation check.** For each active partial match, check whether the new edge kills it. A negation kills if:
- The negation window is open (`between_start` bound, `between_end` not bound)
- The new edge falls within the window (after `between_start`'s interval)
- The new edge matches the negation clause's label and target constraint
- **All** clauses in the negation block are satisfiable for the same entity (checking bound variable consistency against the parent match's bindings)

This is stricter than Winnow's original implementation, which delegated the full check to a DataScript query. Fabula checks clause-by-clause because it doesn't have a Datalog engine.

**Phase 2: Initiate.** For each registered pattern, test whether the new edge matches the first stage. If yes, create a new partial match with bindings from that stage.

**Phase 3: Advance.** For each active partial match, test whether the new edge matches the *next* unmatched stage. If yes, spawn a new partial match with merged bindings. The original partial match survives (Winnow's "pass" behavior).

**Phase 4: Cleanup.** Remove dead partial matches.

### Variable binding within stages

A stage is a group of clauses anchored to a single event/node variable. When a stage matches:

1. The **first clause** is the trigger — the incoming edge must match its label and target.
2. Remaining clauses are verified against the data source using the accumulated bindings.
3. **All Bind targets in all clauses produce bindings**, not just the first clause. This ensures that variables like `?guest` in a multi-clause stage (e.g., `?e --[actor]--> ?host` and `?e --[target]--> ?guest`) are properly bound and available in subsequent stages and negation checks.

This differs from Winnow, where all clauses in an event block are evaluated simultaneously by the Datalog engine. In fabula, the first clause is the trigger and remaining clauses are verified sequentially.

### Gap analysis (whyNot)

Mapped from Felt's `whyNot` function, which tests pattern clauses individually against the database to identify which clause prevents a match. Fabula's `why_not` evaluates stages in order, reporting:

- **Matched** — all clauses in this stage are satisfied
- **PartiallyMatched** — some clauses satisfied, some not (with per-clause reasons)
- **Unmatched** — no clauses satisfied

Analysis stops at the first unmatched stage (since later stages depend on earlier bindings).

---

## Allen Interval Algebra

Fabula's primary extension over Felt/Winnow. Instead of entity ID comparison for temporal ordering, fabula provides proper interval algebra.

**Why this matters:** Felt/Winnow assume events are discrete points ordered by integer IDs. Real temporal graphs have events with duration — a war lasts years, a conversation lasts minutes, a property like "at_location" is valid over a continuous interval. Allen's algebra handles all of these:

| Relation | Meaning | Example |
|---|---|---|
| Before | A ends before B starts | "The storm ended before the battle began" |
| Meets | A ends exactly when B starts | "Training ended the moment combat began" |
| Overlaps | A starts before B, ends during B | "The siege overlapped with the harvest" |
| During | A is entirely within B | "The assassination happened during the feast" |
| Contains | A contains B entirely | "The war contained many battles" |
| Starts | A and B start together, A ends first | "The alliance and the campaign began together" |
| Finishes | A and B end together, A starts later | "The retreat and the rout ended at the same time" |
| Equals | Identical intervals | |

Plus inverses: After, MetBy, OverlappedBy, StartedBy, FinishedBy.

**Implicit ordering:** Stages in a pattern are implicitly ordered by interval start time (left-to-right). Explicit `TemporalConstraint`s can specify any Allen relation.

**Open-ended intervals:** `Interval { start, end: None }` represents ongoing state. Allen relations require bounded intervals and return `None` for open-ended comparisons; fabula falls back to start-time comparison in that case.

---

## Test Scenarios (from Winnow test suite)

The following scenarios are ported from Winnow's `tests.js`:

### Violation of Hospitality
Guest enters town → host shows hospitality → host harms guest. Negation: guest must not have left town between entry and harm. Tests: 3-stage matching with joins (?guest, ?host bound across stages), negation window, unrelated characters don't trigger negation.

### Two Impulsive Betrayals
Same character betrays twice with no actions between. Tests: same-variable join across stages, inline negation.

### Romantic Failure Then Success
Character experiences two negative romantic events then one positive. Tests: tag-based matching, 3-stage arc detection.

### Criticism of Hypocrisy
A character acts against their own values, then gets criticized by an opponent. Tests: semantic rules (in Winnow; in fabula, expressed as property clauses).

---

## Implementation Status

### Core Library (`fabula`)
- Allen interval algebra with all 13 relations (generic over time type)
- Metric temporal constraints (STN-style gap bounds on Allen relations via `gap min..max`)
- DataSource trait (generic over N, L, V, T)
- `SiftEngine<N, L, V, T, E>` -- decoupled from DataSource (with `SiftEngineFor<DS, E = DefaultLetEvaluator>` alias). `E` is a pluggable `LetEvaluator<N, V>`; `V: ArithmeticValue` is no longer required by the engine API.
- Batch evaluation (`evaluate`), incremental matching (`on_edge_added`), gap analysis (`why_not`)
- Drain completed matches (`drain_completed`)
- BoundValue generic over `<N, V>` with closure-based matching (decoupled from DataSource)
- Negation checks verify all clauses + bound variable consistency
- Variable binding from all stage clauses (not just first)
- Fingerprint-based PM deduplication (XOR hashing, zero-allocation rebuild)
- Pattern lifecycle: enable/disable/deregister, per-pattern metrics, staleness detection
- TickDelta reporting for GM integration
- Plant/payoff (Chekhov's gun) tracking with staleness alerts
- Manual `Clone` for MCTS forking (empty tick accumulators in clone)
- Pattern composition: sequence, exclusive choice, repeat with variable sharing
- Surprise scoring: Shannon surprise (`SurpriseScorer`) + StU property-level (`StuScorer`)
- Pattern types with `PartialEq` derives and `map_types()` for type conversion

### DSL (`fabula-dsl`)
- Lexer + parser + compiler for pattern and graph syntax
- Strict variable/literal distinction: `?var.label` vs `name.label` with compile-time scope validation
- Compose operators: `>>` (sequence), `|` (choice), `*` (repeat) with `sharing()`
- `TypeMapper` trait for compiling to arbitrary type systems + `MemMapper` default
- `ParsedDocument<L, V>` generic with defaults for backward compatibility

### Narrative Scoring (`fabula-narratives`)
- Thread lifecycle management (MICE-style open/close, FILO nesting validation)
- Tension trajectory sampling and classification (Rising/Falling/Plateau/Peak/Valley)
- Pivot detection via Jensen-Shannon Divergence on event distributions
- Composite MCTS quality function with configurable weights and explainable breakdown

### Adapters
- `fabula-memory`: MemGraph (Vec-backed linear scan)
- `fabula-petgraph`: PetTemporalGraph (wraps petgraph StableGraph)
- `fabula-grafeo`: GrafeoGraph (wraps Grafeo graph database)

### Testing & Tooling
- Golden test suite: 61 scenarios x 3 adapters = 183 tests
- 422+ total tests across all crates
- Benchmark harness (divan) + profiling binary
- WASM bindings for DSL parsing and evaluation

### Out of Scope (belongs in consuming systems)

| Feature | Where It Belongs |
|---|---|
| Action registration + preconditions | Simulation engine |
| Effect handlers + event processing | Simulation engine |
| Event construction + DB mutation | Simulation engine |
| Datalog rules | Not needed (direct graph traversal) |
| Storage / persistence | DataSource implementations |
| POV-aware querying | DataSource adapter layer |

---

## References

- Kreminski, M., Dickinson, M., & Wardrip-Fruin, N. (2019). **Felt: A Simple Story Sifter.** ICIDS 2019. [Paper](https://mkremins.github.io/publications/Felt_SimpleStorySifter.pdf) | [Code](https://github.com/mkremins/felt)
- Kreminski, M., Dickinson, M., & Mateas, M. (2021). **Winnow: A Domain-Specific Language for Incremental Story Sifting.** AIIDE 2021. [Paper](https://mkremins.github.io/publications/Winnow_AIIDE2021.pdf) | [Code](https://github.com/mkremins/winnow)
- Kreminski, M., et al. (2022). **Select the Unexpected: A Statistical Heuristic for Story Sifting.** ICIDS 2022. [Paper](https://mkremins.github.io/publications/StU_ICIDS2022.pdf)
- Kreminski, M., et al. (2022). **Authoring for Story Sifters.** [Paper](https://mkremins.github.io/publications/AuthoringSifters_TAP.pdf)
- Kreminski, M., et al. (2025). **Stories from the Bottom Up: Composable Story Sifting Patterns.** FDG 2025.
- Allen, J.F. (1983). **Maintaining Knowledge about Temporal Intervals.** CACM 26(11).
- Dechter, R., Meiri, I., Pearl, J. (1991). **Temporal Constraint Networks.** AI 49(1-3), 61-95.
- Meiri, I. (1996). **Combining Qualitative and Quantitative Constraints in Temporal Reasoning.** AI 87(1-2), 343-385.
- Drakengren, T., Jonsson, P. (1997). **Eight Maximal Tractable Subclasses of Allen's Algebra with Metric Time.** JAIR 7, 25-45.
- TABGP (2023). **Temporal Graph Pattern Matching via Timed Automata.** VLDB Journal. [Code](https://github.com/amirpouya/TABGP)
