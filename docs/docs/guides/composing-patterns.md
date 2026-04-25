---
sidebar_position: 4
title: Composing Patterns
---

# Composing Patterns

import PatternPlayground from '@site/src/components/wasm/PatternPlayground';

Build complex patterns from reusable parts using `>>` (sequence), `|` (choice), and `*` (repeat).

| | |
|---|---|
| **Prerequisites** | [Pattern Cookbook](pattern-cookbook), [DSL Reference](../reference/dsl) |
| **Operators** | `>>` sequence, `|` choice, `*N` exact repeat, `*N..M` range repeat |

---

## Recipe 1: Sequence -- Setup then payoff

Chain two patterns with `>>`. The `sharing` clause forces `?char` to bind the same node across both sub-patterns.

<PatternPlayground
  defaultPattern={`pattern setup {
  stage e1 {
    e1.type = "promise"
    e1.actor -> ?char
  }
}
pattern payoff {
  stage e2 {
    e2.type = "fulfill"
    e2.actor -> ?char
  }
}

compose promise_kept = setup >> payoff sharing(char)`}
  defaultGraph={`graph {
  @1 e1.type = "promise"
  @1 e1.actor -> alice

  @3 e2.type = "fulfill"
  @3 e2.actor -> alice

  @2 e3.type = "promise"
  @2 e3.actor -> bob

  now = 10
}`}
  compact
/>

**Result:** 1 match -- Alice promised then fulfilled. Bob promised but never fulfilled, so `promise_kept` does not fire for him. Without `sharing(char)`, the two stages would be independent and any promise followed by any fulfillment would match.

---

## Recipe 2: Choice -- Alternative resolutions

`|` creates a mutual-exclusion group. When one alternative completes, the engine kills active partial matches for the others. Each alternative becomes a separate pattern with a shared group name.

<PatternPlayground
  defaultPattern={`pattern war {
  stage e { e.type = "war" e.region -> ?region }
}
pattern famine {
  stage e { e.type = "famine" e.region -> ?region }
}
pattern plague {
  stage e { e.type = "plague" e.region -> ?region }
}

compose crisis = war | famine | plague`}
  defaultGraph={`graph {
  @1 e1.type = "famine"
  @1 e1.region -> westeros

  @3 e2.type = "war"
  @3 e2.region -> essos

  now = 10
}`}
  compact
/>

**Result:** 2 matches -- `crisis_famine` for Westeros, `crisis_war` for Essos. The generated pattern names are `crisis_war`, `crisis_famine`, `crisis_plague` (group name + underscore + original name). All three share the exclusive group `"crisis"`, so in incremental mode, once one fires for a given partial match, the others are killed.

---

## Recipe 3: Exact repeat -- Three strikes

`* N` unrolls the pattern N times in sequence. Shared variables bind across all repetitions -- the same offender must commit all three offenses.

<PatternPlayground
  defaultPattern={`pattern offense {
  stage e {
    e.type = "offense"
    e.actor -> ?offender
  }
}

compose three_strikes = offense * 3 sharing(offender)`}
  defaultGraph={`graph {
  @1 e1.type = "offense"
  @1 e1.actor -> alice

  @2 e2.type = "offense"
  @2 e2.actor -> alice

  @3 e3.type = "offense"
  @3 e3.actor -> alice

  @4 e4.type = "offense"
  @4 e4.actor -> bob

  now = 10
}`}
  compact
/>

**Result:** 1 match for Alice (three offenses). Bob has only one, so the pattern does not complete for him. The composed pattern has 3 stages with anchors `rep0_e`, `rep1_e`, `rep2_e` -- each repetition's non-shared variables get a `repN_` prefix to prevent collisions.

---

## Recipe 4: Repeat range -- Brute force (5-10 attempts)

`* N..M` uses `repeat_range` internally. The pattern completes at `min` repetitions and continues matching up to `max`. Use `N..` for unbounded.

