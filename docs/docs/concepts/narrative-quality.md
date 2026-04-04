---
sidebar_position: 7
title: Narrative Quality
---

# Narrative Quality

How does a game master decide what should happen next? Nelson and Mateas (2005) frame it as optimization: the GM holds a quality function over narrative states, searches a tree of possible future actions, scores each leaf, and picks the branch that leads to the best story. The `fabula-narratives` crate is that quality function.

## The GM as optimizer

Search-Based Drama Management (SBDM) treats narrative management as a planning problem. The GM has a set of actions it could take (introduce a character, trigger a storm, reveal a secret). Each action changes the narrative state. The GM builds a search tree -- typically Monte Carlo Tree Search (MCTS) -- and evaluates candidate future states with a scoring function. The highest-scoring branch wins.

Fabula's sifting engine provides the *what happened* layer. The narratives crate provides the *was it any good* layer. Together, they close the MCTS loop: simulate candidate actions, sift for patterns, score the result, compare, choose.

The quality function does not judge content directly. It does not know that a betrayal is dramatic or that a sunset is poetic. It measures structural properties of the narrative -- momentum, resolution, pacing, variety, surprise -- and combines them into a score. Content judgment comes from the patterns you write and the weights you assign.

## The composite scorer

The scorer is a pure function. It takes a set of numeric signals and a set of weights, and returns a composite score with an explainable breakdown. No state. No side effects. The caller assembles signals from various tracking components; the scorer combines them into one number.

The architecture is deliberately split into three layers. The *trackers* (`ThreadTracker`, `TensionTracker`, `PivotDetector`) observe engine output and maintain running state. The *signal assembler* collects tracker outputs and engine tick deltas into a flat struct of numeric values. The *scorer* multiplies signals by weights and sums them. Each layer is independently testable and replaceable.

This separation matters for MCTS. Each candidate branch needs its own score, computed from its own signals. Making the scorer stateless means you never have to worry about shared mutable state between branches. The trackers are stateful but cheap to clone -- fork the tracker alongside the engine when branching.

## Scoring signals

Six signals feed the quality function. Each measures a different dimension of narrative health.

### Progress

