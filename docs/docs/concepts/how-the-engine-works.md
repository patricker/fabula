---
sidebar_position: 3
title: How the Engine Works
---

# How the Engine Works

Fabula has two evaluation modes: **batch** and **incremental**. Batch evaluation scans the entire graph and returns all complete matches. Incremental evaluation processes one edge at a time and emits events as patterns advance, complete, or get negated. For the full API, see the [Engine Reference](../reference/engine).

Both modes use the same pattern representation and the same matching logic. The difference is how they iterate over the data.

## Batch evaluation

Batch evaluation works as a stage cascade with filtering.

**Stage cascade.** The engine processes stages left to right. For stage 1, it scans the graph for all edges matching the first clause, building a set of candidate binding maps. For each subsequent stage, it extends each existing candidate with new bindings from that stage. Candidates that fail a clause are dropped.

After a stage's clauses match, any [`let` bindings](../guides/computed-bindings) attached to that stage are evaluated against the current binding map. A failed evaluation (unbound variable, type mismatch, division by zero) silently drops the candidate, the same as a failed clause. Successful let results merge into the binding map and are visible to subsequent stages, negations, and lets.

This is a nested-loop join. If stage 1 produces 50 candidates and stage 2 produces 3 matches per candidate, you get 150 candidates going into stage 3.

**Temporal filter.** After the stage cascade, the engine filters candidates by temporal ordering. Implicit ordering requires that each stage's anchor interval starts strictly before the next stage's anchor interval. Explicit Allen constraints (if any) are checked against the full interval bounds.

**Negation filter.** Surviving candidates are checked against all negation windows. For each negation, the engine scans the graph for entities that satisfy all the negation's clauses within the temporal window. If any such entity exists, the candidate is eliminated.

The result is a list of complete matches, each with a full binding map.

## Incremental evaluation: the 4-phase algorithm

When you call the engine with a new edge, it runs four phases in order. See this in action with the [Step-Through Debugger](../playground/step-through).

### Phase 1: Negation check

Before anything else, the engine checks every active partial match against the new edge. For each partial match with an open negation window (the start event is bound but the end event is not), the engine tests whether the new edge satisfies any negation clause. If all clauses in a negation block are satisfied for the same entity, the partial match is killed.

Negation has priority over advancement. This is deliberate: if the same edge could both advance a partial match and trigger its negation, the negation wins. The partial match is killed before the advance phase runs.

### Phase 2: Initiation

The engine tries to start new partial matches by testing the new edge against the first stage of every registered pattern. For each pattern where the first stage matches, a new partial match is created with the initial bindings and interval.

For single-stage patterns (rare but valid), a match that passes initiation is immediately complete -- but only after checking its negation windows.

**Unordered groups:** If stage 0 is part of a [concurrent group](/docs/reference/dsl#concurrent-groups), the engine tries ALL stages in that group as potential initiators, not just stage 0. Each successful initiation creates a PM with the corresponding bit set in `matched_stages`.

### Phase 3: Advancement

The engine tries to advance every existing active partial match. For each partial match, it tests the new edge against the partial match's next unmatched stage. If the edge satisfies all clauses in that stage, and the temporal ordering is valid (the new edge's start time is after all previously matched stages), the engine **forks** a new partial match with the extended bindings.

**Unordered groups:** When a PM's next stage is in a concurrent group, the engine tries all unmatched stages in that group (checked via the `matched_stages` bitmask), not just the next sequential stage. Temporal ordering is relaxed within the group — stages in the same concurrent group are not required to be time-ordered relative to each other. When all bits for the group are set, the PM advances past the group to the next sequential stage.

The key behavior here: **the original partial match survives**. When the engine forks, it creates a copy with the new bindings and advances the copy. The original stays in its current state, waiting for a potentially different edge to match the same stage. This is critical because the same stage can match multiple future events with different bindings.

**Repeat-range looping**: For patterns created with `compose::repeat_range()` (DSL `* N..M` or `* N..`), when a PM advances past the last stage of the looping segment, the engine creates a completion snapshot (if min repetitions are met) AND a new Active PM that loops back to the start of the segment with incremented `repetition_count`. Non-shared bindings from the looping segment are cleared so stage anchors can match fresh events. Intervals are preserved for temporal ordering between iterations.

### Fork vs consume: the `advance_in_place` tradeoff

By default, when a PM advances from stage S to stage S+1, the engine clones it: the new stage-(S+1) PM is added, and the original stage-S PM stays alive so future edges can still match stage S with the same prefix. This "original survives" invariant is what lets a single prefix spawn multiple distinct matches -- one "enter" can match several later "leave" events, producing one completed match per pairing.

For patterns where that multiplicity is unwanted -- the typical case when sifting "the betrayal after the grudge" style narratives -- the cloned originals accumulate as stage-N PMs that are never useful and never cleaned up until the pattern terminates. In a 200-actor crowd, 200 enters followed by 200 leaves produce roughly 40,000 PMs with the default behavior.

