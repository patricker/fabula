---
sidebar_position: 1
title: Pattern Cookbook
---

# Pattern Cookbook

**Learning objective:** Define and debug complex temporal graph patterns for common scenarios.

Eight worked recipes, each showing a problem, the pattern code, and usage guidance. All examples use `MemGraph` and `MemValue`.

## Recipe 1: Repeated behavior by the same actor

**Problem:** Find two betrayals by the same impulsive character, with no reconciliation between them.

### Pattern

```rust reference file=tests/guides_pattern_cookbook.rs#r1_pattern
```

The variable `"char"` appears in both stages. This forces both betrayals to involve the same actor. The first stage also checks a persistent property (`trait = impulsive`) on that character. The `unless_global` negation blocks the match if the character reconciled at any point between the two betrayals.

### Matching graph

```rust reference file=tests/guides_pattern_cookbook.rs#r1_matching
```

### Non-matching graph

```rust reference file=tests/guides_pattern_cookbook.rs#r1_non_matching
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

```rust reference file=tests/guides_pattern_cookbook.rs#r2_pattern
```

### Matching graph

```rust reference file=tests/guides_pattern_cookbook.rs#r2_matching
```

### Non-matching graph

```rust reference file=tests/guides_pattern_cookbook.rs#r2_non_matching
```

### why_not output

For negation-blocked patterns, `why_not` shows all stages as matched (the positive clauses succeed). The negation is not reported by `why_not` -- it only analyzes positive stages. To debug negation issues, inspect the batch results directly: if `evaluate` returns 0 matches but all stages look correct in `why_not`, a negation window is blocking.

## Recipe 3: Numeric threshold (edge_constrained)

**Problem:** Find a loyalty check event where the loyalty value is below 0.5.

### Pattern

```rust reference file=tests/guides_pattern_cookbook.rs#r3_pattern
```

### Matching graph

```rust reference file=tests/guides_pattern_cookbook.rs#r3_matching
```

### Non-matching graph

```rust reference file=tests/guides_pattern_cookbook.rs#r3_non_matching
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

```rust reference file=tests/guides_pattern_cookbook.rs#r4_pattern
```

The `During` relation means the sortie's interval is entirely contained within the siege's interval.

### Matching graph

```rust reference file=tests/guides_pattern_cookbook.rs#r4_matching
```

Both intervals must be bounded for Allen relation checking to work. The query time (`set_time`) must be within both intervals so the edges are visible.

### Non-matching graph

```rust reference file=tests/guides_pattern_cookbook.rs#r4_non_matching
```

### why_not output

`why_not` does not currently report explicit temporal constraint failures. It only analyzes stage clauses. If all stages match in `why_not` but `evaluate` returns nothing, check your temporal constraints and interval bounds.

## Recipe 5: Absence detection (unless_after)

**Problem:** Find a promise that was never fulfilled afterward (up to the current time).

### Pattern

```rust reference file=tests/guides_pattern_cookbook.rs#r5_pattern
```

