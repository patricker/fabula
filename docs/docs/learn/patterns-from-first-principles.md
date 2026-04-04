---
sidebar_position: 3
title: Patterns from First Principles
---

# Patterns from First Principles

import PatternPlayground from '@site/src/components/wasm/PatternPlayground';

A pattern is a template with holes. A match is a filled template. This page builds the mental model piece by piece.

---

## Edges: the atomic unit

Everything in fabula starts with an edge. An edge connects a source node to a target through a labeled relationship, valid over a time interval.

```
source --[label]--> target   during [start, end)
```

A graph is a collection of edges. A login event might be three edges sharing a source node:

```
login1 --[type]-----> "login"       during [1, ∞)
login1 --[user]-----> @alice        during [1, ∞)
login1 --[location]--> "new_york"   during [1, ∞)
```

The source (`login1`) is the event node. The labels (`type`, `user`, `location`) describe its properties. The targets are values (`"login"`, `"new_york"`) or node references (`@alice`).

## Stages: ordered event slots

A pattern has one or more **stages**, each anchored to an event variable. Each stage says: "find an event that looks like this."

```
pattern one_stage {
  stage e1 {
    e1.type = "login"
  }
}
```

This pattern matches every event with `type = "login"`. The stage variable `e1` binds to the event node.

Multiple stages are time-ordered. Stage 1 must happen before stage 2:

```
pattern two_stages {
  stage e1 { e1.type = "login" }
  stage e2 { e2.type = "logout" }
}
```

This matches any login followed by any logout. Not very useful yet — it doesn't require the same user.

## Variables: the glue

Variables bind to nodes or values during matching. When the same variable appears in multiple stages, it creates a **join** — the engine forces the variable to bind to the same entity everywhere.

<PatternPlayground
  defaultPattern={`pattern same_user_login_logout {
  stage e1 {
    e1.type = "login"
    e1.user -> ?user
  }
  stage e2 {
    e2.type = "logout"
    e2.user -> ?user
  }
}`}
  defaultGraph={`graph {
  @1 e1.type = "login"
  @1 e1.user -> alice

  @3 e2.type = "logout"
  @3 e2.user -> alice

  @2 e3.type = "login"
  @2 e3.user -> bob

  now = 10
}`}
  compact
/>

The variable `?user` appears in both stages. Stage 1 binds it to a node; stage 2 requires the same node. Alice logged in (time 1) and logged out (time 3) — match. Bob logged in (time 2) but never logged out — no match.

Without the shared variable, the pattern would match *any* login followed by *any* logout, even by different people. The variable is what makes the pattern meaningful.

## Clauses: constraints within a stage

Each stage has one or more **clauses**. Each clause constrains which edges must exist at the event node.

| Clause type | DSL syntax | Meaning |
|-------------|-----------|---------|
| Literal match | `e1.type = "login"` | Edge with label `type` must have value `"login"` |
| Binding | `e1.user -> ?user` | Edge with label `user` — bind the target to `?user` |
| Value constraint | `e1.severity > 3` | Edge value must satisfy the numeric constraint |
| Node reference | `e1.actor -> alice` | Edge target must be the specific node `alice` |
| Negated clause | `! e1.type = "admin"` | Edge with this label/value must NOT exist |
| Cross-stage comparison | `e2.score > ?prev_score` | Value must be greater than a previously bound variable |

The first clause in a stage is the **trigger** — it must match the incoming edge. Remaining clauses are verified against the data source using accumulated bindings.

## Negation: the exception clause

A negation window says "these clauses must NOT match between two events."

<PatternPlayground
  defaultPattern={`pattern login_without_logout {
  stage e1 {
    e1.type = "login"
    e1.user -> ?user
  }
  stage e2 {
    e2.type = "login"
    e2.user -> ?user
  }
  unless between e1 e2 {
    mid.type = "logout"
    mid.user -> ?user
  }
}`}
  defaultGraph={`graph {
  @1 e1.type = "login"
  @1 e1.user -> alice

  @3 e2.type = "login"
  @3 e2.user -> alice

  @2 e3.type = "login"
  @2 e3.user -> bob

  @4 mid.type = "logout"
  @4 mid.user -> bob

  @5 e4.type = "login"
  @5 e4.user -> bob

  now = 10
}`}
  compact
/>

Alice logged in twice (time 1, time 3) with no logout between — match. Bob logged in (time 2), logged out (time 4), then logged in again (time 5) — the logout between kills the match.

Three negation forms:
- `unless between e1 e2` — no match between two stages
- `unless after e1` — no match after a stage (open-ended)
- `unless` (global) — no match anywhere in the pattern's span

## Intervals: when things happen

Every edge has a time interval. Stages are implicitly time-ordered: stage 1's start time must be strictly less than stage 2's start time.

For most patterns, implicit ordering is all you need. Explicit Allen constraints are for special cases — "event A happened *during* event B" or "events A and B *overlapped*":

```
temporal inner during outer
```

See the [Allen Visualizer](/docs/playground/allen-visualizer) to explore all 13 temporal relations interactively.

## Putting it together

Every pattern uses the same five building blocks:

| Building block | What it does |
|---------------|-------------|
| **Stages** | Ordered event slots — "first this, then that" |
| **Clauses** | Constraints on what fills each slot |
| **Variables** | Joins across stages — "same entity" |
| **Negation** | Exception clauses — "unless this happened" |
| **Intervals** | Temporal ordering — implicit or explicit |

These five primitives compose to express patterns across every domain: narratives, compliance, observability, process mining, cybersecurity.

## Where to go next

- [Sifting by Example](sifting-by-example) — Four domains using these same primitives.
- [Getting Started](/docs/getting-started) — Build patterns in Rust.
- [DSL Reference](/docs/reference/dsl) — Complete syntax reference for all clause types.
- [Thinking in Time](thinking-in-time) — Deep dive into Allen interval algebra.
