//! Divan benchmarks for the narrative scoring pipeline.
//!
//! Three tiers:
//! - Throughput: full pipeline (all trackers + assemble + score) over a complete trace
//! - Per-tick latency: single tick with pre-warmed trackers
//! - Scaling: sweep one dimension at a time (event diversity, thread count, stall rate)
//!
//! The scoring pipeline under test:
//!   ThreadTracker::observe_delta + check_filo
//!   TensionTracker::push + trajectory
//!   PivotDetector::push + end_tick
//!   assemble_signals + score
//!
//! Workload construction and trace generation are in `with_inputs` (unmeasured setup).
//! Only the scoring pipeline is timed.

use divan::Bencher;
use fabula_bench::{generate_trace, NarrativeShape, NarrativeTrace, NarrativeTraceConfig};
use fabula_narratives::distance::JensenShannon;
use fabula_narratives::pivot::PivotDetector;
use fabula_narratives::scorer::{assemble_signals, score, NarrativeWeights};
use fabula_narratives::tension::TensionTracker;
use fabula_narratives::thread::ThreadTracker;

fn main() {
    divan::main();
}

// ---------------------------------------------------------------------------
// Pipeline runner
// ---------------------------------------------------------------------------

/// Run the full scoring pipeline over every tick in a trace.
fn run_pipeline(trace: &NarrativeTrace) {
    let mut thread_tracker = ThreadTracker::new();
    for (name, open_idx, close_idx) in &trace.thread_registrations {
        thread_tracker.register(name, *open_idx, *close_idx);
    }
    let mut tension_tracker = TensionTracker::new(20);
    let mut pivot_detector = PivotDetector::<JensenShannon>::new();
    let weights = NarrativeWeights::default();

    for tick in &trace.ticks {
        thread_tracker.observe_delta(&tick.delta);
        let filo_violations = thread_tracker.check_filo().len();

        tension_tracker.push(tick.tick, tick.tension_value);
        let trajectory = tension_tracker.trajectory();

        for event_type in &tick.event_types {
            pivot_detector.push(event_type);
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
        std::hint::black_box(score(&signals, &weights));
    }
}

/// Pre-warmed pipeline state for single-tick benchmarks.
struct WarmedPipeline {
    thread_tracker: ThreadTracker,
    tension_tracker: TensionTracker,
    pivot_detector: PivotDetector<JensenShannon>,
    weights: NarrativeWeights,
}

/// Warm up the pipeline by processing ticks [0, warmup_count), return the
/// warmed state and the target tick for measurement.
fn warm_pipeline(
    trace: &NarrativeTrace,
    warmup_count: usize,
) -> (WarmedPipeline, fabula_bench::NarrativeTick) {
    debug_assert!(
        warmup_count < trace.ticks.len(),
        "warmup_count ({}) must be less than tick count ({})",
        warmup_count,
        trace.ticks.len()
    );
    let mut thread_tracker = ThreadTracker::new();
    for (name, open_idx, close_idx) in &trace.thread_registrations {
        thread_tracker.register(name, *open_idx, *close_idx);
    }
    let mut tension_tracker = TensionTracker::new(20);
    let mut pivot_detector = PivotDetector::<JensenShannon>::new();

    for tick in trace.ticks.iter().take(warmup_count) {
        thread_tracker.observe_delta(&tick.delta);
        let _ = thread_tracker.check_filo();
        tension_tracker.push(tick.tick, tick.tension_value);
        let _ = tension_tracker.trajectory();
        for et in &tick.event_types {
            pivot_detector.push(et);
        }
        pivot_detector.end_tick();
    }

    let target = trace.ticks[warmup_count].clone();
    (
        WarmedPipeline {
            thread_tracker,
            tension_tracker,
            pivot_detector,
            weights: NarrativeWeights::default(),
        },
        target,
    )
}

/// Score a single tick on a pre-warmed pipeline.
fn score_one_tick(mut pipeline: WarmedPipeline, tick: fabula_bench::NarrativeTick) {
    pipeline.thread_tracker.observe_delta(&tick.delta);
    let filo_violations = pipeline.thread_tracker.check_filo().len();

    pipeline.tension_tracker.push(tick.tick, tick.tension_value);
    let trajectory = pipeline.tension_tracker.trajectory();

    for event_type in &tick.event_types {
        pipeline.pivot_detector.push(event_type);
    }
    let pivot_magnitude = pipeline.pivot_detector.end_tick();

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
    std::hint::black_box(score(&signals, &pipeline.weights));
}

// ===========================================================================
// Tier 1: Throughput -- full pipeline over complete trace
// ===========================================================================

/// Full pipeline throughput at various character counts.
/// Answers: "at N characters, how many ticks/second can the scorer handle?"
mod throughput {
    use super::*;

    #[divan::bench(args = [2, 10, 50, 200])]
    fn character_scaling(bencher: Bencher, character_count: usize) {
        let config = NarrativeTraceConfig {
            character_count,
            tick_count: 500,
            ..NarrativeTraceConfig::default()
        };
        bencher
            .counter(divan::counter::ItemsCount::new(500usize))
            .with_inputs(|| generate_trace(&config))
            .bench_values(|trace| run_pipeline(&trace));
    }

    #[divan::bench(args = [50, 200, 500, 1000])]
    fn tick_count_scaling(bencher: Bencher, tick_count: usize) {
        let config = NarrativeTraceConfig {
            tick_count,
            character_count: 50,
            ..NarrativeTraceConfig::default()
        };
        bencher
            .counter(divan::counter::ItemsCount::new(tick_count))
            .with_inputs(|| generate_trace(&config))
            .bench_values(|trace| run_pipeline(&trace));
    }
}

// ===========================================================================
// Tier 2: Per-tick latency -- single tick, pre-warmed trackers
// ===========================================================================

/// Single-tick latency with pre-warmed pipeline state.
/// Answers: "what is the per-tick cost at N characters / threads / event types?"
mod per_tick {
    use super::*;

    #[divan::bench(args = [2, 10, 50, 200])]
    fn character_scaling(bencher: Bencher, character_count: usize) {
        let config = NarrativeTraceConfig {
            character_count,
            tick_count: 200,
            ..NarrativeTraceConfig::default()
        };
        bencher
            .with_inputs(|| {
                let trace = generate_trace(&config);
                warm_pipeline(&trace, 150)
            })
            .bench_values(|(pipeline, tick)| score_one_tick(pipeline, tick));
    }

    #[divan::bench(args = [2, 8, 16, 32])]
    fn thread_scaling(bencher: Bencher, threads: usize) {
        let config = NarrativeTraceConfig {
            thread_count: threads,
            character_count: 50,
            tick_count: 200,
            ..NarrativeTraceConfig::default()
        };
        bencher
            .with_inputs(|| {
                let trace = generate_trace(&config);
                warm_pipeline(&trace, 150)
            })
            .bench_values(|(pipeline, tick)| score_one_tick(pipeline, tick));
    }

    #[divan::bench(args = [5, 15, 50, 100])]
    fn event_diversity_scaling(bencher: Bencher, diversity: usize) {
        let config = NarrativeTraceConfig {
            event_diversity: diversity,
            character_count: 50,
            tick_count: 200,
            ..NarrativeTraceConfig::default()
        };
        bencher
            .with_inputs(|| {
                let trace = generate_trace(&config);
                warm_pipeline(&trace, 150)
            })
            .bench_values(|(pipeline, tick)| score_one_tick(pipeline, tick));
    }
}

// ===========================================================================
// Tier 3: Scaling -- sweep one dimension at a time
// ===========================================================================

/// Dimension sweeps to find scaling curves.
/// All benchmarks report items/sec (ticks/sec) via divan counters.
mod scaling {
    use super::*;

    const TICKS: usize = 500;

    /// Event diversity controls PivotDetector HashMap size and JSD cost.
    #[divan::bench(args = [5, 15, 50, 100])]
    fn event_diversity(bencher: Bencher, diversity: usize) {
        let config = NarrativeTraceConfig {
            event_diversity: diversity,
            character_count: 50,
            tick_count: TICKS,
            ..NarrativeTraceConfig::default()
        };
        bencher
            .counter(divan::counter::ItemsCount::new(TICKS))
            .with_inputs(|| generate_trace(&config))
            .bench_values(|trace| run_pipeline(&trace));
    }

    /// Thread count controls ThreadTracker::observe_delta scan cost and FILO check length.
    #[divan::bench(args = [2, 8, 16, 32])]
    fn thread_count(bencher: Bencher, threads: usize) {
        let config = NarrativeTraceConfig {
            thread_count: threads,
            character_count: 50,
            tick_count: TICKS,
            ..NarrativeTraceConfig::default()
        };
        bencher
            .counter(divan::counter::ItemsCount::new(TICKS))
            .with_inputs(|| generate_trace(&config))
            .bench_values(|trace| run_pipeline(&trace));
    }

    /// Stall rate controls how many patterns are stalled per tick.
    #[divan::bench(args = [0.0, 0.1, 0.3, 0.5])]
    fn stall_rate(bencher: Bencher, rate: f64) {
        let config = NarrativeTraceConfig {
            stall_rate: rate,
            character_count: 50,
            tick_count: TICKS,
            ..NarrativeTraceConfig::default()
        };
        bencher
            .counter(divan::counter::ItemsCount::new(TICKS))
            .with_inputs(|| generate_trace(&config))
            .bench_values(|trace| run_pipeline(&trace));
    }

    /// Narrative shape affects event distribution patterns and pivot sensitivity.
    #[divan::bench]
    fn shape_rising_peak_falling(bencher: Bencher) {
        let config = NarrativeTraceConfig {
            narrative_shape: NarrativeShape::RisingPeakFalling,
            character_count: 50,
            tick_count: TICKS,
            ..NarrativeTraceConfig::default()
        };
        bencher
            .counter(divan::counter::ItemsCount::new(TICKS))
            .with_inputs(|| generate_trace(&config))
            .bench_values(|trace| run_pipeline(&trace));
    }

    #[divan::bench]
    fn shape_flat(bencher: Bencher) {
        let config = NarrativeTraceConfig {
            narrative_shape: NarrativeShape::Flat,
            character_count: 50,
            tick_count: TICKS,
            ..NarrativeTraceConfig::default()
        };
        bencher
            .counter(divan::counter::ItemsCount::new(TICKS))
            .with_inputs(|| generate_trace(&config))
            .bench_values(|trace| run_pipeline(&trace));
    }

    #[divan::bench]
    fn shape_chaotic(bencher: Bencher) {
        let config = NarrativeTraceConfig {
            narrative_shape: NarrativeShape::Chaotic,
            character_count: 50,
            tick_count: TICKS,
            ..NarrativeTraceConfig::default()
        };
        bencher
            .counter(divan::counter::ItemsCount::new(TICKS))
            .with_inputs(|| generate_trace(&config))
            .bench_values(|trace| run_pipeline(&trace));
    }

    /// Plant count affects assemble_signals filtering cost.
    #[divan::bench(args = [1, 5, 10, 20])]
    fn plant_count(bencher: Bencher, plants: usize) {
        let config = NarrativeTraceConfig {
            plant_count: plants,
            character_count: 50,
            tick_count: TICKS,
            ..NarrativeTraceConfig::default()
        };
        bencher
            .counter(divan::counter::ItemsCount::new(TICKS))
            .with_inputs(|| generate_trace(&config))
            .bench_values(|trace| run_pipeline(&trace));
    }
}
