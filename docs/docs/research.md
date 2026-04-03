---
sidebar_position: 20
title: Research Lineage
---

# Research Lineage

Fabula builds on a line of academic research into **story sifting** — automatically identifying narratively interesting event sequences in simulation output. This page traces that lineage and explains what fabula adds.

---

## Felt (ICIDS 2019)

**Felt** is a 290-line JavaScript library that introduced the concept of a *story sifter* as a reusable component. It combines a pattern-matching engine with an action-selection system, both backed by [DataScript](https://github.com/tonsky/datascript) (an immutable, in-memory Datalog database).

A *sifting pattern* is an array of Datalog clauses that bind logic variables across events and entities. Running a pattern against a simulation's database returns *nuggets* — sets of variable bindings representing interesting event sequences. The same query language drives action preconditions, so "what has happened" and "what can happen next" are the same operation.

Felt also introduced **whyNot** debugging: when a pattern fails to match, the engine tests each clause individually to identify which one blocked the result.

- **Paper**: Kreminski, M., Dickinson, M., & Wardrip-Fruin, N. (2019). [Felt: A Simple Story Sifter.](https://mkremins.github.io/publications/Felt_SimpleStorySifter.pdf) ICIDS 2019.
- **Code**: [github.com/mkremins/felt](https://github.com/mkremins/felt)

---

## Winnow (AIIDE 2021)

**Winnow** extends Felt with two key innovations: a readable domain-specific language (DSL) for writing patterns, and an **incremental matching** algorithm that tracks partial matches as events stream in.

Instead of re-evaluating every pattern from scratch after each simulation tick, Winnow maintains a set of `PartialMatch` objects. When a new event arrives, the engine:

1. Checks whether the event kills any active partial match via a negation window (`unless-event ... between`).
2. Tests whether the event starts a new match (stage 0).
3. Tests whether the event advances an existing partial match to the next stage.

The original partial match survives alongside any newly spawned matches, preserving the possibility that a different future event will satisfy the same stage differently.

Winnow's DSL compiles to DataScript Datalog queries:

```clojure
(pattern violationOfHospitality
  (event ?e1 where eventType: enterTown, actor: ?guest)
  (event ?e2 where eventType: showHospitality, actor: ?host, target: ?guest)
  (event ?e3 where tag: harm, actor: ?host, target: ?guest)
  (unless-event ?eMid between ?e1 ?e3 where
    eventType: leaveTown, actor: ?guest))
```

- **Paper**: Kreminski, M., Dickinson, M., & Mateas, M. (2021). [Winnow: A Domain-Specific Language for Incremental Story Sifting.](https://mkremins.github.io/publications/Winnow_AIIDE2021.pdf) AIIDE 2021.
- **Code**: [github.com/mkremins/winnow](https://github.com/mkremins/winnow)

---

## Related work

### Select the Unexpected (ICIDS 2022)

Kreminski et al. propose a statistical heuristic for ranking sifting results by *surprise* — how unlikely a matched pattern is given the simulation's base rates. This addresses the problem of sifters returning too many matches: "two characters talk" matches constantly, but "two sworn enemies share a meal" is narratively interesting precisely because it is rare.

- **Paper**: Kreminski, M., et al. (2022). [Select the Unexpected: A Statistical Heuristic for Story Sifting.](https://mkremins.github.io/publications/StU_ICIDS2022.pdf) ICIDS 2022.

### Composable Story Sifting Patterns (FDG 2025)

Kreminski et al. explore how sifting patterns can be composed from smaller, reusable building blocks — analogous to function composition in programming. Instead of writing monolithic patterns, authors define atomic pattern fragments that can be combined, parameterized, and layered.

- **Paper**: Kreminski, M., et al. (2025). Stories from the Bottom Up: Composable Story Sifting Patterns. FDG 2025.

### Authoring for Story Sifters (2022)

An exploration of the authoring challenges in story sifting. What makes a good sifting pattern? How do authors express narrative structure in formal terms? The paper examines the gap between "things the system can detect" and "things the author considers interesting."

- **Paper**: Kreminski, M., et al. (2022). [Authoring for Story Sifters.](https://mkremins.github.io/publications/AuthoringSifters_TAP.pdf)

### Allen's Interval Algebra (CACM 1983)

James Allen's foundational paper defines 13 mutually exclusive relations between two temporal intervals (before, meets, overlaps, during, starts, finishes, equals, and their inverses). This algebra enables reasoning about events that have duration, not just ordering — "the siege overlapped with the harvest" rather than just "the siege came before the harvest."

- **Paper**: Allen, J.F. (1983). Maintaining Knowledge about Temporal Intervals. *Communications of the ACM* 26(11), 832-843.

### Simple Temporal Networks (AI 1991)

Dechter, Meiri, and Pearl formalize quantitative temporal constraints as bounded differences between time points. Meiri (1996) extends this to combine qualitative Allen relations with quantitative metric constraints. Fabula's `gap min..max` syntax implements per-constraint metric bounds checked during evaluation — not a full STN solver, but the same bounded-difference formalism.

- **Paper**: Dechter, R., Meiri, I., & Pearl, J. (1991). Temporal Constraint Networks. *Artificial Intelligence* 49(1-3), 61-95.
- **Paper**: Meiri, I. (1996). Combining Qualitative and Quantitative Constraints in Temporal Reasoning. *Artificial Intelligence* 87(1-2), 343-385.

### TABGP — Temporal Graph Pattern Matching (VLDB 2023)

A database-oriented approach to temporal graph pattern matching using timed automata. TABGP focuses on query performance over large graph databases — a complementary concern to fabula's focus on incremental matching within simulations.

- **Code**: [github.com/amirpouya/TABGP](https://github.com/amirpouya/TABGP)

### Narrative scoring research

Fabula's `fabula-narratives` crate implements scoring signals for MCTS-based narrative management. Each module is grounded in specific research:

**Search-Based Drama Management (AIIDE 2005)**. Nelson and Mateas formalize the game master as an optimizer with a quality function over narrative states. The GM searches a tree of possible future actions, scoring each by narrative desirability. Fabula's `scorer` module is this quality function — a composite of weighted signals producing a single score for MCTS evaluation.

- **Paper**: Nelson, M.J. & Mateas, M. (2005). Search-Based Drama Management in the Interactive Fiction Anchorhead. AIIDE 2005.

**Narrative Information Theory (arXiv 2024)**. Schulz et al. define five information-theoretic measures of narrative (Complexity, Pivot, Predictability, Suspense, Plot Twist). Fabula's `pivot` module implements the Pivot measure: Jensen-Shannon Divergence between consecutive event-type distributions. JSD is symmetric and bounded in [0, 1], making it directly comparable across ticks.

- **Paper**: Schulz, J., et al. (2024). Narrative Information Theory. arXiv:2411.12907.

**Left 4 Dead AI Director (GDC 2009)**. Booth describes Valve's AI Director as a pacing system that tracks player stress via trajectory sampling and adjusts intensity accordingly. Fabula's `tension` module adapts this approach: sample a numeric value per tick, classify the trajectory (Rising/Falling/Plateau/Peak/Valley) via linear regression and split-window analysis.

- **Talk**: Booth, M. (2009). The AI Systems of Left 4 Dead. GDC AI Summit.

**Suspense and Surprise (AER 2015)**. Ely, Frankel, and Kamenica model suspense as entropy of future outcomes and surprise as divergence between predicted and actual outcomes. The key insight for tension scoring: trajectory (how values change) matters more than absolute level.

- **Paper**: Ely, J., Frankel, A., & Kamenica, E. (2015). Suspense and Surprise. *Journal of Political Economy* 123(1), 215-260.

**MICE Quotient (Writing Excuses)**. Mary Robinette Kowal's framework categorizes narrative threads as Milieu, Inquiry, Character, or Event. Well-formed stories close threads in reverse order of opening (FILO — First In, Last Out). Fabula's `thread` module tracks open/close pairs and validates nesting order.

---

## What fabula adds

Fabula is a Rust implementation of the sifting and incremental matching ideas from Felt and Winnow, extended in three directions.

| Dimension | Felt / Winnow | Fabula |
|---|---|---|
| **Temporal model** | Entity ID ordering (`?e1 < ?e2`). Events are points, not intervals. | Allen interval algebra. Edges carry `[start, end)` intervals. All 13 Allen relations available for explicit constraints. |
| **Graph backend** | Coupled to DataScript (Datalog EAV store). | Generic `DataSource` trait. Bring any graph: in-memory, petgraph, Grafeo, or your own implementation. |
| **Query engine** | Patterns compile to Datalog queries executed by DataScript. | Direct graph traversal with variable binding. No Datalog dependency. |
| **Language** | JavaScript / ClojureScript | Rust. Zero-dependency core. |
| **Action system** | Built-in (`registerAction`, `possibleActions`, effects). | Not in scope. Fabula detects patterns; the simulation layer handles actions. |
| **Gap analysis** | Felt's `whyNot` (clause-by-clause). Winnow: none. | `why_not` returns a structured `GapAnalysis` with per-stage, per-clause status and failure reasons. |
| **Negation** | `not-join` (Felt); `unless-event between` (Winnow). | `unless_between`, `unless_after` (open-ended), `unless_global` (full-pattern span). Multi-clause negation bodies. |
| **Death details** | Winnow tracks `{eventID, constraint}`. | `SiftEvent::Negated` carries the triggering clause label and source node. |
| **Text DSL** | N/A | Winnow's S-expression syntax | `fabula-dsl` crate with TypeMapper for compiling to arbitrary type systems. |
| **Composition** | N/A | N/A | Sequence (`>>`), choice (`\|`), repeat (`*`) with variable sharing. |
| **Surprise scoring** | N/A | N/A | Shannon surprise + StU property-level scoring (Kreminski 2022). |
| **Narrative scoring** | N/A | N/A | `fabula-narratives` crate: threads, tension, pivots, composite MCTS quality function. |
| **Metric temporal** | N/A | N/A | STN-style gap bounds on Allen relations (Dechter/Meiri/Pearl 1991). |
| **Pattern lifecycle** | N/A | N/A | Enable/disable, metrics, staleness, plant/payoff tracking, MCTS forking. |

The core insight that sifting patterns and simulation can share a query language carries forward from Felt. Fabula narrows its scope to the sifting side so it can be embedded in any simulation system — from game engines to procedural world generators — without imposing a specific action or effect model.