`unless_after` creates a negation window from `e1` to "now" (the graph's current time). If a fulfillment event by the same person exists anywhere after the promise, the match is blocked.

### Matching graph

```rust reference file=tests/guides_pattern_cookbook.rs#r5_matching
```

### Non-matching graph

```rust reference file=tests/guides_pattern_cookbook.rs#r5_non_matching
```

Note: `unless_after` is a single-stage pattern with negation. The positive part is just one stage. The result changes over time -- a promise is "unfulfilled" until a fulfillment event arrives.

## Recipe 6: Multi-clause negation (all clauses must match)

**Problem:** Find a start-to-end sequence, negated only if the *same person* left between them. A different person leaving should not block the match.

### Pattern

```rust reference file=tests/guides_pattern_cookbook.rs#r6_pattern
```

The negation has two clauses: `eventType = leave` AND `actor = ?person`. Both must match the same entity within the window. A leave event by a different person satisfies the first clause but fails the second (the actor binding does not match), so the negation does not fire.

### Matching graph (different person leaves)

```rust reference file=tests/guides_pattern_cookbook.rs#r6_matching
```

### Non-matching graph (same person leaves)

```rust reference file=tests/guides_pattern_cookbook.rs#r6_non_matching
```

The key insight: negation blocks fire only when ALL clauses in the block are satisfied by the same entity. Partial matches on a negation block (only some clauses match) do not trigger negation. This lets you write precise negation conditions that reference the pattern's bound variables.

## Recipe 7: Cross-stage value comparison (escalation)

**Problem:** Detect when a price increases between two orders — "price_B > price_A."

### Pattern

```rust reference file=tests/guides_pattern_cookbook.rs#r7_pattern
```

Stage 1 binds the price to `?base_price`. Stage 2 uses `edge_gt_var` to require the second price is strictly greater. The engine resolves `GtVar("base_price")` to `Gt(value)` using the PM's bindings at match time.

### DSL equivalent

```fabula reference file=dsl/cookbook/escalating_price.fabula
```

All five comparison operators work: `> ?var`, `< ?var`, `>= ?var`, `<= ?var`, `= ?var`. The `= ?var` form compares the edge value against the bound variable's value — it is not a binding (use `-> ?var` for that).

### Range check (two constraints)

Combine two cross-stage constraints for range checks:

```rust reference file=tests/guides_pattern_cookbook.rs#r7_range_check
```

This matches when `low < value < high`, where `low` and `high` were bound in a prior stage.

## Recipe 8: Threshold detection with repeat range

**Problem:** Detect 3 or more failed logins from the same account — brute force detection.

### Pattern

```rust reference file=tests/guides_pattern_cookbook.rs#r8_pattern
```

`repeat_range` with `min=3, max=None` means "3 or more total occurrences." The `account` variable is shared across all iterations, so only failures for the same account are counted.

### DSL equivalent

```fabula reference file=dsl/cookbook/brute_force.fabula
```

### What you get in the match

- `first_e`, `first_account` — the first failed login (where the attack started)
- `last_e`, `last_account` — the most recent failed login
- `account` — the shared target account (same across all iterations)
- `repetition_count` on the `PartialMatch` — total number of matched occurrences

### Bounded range

For "between 3 and 5 attempts" (stop tracking after 5):

```fabula reference file=dsl/cookbook/brute_force_bounded.fabula
```

### Exact count (unchanged)

For exactly 3 attempts (fully unrolled, distinct per-repetition bindings):

```fabula reference file=dsl/cookbook/brute_force_exact.fabula
```

## Recipe 9: Concurrent signals (unordered stages)

**Problem:** Detect when multiple signals occur in any order before a confirmation. A sensor triggers an alarm, then both a temperature spike AND a pressure drop happen (in either order), then a shutdown occurs.

### DSL

```fabula reference file=dsl/cookbook/multi_signal_shutdown.fabula
```

Stages `e2` and `e3` are in a `concurrent { }` block — they can match in any order. The shared variable `?sensor` ensures all stages refer to the same sensor. Stage `e1` must come before both concurrent stages, and `e4` must come after both.

### Builder API equivalent

```rust reference file=tests/guides_pattern_cookbook.rs#r9_pattern
```

Note: `unless_between` cannot use two anchors that are both inside the same concurrent group (undefined temporal ordering). The compiler rejects this at compile time.

## Next steps

- [Pattern Playground](../playground/pattern-playground) -- try these recipes interactively in the browser without a Rust project.
- [Composing Patterns](./composing-patterns) -- build complex patterns from reusable parts with sequence, choice, and repeat.
- [Incremental Integration](./incremental-integration) -- wire patterns into a live simulation loop.
- [Scoring Reference](../reference/scoring) -- rank matches by surprise or narrative quality after evaluation.
- [Pattern Reference](../reference/patterns) -- full API details for `Pattern`, `Stage`, `Clause`, and `PatternBuilder`.
