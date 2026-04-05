# Automated Sifting Pattern Discovery — Design Spec

## Problem Statement

All existing story sifting systems (Felt, Winnow, fabula) require hand-authored patterns. The authoring burden is the single largest bottleneck. A designer must anticipate every interesting narrative situation and write a pattern for it. Pattern discovery would let the system find "this unusual combination of events keeps happening and it's statistically surprising" without pre-specification.

No published work solves this problem. Kreminski's Synthesifter (AIIDE 2020 Workshop) used ILP to synthesize Felt-style patterns from user-provided examples but remained a prototype — no temporal intervals, no negation, no follow-up. The field is wide open.

## Design Goal

A **framework** — traits, pipelines, and evaluation harness — that lets you plug in different discovery strategies (statistical mining, narrative-scored, gap-based, LLM-assisted) and compare them against the same corpus with the same metrics. The research is *which strategy works*; the engineering is making them swappable and measurable.

The framework ships with at least one concrete generator to validate the design is not abstract nonsense.

## Architecture: Generate-Evaluate Loop

The system works as an iterative loop:

```
    ┌─────────────────────────────────────────────┐
    │              DiscoverySession                │
    │                                              │
    │   ┌───────────┐      ┌───────────────┐      │
    │   │ Generator  │─────▶│  Candidates   │      │
    │   │ (pluggable)│      │  Vec<Pattern> │      │
    │   └───────────┘      └──────┬────────┘      │
    │        ▲                    │               │
    │        │                    ▼               │
    │   ┌────┴──────┐      ┌───────────────┐      │
    │   │  Scored    │◀─────│  Evaluator(s) │      │
    │   │  Feedback  │      │  (pluggable)  │      │
    │   └───────────┘      └──────┬────────┘      │
    │                             │               │
    │                             ▼               │
    │                      ┌───────────────┐      │
    │                      │   Filter      │      │
    │                      │  (pluggable)  │      │
    │                      └──────┬────────┘      │
    │                             │               │
    │                             ▼               │
    │                      ┌───────────────┐      │
    │                      │   Accepted    │      │
    │                      │   Patterns    │      │
    │                      └───────────────┘      │
    └─────────────────────────────────────────────┘
```

