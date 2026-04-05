---
sidebar_position: 3
title: Scoring Matches
---

# Scoring Matches

**Learning objective:** Score and rank pattern matches by surprise using `SurpriseScorer`, `StuScorer`, and `SequentialScorer`.

Prerequisites: familiarity with the engine, pattern evaluation, and the `Match` type. See [Incremental Integration](./incremental-integration) if you need a refresher.

## Step 1: Set up SurpriseScorer

`SurpriseScorer` ranks patterns by how often they fire relative to a baseline expectation. Shannon surprise: `-log2(observed / baseline)`. Higher = more unexpected.

```rust reference file=tests/guides_scoring_matches.rs#step1_surprise
```

`alliance` fires every round -- it will have negative surprise (over-represented vs. 0.5 baseline). `betrayal` fires once out of 10 rounds -- it will have positive surprise (under-represented vs. 0.1 baseline).

## Step 2: Set up StuScorer with property extraction

`StuScorer` scores individual matches by the rarity of their *properties*. Two matches of the same pattern score differently if one involves rarer attributes.

The scorer only does frequency math. You extract properties from match bindings and pass them in as strings.

```rust reference file=tests/guides_scoring_matches.rs#step2_stu
```

Property extraction guidance: use **categorical attributes** (traits, factions, roles, emotional states), not entity IDs. `"actor_faction=rebels"` gives useful signal. `"actor=char_147"` produces near-uniform frequencies where every match looks equally rare.

## Step 3: Try different aggregation modes

```rust reference file=tests/guides_scoring_matches.rs#step3_aggregation
```

| Mode | Formula | Polarity | When to use |
|------|---------|----------|-------------|
| `ArithmeticMean` | `sum(freq) / k` | Lower = more surprising | Default. Balanced sensitivity across all properties. |
| `TfIdf` | `sum(-log2(freq))` | Higher = more surprising | You want total information content. Rare properties dominate via log weighting. |
| `GeometricMean` | `exp(sum(ln(freq)) / k)` | Lower = more surprising | One rare property should pull the entire score down multiplicatively. |
| `Min` | `min(freq)` | Lower = more surprising | Only the single rarest property matters. |

## Step 4: Enable PMI correction

When two properties are correlated (e.g., `faction=rebels` and `location=hideout` always co-occur), their individual rarities get double-counted. PMI correction detects this and discounts the redundant member.

```rust reference file=tests/guides_scoring_matches.rs#step4_pmi
```

Use PMI correction when your properties have known correlations (faction/location, trait/role). Skip it when properties are independent or you have fewer than ~20 observations per pattern -- the pair counts need enough data to be meaningful.

## Step 5: Add sequential surprise

`SequentialScorer` tracks which pattern completed after which, and scores transitions by conditional surprise: `-log2(P(current | previous))`. Higher = more surprising.

```rust reference file=tests/guides_scoring_matches.rs#step5_sequential
```

To integrate with the engine, track the last completed pattern as matches arrive:

```rust reference file=tests/guides_scoring_matches.rs#step5_sequential_integration
```

## Step 6: Combine scores for ranking

The three scorers measure different axes: pattern frequency, property rarity, and transition surprise. Combine them with a weighted sum.

```rust reference file=tests/guides_scoring_matches.rs#step6_combine
```

The `stu_inverted` term flips the default polarity (lower = more surprising) so that all three components share "higher = more surprising" directionality before summing. If you use `StuAggregation::TfIdf`, skip the inversion and use `ss.stu_score` directly.

Print the ranked list:

```rust reference file=tests/guides_scoring_matches.rs#step6_print
```

## Next steps

- [Scoring Reference](../reference/scoring) -- full API details for all three scorers.
- [Narrative Scoring Reference](../reference/narratives) -- thread tracking, tension arcs, and MCTS quality scoring.
