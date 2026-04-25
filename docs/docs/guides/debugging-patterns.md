---
sidebar_position: 7
title: Debugging Patterns
---

# Debugging Patterns

**Learning objective:** Diagnose and fix unmatched patterns using `why_not` and partial match inspection.

## The debugging workflow

### Step 1: Check batch evaluation first

Start with batch evaluation. It is simpler to reason about because it does not depend on edge arrival order.

```rust reference file=tests/guides_debugging_patterns.rs#step1_batch
```

If batch evaluation returns matches, your pattern and data are correct. If incremental evaluation disagrees, the issue is temporal ordering or edge notification order (skip to Step 4).

### Step 2: Run why_not

`why_not` analyzes each stage of a pattern clause-by-clause and reports what matched and what failed.

```rust reference file=tests/guides_debugging_patterns.rs#step2_why_not
```

`why_not` stops at the first unmatched stage. If stage 1 fails, you will not see analysis for stage 2.

### Step 3: Read the reasons

Each clause analysis includes:

- `description` -- a human-readable description like `?e1 --["eventType"]--> Literal(Str("betray"))`
- `matched` -- whether this clause found matching edges
- `reason` -- if unmatched, explains why: variable not bound, no edges found, target constraint failed, or negated edge exists

### Step 4: Compare batch vs incremental

If batch matches but incremental does not (or vice versa), run both and compare:

```rust reference file=tests/guides_debugging_patterns.rs#step4_compare
```

Discrepancies usually fall into the categories in the failure modes table below.

### Step 5: Inspect partial matches

For incremental debugging, inspect the engine's partial match state:

```rust reference file=tests/guides_debugging_patterns.rs#step5_inspect
```

This shows you exactly where each partial match is stuck.

## Common failure modes

| Symptom | Cause | Fix |
|---|---|---|
| All stages `Unmatched` in `why_not` | `graph.set_time()` is 0 or earlier than edge start times. Edges are not visible at query time. | Set the graph's current time to a value where edges are active. For open-ended edges starting at time T, set time >= T. |
| First stage matched, second says "?var is not bound" | Variable name typo between stages. Stage 1 binds `"person"`, stage 2 references `"preson"`. | Check variable name spelling across all stages. Variable names are plain strings -- no compile-time checking. |
| Batch matches, incremental does not | Edges arrive out of temporal order. Incremental matching requires `stage_N.start < stage_N+1.start` using the edge's interval start time, not the graph's current time. | Ensure edges are fed to `on_edge_added` in chronological order by their interval start time. |
| Incremental matches, batch does not | The graph's `now()` time does not cover all relevant edges. Batch evaluation uses `edges_from(node, label, now)`, so edges that start after `now()` are invisible. | Set `now()` to a time >= the latest edge's start time. |
| Negation blocks unexpectedly | The negation window boundary is exclusive on the start side. An event at the exact same timestamp as the window start is outside the window. But an event one tick after the start is inside. | Check your negation event's timestamp relative to the window boundaries. Window is `(start_exclusive, end_exclusive)`. |
| Pattern matches too many times | Missing variable joins between stages, or no negation to exclude duplicate matches. Two stages with independent variables match the Cartesian product. | Add a shared variable name between stages to create a join. Add negation if you want at-most-one semantics. |
| `SiftEvent::Negated` fires unexpectedly | The negation clause matches an edge you did not expect. | Inspect the `clause_label` and `trigger_source` fields on the `Negated` event to identify which edge triggered it. |
| `why_not` shows all stages matched, but `evaluate` returns 0 | A negation window blocks the match. `why_not` only analyzes positive stages, not negation windows. | Temporarily remove the negation from the pattern and re-run `evaluate`. If it now matches, the negation is the cause. Check the negation window's temporal bounds and clause bindings. |
| Partial match stuck at stage N forever | No edge has arrived that matches stage N's clauses with the existing bindings. | Inspect the partial match's bindings to see what variables are bound. Then check whether any edge in the graph satisfies stage N's clauses with those bindings at the right time. |
| `why_not` returns `None` | The pattern name does not match any registered pattern. | Check for typos in the pattern name string. Pattern names are case-sensitive. |
| Explicit temporal constraint fails silently | One or both intervals are open-ended. Allen relations other than Before/Meets require bounded intervals. | Use `add_edge_bounded` to set both start and end times on the relevant edges. |
| Same edge triggers both `Advanced` and `Negated` | This is correct behavior. Phase 1 (negation) runs on existing partial matches, Phase 2 (initiation) creates new ones. The same edge can kill an old partial match and start a new one. | No fix needed -- this is by design. Filter events by `match_id` if you need to track specific partial matches. |
| `SiftEvent::Expired` fires immediately | `deadline_ticks` is too small relative to tick frequency, or `created_at_tick` is much earlier than expected. Deadline uses strict `>`: a PM with `deadline_ticks=1` expires after 2 ticks (created at tick 0, expired when tick 2 starts). | Increase the deadline value. Inspect `pm.created_at_tick` to verify when the PM was initiated. Remember that `created_at_tick` is inherited on advancement, not reset. |
| PM expires but shouldn't | The PM advanced to a later stage but still expired because `created_at_tick` measures total lifecycle from first initiation, not from last advancement. A 3-stage pattern with `deadline 5` expires 5 ticks after stage 1 matched, even if stage 2 matched at tick 4. | Use a larger deadline that accounts for the full multi-stage lifecycle, or remove the deadline and use `stale_patterns()` for softer staleness detection. |
| `end_tick()` returns empty expired events | Patterns don't have `deadline_ticks` set. Expiry only fires for patterns with an explicit deadline. | Add `.deadline(ticks)` to the pattern builder or `deadline N` in the DSL. |
| Stage with a `let` doesn't match, no error | A `?var` referenced in the let's expression is unbound, the operation is unsupported for the value type (e.g., string + number on `MemValue`), or division by zero. Failures are silent — the stage match fails the same as any unsatisfied clause. | Confirm every `?var` in the let is bound by an earlier clause or stage. For non-numeric `V` types, check that the operation has a meaningful `ArithmeticValue` impl (the in-tree `String` impl returns `None` for everything). See [Computed Bindings — When evaluation fails](./computed-bindings#when-evaluation-fails). |

## Next steps

- [Step-Through Debugger](../playground/step-through) -- watch incremental matching unfold visually, step by step.
- [Pattern Cookbook](./pattern-cookbook) -- worked recipes with `why_not` output for each failure case.
- [Troubleshooting](./troubleshooting) -- common operational issues beyond single-pattern debugging.
