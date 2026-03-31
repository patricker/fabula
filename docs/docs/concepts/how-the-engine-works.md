---
sidebar_position: 2
title: How the Engine Works
---

# How the Engine Works

Fabula has two evaluation modes: **batch** and **incremental**. Batch evaluation scans the entire graph and returns all complete matches. Incremental evaluation processes one edge at a time and emits events as patterns advance, complete, or get negated.

Both modes use the same pattern representation and the same matching logic. The difference is how they iterate over the data.

## Batch evaluation

Batch evaluation works as a stage cascade with filtering.

**Stage cascade.** The engine processes stages left to right. For stage 1, it scans the graph for all edges matching the first clause, building a set of candidate binding maps. For each subsequent stage, it extends each existing candidate with new bindings from that stage. Candidates that fail a clause are dropped.

This is a nested-loop join. If stage 1 produces 50 candidates and stage 2 produces 3 matches per candidate, you get 150 candidates going into stage 3.

**Temporal filter.** After the stage cascade, the engine filters candidates by temporal ordering. Implicit ordering requires that each stage's anchor interval starts strictly before the next stage's anchor interval. Explicit Allen constraints (if any) are checked against the full interval bounds.

**Negation filter.** Surviving candidates are checked against all negation windows. For each negation, the engine scans the graph for entities that satisfy all the negation's clauses within the temporal window. If any such entity exists, the candidate is eliminated.

The result is a list of complete matches, each with a full binding map.

## Incremental evaluation: the 4-phase algorithm

When you call the engine with a new edge, it runs four phases in order:

### Phase 1: Negation check

Before anything else, the engine checks every active partial match against the new edge. For each partial match with an open negation window (the start event is bound but the end event is not), the engine tests whether the new edge satisfies any negation clause. If all clauses in a negation block are satisfied for the same entity, the partial match is killed.

Negation has priority over advancement. This is deliberate: if the same edge could both advance a partial match and trigger its negation, the negation wins. The partial match is killed before the advance phase runs.

### Phase 2: Initiation

The engine tries to start new partial matches by testing the new edge against the first stage of every registered pattern. For each pattern where the first stage matches, a new partial match is created with the initial bindings and interval.

For single-stage patterns (rare but valid), a match that passes initiation is immediately complete -- but only after checking its negation windows.

### Phase 3: Advancement

The engine tries to advance every existing active partial match. For each partial match, it tests the new edge against the partial match's next unmatched stage. If the edge satisfies all clauses in that stage, and the temporal ordering is valid (the new edge's start time is after all previously matched stages), the engine **forks** a new partial match with the extended bindings.

The key behavior here: **the original partial match survives**. When the engine forks, it creates a copy with the new bindings and advances the copy. The original stays in its current state, waiting for a potentially different edge to match the same stage. This is critical because the same stage can match multiple future events with different bindings.

### Phase 4: Cleanup

Dead partial matches (killed by negation in Phase 1) are removed from storage. New partial matches from Phases 2 and 3 are added.

The engine returns a list of events describing everything that happened: which partial matches advanced, which completed, which were negated.

## Why negation runs before advancement

Consider a pattern with stages A, B, C and a negation window "unless X between A and C." A partial match has completed stages A and B. A new edge arrives that matches both stage C (completing the pattern) and the negation clause X.

If advancement ran first, the engine would emit a completion event. Then negation would kill an already-completed match, which is confusing and potentially wrong -- the caller already acted on the completion.

By running negation first, the partial match is killed before advancement sees it. The completion never happens. This matches the semantics of Winnow's `tryAdvance` function, which checks negation before attempting to match the next event stage.

## Forking and the memory lifecycle

Each partial match has a unique ID, a set of bindings, a set of anchor intervals, and a state (Active, Complete, or Dead). The engine stores all partial matches in a flat list.

When a partial match advances, the engine does not modify it in place. It creates a new partial match with incremented bindings and a new ID. The original stays Active. The new PM inherits the parent's `created_at` timestamp — this records when the match thread was first initiated, not when the latest stage matched. Consumers can use `pm.created_at` to determine how long a partial match has been waiting.

### Deduplication

The engine automatically deduplicates partial matches. Before creating any new PM in Phase 2 or Phase 3, it computes a deterministic fingerprint from `(pattern_idx, next_stage, bindings, intervals)`. If an identical PM already exists in the pool, the duplicate is silently skipped — no PM is created and no event is emitted. This prevents unbounded accumulation when duplicate edges exist in the graph or when the same edge triggers multiple identical match paths.

Two PMs with the same bindings but **different intervals** (different timestamps) are distinct temporal threads and are both kept.

### Memory management

Complete partial matches accumulate until you drain them. Dead partial matches are removed at the end of each `on_edge_added` call.

Call `drain_completed` to harvest complete matches and remove them from the engine's storage. This is the primary memory management mechanism. In a long-running simulation, drain periodically to prevent unbounded growth.

Active partial matches that will never complete (because the simulation has moved past the point where their next stage could match) are not automatically garbage-collected. This is a known trade-off: the engine does not know whether a future edge might still arrive with an earlier timestamp.

## Batch vs incremental: when to use each

**Use batch when** you have a complete dataset and want all matches at once. Batch evaluation does not maintain partial match state, so it uses less memory. It also handles temporal ordering more naturally because it can see all intervals simultaneously.

**Use incremental when** your data arrives over time -- a running simulation, a streaming event log, a game loop. Incremental evaluation lets you react to matches as they happen rather than waiting for the simulation to finish.

**Use both** to debug. If a pattern matches in batch but not incrementally (or vice versa), the discrepancy usually points to a temporal ordering issue or a negation window boundary problem. See [Debugging Patterns](../guides/debugging-patterns.md) for a systematic approach.

You can also use batch evaluation as a correctness oracle: run the same pattern in both modes on the same data and assert the results agree. The golden test suite does exactly this for consistency scenarios.
