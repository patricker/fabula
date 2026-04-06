---
sidebar_position: 1
title: What is Fabula?
---

# What is Fabula?

Fabula is a Rust library for finding temporal patterns in graphs. You describe a sequence of connected events with variables, ordering constraints, and negation windows. Fabula finds every subgraph in your data that matches.

Think of it as a regular expression engine, but instead of matching character sequences in strings, it matches event sequences in temporal graphs.

## When to use it

**Story sifting in games and simulations.** A social simulation produces hundreds of events -- characters meet, betray, reconcile, travel. Fabula finds the narratively interesting subsequences: "two betrayals by the same impulsive character with no reconciliation between them." This is the use case fabula was designed for, following the Felt and Winnow research lineage.

**Simulation monitoring.** Any system that produces timestamped relational events can be sifted. Monitor an agent-based model for emergent behaviors. Detect when a population crosses a threshold after a policy change, with no intervening correction.

**Process mining.** Given a log of business process events with actors and timestamps, find instances of a multi-step process that completed without deviation, or find the ones that deviated and understand where.

**Compliance checking.** Define forbidden event sequences -- "access after revocation with no re-authorization between" -- and check whether they occurred.

## When NOT to use it

Fabula is not a general-purpose graph database or query engine. Specifically:

- **OLAP / aggregation queries.** Fabula finds pattern instances; it does not compute sums, averages, or histograms across the graph.
- **Non-temporal graphs.** Without time intervals on edges, fabula has no way to order stages or define negation windows. If your graph has no notion of time, fabula adds nothing over a regular graph query.
- **Recursive patterns.** Fabula does not support recursive rules (no transitive closure, no "friend of a friend of a friend"). If you need recursion, consider Datalog.
- **Action selection.** Fabula detects patterns; it does not generate events. The original Felt library combined sifting with an action system. Fabula deliberately omits the action side -- your simulation layer handles that. See [Design Decisions](./design-decisions) for why.

## The 5 core concepts

### Edge

An edge connects a source node to a target (another node or a value) through a labeled relationship, valid over a time interval. This is the atomic unit of data in fabula.

```
source --[label]--> target   during [start, end)
```

Edges come from your graph store via the [`DataSource` trait](../reference/datasource). Fabula never owns your data -- it queries it through the trait's six methods.

### Pattern (stages and clauses)

A pattern is a named template describing a sequence of events to find. It contains ordered **stages**, each anchored to an event variable. Each stage has one or more **clauses** that constrain which edges must (or must not) exist at that event.

```rust reference file=tests/concepts_overview.rs#broken_promise
```

Stages are implicitly ordered by time -- stage 1 must occur before stage 2. Try this in the [Pattern Playground](../playground/pattern-playground).

### Variable (joins)

Variables bind to nodes or values during matching. When the same variable appears in multiple clauses or stages, it creates a **join** -- the engine ensures the variable binds to the same entity everywhere.

In the pattern above, `"person"` appears in both stages. This forces both events to involve the same actor. Without the shared variable, the pattern would match any promise followed by any broken promise, even by different people.

### Interval

Every edge has a time interval: `[start, end)` for bounded events, or `[start, infinity)` for ongoing ones. Fabula uses Allen's interval algebra to reason about temporal relationships between events -- before, during, overlaps, and 10 other relations.

Most of the time you do not need explicit temporal constraints. Stage ordering is implicit: stage N must start before stage N+1. Explicit Allen constraints are for cases like "event A must happen *during* event B" (containment, overlap). See [Temporal Model](./temporal-model) for details.

### Negation

A negation window says "these clauses must NOT match between two events." This is how you express exceptions: "hospitality violation, *unless* the guest left between entry and harm."

```rust reference file=tests/concepts_overview.rs#negation_window
```

Negation windows are scoped to a temporal range (between two bound events, after a single event, or globally across the entire pattern). All clauses in a negation block must match the same entity for the negation to fire.

## Beyond matching: scoring and lifecycle

Fabula's core engine finds pattern matches. Two optional layers add higher-level capabilities:

**Surprise scoring** (`fabula::scoring`). Rank matches by how unexpected they are. `SurpriseScorer` uses Shannon surprise relative to baseline frequencies. `StuScorer` uses the StU heuristic to score matches by the rarity of their individual properties (traits, factions, relationship types). Both operate as post-processing -- no engine modification required.

**Narrative scoring** (`fabula-narratives` crate). Scoring signals for MCTS-based narrative management (MCTS -- Monte Carlo Tree Search, a sampling-based planning algorithm that evaluates possible futures by random simulation): thread lifecycle tracking (MICE-style open/close with FILO nesting validation), tension trajectory sampling (rising/falling/plateau), event distribution shift detection (JSD pivot measure), and a composite quality function with configurable weights.

**Pattern metadata and deadlines.** Patterns can carry arbitrary key-value metadata (propagated to matches and events) and an optional tick deadline. Partial matches that exceed their deadline are killed with `SiftEvent::Expired`, providing automatic garbage collection of stale match threads.

All of these are optional. The core engine works fine without them.

## Where to go next

- [How the Engine Works](./how-the-engine-works) -- the 4-phase incremental algorithm
- [Temporal Model](./temporal-model) -- Allen algebra, implicit ordering, open-ended intervals
- [Design Decisions](./design-decisions) -- why fabula is built the way it is
- [Pattern Cookbook](../guides/pattern-cookbook) -- worked recipes for common pattern types
- [Incremental Integration](../guides/incremental-integration) -- wire fabula into a live simulation
- [Scoring Reference](../reference/scoring) -- SurpriseScorer and StuScorer API
- [Narrative Scoring Reference](../reference/narratives) -- thread, tension, pivot, and composite scorer API
- [Research Lineage](../research) -- the Felt and Winnow papers behind fabula