Setting [`advance_in_place`](../reference/patterns#advance_in_place) on a pattern opts the engine into a simpler rule: after a PM advances strictly forward, the original is marked Dead and cleaned up at the end of the tick. The Complete and Advanced events still fire as usual. Only the PM table is smaller.

The Winnow paper (Kreminski 2021 §4.2) identifies this as a common sifting optimization once all non-event variables are bound. Fabula exposes it as a per-pattern opt-in rather than an automatic detection, because the tradeoff is author intent: occasionally you DO want multiple forward matches from one prefix.

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

Active partial matches that will never complete (because the simulation has moved past the point where their next stage could match) are not automatically garbage-collected unless the pattern has a deadline. This is a known trade-off: the engine does not know whether a future edge might still arrive with an earlier timestamp.

### Deadline expiry

Patterns can declare a `deadline_ticks` — the maximum number of ticks a partial match may remain active before being killed. The expiry scan runs inside `end_tick()`, after the tick counter is incremented but before the delta is built:

1. For each active PM, check if its pattern has a `deadline_ticks`.
2. If `current_tick - pm.created_at_tick > deadline_ticks`, kill the PM (mark Dead) and emit a `SiftEvent::Expired`.
3. Dead PMs are removed from the pool. Their pattern names are added to `TickDelta.expired`.

The `created_at_tick` is set when a PM is first initiated (Phase 2) and inherited unchanged when the PM advances (Phase 3). This means the deadline measures the total lifecycle of a match thread from its first initiation, not from its most recent advancement. A 3-stage pattern with a 10-tick deadline will expire 10 ticks after stage 1 matched, regardless of when stages 2 or 3 matched.

`end_tick()` returns `(TickDelta, Vec<SiftEvent>)` — the second element contains the full `Expired` events with bindings, stage reached, and elapsed ticks, so callers can inspect which partial matches timed out and how far they got.

## Batch vs incremental: when to use each

**Use batch when** you have a complete dataset and want all matches at once. Batch evaluation does not maintain partial match state, so it uses less memory. It also handles temporal ordering more naturally because it can see all intervals simultaneously.

**Use incremental when** your data arrives over time -- a running simulation, a streaming event log, a game loop. Incremental evaluation lets you react to matches as they happen rather than waiting for the simulation to finish.

**Use both** to debug. If a pattern matches in batch but not incrementally (or vice versa), the discrepancy usually points to a temporal ordering issue or a negation window boundary problem. See [Debugging Patterns](../guides/debugging-patterns) for a systematic approach.

You can also use batch evaluation as a correctness oracle: run the same pattern in both modes on the same data and assert the results agree. The golden test suite does exactly this for consistency scenarios.

:::note Stages are patterns, not sequences
Fabula stages are like CEP *patterns*, not *sequences*: other events can occur between matched stages. Stage 1 at time 1 and stage 2 at time 5 is a valid match even if hundreds of unrelated events happened at times 2, 3, and 4. If you need contiguity (no intervening events of a certain type), use a negation window.
:::

## Partial match lifecycle

Every partial match follows this state machine:

```
                 ┌─── [Negated] (Phase 1: negation clause satisfied)
                 │
[Created] → [Active] ──→ [Complete] (all stages matched)
                 │
                 └─── [Expired] (end_tick: deadline exceeded)
```

- **Created → Active**: Phase 2 (initiation) creates the PM with stage 0 matched.
- **Active → Active**: Phase 3 (advancement) forks the PM. The original stays Active; the fork has one more stage matched. Both are Active.
- **Active → Complete**: Phase 3 advances to the final stage. The PM is marked Complete and a `SiftEvent::Completed` is emitted.
- **Active → Negated**: Phase 1 finds an edge that satisfies all clauses in a negation window. The PM is marked Dead and a `SiftEvent::Negated` is emitted.
- **Active → Expired**: `end_tick()` finds that `current_tick - created_at_tick > deadline_ticks`. The PM is marked Dead and a `SiftEvent::Expired` is emitted.

Complete PMs accumulate until you call `drain_completed()`. Dead PMs are removed at the end of each `on_edge_added()` call. Active PMs live until they advance, get negated, or expire.

## Worked example: the Winnow walkthrough

The canonical incremental matching example, adapted from the Winnow paper (AIIDE 2021). Pattern: violation of hospitality (enter town → show hospitality → harm, unless the guest leaves).

| Step | Event | Pool change | Active PMs |
|------|-------|-------------|------------|
| 0 | (initial) | 1 empty PM seed | 1 |
| 1 | Yann enters town | PM advances to stage 1 (accept) | 1 |
| 2 | Mia does something irrelevant | No change — edge doesn't match any stage | 1 |
| 3 | Eve shows hospitality to Yann | PM forks: original stays at stage 1, fork advances to stage 2 | 2 |
| 4 | Eve harms Yann | Fork completes: `SiftEvent::Completed` with `{guest: Yann, host: Eve}` | 1 + 1 complete |
| 5 | Jake shows hospitality to Yann | New fork from the stage-1 PM | 2 |
| 6 | Yann leaves town | All PMs with `guest=Yann` and an open negation window are killed | 0 |
| 7 | Jake harms Yann | No active PMs to advance — Yann left | 0 |

Key observations:
- **Step 3**: The original PM survives alongside the fork. If a different character showed hospitality later, it could still match.
- **Step 4**: Completion happens *because* negation (Phase 1) didn't fire — Yann hadn't left yet.
- **Step 6**: Negation kills both remaining PMs simultaneously. The leave event satisfies the `unless between` clause for all PMs where Yann is the guest.
- **Step 7**: Even though Jake harms Yann, there are no active PMs left to advance. The narrative thread was closed by the departure.