The key difference from a linear pipeline: the **generator receives feedback**. Scored results flow back to the generator, enabling strategies like evolutionary search (mutate high-scoring patterns), LLM refinement ("here's a pattern that scored 0.7, improve it"), and gap-directed search (gap_analysis tells the generator what's missing).

### Why This Architecture

The generate-evaluate loop is the standard framing in program synthesis research (FunSearch, Reflexion, CodeRL). It's the simplest architecture that supports feedback, which is essential for LLM-in-the-loop and evolutionary strategies. A linear pipeline can't express "generate more patterns like the ones that scored well" without hacks. A blackboard architecture (shared state, independent agents) is more flexible but harder to reason about and debug — premature for the initial research phase.

## Core Components

### TraceCorpus

The raw material discovery mines over. Built from a DataSource or deserialized from a trace log.

Provides:
- All edges, with temporal ordering
- Index by label (which edge types exist, how frequently)
- Index by node (which entities participate in which edges)
- Time range and temporal splitting (train/test holdout)
- Label vocabulary and co-occurrence statistics

**Input mode**: Offline first. The corpus is a snapshot of a completed (or partially completed) simulation. Online/incremental discovery is a separate, harder problem that can be layered on once the batch algorithms work.

### CandidateGenerator

Proposes candidate patterns. Receives scored feedback to guide the next round.

The trait contract:
- `generate(budget) → Vec<Pattern>` — propose up to `budget` candidates
- `feedback(scored_results)` — receive scored patterns from the previous round

Different generators implement different strategies. The framework doesn't care which. A generator may maintain internal state across rounds (population of high-scoring patterns, frequency tables, conversation history with an LLM).

### PatternEvaluator

Scores a candidate pattern against a corpus. Multiple evaluators can run in parallel on the same candidate.

Evaluator strategies (all pluggable, research decides which matter):

- **Narrative quality** — register the pattern in a temporary SiftEngine, run against the corpus, score via fabula-narratives (tension, pivots, plant/payoff, surprise). Uses the existing scorer as an oracle.
- **Holdout generalization** — split corpus into train/test. Discover on training portion, measure whether patterns fire on held-out portion. Patterns that generalize are interesting; patterns that only match training data are noise.
- **Statistical surprise** — MINERful's interest factor: `(support × confidence) / expected_probability_under_independence`. Measures how much a pattern deviates from what you'd expect by chance.
- **Human-in-the-loop** — surface discovered patterns to a person who accepts/rejects/edits. Track acceptance rate as quality metric.
- **LLM-as-judge** — prompt an LLM with the pattern, its matches, and the simulation context. Ask whether this is narratively interesting and why.

A `PatternScore` struct holds per-evaluator scores plus metadata (which round, which generator).

### PatternFilter

Decides whether a scored pattern is worth keeping. Deliberately separate from evaluation — you might score with five evaluators but filter on a composite threshold.

### DiscoverySession

Orchestrates the loop. Tracks the full history: every candidate proposed, every score received, every pattern accepted or rejected. Enforces budget (max rounds, max candidates, wall-clock time). Makes research reproducible — you can replay a session to understand why a particular pattern was or wasn't found.

### PatternEmitter

Outputs accepted patterns. Two forms:
- As `Pattern<L, V>` objects (for direct registration in a SiftEngine)
- As fabula DSL text (for human review and version control)

The DSL emitter is new infrastructure — fabula currently has no `pattern_to_dsl()` reverse compiler. This is ~100-200 lines of pretty-printing: iterate stages, emit clauses with labels/constraints/bindings, emit temporal constraints, emit negation windows. The compiler is one-way today (AST → Pattern); the emitter doesn't need to round-trip through the parser.

## Concrete Generator Strategies

The framework needs at least one concrete generator to validate the design. The research literature points to several viable strategies, ordered by implementation complexity:

### Strategy 1: MINERful-Adapted (First Implementation)

Adapt MINERful's single-pass constraint discovery for Allen relations over a DataSource. This is the recommended first generator because it's well-understood, efficient, and directly produces patterns.

**Algorithm:**

1. **Knowledge base construction** — single pass over the corpus:
   - For each edge label pair (A, B), compute the distribution of Allen relations between A-edges and B-edges that share a node variable (same source or same target).
   - Track support (how often the pair co-occurs), confidence (how often the Allen relation holds when the pair co-occurs), and interest factor (deviation from independence).

2. **Constraint discovery** — for each pair exceeding thresholds:
   - Emit a candidate two-stage pattern with the observed Allen relation as a temporal constraint.
   - Variable bindings come from the shared node (the entity that participates in both edges).

3. **Multi-stage assembly** — chain pairwise constraints:
   - If A→B shares variable `?x` and B→C shares variable `?y`, and A also binds `?y`, assemble a three-stage pattern with two join variables.
   - Use transitivity of Allen relations to verify temporal consistency.

4. **Negation refinement** — for each discovered pattern:
   - Test whether adding `unless_between` windows (for edge types that occur between stages but reduce match quality) improves the interest factor.
   - This is a second pass: for each candidate negation clause, compare the pattern's score with and without the negation.

**Why this first**: Single-pass, well-studied algorithm. Directly produces Declare-style constraints that map to fabula patterns. The interest factor connects to fabula-narratives' surprise scoring. Validates the entire framework pipeline without requiring research breakthroughs.

### Strategy 2: TIRP Mining (Allen-Native)

Time-Interval Related Pattern mining (KarmaLego, FastTIRP) discovers frequent patterns over interval-based events using Allen's temporal relations. fabula already speaks Allen algebra, so this is a natural fit.

**Key difference from Strategy 1**: TIRP mining discovers patterns of arbitrary length in a single pass using the KarmaLego algorithm, which exploits transitivity of Allen relations for efficient candidate generation. MINERful-adapted works bottom-up from pairwise constraints; TIRP mining works top-down from frequent intervals.

**Research question**: Which produces more narratively interesting patterns? The framework makes them directly comparable.

### Strategy 3: LLM-Assisted Proposal (FunSearch/Reflexion Hybrid)

Prompt an LLM with:
- 3-5 DSL examples covering the range of constructs (stages, variables, constraints, negation)
- A description of what labels, nodes, and value types exist in the corpus
- The top-K patterns from the current population with their scores and example matches
- For failed patterns: `why_not` gap analysis explaining clause-by-clause what didn't match

The LLM proposes candidate patterns in fabula DSL syntax. The framework parses them (using the existing DSL compiler), evaluates them against the corpus, and feeds scores back.

**Feedback mechanism**: Reflexion-style natural language, not raw scores. "This pattern matched 0 times — the label 'betrayal' never appears in the traces" or "This pattern matched 847 times — too general, add temporal constraints or a negation window." fabula's `why_not` gap analysis provides structured, clause-level feedback that's richer than most synthesis systems offer.

**Syntactic validity**: Either grammar-constrained decoding (Outlines, llama.cpp grammars) if running locally, or validate-and-retry with compiler error messages if using an API model. Expect ~20-40% syntax errors without constraints (typical for DSL generation).

**Research questions**: Does the LLM's "narrative intuition" (trained on fiction) transfer to structured event traces? Is LLM proposal competitive with statistical mining, or better as a refinement step on statistically-discovered patterns?

### Strategy 4: Evolutionary Refinement

Maintain a population of patterns. Each round:
- Select high-scoring parents
- Mutate (add/remove a stage, tighten/loosen a constraint, add/remove negation)
- Crossover (combine stages from two patterns using `compose::sequence` or `compose::choice`)
- Evaluate offspring, replace low-scoring population members

Can use an LLM as the mutation operator (ELM/EvoPrompting style) instead of random structural mutations. The LLM understands which mutations are semantically meaningful.

**Research question**: Does evolutionary refinement find patterns that single-pass mining misses?

## What Fabula Already Provides

The infrastructure gap is smaller than it appears. Verified existing components:

| Component | Location | Discovery Role |
|---|---|---|
| `evaluate_pattern()` | `engine/free.rs` | Test candidates without engine registration |
| `evaluate_pattern_at()` | `engine/free.rs` | Point-in-time speculative evaluation |
| `evaluate_pattern_first()` | `engine/free.rs` | Early-termination O(1) existence check |
| `evaluate_pattern_limit()` | `engine/free.rs` | Capped result sets for expensive patterns |
| `gap_analysis()` | `engine/free.rs` | Clause-level breakdown of why a pattern didn't match |
| `gap_analysis().closeness()` | `engine/free.rs` | 0-1 fraction of matched clauses |
| `why_not()` | `engine/eval.rs` | Structured feedback for LLM generators |
| `SurpriseScorer` | `fabula-narratives` | Shannon surprise per-pattern |
| `SequentialScorer` | `fabula-narratives` | Bigram transition model |
| StU (property surprise) | `fabula-narratives` | Surprise by binding rarity |
| `NarrativeSignals` | `fabula-narratives` | 9+ scoring signals |
| `scorer::score()` | `fabula-narratives` | Pure `(signals, weights) → NarrativeScore` |
| `PatternBuilder` | `fabula/builder.rs` | Fluent API for constructing patterns programmatically |
| `compose::sequence/choice/repeat` | `fabula/compose.rs` | Assembling discovered sub-patterns |
| DSL compiler | `fabula-dsl` | Parsing LLM-generated DSL text |
| Pattern serde | `fabula/pattern.rs` | Serialize/deserialize patterns as JSON |

**What's missing:**
- `TraceCorpus` abstraction (edge log with indexing)
- `pattern_to_dsl()` reverse compiler (~100-200 LOC pretty-printer)
- The framework traits and session orchestration
- At least one concrete generator (Strategy 1)
- Evaluation harness wiring

## The Novel Research Contribution

The core open problem identified across all four literature surveys: **first-order temporal pattern mining with variable bindings**. All existing approaches handle some of what fabula needs:

| Approach | Temporal | Graph Structure | Data Conditions | Variable Bindings | Negation |
|---|---|---|---|---|---|
| Sequential pattern mining | ordering only | no | no | no | no |
| Episode mining | windows | no | no | no | no |
| TIRP mining | Allen algebra | no | no | no | no |
| Subgraph mining | some | yes | no | partial | no |
| Declare/MINERful | LTL | no | recent work | case-ID only | positive only |
| Object-centric PM | ordering | multi-object | yes | closest | no |
| **fabula patterns** | **Allen + metric gaps** | **labeled edges** | **value constraints** | **first-order vars** | **3 negation forms** |

No existing approach integrates all five columns. Adapting MINERful for Allen relations with variable join analysis and negation refinement would be a genuine contribution — publishable at AIIDE, FDG, or ICIDS.

## Six Research Gaps (Publishable Directions)

1. **TIRP mining connected to story sifting** — no one has applied temporal interval pattern mining to narrative simulation output
2. **Incremental pattern discovery** — online discovery as edges arrive, not just batch archaeology
3. **Composable discovered patterns** — auto-compose discovered base patterns via sequence/choice/repeat
4. **Negation-aware discovery** — discovering `unless_between` constraints that make patterns more precise
5. **Surprise-aware discovery** — interestingness beyond frequency, connecting to StU and narrative scoring
6. **LLM-assisted pattern proposal** — using `why_not` gap analysis as structured feedback to an LLM proposing DSL patterns

## Evaluation Criteria

The framework supports pluggable evaluators. Four classes, all composable:

1. **Automated proxy** — fabula-narratives scoring: surprise, tension, pivots, plant/payoff. No human needed. Trusts the scorer as ground truth.
2. **Corpus holdout** — train/test split. Discover on training portion, measure generalization on held-out portion.
3. **Human-in-the-loop** — discovered patterns surfaced for accept/reject/edit. Track acceptance rate.
4. **LLM-as-judge** — prompted with pattern + matches + context. Judges narrative interestingness.

Quality metrics (adapted from process mining's established framework):
- **Support**: how often the pattern fires (too low = noise, too high = trivial)
- **Confidence**: how often the temporal/negation constraints hold when the base co-occurrence is present
- **Interest factor**: deviation from independence (surprise)
- **Specificity**: patterns with more stages/constraints/variables score higher than trivial ones
- **Generalization**: holdout performance — does the pattern find new matches, not just memorize training data?
- **Novelty**: does the pattern find different events than existing hand-authored patterns?

## Scope and Boundaries

**In scope:**
- Framework traits and session orchestration
- TraceCorpus abstraction
- DSL reverse compiler (pattern_to_dsl)
- One concrete generator (MINERful-adapted) as proof of concept
- Evaluation harness with at least narrative-quality and statistical-surprise evaluators
- Serializable session history for reproducibility

**Out of scope (future work):**
- Incremental/online discovery (layer on after batch works)
- Grammar-constrained LLM decoding integration (depends on inference setup)
- GUI for human-in-the-loop evaluation (CLI/log-based first)
- Automatic composition of discovered patterns (manual review first)
- Integration with salience-mcts (discovered patterns as MCTS evaluation targets)

## Crate Location

Likely `fabula-discovery` in the fabula workspace. It depends on:
- `fabula` (Pattern, SiftEngine, gap_analysis, evaluate_pattern)
- `fabula-narratives` (scorer, surprise)
- `fabula-memory` (MemGraph as default DataSource for testing)
- `fabula-dsl` (parsing LLM-generated patterns, DSL emission)

This follows the same pattern as fabula-narratives: a workspace crate that consumes fabula's public API for a higher-level purpose.

## Prior Art References

### Story Sifting
- Kreminski et al., "Felt: A Simple Story Sifter" (ICIDS 2019)
- Kreminski et al., "Winnow: A Domain-Specific Language for Incremental Story Sifting" (AIIDE 2021)
- Kreminski, "Synthesifter: Toward Example-Driven Program Synthesis of Story Sifting Patterns" (AIIDE 2020 Workshop)
- Kreminski, "Select the Unexpected" / StU (ICIDS 2022)
- Kreminski, "Composable Story Sifting Patterns" (FDG 2025)
- Samuel et al., "A Quantified Analysis of Bad News" (ICIDS 2021)
- Leong et al., "Arc Sift: Automated Sifting of Stories" (IJCAI 2022)
- Clothier & Millard, "Awash: Prospective Story Sifting" (ICIDS 2023)

### Temporal Pattern Mining
- Mannila et al., "Discovery of Frequent Episodes in Event Sequences" (DMKD 1997)
- Fournier-Viger et al., "FastTIRP" (BDA 2022) — Allen-relation temporal interval pattern mining
- Fournier-Viger et al., "A Survey of Pattern Mining in Dynamic Graphs" (WIREs 2020)
- Paranjape et al., "Motifs in Temporal Networks" (WSDM 2017)
- Abdelhamid et al., "IncGM+: Incremental Frequent Subgraph Mining" (ICDE 2018)
- Yang et al., "Diversified Temporal Subgraph Pattern Mining" (KDD 2016)

### Process Mining
- Di Ciccio & Mecella, "MINERful" (2015) — single-pass Declare constraint mining
- Pesic & van der Aalst, "Declare" (2007) — declarative process constraints via LTLf
- Tax et al., "Local Process Model Mining" (Information Systems 2016)
- Van der Aalst, "Object-Centric Process Mining" (2020+)
- Nguyen et al., "Business Process Deviance Mining" (2014-2016)

### Temporal Logic Learning
- Neider & Gavran, "Learning Linear Temporal Properties" (FMCAD 2018)
- Raha et al., "Scarlet: Scalable Anytime Algorithms for Learning Fragments of LTL" (TACAS 2022)
- Neider & Roy, "Survey on Mining LTL Specifications" (2025)

### LLM-Assisted Synthesis
- Romera-Paredes et al., "FunSearch" (Nature 2024) — evolutionary LLM program synthesis
- Shinn et al., "Reflexion" (NeurIPS 2023) — natural language feedback for LLM refinement
- Lehman et al., "Evolution through Large Models" (2023) — LLMs as evolutionary operators
- Cosler et al., "nl2spec" (2023) — LLM translation to temporal logic
