---
sidebar_position: 1
title: Pattern Cookbook
---

# Pattern Cookbook

**Learning objective:** Define and debug complex temporal graph patterns for common scenarios.

Six worked recipes, each showing a problem, the pattern code, a matching graph, a non-matching graph, and the `why_not` output for the non-matching case. All examples use `MemGraph` and `MemValue`.

## Recipe 1: Repeated behavior by the same actor

**Problem:** Find two betrayals by the same impulsive character, with no reconciliation between them.

### Pattern

```rust
use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

let pattern = PatternBuilder::<String, MemValue>::new("two_impulsive_betrayals")
    .stage("e1", |s| s
        .edge("e1", "eventType".into(), MemValue::Str("betray".into()))
        .edge_bind("e1", "actor".into(), "char")
        .edge("char", "trait".into(), MemValue::Str("impulsive".into())))
    .stage("e2", |s| s
        .edge("e2", "eventType".into(), MemValue::Str("betray".into()))
        .edge_bind("e2", "actor".into(), "char"))
    .unless_global(|neg| neg
        .edge("mid", "eventType".into(), MemValue::Str("reconcile".into()))
        .edge_bind("mid", "actor".into(), "char"))
    .build();
```

The variable `"char"` appears in both stages. This forces both betrayals to involve the same actor. The first stage also checks a persistent property (`trait = impulsive`) on that character. The `unless_global` negation blocks the match if the character reconciled at any point between the two betrayals.

### Matching graph

```rust
let mut g = MemGraph::new();
g.add_str("alice", "trait", "impulsive", 0);
g.add_str("ev1", "eventType", "betray", 1);
g.add_ref("ev1", "actor", "alice", 1);
g.add_str("ev2", "eventType", "betray", 3);
g.add_ref("ev2", "actor", "alice", 3);
g.set_time(10);

let mut engine: SiftEngine<MemGraph> = SiftEngine::new();
engine.register(pattern);
let matches = engine.evaluate(&g);
assert_eq!(matches.len(), 1);
assert_eq!(matches[0].bindings["char"], BoundValue::Node("alice".into()));
```

### Non-matching graph

```rust
let mut g = MemGraph::new();
g.add_str("alice", "trait", "cautious", 0);  // not impulsive
g.add_str("ev1", "eventType", "betray", 1);
g.add_ref("ev1", "actor", "alice", 1);
g.add_str("ev2", "eventType", "betray", 3);
g.add_ref("ev2", "actor", "alice", 3);
g.set_time(10);
```

### why_not output

```
Pattern: two_impulsive_betrayals
  Stage "e1": Unmatched
    ?e1 --["eventType"]--> Literal(Str("betray"))
      => matched: false, reason: "?e1 is not bound"
```

The first stage fails because no event has both `eventType = betray` AND an actor with `trait = impulsive`. The `why_not` output reports that `?e1` is not bound because the scan for the first clause does not propagate bindings to subsequent clauses during gap analysis.

## Recipe 2: Violation with exception (negation between)

**Problem:** Find a promise followed by a broken promise by the same person, unless they apologized between the two events.

### Pattern

```rust
let pattern = PatternBuilder::<String, MemValue>::new("broken_promise")
    .stage("e1", |s| s
        .edge("e1", "eventType".into(), MemValue::Str("promise".into()))
        .edge_bind("e1", "actor".into(), "person"))
    .stage("e2", |s| s
        .edge("e2", "eventType".into(), MemValue::Str("break_promise".into()))
        .edge_bind("e2", "actor".into(), "person"))
    .unless_between("e1", "e2", |neg| neg
        .edge("apology", "eventType".into(), MemValue::Str("apologize".into()))
        .edge_bind("apology", "actor".into(), "person"))
    .build();
```

### Matching graph

```rust
let mut g = MemGraph::new();
g.add_str("ev1", "eventType", "promise", 1);
g.add_ref("ev1", "actor", "alice", 1);
g.add_str("ev2", "eventType", "break_promise", 3);
g.add_ref("ev2", "actor", "alice", 3);
g.set_time(10);
// No apology between t=1 and t=3 -> match
```

