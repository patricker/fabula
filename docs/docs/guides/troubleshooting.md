---
sidebar_position: 13
title: Troubleshooting
---

# Troubleshooting

Three scenarios that account for most fabula support questions. For debugging an individual pattern during authoring, see [Debugging Patterns](./debugging-patterns).

## Scenario 1: Pattern matches in batch but not incrementally

**Symptom:** `engine.evaluate(&graph)` returns a match, but the same edges fed via `on_edge_added` never produce `SiftEvent::Completed`.

**Likely causes:**

- Edge arrival order violates implicit temporal ordering -- the trigger edge for stage N arrived before stage N-1 bindings existed.
- `end_tick()` was not called between phases, so negation never settled.
- `ds.now()` did not advance between edges, collapsing temporal comparisons onto a degenerate timestamp.

**Diagnostic steps:**

1. Diff `engine.partial_matches()` between batch and incremental runs on the same inputs to find where incremental gets stuck.
2. Run `why_not(...)` on the missing match for a clause-by-clause breakdown.
3. Confirm `now()` advances monotonically by logging it before each `on_edge_added` call.

**Fix:** Feed edges chronologically by interval start time, call `end_tick()` between simulation phases, and advance `now()` with simulation time. See [Why negation runs before advancement](../concepts/how-the-engine-works#why-negation-runs-before-advancement) and [DataSource Reference](../reference/datasource) for `now()` semantics.

## Scenario 2: Performance regressed after a recent change

**Symptom:** per-edge latency doubled without a workload change. Frame budget is suddenly tight or `cargo bench` shows a new baseline.

**Likely causes:**

- A newly added pattern has high fanout at stage 0, so every incoming edge tries to initiate against it.
- `drain_completed()` is no longer being called, so Complete partial matches accumulate in the engine and inflate fingerprint and dedup costs.
- The MemGraph adapter is being used with more than ~1K edges. MemGraph's batch path is O(E^2); switch to PetGraph at scale.

**Diagnostic steps:**

1. Log `engine.partial_matches().len()` after each tick. A monotonically rising line is the smoking gun.
2. Call `stale_patterns(threshold)` to find patterns that are accumulating PMs but never completing.
3. Re-run `cargo bench -p fabula-bench --bench engine` and diff against the previous baseline to localize the regression.

**Fix:** See the [Performance tuning checklist](./performance#memory) for the standard remediations -- `drain_completed`, `set_pattern_enabled(false)`, deadlines, and adapter choice.

## Scenario 3: Pattern stopped matching after a fabula upgrade

**Symptom:** code still compiles, but a pattern that completed yesterday no longer fires after `cargo update` or a version bump.

**Likely causes:**

- The DSL grammar changed. Check `git log` against `crates/fabula-dsl/src/parser.rs` for keyword or syntax updates.
- Your custom `DataSource`'s `now()` semantics drifted from what the engine expects.
- Negation window semantics were updated. Boundaries are exclusive on the start side to match Winnow; a tightened or relaxed release can flip individual matches.

**Diagnostic steps:**

1. Run `why_not(pattern, graph)` for a clause-by-clause breakdown of which stage now fails.
2. Run the pattern through batch `evaluate` to isolate incremental-specific regressions.
3. Re-read the changelog and DSL reference for grammar or semantics changes.

**Fix:** Update DSL syntax to match the current grammar in [DSL Reference](../reference/dsl), and re-check negation and temporal semantics in [How the Engine Works](../concepts/how-the-engine-works).

## Where to go next

- [Debugging Patterns](./debugging-patterns) -- why_not and batch-vs-incremental workflows.
- [Performance](./performance) -- tuning checklist and benchmarking.
- [How the Engine Works](../concepts/how-the-engine-works) -- 4-phase algorithm and negation priority.
