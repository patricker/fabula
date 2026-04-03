//! Divan benchmarks for fabula engine.
//!
//! Two tiers:
//! - GM-profile: "is fabula fast enough for a 60fps GM?" (fixed 30 patterns, 5K edges)
//! - Scaling: "where does it break?" (sweep one dimension at a time)
//!
//! Workload construction is in `with_inputs` (unmeasured setup).
//! Only `on_edge_added` / `evaluate` calls are timed.

use divan::Bencher;
use fabula_bench::{build_isolated_workload, WorkloadConfig, IsolatedWorkload};
use fabula_test_suite::{MemGraph, PetGraph, TestGraph};

fn main() {
    divan::main();
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Feed all pending edges into the engine. Takes ownership to avoid clone.
fn feed_edges<G: TestGraph>(
    graph: &mut G,
    engine: &mut fabula::prelude::SiftEngineFor<G>,
    edges: Vec<fabula_bench::PendingEdge>,
) {
    // Insert all edges first so secondary clauses are visible
    for edge in &edges {
        edge.insert(graph);
    }
    for edge in &edges {
        edge.notify(graph, engine);
    }
}

/// Build a workload and warm it up with N ticks of pre-existing notifications,
/// so the engine has active PMs before measurement begins.
fn build_warm_workload<G: TestGraph>(
    config: &WorkloadConfig,
    warm_ticks: usize,
) -> IsolatedWorkload<G> {
    // Build a larger workload with extra edges for warmup
    let warm_config = WorkloadConfig {
        edges_per_tick: config.edges_per_tick,
        // Generate warmup edges + measurement edges
        pre_existing_edges: config.pre_existing_edges,
        ..config.clone()
    };
    let mut w = build_isolated_workload::<G>(&warm_config);

    // Generate and process warmup edges (not measured)
    let mut rng_state: u64 = config.seed.wrapping_mul(7919);
    for tick in 0..warm_ticks {
        let time = (config.pre_existing_edges + tick * config.edges_per_tick) as i64 + 1;
        let mut warmup_edges = Vec::new();
        for i in 0..config.edges_per_tick {
            let edge_time = time + i as i64;
            rng_state ^= rng_state << 13;
            rng_state ^= rng_state >> 7;
            rng_state ^= rng_state << 17;
            let actor_idx = (rng_state % config.character_count as u64) as usize;
            let actor = format!("char_{}", actor_idx);
            let events = ["move", "talk", "trade", "harm", "betray",
                          "observe", "wait", "promise", "steal", "gift"];
            let event_type = events[(rng_state as usize / 3) % events.len()];
            let ev_node = format!("warm_ev_{}_{}", tick, i);
            warmup_edges.push(fabula_bench::PendingEdge::new_str(&ev_node, "eventType", event_type, edge_time));
            warmup_edges.push(fabula_bench::PendingEdge::new_ref(&ev_node, "actor", &actor, edge_time));
        }
        // Insert then notify
        for edge in &warmup_edges {
            edge.insert(&mut w.graph);
        }
        w.graph.set_current_time(time + config.edges_per_tick as i64);
        for edge in &warmup_edges {
            edge.notify(&w.graph, &mut w.engine);
        }
        // Drain completed every 10 ticks
        if tick % 10 == 0 {
            w.engine.drain_completed();
        }
    }

    w
}

// ===========================================================================
// Tier 1: GM-profile benchmarks
// ===========================================================================

mod gm_profile {
    use super::*;

    #[divan::bench(args = [1, 10, 50])]
    fn edges_per_tick_petgraph(bencher: Bencher, edges_per_tick: usize) {
        let config = WorkloadConfig {
            edges_per_tick,
            ..WorkloadConfig::default()
        };
        bencher
            .with_inputs(|| {
                let w = build_isolated_workload::<PetGraph>(&config);
                (w.graph, w.engine, w.pending_edges)
            })
            .bench_values(|(mut g, mut e, edges)| feed_edges(&mut g, &mut e, edges));
    }

    #[divan::bench(args = [1, 10, 50])]
    fn edges_per_tick_memgraph(bencher: Bencher, edges_per_tick: usize) {
        let config = WorkloadConfig {
            edges_per_tick,
            ..WorkloadConfig::default()
        };
        bencher
            .with_inputs(|| {
                let w = build_isolated_workload::<MemGraph>(&config);
                (w.graph, w.engine, w.pending_edges)
            })
            .bench_values(|(mut g, mut e, edges)| feed_edges(&mut g, &mut e, edges));
    }

    #[divan::bench(args = [0.0, 0.5, 1.0])]
    fn negation_fraction_petgraph(bencher: Bencher, frac: f64) {
        let config = WorkloadConfig {
            negation_fraction: frac,
            ..WorkloadConfig::default()
        };
        bencher
            .with_inputs(|| {
                let w = build_isolated_workload::<PetGraph>(&config);
                (w.graph, w.engine, w.pending_edges)
            })
            .bench_values(|(mut g, mut e, edges)| feed_edges(&mut g, &mut e, edges));
    }

    #[divan::bench(args = [0.0, 0.5, 1.0])]
    fn negation_fraction_memgraph(bencher: Bencher, frac: f64) {
        let config = WorkloadConfig {
            negation_fraction: frac,
            ..WorkloadConfig::default()
        };
        bencher
            .with_inputs(|| {
                let w = build_isolated_workload::<MemGraph>(&config);
                (w.graph, w.engine, w.pending_edges)
            })
            .bench_values(|(mut g, mut e, edges)| feed_edges(&mut g, &mut e, edges));
    }

    /// Warm-start: engine has ~100-200 active PMs before measurement.
    /// This is the steady-state cost that matters for real-time GM use.
    #[divan::bench(args = [1, 10])]
    fn warm_edges_per_tick_petgraph(bencher: Bencher, edges_per_tick: usize) {
        let config = WorkloadConfig {
            edges_per_tick,
            ..WorkloadConfig::default()
        };
        bencher
            .with_inputs(|| {
                let w = build_warm_workload::<PetGraph>(&config, 20);
                (w.graph, w.engine, w.pending_edges)
            })
            .bench_values(|(mut g, mut e, edges)| feed_edges(&mut g, &mut e, edges));
    }

    #[divan::bench(args = [1, 10])]
    fn warm_edges_per_tick_memgraph(bencher: Bencher, edges_per_tick: usize) {
        let config = WorkloadConfig {
            edges_per_tick,
            ..WorkloadConfig::default()
        };
        bencher
            .with_inputs(|| {
                let w = build_warm_workload::<MemGraph>(&config, 20);
                (w.graph, w.engine, w.pending_edges)
            })
            .bench_values(|(mut g, mut e, edges)| feed_edges(&mut g, &mut e, edges));
    }
}

// ===========================================================================
// Tier 2: Scaling benchmarks
// ===========================================================================

mod scaling {
    use super::*;

    #[divan::bench(args = [1, 10, 30, 100])]
    fn pattern_count_petgraph(bencher: Bencher, count: usize) {
        let config = WorkloadConfig {
            pattern_count: count,
            pre_existing_edges: 1000,
            ..WorkloadConfig::default()
        };
        bencher
            .with_inputs(|| {
                let w = build_isolated_workload::<PetGraph>(&config);
                (w.graph, w.engine, w.pending_edges)
            })
            .bench_values(|(mut g, mut e, edges)| feed_edges(&mut g, &mut e, edges));
    }

    #[divan::bench(args = [1, 10, 30, 100])]
    fn pattern_count_memgraph(bencher: Bencher, count: usize) {
        let config = WorkloadConfig {
            pattern_count: count,
            pre_existing_edges: 1000,
            ..WorkloadConfig::default()
        };
        bencher
            .with_inputs(|| {
                let w = build_isolated_workload::<MemGraph>(&config);
                (w.graph, w.engine, w.pending_edges)
            })
            .bench_values(|(mut g, mut e, edges)| feed_edges(&mut g, &mut e, edges));
    }

    #[divan::bench(args = [100, 1000, 5000])]
    fn edge_count_petgraph(bencher: Bencher, edges: usize) {
        let config = WorkloadConfig {
            pattern_count: 10,
            pre_existing_edges: edges,
            ..WorkloadConfig::default()
        };
        bencher
            .with_inputs(|| {
                let w = build_isolated_workload::<PetGraph>(&config);
                (w.graph, w.engine, w.pending_edges)
            })
            .bench_values(|(mut g, mut e, edges)| feed_edges(&mut g, &mut e, edges));
    }

    #[divan::bench(args = [100, 1000, 5000])]
    fn edge_count_memgraph(bencher: Bencher, edges: usize) {
        let config = WorkloadConfig {
            pattern_count: 10,
            pre_existing_edges: edges,
            ..WorkloadConfig::default()
        };
        bencher
            .with_inputs(|| {
                let w = build_isolated_workload::<MemGraph>(&config);
                (w.graph, w.engine, w.pending_edges)
            })
            .bench_values(|(mut g, mut e, edges)| feed_edges(&mut g, &mut e, edges));
    }

    #[divan::bench(args = [1, 3, 5])]
    fn stage_count_petgraph(bencher: Bencher, stages: usize) {
        let config = WorkloadConfig {
            pattern_count: 10,
            stages_per_pattern: stages,
            pre_existing_edges: 1000,
            ..WorkloadConfig::default()
        };
        bencher
            .with_inputs(|| {
                let w = build_isolated_workload::<PetGraph>(&config);
                (w.graph, w.engine, w.pending_edges)
            })
            .bench_values(|(mut g, mut e, edges)| feed_edges(&mut g, &mut e, edges));
    }

    /// Batch vs incremental. MemGraph batch at 1K+ is O(E^2) — too slow to bench.
    #[divan::bench(args = [100, 1000])]
    fn batch_petgraph(bencher: Bencher, edges: usize) {
        let config = WorkloadConfig {
            pattern_count: 10,
            pre_existing_edges: edges,
            ..WorkloadConfig::default()
        };
        bencher
            .with_inputs(|| {
                let w = build_isolated_workload::<PetGraph>(&config);
                (w.graph, w.engine)
            })
            .bench_values(|(g, e)| {
                std::hint::black_box(e.evaluate(&g));
            });
    }

    #[divan::bench(args = [100])]
    fn batch_memgraph(bencher: Bencher, edges: usize) {
        let config = WorkloadConfig {
            pattern_count: 10,
            pre_existing_edges: edges,
            ..WorkloadConfig::default()
        };
        bencher
            .with_inputs(|| {
                let w = build_isolated_workload::<MemGraph>(&config);
                (w.graph, w.engine)
            })
            .bench_values(|(g, e)| {
                std::hint::black_box(e.evaluate(&g));
            });
    }
}
