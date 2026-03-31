---
title: DSL Reference
description: Complete syntax reference for the fabula pattern and graph DSL
---

# DSL Reference

Fabula provides a text DSL for defining patterns and graphs. The DSL compiles to the same types as the Rust builder API — every construct maps 1:1.

## Pattern Syntax

```
pattern <name> {
  stage <event_var> {
    <clause>+
  }+

  [unless between <start_var> <end_var> { <clause>+ }]*
  [unless after <start_var> { <clause>+ }]*
  [unless { <clause>+ }]*
  [temporal <left_var> <relation> <right_var>]*
}
```

### Sources

The left side of the dot identifies which node to query edges from. There are three kinds:

| Syntax | Meaning | Example |
|--------|---------|---------|
| `e1.label` | **Stage anchor** — the node this stage matches against | `e1.eventType = "enter"` inside `stage e1 { ... }` |
| `alice.label` | **Literal node** — a specific named node in the graph | `alice.trait = "impulsive"` |
| `?char.label` | **Bound variable** — must have been bound by `-> ?char` earlier | `?char.trait = "impulsive"` after `e1.actor -> ?char` |

Stage anchors do not need `?` — they are implicitly variables within their stage. Bare identifiers that are not the stage anchor are treated as literal node names. Use `?` to reference a bound variable.

Using `?` on an unbound variable is a compile error:

```
stage e1 {
  e1.eventType = "betray"
  ?char.trait = "impulsive"  // ERROR: ?char not yet bound
}
```

### Clauses

| Syntax | Meaning | Builder equivalent |
|--------|---------|-------------------|
| `e1.label = "value"` | Literal string match (stage anchor) | `.edge("e1", label, Str(value))` |
| `?char.label = "value"` | Match on bound variable | `.edge("char", label, Str(value))` |
| `alice.label = "value"` | Match on literal node | `.edge("alice", label, Str(value))` |
| `source.label = 42` | Literal number match | `.edge(source, label, Num(42.0))` |
| `source.label = true` | Literal boolean match | `.edge(source, label, Bool(true))` |
| `source.label -> ?var` | Bind target to variable | `.edge_bind(source, label, var)` |
| `source.label -> node` | Match node reference | `.edge(source, label, Node(node))` |
| `source.label < 0.5` | Value constraint (Lt) | `.edge_constrained(source, label, Lt(0.5))` |
| `source.label > 10` | Value constraint (Gt) | `.edge_constrained(source, label, Gt(10))` |
| `source.label <= 100` | Value constraint (Lte) | `.edge_constrained(source, label, Lte(100))` |
| `source.label >= 0` | Value constraint (Gte) | `.edge_constrained(source, label, Gte(0))` |
| `! source.label = "value"` | Negated clause (literals/refs only) | `.not_edge(source, label, value)` |

Negation (`!`) works with literal values (`= "value"`, `= 42`, `= true`) and node references (`-> node`). It is **not** supported with value constraints (`<`, `>`, `<=`, `>=`) or variable bindings (`-> ?var`) — rewrite as the inverse constraint instead (e.g., `! e.x < 0.5` becomes `e.x >= 0.5`).

### Negation Windows

| Syntax | Meaning |
|--------|---------|
| `unless between e1 e3 { ... }` | Clauses must NOT match between e1 and e3 |
| `unless after e1 { ... }` | Clauses must NOT match after e1 (open-ended) |
| `unless { ... }` | Global negation (first stage to last stage) |

### Temporal Constraints

```
temporal e1 before e2                   // qualitative only
temporal e1 before e2 gap 3..10         // gap in [3, 10]
temporal e1 before e2 gap ..10          // gap in [0, 10]
temporal e1 before e2 gap 3..           // gap in [3, infinity)
temporal e1 during e2 gap 5..50         // start margin in [5, 50]
```

All 13 Allen relations are supported: `before`, `after`, `meets`, `met_by`, `overlaps`, `overlapped_by`, `during`, `contains`, `starts`, `started_by`, `finishes`, `finished_by`, `equals`.

The optional `gap` keyword adds a metric bound (STN-style bounded difference constraint). The meaning of "gap" depends on the Allen relation — for `before` it's the separation between end(A) and start(B); for `during` it's the start margin; for `overlaps` it's the overlap duration. See the [Temporal Model](/docs/concepts/temporal-model) for details.

## Graph Syntax

```
graph {
  @<timestamp> <source>.<label> = <value>
  @<timestamp> <source>.<label> -> <node>
  @<start>..<end> <source>.<label> = <value>
  now = <number>
}
```

### Edge Types

| Syntax | Value Type |
|--------|-----------|
| `@1 e.type = "enter"` | String |
| `@1 e.score = 42.5` | Number |
| `@1 e.active = true` | Boolean |
| `@1 e.actor -> alice` | Node reference |
| `@1..10 e.type = "siege"` | Bounded interval `[1, 10)` |

Edges without `..` create open-ended intervals `[start, infinity)`.

### `now`

The `now = <number>` statement sets the graph's current time. If omitted, the playground uses `max(timestamps) + 1`.

## Comments

Line comments start with `//`:

```
// This is a comment
pattern test {
  stage e1 {
    e1.type = "hello" // end-of-line comment
  }
}
```

## Complete Example

```
pattern violation_of_hospitality {
  stage e1 {
    e1.eventType = "enterTown"
    e1.actor -> ?guest
  }
  stage e2 {
    e2.eventType = "showHospitality"
    e2.actor -> ?host
    e2.target -> ?guest      // joins on ?guest from e1
  }
  stage e3 {
    e3.eventType = "harm"
    e3.actor -> ?host        // same host as e2
    e3.target -> ?guest
  }
  unless between e1 e3 {
    eMid.eventType = "leaveTown"
    eMid.actor -> ?guest     // same guest — if they leave, no violation
  }
}

graph {
  @1 e1.eventType = "enterTown"
  @1 e1.actor -> alice
  @2 e2.eventType = "showHospitality"
  @2 e2.actor -> bob
  @2 e2.target -> alice
  @3 e3.eventType = "harm"
  @3 e3.actor -> bob
  @3 e3.target -> alice
  now = 10
}
```

### Variable source example

This pattern uses `?char` to check a property on a bound variable:

```
pattern two_impulsive_betrayals {
  stage e1 {
    e1.eventType = "betray"
    e1.actor -> ?char           // bind ?char
    ?char.trait = "impulsive"   // follow ?char, check its trait
  }
  stage e2 {
    e2.eventType = "betray"
    e2.actor -> ?char           // same character betrays again
  }
  unless {
    mid.eventType = "reconcile"
    mid.actor -> ?char          // mid is a scan root (no ?), ?char is a target binding
  }
}
```

Note that `mid` in the negation block has no `?` — it is a scan root (the engine searches for any node matching the clauses). But `?char` on the right side of `->` references the bound variable from the parent pattern.

Try this in the [Pattern Playground](/docs/playground/pattern-playground).
