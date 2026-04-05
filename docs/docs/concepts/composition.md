---
sidebar_position: 5
title: Composition
---

# Composition

import PatternPlayground from '@site/src/components/wasm/PatternPlayground';

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
/>

Two simple patterns. One compose directive. The result is a new pattern that matches a promise followed by a fulfillment by the same character. Each fragment is independently testable; the composed pattern inherits their stages, negations, and temporal constraints.

## The authoring problem

A monolithic 5-stage pattern with 3 negation windows is fragile. Change one stage and you break the negations that reference it. Rename a variable and you miss one of its 8 occurrences. Duplicate the pattern with minor variations and you maintain two copies forever.

Composition solves this the same way functions solve long procedures: break the pattern into named fragments, test each fragment in isolation, compose them into larger structures. The fragments are real patterns -- you can register them individually or combine them. The composed result is also a real pattern, indistinguishable to the engine from one built monolithically.

This matters most in three situations. First, when patterns share substructure -- a "betrayal" fragment appears in both "broken promise" and "escalating conflict." Second, when you need variations -- the same setup with three possible resolutions. Third, when patterns grow beyond what you can hold in your head.

## Three operators

### Sequence (`>>`)

A then B. The composed pattern's stages are A's stages followed by B's stages. The engine's implicit temporal ordering ensures B happens after A.

Shared variables create joins across the boundary. In `setup >> payoff sharing(char)`, the `char` variable must bind to the same node in both halves. Without sharing, the setup and payoff could involve different characters -- technically a valid match, but not the one you want.

Use sequence for: setup/payoff pairs, multi-phase processes, escalation chains, any "this happened and then later that happened" structure.

### Choice (`|`)

A or B or C. Each alternative is registered as a separate pattern in a mutual-exclusion group. When one completes, the engine kills active partial matches for all siblings.

This is exclusive by design. In `war | famine | plague`, a world that experiences war will not also match famine or plague through this composed pattern. The alternatives compete. If you want non-exclusive alternatives (all can match independently), register the patterns separately without composition.

Use choice for: branching narratives, alternative resolutions, crisis types, any "one of these outcomes" structure.

### Repeat (`*`)

A happens N times. Exact repeat (`* 3`) unrolls the pattern into a sequence of N copies. Repeat-range (`* 3..5` or `* 3..`) uses a looping engine that avoids full unrolling.

Shared variables bind across all repetitions. In `offense * 3 sharing(offender)`, the same offender must appear in all three offenses. Each repetition gets its own non-shared variables, so you can inspect what happened in each individual occurrence.

Use repeat for: brute force detection, escalation counting, recurring behaviors, threshold patterns.

## How variables work across composition

The core mechanism is variable renaming. When you compose two patterns, every variable in each sub-pattern is prefixed to prevent collisions -- `a_` for the first pattern in a sequence, `b_` for the second, `rep0_`, `rep1_`, `rep2_` for repetitions. An anchor `e1` in the first pattern becomes `a_e1`. An anchor `e1` in the second becomes `b_e1`.

Variables listed in `sharing(...)` are exempt from renaming. They keep their original names in all sub-patterns, which is exactly what creates the cross-pattern join. When the engine sees the same variable name in two stages, it requires the same binding.

You never manage prefixes yourself. The compose operators handle renaming automatically. In the match output, you see the prefixed names (`a_e1`, `b_e2`) alongside the shared names (`char`), which tells you exactly which sub-pattern each binding came from.

### Worked example: sequence binding output

Given two patterns composed with `sequence(a, b)` and `sharing("char")`:

- Pattern `a` has stage anchor `e1` and binds `char`
- Pattern `b` has stage anchor `e1` and binds `char`

After composition and evaluation, the match bindings look like:

```
a_e1  = Node("event1")     // anchor from pattern a, prefixed
b_e1  = Node("event3")     // anchor from pattern b, prefixed
char  = Node("macbeth")    // shared variable, unprefixed
```

The `a_` and `b_` prefixes scope each sub-pattern's variables so they cannot collide -- both patterns had an `e1` anchor, but the composed result distinguishes them as `a_e1` and `b_e1`. The shared variable `char` keeps its original name and must bind to the same node in both sub-patterns. This is what creates the cross-pattern join: Macbeth must be the actor in both halves.

## Repeat-range internals

Exact repeat (`* N`) is straightforward: unroll N copies, prefix each copy's variables, done. The result has `N * stages_per_pattern` stages.

Repeat-range (`* N..M` or `* N..`) is different. Unrolling is impractical when M is large or unbounded. Instead, the engine creates two copies of the pattern: a `first_` copy and a `last_` copy. The first iteration binds `first_` prefixed variables. Subsequent iterations match the `last_` segment, overwriting `last_` prefixed variables each time. Shared variables persist unchanged across all iterations.

The engine emits a completion when the minimum count is reached. But the partial match stays active, continuing to loop through the `last_` segment up to the maximum. Each additional match overwrites `last_` bindings and increments `repetition_count` on the partial match. For unbounded repeat (`* N..`), the match continues indefinitely until the partial match expires or is drained.

This gives you two useful reference points in the output: `first_*` variables tell you where the sequence started, `last_*` variables tell you where it currently stands, and `repetition_count` tells you how many times it matched.

## Design patterns

**Setup/payoff.** The canonical composition. A plant event followed by a payoff event, joined on the entity that was planted. Chekhov's gun monitoring: register the composed pattern incrementally, and the engine tracks which plants are still waiting for payoff.

```
compose chekhov = plant >> payoff sharing(entity)
```

**Escalation.** The same offense repeated by the same actor. Exact count for "three strikes" policies; repeat-range for flexible thresholds.

```
compose three_strikes = offense * 3 sharing(offender)
```

**Branching resolution.** Multiple possible outcomes for the same situation. The choice operator ensures only one resolution is counted.

```
compose crisis = war | famine | plague
```

**Flexible repetition.** When the exact count does not matter but you need a minimum. Brute force detection, sustained anomalies, recurring violations.

```
compose brute_force = login_fail * 5..10 sharing(account)
```

**Layered composition.** Compose directives chain. Build an arc from two fragments, then sequence that arc with a third pattern. Variables shared at each level propagate through.

```
compose arc = setup >> payoff sharing(char)
compose full_story = arc >> aftermath sharing(char)
```

## Connection to research

Pattern composition in fabula follows Kreminski et al. (FDG 2025), "Composable Story Sifting Patterns," which argues that sifting patterns should be reusable building blocks rather than monolithic queries. The key insight: narrative structures are recursive -- a betrayal arc is a component of a revenge arc, which is a component of a tragedy. Composition operators let you mirror this structure in your pattern definitions.

Fabula implements the three operators proposed in that work (sequence, choice, repeat) and extends repeat with the range variant for flexible counting. The variable renaming scheme ensures composability without accidental name collisions, which is critical when the same fragment appears in multiple composed patterns.

## Where to go next

- [DSL Reference](../reference/dsl) -- compose syntax, sharing rules, repeat-range semantics
- [Pattern Cookbook](../guides/pattern-cookbook) -- worked recipes including composition (Recipe 8)
- [How the Engine Works](./how-the-engine-works) -- repeat-range looping in the 4-phase algorithm
- [Research Lineage](../research) -- the Felt, Winnow, and FDG 2025 papers behind fabula