<PatternPlayground
  defaultPattern={`pattern login_fail {
  stage e {
    e.type = "login_failed"
    e.account -> ?account
  }
}

compose brute_force = login_fail * 5..10 sharing(account)`}
  defaultGraph={`graph {
  @1 e1.type = "login_failed"
  @1 e1.account -> admin

  @2 e2.type = "login_failed"
  @2 e2.account -> admin

  @3 e3.type = "login_failed"
  @3 e3.account -> admin

  @4 e4.type = "login_failed"
  @4 e4.account -> admin

  @5 e5.type = "login_failed"
  @5 e5.account -> admin

  now = 10
}`}
  compact
/>

**Result:** 1 match after 5 failures. The pattern will keep matching up to 10 total.

**Variable bindings in range repeats:**

| Binding | Meaning |
|---|---|
| `first_e` | Anchor from the first iteration |
| `last_e` | Anchor from the most recent iteration (updated on each new match) |
| `account` | Shared variable -- same across all iterations |
| `repetition_count` | Number of matched repetitions (on the `PartialMatch`) |

The `first_` / `last_` prefixes come from the two-copy layout: `repeat_range` generates a `first_` copy and a `last_` copy. The `last_` stages loop, overwriting their bindings on each new repetition.

---

## Recipe 5: Chaining compositions

A `compose` directive produces a named pattern. Use that name in subsequent `compose` directives to build deeper structures.

<PatternPlayground
  defaultPattern={`pattern setup {
  stage e1 {
    e1.type = "promise"
    e1.actor -> ?char
  }
}
pattern payoff {
  stage e2 {
    e2.type = "fulfill"
    e2.actor -> ?char
  }
}
pattern aftermath {
  stage e3 {
    e3.type = "reward"
    e3.actor -> ?char
  }
}

compose setup_payoff = setup >> payoff sharing(char)
compose full_arc = setup_payoff >> aftermath sharing(char)`}
  defaultGraph={`graph {
  @1 e1.type = "promise"
  @1 e1.actor -> alice

  @3 e2.type = "fulfill"
  @3 e2.actor -> alice

  @5 e3.type = "reward"
  @5 e3.actor -> alice

  now = 10
}`}
  compact
/>

**Result:** `full_arc` matches Alice's three-beat arc (promise, fulfill, reward). The intermediate `setup_payoff` also matches independently since it is registered as its own pattern.

When chaining, the second `compose` treats `setup_payoff` as a two-stage pattern. The `>>` appends `aftermath`'s stage, producing a three-stage `full_arc`. Variable renaming applies at each level: `full_arc` has anchors `a_a_e1`, `a_b_e2`, `b_e3` -- prefixes nest. The `sharing(char)` at each level keeps `?char` unprefixed throughout.

---

## How variable renaming works

Every `compose` renames non-shared variables to prevent collisions between sub-patterns:

| Operator | Prefix scheme |
|---|---|
| `A >> B` | `a_` for A's variables, `b_` for B's |
| `A * 3` | `rep0_`, `rep1_`, `rep2_` per repetition |
| `A * N..M` | `first_` for the initial copy, `last_` for the looping copy |
| `A \| B \| C` | No renaming (each alternative is a separate pattern) |

Variables listed in `sharing(...)` are exempt from renaming. Use `sharing` when the same entity must appear across sub-patterns (same character, same account, same region). Omit it when sub-patterns are independent.

[`let`](./computed-bindings) binding names and the variable references inside their expressions are renamed under the same scheme — a `let deadline = ?ts + 5` in pattern `a` becomes `let a_deadline = ?a_ts + 5` after `sequence("seq", &a, &b, &[])`. Lets referencing shared variables keep those references unprefixed.

---

## Next steps

- [DSL Reference](../reference/dsl) -- full `compose` syntax, including `sharing` and range operators.
- [Composition concepts](../concepts/composition) -- why composition works, variable scoping rules, exclusive groups.
- [Pattern Cookbook](pattern-cookbook) -- single-pattern recipes (negation, constraints, Allen relations).
- [Incremental Integration](incremental-integration) -- use composed patterns in a live simulation loop.
