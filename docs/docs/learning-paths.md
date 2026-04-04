---
sidebar_position: 31
title: Learning Paths
---

# Learning Paths

Two structured tracks through the documentation. Pick the one that matches your goal.

## Track 1: Narrative AI Developer

Build a narrative sifting and scoring pipeline for games or simulations.

**Total time: ~2.5 hours**

| # | Page | Time | What you learn |
|---|------|------|----------------|
| 1 | [Getting Started](/docs/getting-started) | 10 min | Build and evaluate your first pattern in batch and incremental mode. |
| 2 | [What is Sifting](/docs/learn/what-is-sifting) | 10 min | The concept of sifting and why it matters across domains. |
| 3 | [Patterns from First Principles](/docs/learn/patterns-from-first-principles) | 15 min | Mental model: stages, clauses, variables, joins, negation. |
| 4 | [Sifting by Example](/docs/learn/sifting-by-example) | 15 min | The same pattern across 4 domains, with interactive playgrounds. |
| 5 | [Pattern Cookbook](/docs/guides/pattern-cookbook) | 20 min | Worked recipes for common pattern types. |
| 6 | [Scoring and Surprise](/docs/concepts/scoring-and-surprise) | 15 min | Information theory for ranking matches by rarity. |
| 7 | [Incremental Integration](/docs/guides/incremental-integration) | 15 min | Wire fabula into a simulation loop. |
| 8 | [Narrative Quality](/docs/concepts/narrative-quality) | 15 min | Threads, tension, pivots, and composite scoring for MCTS. |
| 9 | [Scoring Matches](/docs/guides/scoring-matches) | 15 min | Full observe → score → rank workflow. |
| 10 | [Forking for MCTS](/docs/guides/forking-for-mcts) | 15 min | Clone the engine, speculate, score, select. |

After completing this track, you can detect narrative patterns in a running simulation, rank them by surprise, and use scoring to drive an AI director.

## Track 2: Engine Integrator

Integrate fabula into an existing system as a temporal pattern detection engine.

**Total time: ~90 minutes**

| # | Page | Time | What you learn |
|---|------|------|----------------|
| 1 | [Getting Started](/docs/getting-started) | 10 min | Build and evaluate your first pattern in batch and incremental mode. |
| 2 | [Sifting by Example](/docs/learn/sifting-by-example) | 10 min | The same pattern concept across 4 domains. |
| 3 | [How the Engine Works](/docs/concepts/how-the-engine-works) | 15 min | The 4-phase incremental algorithm, forking, deduplication. |
| 4 | [Incremental Integration](/docs/guides/incremental-integration) | 15 min | Wire fabula into your event loop. |
| 5 | [DSL in Rust](/docs/guides/dsl-in-rust) | 10 min | Parse, compile, and evaluate DSL patterns in Rust. |
| 6 | [Debugging Patterns](/docs/guides/debugging-patterns) | 10 min | Troubleshoot unmatched patterns with gap analysis. |
| 7 | [Custom Adapter](/docs/guides/custom-adapter) | 15 min | Implement DataSource for your graph store. |
| 8 | [Design Decisions](/docs/concepts/design-decisions) | 10 min | Why fabula is built the way it is — tradeoffs and alternatives. |

After completing this track, you can integrate fabula into any event-producing system, define patterns in Rust or DSL, debug failures, and write a custom graph adapter.

## Choosing a track

**Pick Track 1** if you're building a game, simulation, or interactive narrative system and want to detect and score narrative events.

**Pick Track 2** if you're integrating fabula into an existing system (monitoring, compliance, process mining) and need to understand the engine's behavior and integration points.

Both tracks start with Getting Started. You can switch tracks at any point -- the pages are self-contained.

