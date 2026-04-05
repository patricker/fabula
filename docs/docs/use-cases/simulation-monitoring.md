---
sidebar_position: 6
title: Simulation Monitoring
description: Detect emergent behaviors in agent-based models
---

# Simulation Monitoring

import PatternPlayground from '@site/src/components/wasm/PatternPlayground';

Agent-based models produce emergent behaviors -- resource hoarding, cascade failures, oscillating populations. These behaviors are hard to predict but easy to describe as temporal patterns. Sifting detects them as they happen.

| | |
|---|---|
| **Time** | ~15 minutes |
| **Prerequisites** | [What is Sifting?](/docs/learn/what-is-sifting) |

---

## 1. Resource Hoarding

An agent acquires resources repeatedly without ever sharing. The pattern triggers when the same agent acquires two different resources with no sharing event between them.

<PatternPlayground
  defaultPattern={`pattern resource_hoarding {
  stage e1 {
    e1.type = "acquire"
    e1.agent -> ?agent
    e1.resource -> ?r1
  }
  stage e2 {
    e2.type = "acquire"
    e2.agent -> ?agent
    e2.resource -> ?r2
  }
  unless between e1 e2 {
    mid.type = "share"
    mid.agent -> ?agent
  }
}`}
  defaultGraph={`graph {
  @1 e1.type = "acquire"
  @1 e1.agent -> agent_alpha
  @1 e1.resource -> food

  @2 e2.type = "acquire"
  @2 e2.agent -> agent_beta
  @2 e2.resource -> food

  @3 e3.type = "acquire"
  @3 e3.agent -> agent_alpha
  @3 e3.resource -> water

  @4 e4.type = "share"
  @4 e4.agent -> agent_beta
  @4 e4.resource -> food

  @5 e5.type = "acquire"
  @5 e5.agent -> agent_alpha
  @5 e5.resource -> shelter

  @6 e6.type = "acquire"
  @6 e6.agent -> agent_beta
  @6 e6.resource -> water

  now = 10
}`}
  compact
/>

**Result:** agent_alpha matches -- acquired food then water (and water then shelter) with no sharing between acquisitions. agent_beta acquired food, shared at t=4, then acquired water -- the sharing event between acquisitions kills the match via `unless between`.

:::tip What to notice
The variable `?agent` joins both acquisitions and the negation clause to the same agent. A sharing event by a *different* agent does not suppress the match. The pattern fires for each consecutive pair of hoarding acquisitions, so a persistent hoarder generates multiple matches.
:::

---

## 2. Cascade Failure

One agent's failure triggers a dependent agent's failure. The pattern chains two failures through a shared dependency variable and uses negation to exclude cases where the first agent recovered in time.

<PatternPlayground
  defaultPattern={`pattern cascade_failure {
  stage e1 {
    e1.type = "fail"
    e1.agent -> ?agent_a
  }
  stage e2 {
    e2.type = "depends_on"
    e2.agent -> ?agent_b
    e2.dependency -> ?agent_a
  }
  stage e3 {
    e3.type = "fail"
    e3.agent -> ?agent_b
  }
  unless between e1 e3 {
    mid.type = "recover"
    mid.agent -> ?agent_a
  }
}`}
  defaultGraph={`graph {
  @1 e1.type = "fail"
  @1 e1.agent -> power_plant

  @1 dep1.type = "depends_on"
  @1 dep1.agent -> factory
  @1 dep1.dependency -> power_plant

  @2 e2.type = "fail"
  @2 e2.agent -> water_plant

  @2 dep2.type = "depends_on"
  @2 dep2.agent -> farm
  @2 dep2.dependency -> water_plant

  @3 e3.type = "recover"
  @3 e3.agent -> water_plant

  @3 e4.type = "fail"
  @3 e4.agent -> factory

  @5 e5.type = "fail"
  @5 e5.agent -> farm

  now = 10
}`}
  compact
/>

**Result:** 1 match -- power_plant failed at t=1, factory depends on power_plant, factory failed at t=3, and power_plant never recovered. water_plant also failed, and farm depends on it, but water_plant recovered at t=3 before farm could fail at t=5 -- the `unless between` negation kills that cascade.

:::tip What to notice
The dependency relationship is an edge in the graph, not hardcoded in the pattern. The variable `?agent_a` threads from the first failure through the dependency edge to the negation clause. Any dependency topology the simulation produces -- chains, trees, cycles -- is surfaced by the same pattern.
:::

---

## 3. Population Oscillation

A population rises then falls in the same region. This two-stage pattern detects boom-bust cycles by joining on a shared region variable.

<PatternPlayground
  defaultPattern={`pattern population_oscillation {
  stage e1 {
    e1.type = "population_increase"
    e1.region -> ?region
  }
  stage e2 {
    e2.type = "population_decrease"
    e2.region -> ?region
  }
}`}
  defaultGraph={`graph {
  @1 e1.type = "population_increase"
  @1 e1.region -> region_north

  @2 e2.type = "population_increase"
  @2 e2.region -> region_south

  @3 e3.type = "population_increase"
  @3 e3.region -> region_north

  @4 e4.type = "population_increase"
  @4 e4.region -> region_south

  @5 e5.type = "population_decrease"
  @5 e5.region -> region_north

  now = 10
}`}
  compact
/>

**Result:** region_north matches -- population increased at t=1 (and again at t=3), then decreased at t=5. region_south has two increases but no decrease, so no match. The pattern fires once per increase-decrease pair in the same region, so region_north produces two matches (t=1 to t=5, and t=3 to t=5).

:::tip What to notice
No negation needed. The pattern is purely structural: rise then fall in the same region. To detect sustained oscillation (rise-fall-rise-fall), compose two instances with `sequence` or add more stages. To require a minimum gap between rise and fall, add `temporal e1 before e2 gap 3..100`.
:::

---

## Real-Time Monitoring with Incremental Mode

The playgrounds above use batch evaluation. In a running simulation, use incremental mode instead: feed edges as they happen and react immediately.

```rust reference file=tests/use_cases_simulation.rs#incremental_monitoring
```

Call `on_edge_added()` for each edge produced by the simulation. Call `end_tick()` once per round to finalize the tick, expire stale partial matches, and produce a `TickDelta` for narrative scoring. React to `SiftEvent::Completed` to trigger simulation-level responses -- spawn rescue agents, adjust resource allocation, log anomalies.

## Mapping your data

Agent-based model events map to fabula edges as follows:

| Real-world field | Fabula edge |
|---|---|
| eventID or step+agentID | source node |
| action type | label value |
| agent, target entity, resource | target nodes |
| simulation step | interval start |

Each simulation step produces edges for agent actions. The agent and resource fields become target nodes, so patterns can join across events by the same agent or involving the same resource.

---

## How fabula compares

- **vs custom observer code:** Hard-coded callbacks that check specific conditions each tick. No gap analysis (you cannot ask "how close did we get to a cascade?"), no composition for building complex detection from reusable fragments, no variable-scoped negation.
- **vs Flink CEP:** Complex event processing over flat event streams. No graph topology -- Flink patterns match sequences of events, not events connected by shared entities in a graph. No Allen algebra for temporal relations, no incremental partial match tracking with negation windows.

---

## Where to go next

- [Incremental Integration](/docs/guides/incremental-integration) -- Full walkthrough of the incremental API with memory management and scoring.
- [How the Engine Works](/docs/concepts/how-the-engine-works) -- The four-phase evaluation algorithm under the hood.
- [Scoring Reference](/docs/reference/scoring) -- Narrative quality scoring for MCTS evaluation.
