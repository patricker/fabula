---
sidebar_position: 6
title: "5. Score and Rank"
---

# 5. Score and Rank

Chapter 4 reacted to events and managed match lifecycle. Now you will score completed matches by how surprising they are, using three scorers: `SurpriseScorer` (pattern-level), `StuScorer` (property-level), and `SequentialScorer` (transition-level). The engine finds matches; the scorers rank them. This separation is deliberate.

## Scorers

- **SurpriseScorer** -- set a baseline probability per pattern, observe each tick, then score. Shannon surprise in bits: positive = rarer than expected, negative = more common.
- **StuScorer** -- observe *properties* extracted from each completed match (categorical attributes, not entity IDs). Score by how rare those properties are. Two aggregation modes shown side by side: `ArithmeticMean` (lower = more surprising) and `TfIdf` (higher = more surprising).
- **SequentialScorer** -- bigram model over pattern transitions. Records which pattern completed after which. Scores transition surprise.

## Complete code

```rust reference file=tests/build_ch05.rs#score_and_rank
```

## Expected output

```text
=== Pattern-Level Surprise (SurpriseScorer) ===
  insider_trading -> ... bits
  pump_and_dump -> ... bits
  insider_trading -> ... bits
  pump_and_dump -> ... bits
  flash_crash -> ... bits

=== Property-Level Surprise (StuScorer) ===
match                   ArithMean        TfIdf
----------------------------------------------
insider_trading            ...            ...
  rarest: ...
pump_and_dump              ...            ...
  rarest: ...
insider_trading            ...            ...
  rarest: ...
pump_and_dump              ...            ...
  rarest: ...
flash_crash                ...            ...
  rarest: ...

=== Sequential Surprise ===
  insider_trading -> pump_and_dump : ... bits
  pump_and_dump -> insider_trading : ... bits
  insider_trading -> pump_and_dump : ... bits
  pump_and_dump -> flash_crash : ... bits

total completed: 5
```

Exact numbers depend on Laplace smoothing and observation counts. The structure matters: pattern-level scores show how often each pattern fires vs. baseline; StU scores rank by property rarity with two different aggregation modes; sequential scores show transition surprise between consecutive completions.

## What you learned

- **SurpriseScorer** -- `set_baseline()` per pattern index, `observe_events()` + `tick()` each step, `score()` to rank. Positive bits = rarer than expected.
- **StuScorer** -- `observe_one(pattern, properties)` per completed match, `score()` with pre-extracted properties. Lower = more surprising for `ArithmeticMean`; higher = more surprising for `TfIdf`.
- **StuAggregation** -- `ArithmeticMean` (original StU), `TfIdf` (log-weighted, reversed polarity), `GeometricMean`, `Min`. Same data, different theories of surprise.
- **SequentialScorer** -- `observe_transition(prev, current)` records what followed what. `score_transition()` returns conditional surprise in bits.
- **Property extraction** -- use categorical attributes (`suspect_role=trader`), not entity IDs. IDs produce uniform frequencies and make everything equally "surprising."

[Next: Speculate with MCTS ->](speculate-with-mcts)
