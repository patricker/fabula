---
sidebar_position: 1
title: "Build: Overview"
---

# Build a Simulation Monitor

A 6-chapter project tutorial. You will build a trading simulation that generates events, detect patterns with fabula's incremental engine, react to matches in real time, score them by surprise, and use engine forking for speculative "what-if" analysis.

| | |
|---|---|
| **Total time** | ~2 hours |
| **Difficulty** | Intermediate |
| **Prerequisites** | [Getting Started](/docs/getting-started), basic Rust |

## What you'll build

A command-line program that:

1. Runs a simple trading simulation producing timestamped events (trades, price changes, alerts)
2. Detects three patterns: insider trading (sequence + negation), market manipulation (repeat-range), and flash crash (concurrent group)
3. Reacts to `SiftEvent::Completed` and `SiftEvent::Negated` as they occur
4. Uses `why_not` to explain near-misses
5. Scores matches with `SurpriseScorer` and `StuScorer`
6. Forks the engine to evaluate hypothetical "what if this trade happened?" scenarios

## Chapters

| # | Chapter | What you learn |
|---|---------|----------------|
| 1 | [Simulation Loop](01-simulation-loop) | Build an event-producing simulation with MemGraph |
| 2 | [Define Patterns](02-define-patterns) | Write 3 patterns using both builder API and DSL |
| 3 | [Incremental Matching](03-incremental-matching) | Wire the engine into the simulation loop |
| 4 | [React to Events](04-react-to-events) | Handle SiftEvents, drain matches, use deadlines |
| 5 | [Score and Rank](05-score-and-rank) | Add surprise scoring to rank matches |
| 6 | [Speculate with MCTS](06-speculate-with-mcts) | Fork the engine for what-if analysis |

Each chapter builds on the previous one. Complete code at the end of each chapter.

## Setup

```bash
cargo new fabula-monitor
cd fabula-monitor
```

Add to `Cargo.toml`:

```toml
[dependencies]
fabula = { path = "../fabula/crates/fabula" }
fabula-memory = { path = "../fabula/crates/fabula-memory" }
fabula-dsl = { path = "../fabula/crates/fabula-dsl" }
fabula-narratives = { path = "../fabula/crates/fabula-narratives" }
```

Or, if using published crates:

```toml
[dependencies]
fabula = "0.1"
fabula-memory = "0.1"
fabula-dsl = "0.1"
fabula-narratives = "0.1"
```

[Start Chapter 1 →](01-simulation-loop)
