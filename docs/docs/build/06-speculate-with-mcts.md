---
sidebar_position: 7
title: "6. Speculate with MCTS"
---

# 6. Speculate with MCTS

Chapter 5 scored completed matches. Now you will fork the engine to ask "what if?" before committing to an action. Clone the engine, add a hypothetical event to a separate graph, score the result, and compare alternatives. The original engine is never touched.

## How forking works

`engine.clone()` copies all patterns, partial matches, enabled flags, completion counts, and lifecycle metrics. Tick accumulators are intentionally left empty -- the forked engine starts with a clean slate for its speculative timeline.

Pair the cloned engine with a cloned `MemGraph`. Add your hypothetical event to the cloned graph, call `on_edge_added()` on the cloned engine, and score what happens. The original engine and graph are unaffected.

## Complete code

```rust
use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

fn add_event(g: &mut MemGraph, id: &str, action: &str, actor: &str, stock: &str, tick: i64) {
    g.add_str(id, "action", action, tick);
    g.add_ref(id, "actor", actor, tick);
    g.add_ref(id, "stock", stock, tick);
    g.set_time(tick);
}

fn insider_trading_pattern() -> Pattern<String, MemValue> {
    PatternBuilder::new("insider_trading")
        .stage("tip", |s| {
            s.edge("tip", "action".into(), MemValue::Str("insider_tip".into()))
                .edge_bind("tip", "actor".into(), "suspect")
                .edge_bind("tip", "stock".into(), "ticker")
        })
        .stage("trade", |s| {
            s.edge("trade", "action".into(), MemValue::Str("trade".into()))
                .edge_bind("trade", "actor".into(), "suspect")
                .edge_bind("trade", "stock".into(), "ticker")
        })
        .unless_between("tip", "trade", |neg| {
            neg.edge("disclosure", "action".into(), MemValue::Str("alert".into()))
                .edge_bind("disclosure", "stock".into(), "ticker")
        })
        .build()
}

fn flash_crash_pattern() -> Pattern<String, MemValue> {
    PatternBuilder::new("flash_crash")
        .unordered_group(|g| {
            g.stage("drop", |s| {
                s.edge("drop", "action".into(), MemValue::Str("price_change".into()))
                    .edge_bind("drop", "stock".into(), "ticker")
            })
            .stage("alarm", |s| {
                s.edge("alarm", "action".into(), MemValue::Str("alert".into()))
                    .edge_bind("alarm", "stock".into(), "ticker")
            })
        })
        .stage("sell", |s| {
            s.edge("sell", "action".into(), MemValue::Str("trade".into()))
                .edge_bind("sell", "stock".into(), "ticker")
        })
        .build()
}

fn pump_dump_pattern() -> Pattern<String, MemValue> {
    PatternBuilder::new("pump_and_dump")
        .stage("pump", |s| {
            s.edge("pump", "action".into(), MemValue::Str("trade".into()))
                .edge_bind("pump", "actor".into(), "manipulator")
                .edge_bind("pump", "stock".into(), "ticker")
        })
        .stage("hype", |s| {
            s.edge("hype", "action".into(), MemValue::Str("promote".into()))
                .edge_bind("hype", "actor".into(), "manipulator")
                .edge_bind("hype", "stock".into(), "ticker")
        })
        .stage("dump", |s| {
            s.edge("dump", "action".into(), MemValue::Str("sell".into()))
                .edge_bind("dump", "actor".into(), "manipulator")
                .edge_bind("dump", "stock".into(), "ticker")
        })
        .build()
}

/// Fork the engine, add a hypothetical event, and score the result.
/// Returns (completed pattern names, total surprise score).
fn evaluate_hypothesis(
    engine: &SiftEngineFor<MemGraph>,
    graph: &MemGraph,
    surprise: &SurpriseScorer,
    hyp_id: &str,
    action: &str,
    actor: &str,
    stock: &str,
    tick: i64,
) -> (Vec<String>, f64) {
    let mut forked_engine = engine.clone();
    let mut forked_graph = graph.clone();

    add_event(&mut forked_graph, hyp_id, action, actor, stock, tick);

    let events = forked_engine.on_edge_added(
        &forked_graph,
        &hyp_id.to_string(),
        &"action".to_string(),
        &MemValue::Str(action.to_string()),
        &Interval::open(tick),
    );

    let completed: Vec<String> = events.iter().filter_map(|e| {
        if let SiftEvent::Completed { pattern, .. } = e { Some(pattern.clone()) } else { None }
    }).collect();

    let drained = forked_engine.drain_completed();
    let scored = surprise.score(&drained, forked_engine.patterns());
    let total: f64 = scored.iter().map(|s| s.surprise).sum();

    (completed, total)
}

fn main() {
    let mut graph = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    let idx_insider = engine.register(insider_trading_pattern());
    let idx_crash = engine.register(flash_crash_pattern());
    let idx_pump = engine.register(pump_dump_pattern());

    let mut surprise = SurpriseScorer::new();
    surprise.set_baseline(idx_insider, 0.1);
    surprise.set_baseline(idx_crash, 0.05);
    surprise.set_baseline(idx_pump, 0.15);

    // -- run setup: build state up to a decision point (tick 4) --
    let setup: Vec<Vec<(&str, &str, &str)>> = vec![
        vec![("insider_tip", "alice", "ACME"), ("trade", "carol", "ACME")],
        vec![("promote", "carol", "ACME"), ("price_change", "market", "ACME")],
        vec![("alert", "system", "ACME")],
        vec![("insider_tip", "bob", "ZINC")],
    ];

    let mut event_id = 0;
    for (tick_idx, tick_events) in setup.iter().enumerate() {
        let tick = (tick_idx + 1) as i64;
        for &(action, actor, stock) in tick_events {
            let id = format!("ev{}", event_id);
            add_event(&mut graph, &id, action, actor, stock, tick);
            let events = engine.on_edge_added(
                &graph,
                &id,
                &"action".to_string(),
                &MemValue::Str(action.to_string()),
                &Interval::open(tick),
            );
            surprise.observe_events(&events, engine.patterns());
            event_id += 1;
        }
        surprise.tick();
        engine.end_tick(50);
        engine.drain_completed();
    }

    println!("=== state at decision point (tick 4) ===");
    println!("active partial matches: {}", engine.partial_matches().len());
    println!("tick counter: {}", engine.current_tick());

    // -- define 3 hypothetical actions at tick 5 --
    let hypotheses: Vec<(&str, &str, &str, &str)> = vec![
        ("A: alice trades ACME",  "trade",  "alice",  "ACME"),
        ("B: bob trades ZINC",    "trade",  "bob",    "ZINC"),
        ("C: carol sells ACME",   "sell",   "carol",  "ACME"),
    ];

    // -- fork, evaluate, score each --
    println!("\n=== speculative evaluation (tick 5) ===");
    let mut results: Vec<(&str, Vec<String>, f64)> = Vec::new();

    for (label, action, actor, stock) in &hypotheses {
        let (completed, score) = evaluate_hypothesis(
            &engine, &graph, &surprise, "hyp", action, actor, stock, 5,
        );
        println!("{}", label);
        if completed.is_empty() {
            println!("  no completions");
        } else {
            for p in &completed {
                println!("  completed: {}", p);
            }
        }
        println!("  surprise: {:.2} bits", score);
        results.push((label, completed, score));
    }

    // -- pick the most surprising --
    results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    println!("\nbest hypothesis: {}", results[0].0);

    // -- verify original engine is untouched --
    println!("\n=== original engine (unchanged) ===");
    println!("active partial matches: {}", engine.partial_matches().len());
    println!("tick counter: {}", engine.current_tick());
    let stale = engine.drain_completed();
    println!("completed in original: {}", stale.len());
}
```

