---
sidebar_position: 3
title: Temporal Model
---

# Temporal Model

Fabula uses Allen's interval algebra to reason about time. Every edge in the graph has a time interval -- `[start, end)` for bounded events or `[start, infinity)` for ongoing ones. The engine uses these intervals to enforce ordering between pattern stages and to evaluate explicit temporal constraints.

## Why Allen algebra instead of entity-ID ordering

The original Winnow library determines temporal order by comparing DataScript entity IDs. Events added earlier get lower IDs, so `?eventA < ?eventB` means "A happened before B." This works because DataScript assigns IDs monotonically.

Fabula replaces this with interval-based ordering for three reasons.

**Richer constraints.** Entity-ID ordering can only express "before" and "after." Allen algebra supports 13 relations: before, after, meets, overlaps, during, contains, starts, finishes, equals, and their inverses. This lets you express patterns like "event A happened *during* event B" or "events A and B overlapped" -- constraints that cannot be stated with ID comparison alone.

**Backend independence.** Not every graph store assigns IDs monotonically. By using intervals attached to edges, fabula works with any backing store that can provide a start time and optional end time for each edge.

**Duration awareness.** Events in simulations often have duration. A battle lasts from tick 5 to tick 20. A treaty is in effect from turn 10 to turn 45. Intervals capture this naturally. Point-in-time entity IDs cannot.

## What it costs

**Same-timestamp events cannot be sequenced.** If two events start at the same time (e.g., both at tick 5), the implicit stage ordering rejects them -- stage N requires `start < start` of stage N+1, with strict inequality. This is a fundamental limitation: without sub-tick ordering information, the engine cannot determine which came first.

If your simulation produces events at the same timestamp, you have two options: add sub-tick resolution to your time type (e.g., use `(tick, sequence_number)` tuples), or use batch evaluation where both events are visible simultaneously and the engine can test all permutations.

**Open-ended intervals limit relation checking.** Allen's 13 relations are defined between two bounded intervals. If either interval is open-ended (`end = None`), the `relation()` method returns `None`. For the common case of implicit stage ordering (Before/Meets), the engine falls back to comparing start times. For other Allen relations (During, Overlaps, etc.), you need bounded intervals.

## Implicit stage ordering

Every pattern has an implicit temporal constraint: stages must occur in order. Stage 1 before stage 2 before stage 3. The engine enforces this by requiring that each stage's anchor interval starts strictly before the next stage's anchor interval.

```
stage_1.start < stage_2.start < stage_3.start
```

This uses start times only, not full Allen relations. It works for both bounded and open-ended intervals because every interval has a start time.

You do not declare this constraint. It is always present. You cannot disable it. If you need two stages to have the same timestamp, they must be clauses within a single stage, not separate stages.

## Open-ended intervals

An open-ended interval `[start, infinity)` represents an ongoing event or a persistent fact. A character's trait, a relationship that has not ended, a state that is still active.

Open-ended intervals interact with the engine in two ways:

**Visibility.** An open-ended interval is visible at any query time >= its start. When the engine calls `edges_from(node, label, at)`, an open-ended edge from time 5 is visible at time 5, 10, 100, or any later time.

**Negation windows.** An `unless_after` negation has an open end -- it checks from the start event up to "now" (the graph's current time). An open-ended negation event starting at time 7 is within the window if `7 > window_start` (exclusive start boundary).

## Explicit temporal constraints

Most patterns need only implicit stage ordering. Explicit Allen constraints are for the uncommon case where you need a specific geometric relationship between intervals.

```rust
PatternBuilder::new("during_pattern")
    .stage("outer", |s| s
        .edge("outer", "eventType".into(), "siege".into()))
    .stage("inner", |s| s
        .edge("inner", "eventType".into(), "sortie".into()))
    .temporal("inner", AllenRelation::During, "outer")
    .build();
```

This pattern finds a sortie that happened during a siege. The `During` relation means the sortie's interval is entirely contained within the siege's interval.

Explicit constraints are additive -- they layer on top of implicit stage ordering, not replace it. The pattern above still requires `outer.start < inner.start` (implicit ordering) AND `inner During outer` (explicit constraint). Since `During` implies the inner event starts after the outer event, these are consistent.

For explicit constraints to work, both intervals must be bounded. If either interval is open-ended and the constraint is not Before or Meets, the check fails and the candidate is rejected.

For the full list of Allen relations and their semantics, see the [Interval reference](../reference/interval.md).

## When you need explicit constraints

In practice, explicit constraints are rare. Consider using them when:

- You need to detect events that **overlap** or **contain** each other, not just sequence.
- Your simulation models long-duration events (battles, negotiations, seasons) and you want to find events that occur within them.
- You are porting a process mining query that uses interval relationships.

If all you need is "A before B before C," implicit stage ordering handles it automatically. Do not add explicit Before constraints -- they are redundant with the implicit ordering.