Are patterns advancing? The engine reports how many partial matches moved forward this tick via the `TickDelta`. More advancements mean things are happening -- the narrative has momentum. Zero advancements across many ticks means the story is stuck. The scorer rewards advancements, rewards completions at a higher rate (finishing is worth more than continuing), and penalizes stalls (active partial matches that haven't advanced recently).

### Resolution

Are setups paying off? Plant/payoff tracking monitors Chekhov's gun: a pattern that "plants" something (a weapon on the mantel, a prophecy, a threat) creates narrative tension that must eventually resolve. The engine tracks plant/payoff pairs via `plant_payoff_pairs` -- the plant pattern is the setup, the payoff pattern is the resolution. The engine knows how many plant instances are active and how many payoff completions have occurred.

The scorer penalizes unresolved plants -- promises made to the audience but not kept -- and rewards resolutions at a high rate (default 5.0, the highest single-signal weight). The unresolved penalty is mild per plant (-0.5) but accumulates, so a GM that keeps planting without resolving will see its score degrade steadily over time. This creates pressure to pay things off rather than endlessly adding new threads.

### Thread balance

Kowal's MICE quotient classifies narrative threads into four types: Milieu (entering/leaving a space), Inquiry (question posed/answered), Character (internal conflict resolved), Event (disruption restored). The structural rule: threads should close in FILO order. The last thread opened should be the first one closed, like nested parentheses.

The `ThreadTracker` monitors open/close events for registered thread pairs. A thread "opens" when its open pattern begins matching (detected from `TickDelta.advanced`). A thread "closes" when its close pattern fully resolves (detected from `TickDelta.completed`). When a thread closes out of order -- a milieu thread closing while a later-opened inquiry thread is still open -- the tracker records a FILO violation. The scorer penalizes each violation. This pushes the GM toward structurally well-formed narratives, not just eventful ones.

### Tension fit

Is tension moving in the right direction? The `TensionTracker` accepts a caller-supplied numeric sample each tick (stress level, faction hostility, danger rating -- whatever the simulation exposes) and classifies the trajectory over a sliding window: Rising, Falling, Plateau, Peak, or Valley. Classification uses linear regression for monotonic trends and half-window analysis for peaks and valleys.

The GM declares a *desired* trajectory -- "I want tension to be rising right now." The scorer compares actual to desired via a three-valued fit function: +1.0 if the trajectories match (both Rising, or both Falling), -1.0 if they are opposites (Rising vs. Falling, Peak vs. Valley), and 0.0 for any other combination (including Unknown). This is the AI Director principle from Left 4 Dead (Booth 2009): pacing is not about maximizing intensity but about controlling its trajectory.

The insight from Ely, Frankel, and Kamenica (2015) is that trajectory matters more than absolute value for suspense. A slow climb from low tension is more suspenseful than a constant high. The scorer captures this by rewarding trajectory alignment, not tension magnitude.

### Pivot magnitude

How much did the narrative state shift? The `PivotDetector` maintains a categorical distribution over event types each tick. At tick end, it computes the Jensen-Shannon Divergence between the current tick's distribution and the previous tick's. JSD is symmetric and bounded in [0, 1] when using log base 2.

Low JSD means this tick looks like the last one -- continuation. High JSD means the event landscape changed dramatically -- a pivot. A tick full of trade and diplomacy followed by a tick full of combat and flight produces high JSD. The scorer rewards pivots, giving the GM an incentive to create dramatic turns rather than monotonous event streams.

This implements the Pivot measure from Schulz et al. (2024), one of five information-theoretic narrative measures proposed in their Narrative Information Theory framework. Empty ticks (no events pushed) produce JSD of 0.0 and leave the previous distribution unchanged, so the next non-empty tick compares against the last non-empty one. This avoids penalizing quiet ticks.

### Surprise

Pattern-level and sequential surprise from fabula's core scoring module feed into the composite scorer as two separate signals. Pattern-level surprise measures how unexpected a particular pattern completion is given its historical base rate -- a `SurpriseScorer` tracks per-pattern match counts and computes Shannon surprise against baseline frequencies. Sequential surprise measures how unexpected the transition between consecutive pattern completions is -- a `SequentialScorer` builds a bigram model of pattern-to-pattern transitions and scores each by its conditional surprise.

Both are information-theoretic values in bits. Higher means rarer. A betrayal that fires in 1% of rounds is more surprising than one that fires in 50%. A betrayal *following an alliance* is more surprising if alliances usually lead to trade.

The two signals are independent. A common pattern can still produce high sequential surprise if it follows an unusual predecessor. The scorer weights them separately so the GM can decide how much to value each kind of unexpectedness. See [Scoring and Surprise](../reference/scoring) for the underlying math.

## Configurable weights

Every signal has a weight. The defaults reflect a general-purpose narrative sensibility:

| Signal | Default weight | Rationale |
|--------|---------------|-----------|
| Progress | 1.0 | Baseline reward for forward motion. |
| Completion | 3.0 | Finishing a pattern is worth more than advancing one. |
| Stall penalty | -2.0 | Stuck patterns drag the score down. |
| Unresolved penalty | -0.5 | Mild per-plant drag; accumulates over time. |
| Resolution reward | 5.0 | Paying off a setup is the single highest-value action. |
| FILO violation penalty | -3.0 | Structural problems are costly. |
| Tension fit | 2.0 | Pacing alignment matters but is not dominant. |
| Pivot reward | 1.5 | Dramatic turns are good but should not be constant. |
| Surprise | 1.0 | Baseline reward for unexpected patterns. |
| Sequential surprise | 1.0 | Baseline reward for unexpected transitions. |

Tuning weights changes what "good narrative" means. A horror game might increase the tension fit weight and lower the resolution reward to keep the audience in sustained dread. A comedy might increase the surprise and pivot weights to reward unexpected turns. A detective story might heavily weight resolution (every clue must pay off) and thread balance (every question must be answered before the denouement).

Weights can also change over the course of a single playthrough. Early in a story, the GM might prioritize progress and thread-opening (high progress weight, low resolution penalty). In the climax, shift toward resolution and tension peak. In the denouement, maximize thread-closing and minimize new plants. The scorer does not manage this scheduling -- the caller swaps in different `NarrativeWeights` at the appropriate time.

## How MCTS uses the score

The evaluation loop:

1. **Fork** the sifting engine. `engine.clone()` copies patterns, partial matches, and enabled state. The forked engine starts with a clean tick -- tick accumulators are reset so the fork's first tick delta reflects only the hypothetical events.
2. **Fork the data source.** Clone the graph adapter (or create a scratch copy). The fork needs to accept hypothetical events without contaminating the real graph.
3. **Add hypothetical events** to the forked data source. These represent the candidate action the GM is considering.
4. **Run the engine** on the forked graph. Call `on_edge_added` for each hypothetical event, then `end_tick`. The engine returns a `TickDelta` summarizing what happened.
5. **Collect signals** from the trackers (thread violations, tension trajectory, pivot JSD, surprise scores) and the engine's tick delta (advancements, completions, stalls, resolutions).
6. **Score.** Pass signals and weights to the scorer. Get back a `NarrativeScore` with total and per-signal breakdown.
7. **Compare** scores across candidate branches. The highest total wins.
8. **Discard** the fork. The main engine and data source are untouched.

The key design enabler is that `SiftEngine` is decoupled from `DataSource`. The engine does not hold a reference to the graph -- it takes `&impl DataSource` as a method parameter. This means you can clone the engine once and call it against different data sources in different branches without any lifetime entanglement.

The scorer's explainable breakdown is useful beyond just picking a winner. If one branch scores high on progress but low on resolution, the GM knows it is advancing the plot without paying anything off. If every branch scores negatively on tension fit, the GM's desired trajectory may be unreachable given the current simulation state. These diagnostics inform weight tuning and action design beyond the immediate selection decision.

Note that `fabula-narratives` provides the evaluation function but not the search. MCTS implementation -- tree policy, expansion, backpropagation, selection -- is the caller's responsibility. The crate gives you the leaf evaluator; you supply the tree.

## Connection to research

Each component in `fabula-narratives` traces to published research. The crate does not invent new narrative theory -- it operationalizes existing theory into computable signals.

- **Composite scorer**: Nelson, M. J. & Mateas, M. (2005). "Search-Based Drama Management in the Interactive Fiction Anchorhead." AIIDE 2005. The GM-as-optimizer model and the idea of a quality function over narrative states. The original paper used hand-tuned heuristics; fabula makes the quality function modular and weight-configurable.
- **Thread tracking**: Kowal, M. R. MICE Quotient (Writing Excuses). Structural nesting of narrative threads as a quality signal. The MICE model is a craft heuristic from fiction writing, not a formal system. Fabula formalizes the FILO property as a computable structural invariant.
- **Tension tracking**: Booth, M. (2009). "The AI Systems of Left 4 Dead." GDC 2009. Pacing via trajectory sampling and desired-vs-actual comparison. Ely, J., Frankel, A., & Kamenica, E. (2015). "Suspense and Surprise." *Journal of Political Economy*. Trajectory matters more than level for suspense. The economic model of suspense as expected future variance informs the design decision to score trajectory fit rather than absolute tension.
- **Pivot detection**: Schulz, K. et al. (2024). "Narrative Information Theory." arXiv:2411.12907. JSD between consecutive event distributions as a pivot measure. Of the five measures proposed (Complexity, Pivot, Predictability, Suspense, Plot Twist), fabula currently implements Pivot. The others are candidates for future signals.
- **Surprise scoring**: Kreminski, M. et al. (2022). "Select the Unexpected: A Statistical Heuristic for Story Sifting." ICIDS 2022. Shannon surprise and the StU heuristic for ranking pattern matches by unexpectedness.

---

**See also:**
- [Narrative Scoring Reference](../reference/narratives) -- API documentation for all tracker and scorer types
- [Scoring and Surprise](../reference/scoring) -- pattern-level and sequential surprise scoring
- [How the Engine Works](./how-the-engine-works) -- the 4-phase incremental algorithm that feeds the trackers
- [Research Lineage](../research) -- the full academic genealogy from Felt to fabula