### Non-matching graph

```rust
let mut g = MemGraph::new();
g.add_str("ev1", "eventType", "promise", 1);
g.add_ref("ev1", "actor", "alice", 1);
g.add_str("ev_apology", "eventType", "apologize", 2);
g.add_ref("ev_apology", "actor", "alice", 2);  // apology at t=2
g.add_str("ev2", "eventType", "break_promise", 3);
g.add_ref("ev2", "actor", "alice", 3);
g.set_time(10);
// Apology at t=2 is between e1 (t=1) and e2 (t=3) -> negated
```

### why_not output

For negation-blocked patterns, `why_not` shows all stages as matched (the positive clauses succeed). The negation is not reported by `why_not` -- it only analyzes positive stages. To debug negation issues, inspect the batch results directly: if `evaluate` returns 0 matches but all stages look correct in `why_not`, a negation window is blocking.

## Recipe 3: Numeric threshold (edge_constrained)

**Problem:** Find a loyalty check event where the loyalty value is below 0.5.

### Pattern

```rust
let pattern = PatternBuilder::<String, MemValue>::new("low_loyalty")
    .stage("e", |s| s
        .edge("e", "eventType".into(), MemValue::Str("loyalty_check".into()))
        .edge_constrained(
            "e",
            "loyalty".into(),
            ValueConstraint::Lt(MemValue::Num(0.5)),
        ))
    .build();
```

### Matching graph

```rust
let mut g = MemGraph::new();
g.add_str("ev1", "eventType", "loyalty_check", 1);
g.add_num("ev1", "loyalty", 0.3, 1);  // 0.3 < 0.5
g.set_time(10);
```

### Non-matching graph

```rust
let mut g = MemGraph::new();
g.add_str("ev1", "eventType", "loyalty_check", 1);
g.add_num("ev1", "loyalty", 0.8, 1);  // 0.8 is NOT < 0.5
g.set_time(10);
```

### why_not output

```
Pattern: low_loyalty
  Stage "e": Unmatched
    ?e --["eventType"]--> Literal(Str("loyalty_check"))
      => matched: false, reason: "?e is not bound"
```

The stage reports unmatched because gap analysis evaluates from an empty binding context. The event exists but the constraint on the second clause (`loyalty < 0.5`) fails.

## Recipe 4: Overlapping events (explicit Allen constraint)

**Problem:** Find a sortie that happened during a siege. Both events have bounded intervals (start and end times).

### Pattern

```rust
let pattern = PatternBuilder::<String, MemValue>::new("sortie_during_siege")
    .stage("siege", |s| s
        .edge("siege", "eventType".into(), MemValue::Str("siege".into())))
    .stage("sortie", |s| s
        .edge("sortie", "eventType".into(), MemValue::Str("sortie".into())))
    .temporal("sortie", AllenRelation::During, "siege")
    .build();
```

The `During` relation means the sortie's interval is entirely contained within the siege's interval.

### Matching graph

```rust
let mut g = MemGraph::new();
g.add_edge_bounded("ev_siege", "eventType", MemValue::Str("siege".into()), 1, 100);
g.add_edge_bounded("ev_sortie", "eventType", MemValue::Str("sortie".into()), 3, 5);
g.set_time(4);  // Both intervals active at t=4
// sortie [3, 5) is During siege [1, 100) -> match
```

Both intervals must be bounded for Allen relation checking to work. The query time (`set_time`) must be within both intervals so the edges are visible.

### Non-matching graph

```rust
let mut g = MemGraph::new();
g.add_edge_bounded("ev_siege", "eventType", MemValue::Str("siege".into()), 1, 4);
g.add_edge_bounded("ev_sortie", "eventType", MemValue::Str("sortie".into()), 3, 7);
g.set_time(3);
// sortie [3, 7) is NOT During siege [1, 4) -- sortie extends past siege
// The Allen relation here is OverlappedBy, not During
```

### why_not output

`why_not` does not currently report explicit temporal constraint failures. It only analyzes stage clauses. If all stages match in `why_not` but `evaluate` returns nothing, check your temporal constraints and interval bounds.

