---
sidebar_position: 1
title: Narrative Sifting
description: Detect narrative patterns in games and social simulations
---

# Narrative Sifting

import PatternPlayground from '@site/src/components/wasm/PatternPlayground';

Social simulations produce hundreds of events -- characters meet, betray, reconcile, travel. Most are mundane. Sifting finds the narratively interesting subsequences: arcs, violations, escalations.

This tutorial builds three patterns that a game's narrative manager might run every tick to surface story-worthy moments and rank them by surprise.

| | |
|---|---|
| **Time** | ~15 minutes |
| **Prerequisites** | [What is Sifting?](/docs/learn/what-is-sifting) |

---

## 1. Violation of Hospitality

The canonical sifting example from Kreminski's Winnow paper. A guest enters town, a host shows hospitality, then the host harms the guest -- unless the guest left between entry and harm. The "unless" clause is what makes this a sifting pattern rather than a simple sequence query.

<PatternPlayground
  defaultPattern={`pattern hospitality_violation {
  stage e1 {
    e1.type = "enter_town"
    e1.actor -> ?guest
  }
  stage e2 {
    e2.type = "show_hospitality"
    e2.actor -> ?host
    e2.target -> ?guest
  }
  stage e3 {
    e3.type = "harm"
    e3.actor -> ?host
    e3.target -> ?guest
  }
  unless between e1 e3 {
    mid.type = "leave_town"
    mid.actor -> ?guest
  }
}`}
  defaultGraph={`graph {
  @1 e1.type = "enter_town"
  @1 e1.actor -> alice

  @2 e2.type = "show_hospitality"
  @2 e2.actor -> bob
  @2 e2.target -> alice

  @3 e3.type = "harm"
  @3 e3.actor -> bob
  @3 e3.target -> alice

  @1 e4.type = "enter_town"
  @1 e4.actor -> carol

  @2 e5.type = "show_hospitality"
  @2 e5.actor -> dave
  @2 e5.target -> carol

  @3 e6.type = "leave_town"
  @3 e6.actor -> carol

  @4 e7.type = "harm"
  @4 e7.actor -> dave
  @4 e7.target -> carol

  now = 10
}`}
  compact
/>

**Result:** 1 match -- Bob showed hospitality to Alice then harmed her. Dave also showed hospitality to Carol then harmed her, but Carol left town at t=3, so `unless between e1 e3` kills that match.

:::tip[What to notice]
The negation window spans `e1` to `e3` (entry to harm), not `e2` to `e3` (hospitality to harm). A guest who leaves *before* receiving hospitality but returns later would not trigger the exception. Choose your window boundaries deliberately.
:::

---

## 2. Escalating Conflict

Two acts of aggression between the same pair, with the second more severe than the first. The cross-stage comparison `e2.severity > ?sev` enforces escalation -- same-severity or de-escalating conflicts do not match. Reconciliation between the two acts kills the match.

<PatternPlayground
  defaultPattern={`pattern escalating_conflict {
  stage e1 {
    e1.type = "aggression"
    e1.actor -> ?aggressor
    e1.target -> ?victim
    e1.severity -> ?sev
  }
  stage e2 {
    e2.type = "aggression"
    e2.actor -> ?aggressor
    e2.target -> ?victim
    e2.severity > ?sev
  }
  unless between e1 e2 {
    mid.type = "reconcile"
    mid.actor -> ?aggressor
    mid.target -> ?victim
  }
}`}
  defaultGraph={`graph {
  @1 e1.type = "aggression"
  @1 e1.actor -> macbeth
  @1 e1.target -> duncan
  @1 e1.severity = 2

  @3 e2.type = "aggression"
  @3 e2.actor -> macbeth
  @3 e2.target -> duncan
  @3 e2.severity = 5

  @2 e3.type = "aggression"
  @2 e3.actor -> banquo
  @2 e3.target -> macbeth
  @2 e3.severity = 3

  @4 e4.type = "reconcile"
  @4 e4.actor -> banquo
  @4 e4.target -> macbeth

  @5 e5.type = "aggression"
  @5 e5.actor -> banquo
  @5 e5.target -> macbeth
  @5 e5.severity = 7

  now = 10
}`}
  compact
/>

**Result:** 1 match -- Macbeth insulted Duncan (severity 2) then attacked him (severity 5). Banquo also escalated against Macbeth (3 to 7), but the reconciliation at t=4 kills that match.