## Expected output

```text
=== state at decision point (tick 4) ===
active partial matches: ...
tick counter: 4

=== speculative evaluation (tick 5) ===
A: alice trades ACME
  completed: insider_trading
  surprise: ... bits
B: bob trades ZINC
  completed: insider_trading
  surprise: ... bits
C: carol sells ACME
  completed: pump_and_dump
  surprise: ... bits

best hypothesis: ...

=== original engine (unchanged) ===
active partial matches: ...
tick counter: 4
completed in original: 0
```

Key outcomes:
- **Hypothesis A** (alice trades ACME): the insider_trading pattern was negated by the ACME alert at tick 3, so this may not complete -- or it starts a fresh match depending on engine state. The exact behavior depends on whether the negation killed all active PMs for alice/ACME.
- **Hypothesis B** (bob trades ZINC): completes insider_trading for bob/ZINC (tipped at tick 4, no alert on ZINC).
- **Hypothesis C** (carol sells ACME): completes pump_and_dump (carol bought at tick 1, promoted at tick 2, sells now).
- The original engine has the same number of active partial matches and tick counter as before forking. Zero completions in the original.

## Connection to MCTS

This fork-speculate-score loop is the core of Monte Carlo Tree Search for narrative generation:

1. **Select** a decision point (the simulation reaches a moment where multiple actions are possible).
2. **Expand** by cloning the engine once per candidate action.
3. **Simulate** by calling `on_edge_added()` on each fork with the hypothetical event.
4. **Evaluate** with the scoring pipeline (SurpriseScorer, StuScorer, SequentialScorer, or a composite NarrativeScore from `fabula-narratives`).
5. **Backpropagate** by choosing the best-scoring action and committing it to the real engine.

Each hypothesis is a branch in the search tree. The surprise score is the evaluation function. `engine.clone()` is cheap -- it copies patterns, partial matches, and counters, not the graph. The graph is your simulation state; fork it however your simulation layer requires.

For deeper tree search, nest the loop: commit the best action to a fork, advance its simulation, fork again at the next decision point. The engine's `Clone` is designed for exactly this pattern.

## What you learned

This chapter and the full tutorial:

- **Chapter 1** -- Built a simulation loop producing timestamped events into a MemGraph.
- **Chapter 2** -- Defined patterns with PatternBuilder (insider trading with negation, flash crash with concurrent group, pump-and-dump sequence) and the DSL.
- **Chapter 3** -- Wired the engine into the loop with `on_edge_added()` for incremental matching. Learned the 4-phase algorithm.
- **Chapter 4** -- Handled SiftEvent variants, drained completed matches, used deadlines for expiry, and ran gap analysis.
- **Chapter 5** -- Scored matches with SurpriseScorer (pattern-level), StuScorer (property-level, ArithmeticMean vs TfIdf), and SequentialScorer (transition-level).
- **Chapter 6** -- Forked the engine with `clone()`, evaluated hypothetical actions on separate graphs, scored and compared results, confirmed the original was untouched.

## Where to go next

- [How the Engine Works](/docs/concepts/how-the-engine-works) -- The 4-phase algorithm, forking, deduplication, partial match lifecycle.
- [Scoring and Surprise](/docs/concepts/scoring-and-surprise) -- Information theory foundations for the scoring pipeline.
- [DSL Reference](/docs/reference/dsl) -- Complete syntax reference for patterns, graphs, and compose operators.
- [fabula-narratives](/docs/reference/narratives) -- Composite NarrativeScore combining thread tracking (MICE), tension arcs, pivot detection, and the surprise scorers from this tutorial into a single evaluation function for MCTS.
