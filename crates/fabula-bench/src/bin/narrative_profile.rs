//! Profiling binary for the narrative scoring pipeline.
//!
//! Runs the full scoring pipeline on a synthetic 1000-tick trace and prints
//! per-tick CSV for analysis. With `--features dhat-heap`, reports peak
//! heap allocation.
//!
//! Usage:
//!   cargo run --release --bin narrative-profile
//!   cargo run --release --bin narrative-profile --features dhat-heap  # allocation profile
//!   cargo run --release --bin narrative-profile -- --characters 200   # custom character count

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use fabula_bench::narrative_workload::{generate_trace, NarrativeShape, NarrativeTraceConfig};
use fabula_narratives::distance::JensenShannon;
use fabula_narratives::pivot::PivotDetector;
use fabula_narratives::scorer::{assemble_signals, score, NarrativeWeights};
use fabula_narratives::tension::TensionTracker;
use fabula_narratives::thread::ThreadTracker;
use std::time::Instant;

fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    let character_count: usize = std::env::args()
        .skip_while(|a| a != "--characters")
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);

    let shape_str: String = std::env::args()
        .skip_while(|a| a != "--shape")
        .nth(1)
        .unwrap_or_else(|| "rising".to_string());

    let narrative_shape = match shape_str.as_str() {
        "flat" => NarrativeShape::Flat,
        "chaotic" => NarrativeShape::Chaotic,
        _ => NarrativeShape::RisingPeakFalling,
    };

    let config = NarrativeTraceConfig {
        character_count,
        tick_count: 1000,
        narrative_shape,
        ..NarrativeTraceConfig::default()
    };

    eprintln!(
        "Narrative profile: {} characters, 1000 ticks, {:?} shape",
        character_count, narrative_shape
    );

    let trace = generate_trace(&config);

    // Set up pipeline
    let mut thread_tracker = ThreadTracker::new();
    for (name, open_idx, close_idx) in &trace.thread_registrations {
        thread_tracker.register(name, *open_idx, *close_idx);
    }
    let mut tension_tracker = TensionTracker::new(20);
    let mut pivot_detector = PivotDetector::<JensenShannon>::new();
    let weights = NarrativeWeights::default();

    // CSV header
    println!(
        "tick,elapsed_us,advancements,completions,stalled,filo_violations,pivot,tension_fit,score"
    );

    let overall_start = Instant::now();

    for tick in &trace.ticks {
        let tick_start = Instant::now();

        thread_tracker.observe_delta(&tick.delta);
        let filo_violations = thread_tracker.check_filo().len();

        tension_tracker.push(tick.tick, tick.tension_value);
        let trajectory = tension_tracker.trajectory();

        for et in &tick.event_types {
            pivot_detector.push(et);
        }
        let pivot_magnitude = pivot_detector.end_tick();

        let signals = assemble_signals(
            &tick.delta,
            &tick.plant_statuses,
            filo_violations,
            trajectory,
            tick.desired_trajectory,
            pivot_magnitude,
            tick.surprise,
            tick.sequential_surprise,
        );
        let result = score(&signals, &weights);

        let elapsed = tick_start.elapsed();
        println!(
            "{},{},{},{},{},{},{:.4},{:.2},{:.2}",
            tick.tick,
            elapsed.as_micros(),
            signals.advancements,
            signals.completions,
            signals.stalled,
            filo_violations,
            pivot_magnitude,
            signals.tension_fit,
            result.total,
        );
    }

    let total_elapsed = overall_start.elapsed();
    let total_ticks = trace.ticks.len();
    eprintln!("\n--- Summary ---");
    eprintln!("Total time:      {:?}", total_elapsed);
    eprintln!("Ticks:           {}", total_ticks);
    eprintln!(
        "Avg per tick:    {:.1} us",
        total_elapsed.as_micros() as f64 / total_ticks as f64
    );
    eprintln!(
        "Throughput:      {:.0} ticks/sec",
        total_ticks as f64 / total_elapsed.as_secs_f64()
    );
    eprintln!(
        "MCTS 1000 est:   {:.2} ms",
        (total_elapsed.as_micros() as f64 / total_ticks as f64) * 1000.0 / 1000.0
    );
}
