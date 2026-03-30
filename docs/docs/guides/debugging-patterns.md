---
sidebar_position: 3
title: Debugging Patterns
---

# Debugging Patterns

**Learning objective:** Diagnose and fix unmatched patterns using `why_not` and partial match inspection.

## The debugging workflow

### Step 1: Check batch evaluation first

Start with batch evaluation. It is simpler to reason about because it does not depend on edge arrival order.

```rust
let matches = engine.evaluate(&graph);
println!("Batch matches: {}", matches.len());
```

If batch evaluation returns matches, your pattern and data are correct. If incremental evaluation disagrees, the issue is temporal ordering or edge notification order (skip to Step 4).

### Step 2: Run why_not

`why_not` analyzes each stage of a pattern clause-by-clause and reports what matched and what failed.

```rust
if let Some(analysis) = engine.why_not(&graph, "my_pattern") {
    println!("Pattern: {}", analysis.pattern);
    for stage in &analysis.stages {
        println!("  Stage '{}': {:?}", stage.anchor, stage.status);
        for clause in &stage.clauses {
            println!("    {} => matched: {}, reason: {:?}",
                     clause.description, clause.matched, clause.reason);
        }
    }
}
```

`why_not` stops at the first unmatched stage. If stage 1 fails, you will not see analysis for stage 2.

### Step 3: Read the reasons

Each clause analysis includes:

- `description` -- a human-readable description like `?e1 --["eventType"]--> Literal(Str("betray"))`
- `matched` -- whether this clause found matching edges
- `reason` -- if unmatched, explains why: variable not bound, no edges found, target constraint failed, or negated edge exists

### Step 4: Compare batch vs incremental

If batch matches but incremental does not (or vice versa), run both and compare:

```rust
let batch_matches = engine.evaluate(&graph);
let completed = engine.drain_completed();
println!("Batch: {}, Incremental: {}", batch_matches.len(), completed.len());
```

Discrepancies usually fall into the categories in the failure modes table below.

### Step 5: Inspect partial matches

For incremental debugging, inspect the engine's partial match state:

```rust
for pm in engine.partial_matches() {
    println!("Match #{}: pattern_idx={}, next_stage={}, state={:?}",
             pm.id, pm.pattern_idx, pm.next_stage, pm.state);
    for (var, val) in &pm.bindings {
        println!("  {} = {:?}", var, val);
    }
    for (anchor, iv) in &pm.intervals {
        println!("  {} at {}", anchor, iv);
    }
}
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
