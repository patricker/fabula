---
sidebar_position: 7
title: "6. Speculate with MCTS"
---

# 6. Speculate with MCTS

Chapter 5 scored completed matches. Now you will fork the engine to ask "what if?" before committing to an action. Clone the engine, add a hypothetical event to a separate graph, score the result, and compare alternatives. The original engine is never touched.

## How forking works

`engine.clone()` copies all patterns, partial matches, enabled flags, completion counts, and lifecycle metrics. Tick accumulators are intentionally left empty -- the forked engine starts with a clean slate for its speculative timeline.

Pair the cloned engine with a cloned `MemGraph`. Add your hypothetical event to the cloned graph, call `on_edge_added()` on the cloned engine, and score what happens. The original engine and graph are unaffected.

## Complete code

```rust reference file=tests/build_ch06.rs#speculate_with_mcts
```

## Expected output

```text
=== state at decision point (tick 4) ===
active partial matches: ...
tick counter: 4

=== speculative evaluation (tick 5) ===
A: alice trades ACME
  completed: insider_trading
  surprise: ... bits
B: bob trades ZINC
  completed: insider_trading
  surprise: ... bits
C: carol sells ACME
  completed: pump_and_dump
  surprise: ... bits

best hypothesis: ...

=== original engine (unchanged) ===
active partial matches: ...
tick counter: 4
completed in original: 0
```

Key outcomes:
- **Hypothesis A** (alice trades ACME): the insider_trading pattern was negated by the ACME alert at tick 3, so this may not complete -- or it starts a fresh match depending on engine state. The exact behavior depends on whether the negation killed all active PMs for alice/ACME.
- **Hypothesis B** (bob trades ZINC): completes insider_trading for bob/ZINC (tipped at tick 4, no alert on ZINC).
- **Hypothesis C** (carol sells ACME): completes pump_and_dump (carol bought at tick 1, promoted at tick 2, sells now).
- The original engine has the same number of active partial matches and tick counter as before forking. Zero completions in the original.

## Connection to MCTS

This fork-speculate-score loop is the core of Monte Carlo Tree Search for narrative generation:

1. **Select** a decision point (the simulation reaches a moment where multiple actions are possible).
2. **Expand** by cloning the engine once per candidate action.
3. **Simulate** by calling `on_edge_added()` on each fork with the hypothetical event.
4. **Evaluate** with the scoring pipeline (SurpriseScorer, StuScorer, SequentialScorer, or a composite NarrativeScore from `fabula-narratives`).
5. **Backpropagate** by choosing the best-scoring action and committing it to the real engine.

Each hypothesis is a branch in the search tree. The surprise score is the evaluation function. `engine.clone()` is cheap -- it copies patterns, partial matches, and counters, not the graph. The graph is your simulation state; fork it however your simulation layer requires.

For deeper tree search, nest the loop: commit the best action to a fork, advance its simulation, fork again at the next decision point. The engine's `Clone` is designed for exactly this pattern.

## What you learned

This chapter and the full tutorial:

- **Chapter 1** -- Built a simulation loop producing timestamped events into a MemGraph.
- **Chapter 2** -- Defined patterns with PatternBuilder (insider trading with negation, flash crash with concurrent group, pump-and-dump sequence) and the DSL.
- **Chapter 3** -- Wired the engine into the loop with `on_edge_added()` for incremental matching. Learned the 4-phase algorithm.
- **Chapter 4** -- Handled SiftEvent variants, drained completed matches, used deadlines for expiry, and ran gap analysis.
- **Chapter 5** -- Scored matches with SurpriseScorer (pattern-level), StuScorer (property-level, ArithmeticMean vs TfIdf), and SequentialScorer (transition-level).
- **Chapter 6** -- Forked the engine with `clone()`, evaluated hypothetical actions on separate graphs, scored and compared results, confirmed the original was untouched.

## Where to go next

- [How the Engine Works](/docs/concepts/how-the-engine-works) -- The 4-phase algorithm, forking, deduplication, partial match lifecycle.
- [Scoring and Surprise](/docs/concepts/scoring-and-surprise) -- Information theory foundations for the scoring pipeline.
- [DSL Reference](/docs/reference/dsl) -- Complete syntax reference for patterns, graphs, and compose operators.
- [fabula-narratives](/docs/reference/narratives) -- Composite NarrativeScore combining thread tracking (MICE), tension arcs, pivot detection, and the surprise scorers from this tutorial into a single evaluation function for MCTS.
