---
sidebar_position: 3
title: "2. Define Patterns"
---

# Chapter 2: Define Patterns

Three detection patterns, shown in both the builder API and the DSL. Building on the simulation from [Chapter 1](simulation-loop).

## Pattern 1: Insider Trading

Someone receives an insider tip about a stock, then trades that stock. No public disclosure (alert) in between. Two stages plus a negation window.

**Builder API:**

```rust reference file=tests/build_ch02.rs#insider_trading
```

**DSL equivalent:**

```fabula reference file=dsl/build/insider_trading.fabula
```

Two stages: `tip` then `trade`, joined on `?suspect` (same actor) and `?ticker` (same stock). The negation window kills the match if an alert fires for that stock between the tip and the trade.

## Pattern 2: Repeated Manipulation

Same actor makes 3 or more trades on the same stock. Uses `compose::repeat` with shared variables.

**Builder API:**

```rust reference file=tests/build_ch02.rs#repeated_manipulation
```

**DSL equivalent:**

```fabula reference file=dsl/build/repeated_manipulation.fabula
```

The `repeat` operator creates 3 copies of the stage with prefixed variables (`rep0_ev`, `rep1_ev`, `rep2_ev`), but `manipulator` and `ticker` stay shared across all repetitions -- forcing the same actor and stock in all three trades.

## Pattern 3: Flash Crash Signals

A price drop and an alert fire for the same stock (in either order), then a large sell follows. Uses an unordered group for the concurrent events, then a sequential stage.

**Builder API:**

```rust reference file=tests/build_ch02.rs#flash_crash
```

**DSL equivalent:**

```fabula reference file=dsl/build/flash_crash.fabula
```

The `concurrent` block means `drop` and `alarm` can match in any order. Once both are satisfied, the engine advances to `sell`. All three stages share `?ticker`.

## Putting it together

Here is the complete code from Chapter 1 with all three patterns registered on an engine.

```rust reference file=tests/build_ch02.rs#putting_it_together
```

## Expected output

```text
registered 3 patterns:
  [0] insider_trading (2 stages)
  [1] repeated_manipulation (3 stages)
  [2] flash_crash (3 stages)

graph has 90 edges
engine ready for incremental matching
```

## What you learned

- `PatternBuilder` constructs patterns with stages, variable bindings, and negation windows
- `edge` matches a literal value; `edge_bind` binds the target to a variable for joins
- `unless_between` defines a negation window between two stage anchors
- `compose::repeat` creates multi-repetition patterns with shared variables
- `unordered_group` allows stages to match in any order (concurrent events)
- The DSL and builder API produce identical `Pattern` structs

[Next: Incremental Matching ->](incremental-matching)