:::tip[What to notice]
`e2.severity > ?sev` is a cross-stage comparison: the value bound to `?sev` in stage 1 becomes the threshold for stage 2. This is how you express "more severe than before" without hardcoding levels.
:::

---

## 3. Broken Promise with Surprise Scoring

The broken promise pattern from [Sifting by Example](/docs/learn/sifting-by-example) detects when a character promises then betrays with no fulfillment between. Run it first:

<PatternPlayground
  defaultPattern={`pattern broken_promise {
  stage e1 {
    e1.type = "promise"
    e1.actor -> ?char
    e1.target -> ?recipient
  }
  stage e2 {
    e2.type = "betray"
    e2.actor -> ?char
    e2.target -> ?recipient
  }
  unless between e1 e2 {
    mid.type = "fulfill"
    mid.actor -> ?char
    mid.target -> ?recipient
  }
}`}
  defaultGraph={`graph {
  @1 e1.type = "promise"
  @1 e1.actor -> macbeth
  @1 e1.target -> duncan

  @3 e2.type = "betray"
  @3 e2.actor -> macbeth
  @3 e2.target -> duncan

  @2 e3.type = "promise"
  @2 e3.actor -> lady_macbeth
  @2 e3.target -> duncan

  @5 e4.type = "betray"
  @5 e4.actor -> lady_macbeth
  @5 e4.target -> duncan

  now = 10
}`}
  compact
/>

**Result:** 2 matches -- both Macbeth and Lady Macbeth broke their promises to Duncan. But are both equally interesting? That depends on context.

### Ranking by surprise

Finding matches is step one. A narrative manager also needs to rank them. `SurpriseScorer` computes Shannon surprise: patterns that fire often score low; rare patterns score high.

```rust
use fabula::scoring::SurpriseScorer;

let mut scorer = SurpriseScorer::new();

// Baseline: broken promises happen ~30% of rounds in this simulation
scorer.set_baseline(broken_promise_idx, 0.3);

// Run 20 rounds of simulation, observing matches each round
for round in 0..20 {
    let matches = engine.evaluate(&graph);
    scorer.observe(&matches, engine.patterns());
    simulation.advance_one_tick(&mut graph);
}

// Score the latest round's matches
let scored = scorer.score(&matches, engine.patterns());
for m in &scored {
    println!("{}: surprise = {:.2} bits", m.pattern, m.surprise);
}
```

If broken promises fire in 18 of 20 rounds, surprise is low (~0.2 bits). If they fire in 2 of 20 rounds, surprise is high (~3.9 bits). A betrayal by a consistently loyal character in a peaceful simulation is genuinely surprising; the same betrayal in a war simulation is background noise.

For property-level surprise -- scoring *which* character betrayed *whom* rather than just whether any betrayal happened -- use `StuScorer` with extracted properties like `actor_trait=loyal` or `target_role=king`. See the [Scoring Reference](/docs/reference/scoring) for the full API.

---

## Putting it together

A narrative manager registers all three patterns, runs them each tick, and surfaces the top-scoring matches to the player or AI director:

1. **Detect** -- `engine.on_edge_added()` feeds simulation events incrementally
2. **Complete** -- `engine.end_tick()` finalizes the tick and produces a `TickDelta`
3. **Score** -- `SurpriseScorer` ranks completed matches by unexpectedness
4. **Surface** -- highest-scoring matches become dialogue hints, camera focus, or journal entries

For MCTS-driven narrative management, the `fabula-narratives` crate adds thread tracking (MICE quotient open/close pairs), tension arc classification (rising/falling/plateau/peak/valley), and pivot detection (Jensen-Shannon divergence between tick distributions). These feed into a composite scorer that evaluates candidate actions by their narrative quality. See the [Narrative Scoring Reference](/docs/reference/narratives).

## Where to go next

- [Pattern Cookbook](/docs/guides/pattern-cookbook) -- More pattern recipes with matching and non-matching graphs
- [Scoring Reference](/docs/reference/scoring) -- `SurpriseScorer`, `StuScorer`, and `SequentialScorer` API
- [Narrative Scoring Reference](/docs/reference/narratives) -- Thread tracking, tension arcs, pivot detection, composite scorer
- [Incremental Integration](/docs/guides/incremental-integration) -- Wire fabula into a live simulation loop
