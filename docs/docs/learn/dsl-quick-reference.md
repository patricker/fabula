---
sidebar_position: 7
title: DSL Quick Reference
---

# DSL Quick Reference

One-line syntax for every construct. For full semantics, see the [DSL Reference](/docs/reference/dsl).

## Patterns

```
pattern <name> {
  ...
}
```

```fabula
pattern betrayal {
  stage e1 {
    e1.type = "betray"
  }
}
```

## Stages

A stage is an event slot. The anchor name (`e1`) becomes the source for clauses inside the block.

```
stage <anchor> {
  <clauses>
}
```

```fabula
stage e1 {
  e1.type = "greet"
  e1.actor -> ?who
}
```

## Clauses

### String match

```
source.label = "value"
```

```fabula
e1.location = "throne_room"
```

### Numeric match

```
source.label = <number>
```

```fabula
e1.points = 100
```

### Boolean match

```
source.label = true | false
```

```fabula
e1.active = true
```

### Variable binding

Bind a target node to a variable with `->`. The same `?var` in a later stage creates a join.

```
source.label -> ?var
```

```fabula
e1.actor -> ?char
```

### Node reference

Match a specific named node (no `?` prefix).

```
source.label -> node_name
```

```fabula
e1.actor -> alice
```

### Cross-stage comparison

Compare a value against a previously bound variable. Operators: `>`, `<`, `>=`, `<=`, `=`.

```
source.label > ?var
source.label < ?var
source.label >= ?var
source.label <= ?var
source.label = ?var
```

```fabula
e2.severity > ?level
```

Note: `= ?var` compares values. `-> ?var` binds/joins node references. They are not interchangeable.

### Negated clause

Prefix with `!`. Works with literal values and node references only.

```
! source.label = "value"
```

```fabula
! e1.status = "invited"
```

## Negation Windows

### Unless between

No matching event between two stages.

```
unless between <start> <end> {
  <clauses>
}
```

```fabula
unless between e1 e2 {
  mid.type = "fulfill"
  mid.actor -> ?char
}
```

### Unless after

No matching event after a stage (open-ended).

```
unless after <start> {
  <clauses>
}
```

```fabula
unless after e2 {
  end.type = "resolve"
  end.actor -> ?char
}
```

### Unless global

No matching event across the entire pattern span.

```
unless {
  <clauses>
}
```

```fabula
unless {
  mid.type = "reconcile"
  mid.actor -> ?char
}
```

## Temporal Constraints

### Allen relation (qualitative)

All 13 Allen relations: `before`, `after`, `meets`, `met_by`, `overlaps`, `overlapped_by`, `during`, `contains`, `starts`, `started_by`, `finishes`, `finished_by`, `equals`.

```
temporal <left> <relation> <right>
```

```fabula
temporal e1 before e2
```

### Metric gap

Add a bounded distance constraint to any Allen relation.

```
temporal <left> <relation> <right> gap <min>..<max>
```

```fabula
temporal e1 before e2 gap 0..100
```

Other gap forms: `..10` (no min), `3..` (no max), `5` (exact).

## Concurrent Groups

Stages inside `concurrent { }` can match in any order. All must match before the pattern advances past the group.

```
concurrent {
  stage <a> { ... }
  stage <b> { ... }
}
```

```fabula
concurrent {
  stage e2 {
    e2.type = "temp_spike"
    e2.sensor -> ?s
  }
  stage e3 {
    e3.type = "pressure_drop"
    e3.sensor -> ?s
  }
}
```

## Composition

### Sequence

Concatenate two patterns. `sharing(...)` joins variables across them.

```
compose <name> = <a> >> <b> sharing(<var>, ...)
```

```fabula
compose promise_kept = setup >> payoff sharing(char)
```

### Choice

Exclusive choice -- when one alternative completes, siblings are killed.

```
compose <name> = <a> | <b> | <c>
```

```fabula
compose crisis = war | famine | plague
```

### Exact repeat

```
compose <name> = <pattern> * <count> sharing(<var>, ...)
```

```fabula
compose three_strikes = offense * 3 sharing(offender)
```

### Range repeat

Match N to M times. Also supports unbounded: `* 3..` (3 or more).

```
compose <name> = <pattern> * <min>..<max> sharing(<var>, ...)
```

```fabula
compose brute_force = login_fail * 5..10 sharing(acct)
```

## Metadata

Attach key-value pairs to a pattern. Propagated to matches and events.

```
meta("<key>", "<value>")
```

```fabula
meta("thread_type", "conflict")
meta("priority", "high")
```

## Deadline

Expire partial matches after N ticks without completion.

```
deadline <ticks>
```

```fabula
deadline 30
```

## Graph (testing / playground)

Define a test graph with timestamped edges.

```
graph {
  @<time> source.label = <value>
  @<time> source.label -> <node>
  @<start>..<end> source.label = <value>
  now = <number>
}
```

```fabula
graph {
  @1 e1.type = "enter"
  @1 e1.actor -> alice
  @5..10 e2.type = "siege"
  now = 15
}
```

## Comments

```fabula
// Line comments start with //
e1.type = "enter" // end-of-line comments too
```
