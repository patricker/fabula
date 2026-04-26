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
