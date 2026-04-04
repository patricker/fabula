---
sidebar_position: 30
title: Glossary
---

# Glossary

Alphabetical reference of sifting and fabula terminology.

---

**Advancement** — When a partial match progresses to its next stage. See [How the Engine Works](/docs/concepts/how-the-engine-works).

---

**Allen relation** — One of 13 mutually exclusive temporal relationships between two intervals (before, meets, overlaps, during, starts, finishes, equals, and their inverses). See [Temporal Model](/docs/concepts/temporal-model).

---

**Batch evaluation** — Scanning the entire graph for all complete matches in one pass. See [How the Engine Works](/docs/concepts/how-the-engine-works).

---

**Binding** — A variable-to-value assignment within a match, mapping a variable name to a node or value. See [Patterns Reference](/docs/reference/patterns).

---

**Clause** — A single constraint within a stage: an edge match, value constraint, or variable binding. See [DSL Reference](/docs/reference/dsl).

---

**Cold-start** — Insufficient observation data for reliable surprise scoring; confidence weighting attenuates scores toward "unsurprising" until enough data accumulates. See [Scoring Reference](/docs/reference/scoring).

---

**Completion** — When all stages of a pattern have been matched, producing a full set of bindings. See [How the Engine Works](/docs/concepts/how-the-engine-works).

---

**Composition** — Combining patterns using sequence (`>>`), choice (`|`), or repeat (`*`) operators to build complex patterns from reusable parts. See [DSL Reference — Compose](/docs/reference/dsl#compose-syntax).

---

**Concurrent group** — A set of stages that can match in any order, tracked via a bitmask. Also called **unordered group** in the Rust API (`PatternBuilder::unordered_group()`). See [DSL Reference — Concurrent Groups](/docs/reference/dsl#concurrent-groups).

---

**DataSource** — The trait abstracting graph storage backends, with 4 associated types and 6 methods. See [DataSource Reference](/docs/reference/datasource).

---

**Deadline** — Maximum ticks a partial match may remain active before being killed with `SiftEvent::Expired`. See [Engine Reference](/docs/reference/engine).

---

**Edge** — The atomic unit of data: a source node connected to a target (node or value) through a labeled relationship, valid over a time interval. See [Overview](/docs/concepts/overview).

---

**Fingerprint** — A deterministic hash of a partial match's state used for deduplication, preventing identical PMs from accumulating. See [Engine Reference](/docs/reference/engine).

---

**Forking** — Cloning the engine for speculative evaluation; the clone gets independent partial match state while the original is unaffected. See [How the Engine Works](/docs/concepts/how-the-engine-works).

---

**Gap analysis** — Clause-by-clause diagnosis of why a pattern didn't match, via the `why_not` function. See [Debugging Patterns](/docs/guides/debugging-patterns).

---

**Incremental evaluation** — Processing edges one at a time and emitting events as patterns advance, complete, or get negated. See [How the Engine Works](/docs/concepts/how-the-engine-works).

---

**Interval** — A time span `[start, end)` attached to every edge; open-ended intervals use `[start, infinity)` for ongoing events. See [Interval Reference](/docs/reference/interval).

---

**Join** — When a variable appears in multiple stages, forcing the same entity to be bound everywhere the variable is used. See [Overview](/docs/concepts/overview).

---

**Match** — A complete set of bindings satisfying all stages, temporal constraints, and negation windows of a pattern. See [Engine Reference](/docs/reference/engine).

---

**Negation window** — A temporal range where specified events must NOT occur; the match is killed if the negation clauses are satisfied within the window. See [DSL Reference — Negation Windows](/docs/reference/dsl#negation-windows).

---

**Nugget** — Felt's term for a set of variable bindings representing an interesting event sequence; equivalent to fabula's `Match`. See [Research Lineage](/docs/research).

---

**Partial match** — An in-progress match with some stages satisfied, tracked by the engine in incremental mode. See [Engine Reference](/docs/reference/engine).

---

**Pattern** — A named template describing a sequence of events to find, composed of stages, clauses, temporal constraints, and negation windows. See [Patterns Reference](/docs/reference/patterns).

---

**Plant/payoff** — Chekhov's gun tracking: a setup event (plant) that should eventually resolve via a corresponding payoff event. See [Engine Reference](/docs/reference/engine).

---

**PMI** — Pointwise Mutual Information; measures how much more often two properties co-occur than expected by chance, used to correct double-counting in StU scoring. See [Scoring Reference](/docs/reference/scoring).

---

**Sequential surprise** — How unexpected a pattern completion is given the previously completed pattern, scored as `-log2(P(B|A))` from observed transition frequencies. See [Scoring Reference](/docs/reference/scoring).

---

**Sifting** — Automatically identifying interesting event sequences in temporal data by matching ordered subgraph templates with variable joins.

---

**Stage** — An ordered event slot within a pattern, anchored to a named variable and containing one or more clauses. See [Patterns Reference](/docs/reference/patterns).

---

**StU** — "Select the Unexpected" (Kreminski et al., ICIDS 2022); a scoring heuristic that ranks matches by the empirical rarity of their properties. See [Scoring Reference](/docs/reference/scoring).

---

**Surprise** — Shannon surprise (`-log2(p)`) measuring how unexpected a pattern's match frequency is relative to a baseline. See [Scoring Reference](/docs/reference/scoring).

---

**Tick** — One logical time step in incremental evaluation; `end_tick()` finalizes the tick, checks deadlines, and produces a `TickDelta`. See [Engine Reference](/docs/reference/engine).

---

**TypeMapper** — A trait for converting DSL literals to custom label/value types during compilation, enabling the DSL to target arbitrary type systems. See [DSL Reference — TypeMapper](/docs/reference/dsl#typemapper).

---

**Variable** — A named placeholder (`?var` in DSL, string in builder API) that binds to nodes or values during matching; shared variables across stages create joins. See [Overview](/docs/concepts/overview).
