//! Profiling binary for fabula engine.
//!
//! Runs a realistic GM-profile workload (30 patterns, ~200 ticks, ~3K edges)
//! and prints per-tick CSV for analysis.
//!
//! Usage:
//!   cargo run --release --bin fabula-profile [-- --adapter petgraph|memgraph]
//!   cargo run --release --bin fabula-profile --features dhat-heap  # allocation profile
//!   samply record target/release/fabula-profile                    # CPU flamegraph

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use fabula_bench::build_gm_workload;
use fabula_bench::GmWorkload;
use fabula_test_suite::TestGraph;
use std::time::Instant;

fn run_profile<G: TestGraph>(_adapter_name: &str) {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    let GmWorkload {
        mut graph,
        mut engine,
        ticks,
    } = build_gm_workload::<G>();

    let pattern_count = engine.patterns().len();
    let total_edges: usize = ticks.iter().map(|t| t.edges.len()).sum();
    eprintln!(
        "GM workload: {} patterns, {} ticks, {} total edges",
        pattern_count,
        ticks.len(),
        total_edges
    );

    // CSV header — all columns are per-tick except peak_pms (lifetime high-water)
    println!("tick,edges,elapsed_us,active_pms,tick_on_edge,tick_fingerprints,tick_neg_checks,peak_pms,advanced,completed,negated");

    let overall_start = Instant::now();
    let mut prev_on_edge = 0u64;
    let mut prev_fingerprints = 0u64;
    let mut prev_neg_checks = 0u64;

    for tick in &ticks {
        graph.set_current_time(tick.time);

        // Insert ALL edges for this tick into the graph first,
        // then notify the engine about each one. This ensures secondary
        // clauses (e.g., actor, target) are visible when the primary
        // clause (eventType) triggers pattern matching.
        for edge in &tick.edges {
            edge.insert(&mut graph);
        }

        let tick_start = Instant::now();
        let mut advanced = 0u64;
        let mut completed = 0u64;
        let mut negated = 0u64;

        for edge in &tick.edges {
            let events = edge.notify(&graph, &mut engine);
            for event in &events {
                match event {
                    fabula::prelude::SiftEvent::Advanced { .. } => advanced += 1,
                    fabula::prelude::SiftEvent::Completed { .. } => completed += 1,
                    fabula::prelude::SiftEvent::Negated { .. } => negated += 1,
                    fabula::prelude::SiftEvent::Expired { .. } => {}
                }
            }
        }

        let elapsed = tick_start.elapsed();
        let stats = engine.stats();
        let active_pms = engine
            .partial_matches()
            .iter()
            .filter(|pm| pm.state == fabula::prelude::MatchState::Active)
            .count();

        // Compute per-tick deltas from cumulative counters
        let tick_on_edge = stats.total_on_edge_added - prev_on_edge;
        let tick_fingerprints = stats.total_fingerprints - prev_fingerprints;
        let tick_neg_checks = stats.total_negation_checks - prev_neg_checks;
        prev_on_edge = stats.total_on_edge_added;
        prev_fingerprints = stats.total_fingerprints;
        prev_neg_checks = stats.total_negation_checks;

        println!(
            "{},{},{},{},{},{},{},{},{},{},{}",
            tick.time,
            tick.edges.len(),
            elapsed.as_micros(),
            active_pms,
            tick_on_edge,
            tick_fingerprints,
            tick_neg_checks,
            stats.peak_active_pms,
            advanced,
            completed,
            negated,
        );

        // Drain completed matches every 10 ticks (realistic GM behavior)
        if tick.time % 10 == 0 {
            engine.drain_completed();
        }
    }

    let total_elapsed = overall_start.elapsed();
    let stats = engine.stats();
    eprintln!("\n--- Summary ---");
    eprintln!("Total time:           {:?}", total_elapsed);
    eprintln!("Total on_edge_added:  {}", stats.total_on_edge_added);
    eprintln!("Total fingerprints:   {}", stats.total_fingerprints);
    eprintln!("Total negation checks:{}", stats.total_negation_checks);
    eprintln!("Peak active PMs:      {}", stats.peak_active_pms);
    eprintln!(
        "Avg per on_edge_added: {:.1} us",
        total_elapsed.as_micros() as f64 / stats.total_on_edge_added as f64
    );
}

fn main() {
    let adapter = std::env::args()
        .skip_while(|a| a != "--adapter")
        .nth(1)
        .unwrap_or_else(|| "petgraph".to_string());

    match adapter.as_str() {
        "petgraph" => run_profile::<fabula_test_suite::PetGraph>("petgraph"),
        "memgraph" => run_profile::<fabula_test_suite::MemGraph>("memgraph"),
        other => {
            eprintln!("Unknown adapter: {}. Use 'petgraph' or 'memgraph'.", other);
            std::process::exit(1);
        }
    }
}
