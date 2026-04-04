---
sidebar_position: 4
title: Thinking in Time
---

# Thinking in Time

Most pattern matching works on sequences: event A, then event B, then event C. Fabula works on intervals: event A lasted from tick 1 to tick 5, event B lasted from tick 3 to tick 8. They overlapped.

This distinction matters. Intervals let you express things sequences cannot.

---

## Point events vs. duration events

A point event happens at a single moment: a login, a trade, a sensor reading. Its interval is `[t, ∞)` — it started at time `t` and has no defined end.

A duration event spans a range: a siege lasted from tick 5 to tick 20, a treaty was in effect from turn 10 to turn 45. Its interval is `[start, end)`.

Both are edges in the graph, with different intervals. Fabula handles both.

## Implicit ordering: the default

Every pattern has an implicit temporal constraint: stages must occur in order.

```
stage_1.start < stage_2.start < stage_3.start
```

This is always enforced. You never declare it. You cannot disable it. If two events have the same timestamp, they cannot be in consecutive stages — strict inequality.

For most patterns, implicit ordering is all you need. "Login, then access, then logout" — three stages, implicit ordering handles the time.

## When implicit ordering isn't enough

Implicit ordering only says "before." It cannot express:

- Event A happened **during** event B (containment)
- Events A and B **overlapped** (partial temporal intersection)
- Event A **meets** event B (A ends exactly when B starts)
- Event A **started at the same time as** event B

These require explicit Allen constraints.

## Allen's 13 relations

James Allen (1983) defined 13 mutually exclusive relationships between two bounded intervals. Every pair of intervals has exactly one of these relationships.

Try the [Allen Visualizer](/docs/playground/allen-visualizer) to explore them interactively — dragging intervals is faster than reading a table.

The 7 base relations (plus their inverses) and equality:

| Relation | Meaning | Inverse |
|----------|---------|---------|
| **Before** | A ends before B starts (with gap) | After |
| **Meets** | A ends exactly when B starts (no gap) | Met-by |
| **Overlaps** | A starts before B, they share time, A ends before B | Overlapped-by |
| **During** | A is entirely contained within B | Contains |
| **Starts** | A and B start together, A ends first | Started-by |
| **Finishes** | A and B end together, A starts later | Finished-by |
| **Equals** | A and B have identical start and end | (self-inverse) |

## Using explicit constraints

Add an Allen constraint with the `temporal` keyword:

```
pattern sortie_during_siege {
  stage outer { outer.type = "siege" }
  stage inner { inner.type = "sortie" }
  temporal inner during outer
}
```

The `During` relation means the sortie's interval is entirely contained within the siege's interval. Both start and end of the inner event must fall within the outer event's bounds.

Explicit constraints are **additive** — they layer on top of implicit stage ordering. The pattern above still requires `outer.start < inner.start` (implicit) AND `inner During outer` (explicit).

## Gap constraints

Add metric bounds to Allen relations:

```
temporal e1 before e2 gap 5..10
```

The gap between e1's end and e2's start must be between 5 and 10 ticks. This is STN-style bounded-difference constraint checking — useful for SLA thresholds, timeout detection, and rate limiting.

Gap variants:
- `gap 5..10` — between 5 and 10 ticks
- `gap ..10` — at most 10 ticks
- `gap 5..` — at least 5 ticks

## The cost of intervals

**Same-timestamp events cannot be sequenced.** If two events both start at tick 5, they cannot be in consecutive stages (strict `<` inequality). Workarounds: add sub-tick resolution to your time type, or use batch evaluation where all events are visible simultaneously.

**Open-ended intervals limit relation checking.** Allen's 13 relations are defined between two *bounded* intervals. If either interval is open-ended, only `Before`/`Meets` (via start-time comparison) can be checked. `During`, `Overlaps`, and other geometric relations require bounded intervals.

## When you need explicit constraints

In practice, explicit constraints are rare. Use them when:

- Your domain has **duration events** (battles, negotiations, sessions) and you want to find events that occur within them
- You need **gap constraints** for SLA or timing requirements
- You're porting a process mining query that uses interval relationships

If all you need is "A before B before C," implicit stage ordering handles it. Do not add explicit `Before` constraints — they are redundant.

## Where to go next

- [Allen Visualizer](/docs/playground/allen-visualizer) — Interactive exploration of all 13 relations.
- [Temporal Model](/docs/concepts/temporal-model) — Deeper dive into fabula's temporal semantics.
- [Interval Reference](/docs/reference/interval) — API details for `Interval<T>` and `AllenRelation`.
