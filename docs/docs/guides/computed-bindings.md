---
sidebar_position: 16
title: Computed Bindings
---

# Computed Bindings (`let`)

**Goal:** introduce a derived variable in your pattern, computed from already-bound variables and literals, without extending the constraint surface.

**When to reach for this:** when a constraint's right-hand side needs arithmetic on a previously bound variable -- e.g., "an event must occur exactly 5 pulses after the trigger."

## Recipe

```fab
pattern arrival_with_deadline {
    stage e1 {
        e1.type = "world"
        e1.pulse_count -> ?ts
    }
    let deadline = ?ts + 5
    stage e2 {
        e2.type = "world"
        e2.pulse_count = ?deadline
    }
}
```

The let:

1. Attaches to the most recent stage (here, `e1`).
2. Evaluates after that stage's clauses bind their variables.
3. Inserts the result into the binding map under `deadline`.
4. Is referenced by subsequent stages via `?deadline`, like any other bound variable.

## When evaluation fails

`Expr::eval` returns `None` (and the stage match fails) if:

- A referenced variable is unbound, or bound to a node rather than a value.
- An arithmetic op is unsupported (e.g., `string + number` for `MemValue`).
- Division by zero.

Failures are silent: the stage simply doesn't match, the same as any other unsatisfied clause.

## Composition

When you compose patterns, every non-shared variable gets a per-branch prefix to prevent collisions. Lets follow the same rule -- both the let's *name* and the `?var` references inside its expression are renamed.

```rust
use fabula::compose::sequence;
use fabula::builder::PatternBuilder;
use fabula::expr::{BinOp, Expr};
use fabula_memory::MemValue;

let arrival = PatternBuilder::<String, MemValue>::new("arrival")
    .stage("e1", |s| {
        s.edge("e1", "type".into(), MemValue::Str("arrival".into()))
            .edge_bind("e1", "pulse_count".into(), "ts")
            .let_binding(
                "deadline",
                Expr::bin(BinOp::Add, Expr::var("ts"), Expr::lit(MemValue::Num(5.0))),
            )
    })
    .build();

let response = PatternBuilder::<String, MemValue>::new("response")
    .stage("e2", |s| {
        s.edge("e2", "type".into(), MemValue::Str("response".into()))
            .edge_eq_var("e2", "pulse_count".into(), "a_deadline")
    })
    .build();

let composed = sequence("arrival_then_response", &arrival, &response, &[]);
```

After `sequence("arrival_then_response", &arrival, &response, &[])`:

- `?ts` becomes `?a_ts`
- The let's name `deadline` becomes `a_deadline`
- The `?ts` reference inside the let's expression becomes `?a_ts`
- Stage `e1` becomes `?a_e1`; stage `e2` becomes `?b_e2`
- The `edge_eq_var` in pattern B references `a_deadline` directly to bind across the composition boundary

To share a variable across the composition (no rename), pass it in the `shared` argument:

```rust
let composed = sequence("shared_deadline", &arrival, &response, &["deadline"]);
```

Now the let's name stays `deadline` in both branches, and pattern B can reference `?deadline` instead of `?a_deadline`. Var references inside the let expression that name a shared variable also stay unprefixed.

In `repeat_range`, lets in the looping segment are re-evaluated each iteration; their values do not persist across iterations. See the next section for a worked iteration trace.

## Repeat-range: re-evaluation by example

In a `repeat_range` pattern, lets attached to stages inside the looping segment are re-evaluated on every iteration. The previous iteration's let values are cleared from the binding map before the next iteration's stage matches; only `shared(...)` variables persist.

Concrete trace -- pattern with one stage that binds `?n` and computes `let doubled = ?n * 2`, looping `2..` times:

```rust
use fabula::compose::repeat_range;
use fabula::builder::PatternBuilder;
use fabula::expr::{BinOp, Expr};
use fabula_memory::MemValue;

let step = PatternBuilder::<String, MemValue>::new("step")
    .stage("e", |s| {
        s.edge_bind("e", "v".into(), "n").let_binding(
            "doubled",
            Expr::bin(BinOp::Mul, Expr::var("n"), Expr::lit(MemValue::Num(2.0))),
        )
    })
    .build();

let looped = repeat_range("looped", &step, 2, None, &[]);
```

Feeding three events with values `1, 2, 3`:

| Iteration | Event matched | `?n` | `?doubled` (let) | Completion emitted? |
|---|---|---|---|---|
| 1 | `v=1` | `1.0` | `2.0` | no (rep < min_reps) |
| 2 | `v=2` | `2.0` | `4.0` | yes (rep == 2) |
| 3 | `v=3` | `3.0` | `6.0` | yes (rep == 3) |

Each iteration gets a fresh `?doubled` computed against the new `?n`. The previous iteration's `?doubled` is not visible.

To carry a value across iterations, name it in `shared`:

```rust
let looped = repeat_range("looped", &step, 2, None, &["accumulator"]);
```

Shared variables persist across iterations and are NOT cleared between loops. (Lets cannot themselves be marked shared in `repeat_range` -- the `shared` slot only retains clause-bound variables. To accumulate across iterations, use a clause-bound shared variable updated by your data source rather than a let.)

## Concurrent groups

The DSL forbids `let` inside a `concurrent { }` block (only stages are allowed there). At the Rust API, `PatternBuilder::unordered_group` does not enforce this -- if you attach a `let` to a stage inside an unordered group, evaluation order depends on which sibling matches first, so a let that references a sibling's bindings may succeed or fail nondeterministically. Either keep lets out of grouped stages, or only reference variables bound outside the group.

## Rust builder API

```rust
use fabula::builder::PatternBuilder;
use fabula::expr::{BinOp, Expr};
use fabula_memory::MemValue;

let pattern = PatternBuilder::<String, MemValue>::new("deadline_match")
    .stage("e1", |s| {
        s.edge("e1", "type".into(), MemValue::Str("world".into()))
            .edge_bind("e1", "pulse_count".into(), "ts")
            .let_binding(
                "deadline",
                Expr::bin(BinOp::Add, Expr::var("ts"), Expr::lit(MemValue::Num(5.0))),
            )
    })
    .stage("e2", |s| {
        s.edge("e2", "type".into(), MemValue::Str("world".into()))
            .edge_eq_var("e2", "pulse_count".into(), "deadline")
    })
    .build();
```

## Custom value types

Custom value types must implement [`ArithmeticValue`](../reference/patterns#arithmeticvalue) to participate in let evaluation. For non-numeric `V`, return `None` from every method -- patterns can still reference the type, lets just won't compute against it.

## Related

- [`Stage::let_bindings`](../reference/patterns#stagel-v) -- the underlying field.
- [`Expr` and `ArithmeticValue`](../reference/patterns#expr) -- the expression AST and trait.
- [DSL `let` syntax](../reference/dsl#computed-bindings-let) -- full grammar.