## Recipe 5: Absence detection (unless_after)

**Problem:** Find a promise that was never fulfilled afterward (up to the current time).

### Pattern

```rust
let pattern = PatternBuilder::<String, MemValue>::new("unfulfilled_promise")
    .stage("e1", |s| s
        .edge("e1", "eventType".into(), MemValue::Str("promise".into()))
        .edge_bind("e1", "actor".into(), "person"))
    .unless_after("e1", |neg| neg
        .edge("fulfillment", "eventType".into(), MemValue::Str("fulfill".into()))
        .edge_bind("fulfillment", "actor".into(), "person"))
    .build();
```

`unless_after` creates a negation window from `e1` to "now" (the graph's current time). If a fulfillment event by the same person exists anywhere after the promise, the match is blocked.

### Matching graph

```rust
let mut g = MemGraph::new();
g.add_str("ev1", "eventType", "promise", 1);
g.add_ref("ev1", "actor", "alice", 1);
g.set_time(10);
// No fulfill event by alice after t=1 -> match
```

### Non-matching graph

```rust
let mut g = MemGraph::new();
g.add_str("ev1", "eventType", "promise", 1);
g.add_ref("ev1", "actor", "alice", 1);
g.add_str("ev2", "eventType", "fulfill", 5);
g.add_ref("ev2", "actor", "alice", 5);
g.set_time(10);
// fulfill by alice at t=5, which is after promise at t=1 -> negated
```

Note: `unless_after` is a single-stage pattern with negation. The positive part is just one stage. The result changes over time -- a promise is "unfulfilled" until a fulfillment event arrives.

## Recipe 6: Multi-clause negation (all clauses must match)

**Problem:** Find a start-to-end sequence, negated only if the *same person* left between them. A different person leaving should not block the match.

### Pattern

```rust
let pattern = PatternBuilder::<String, MemValue>::new("kept_promise")
    .stage("e1", |s| s
        .edge("e1", "eventType".into(), MemValue::Str("promise".into()))
        .edge_bind("e1", "actor".into(), "person"))
    .stage("e2", |s| s
        .edge("e2", "eventType".into(), MemValue::Str("fulfill".into()))
        .edge_bind("e2", "actor".into(), "person"))
    .unless_between("e1", "e2", |neg| neg
        .edge("mid", "eventType".into(), MemValue::Str("leave".into()))
        .edge_bind("mid", "actor".into(), "person"))
    .build();
```

The negation has two clauses: `eventType = leave` AND `actor = ?person`. Both must match the same entity within the window. A leave event by a different person satisfies the first clause but fails the second (the actor binding does not match), so the negation does not fire.

### Matching graph (different person leaves)

```rust
let mut g = MemGraph::new();
g.add_str("ev1", "eventType", "promise", 1);
g.add_ref("ev1", "actor", "alice", 1);
g.add_str("ev_leave", "eventType", "leave", 2);
g.add_ref("ev_leave", "actor", "bob", 2);  // bob leaves, not alice
g.add_str("ev2", "eventType", "fulfill", 3);
g.add_ref("ev2", "actor", "alice", 3);
g.set_time(10);
// bob's leave does not block alice's pattern -> match
```

### Non-matching graph (same person leaves)

```rust
let mut g = MemGraph::new();
g.add_str("ev1", "eventType", "promise", 1);
g.add_ref("ev1", "actor", "alice", 1);
g.add_str("ev_leave", "eventType", "leave", 2);
g.add_ref("ev_leave", "actor", "alice", 2);  // alice leaves
g.add_str("ev2", "eventType", "fulfill", 3);
g.add_ref("ev2", "actor", "alice", 3);
g.set_time(10);
// alice's leave at t=2 is between t=1 and t=3, all clauses match -> negated
```

The key insight: negation blocks fire only when ALL clauses in the block are satisfied by the same entity. Partial matches on a negation block (only some clauses match) do not trigger negation. This lets you write precise negation conditions that reference the pattern's bound variables.
