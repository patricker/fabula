//! Synthetic narrative trace generator for benchmarking the scoring pipeline.
//!
//! Produces a stream of [`NarrativeTick`]s without requiring a real `SiftEngine`
//! or `DataSource`. Parameters control character count, tick count, narrative
//! shape, event diversity, thread density, and stall rate.
//!
//! The generator directly constructs [`TickDelta`], [`PlantStatus`], tension
//! values, and event distributions — this is what makes it self-contained.

use fabula::engine::{PlantStatus, TickDelta};
use fabula_narratives::tension::Trajectory;

// ---------------------------------------------------------------------------
// Seeded PRNG (minimal xorshift64 — no external dep)
// ---------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { 1 } else { seed })
    }
    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn usize(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }
    fn f64(&mut self) -> f64 {
        (self.next_u64() & 0x000F_FFFF_FFFF_FFFF) as f64 / (1u64 << 52) as f64
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Shape of the synthetic narrative tension arc.
#[derive(Debug, Clone, Copy)]
pub enum NarrativeShape {
    /// Rising -> peak -> falling (classic dramatic arc).
    RisingPeakFalling,
    /// Flat intensity throughout.
    Flat,
    /// Unpredictable intensity jumps.
    Chaotic,
}

/// Configuration for synthetic narrative trace generation.
#[derive(Debug, Clone)]
pub struct NarrativeTraceConfig {
    /// Number of concurrent characters (2-200). Controls event volume per tick.
    pub character_count: usize,
    /// Total ticks in the session (50-2000).
    pub tick_count: usize,
    /// Shape of the narrative tension arc.
    pub narrative_shape: NarrativeShape,
    /// Number of distinct event types (affects JSD/pivot sensitivity).
    pub event_diversity: usize,
    /// Number of MICE threads to register.
    pub thread_count: usize,
    /// Fraction of ticks where patterns stall (0.0-1.0).
    pub stall_rate: f64,
    /// Number of plant/payoff pairs.
    pub plant_count: usize,
    /// PRNG seed for reproducibility.
    pub seed: u64,
}

impl Default for NarrativeTraceConfig {
    fn default() -> Self {
        Self {
            character_count: 10,
            tick_count: 200,
            narrative_shape: NarrativeShape::RisingPeakFalling,
            event_diversity: 15,
            thread_count: 4,
            stall_rate: 0.1,
            plant_count: 3,
            seed: 42,
        }
    }
}

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// A single tick's worth of narrative scoring inputs.
#[derive(Clone)]
pub struct NarrativeTick {
    pub tick: u64,
    pub delta: TickDelta,
    pub plant_statuses: Vec<PlantStatus>,
    pub tension_value: f64,
    pub desired_trajectory: Trajectory,
    pub event_types: Vec<String>,
    pub surprise: f64,
    pub sequential_surprise: f64,
}

/// Complete synthetic trace ready for benchmarking.
pub struct NarrativeTrace {
    pub ticks: Vec<NarrativeTick>,
    /// Thread registrations: (name, open_pattern_idx, close_pattern_idx).
    pub thread_registrations: Vec<(String, usize, usize)>,
}

// ---------------------------------------------------------------------------
// Generator
// ---------------------------------------------------------------------------

/// Generate a complete synthetic narrative trace.
///
/// The trace exercises the full scoring pipeline: `ThreadTracker::observe_delta`,
/// `TensionTracker::push`, `PivotDetector::push` + `end_tick`, `assemble_signals`,
/// and `score`. No real `SiftEngine` or `DataSource` is needed.
pub fn generate_trace(config: &NarrativeTraceConfig) -> NarrativeTrace {
    let mut rng = Rng::new(config.seed);

    // Thread registrations — pattern indices are synthetic (not used by engine)
    let thread_registrations: Vec<(String, usize, usize)> = (0..config.thread_count)
        .map(|i| (format!("thread_{}", i), i * 2, i * 2 + 1))
        .collect();

    // Event type vocabulary
    let event_vocab: Vec<String> = (0..config.event_diversity)
        .map(|i| format!("evt_{}", i))
        .collect();

    // Activity pattern names — pool size scales with character count
    let pattern_pool_size = config.character_count.max(10);
    let pattern_names: Vec<String> = (0..pattern_pool_size)
        .map(|i| format!("pat_{}", i))
        .collect();

    // Plant/payoff pairs
    let plant_pairs: Vec<(String, String)> = (0..config.plant_count)
        .map(|i| (format!("plant_{}", i), format!("payoff_{}", i)))
        .collect();

    // Cumulative plant state
    let mut plant_payoff_completions: Vec<u64> = vec![0; config.plant_count];
    let mut plant_ticks_since_advanced: Vec<u64> = vec![0; config.plant_count];

    let mut ticks = Vec::with_capacity(config.tick_count);

    for t in 0..config.tick_count {
        let tick = t as u64;
        let progress = t as f64 / config.tick_count.max(1) as f64;

        // -- Tension value --
        let tension_value = match config.narrative_shape {
            NarrativeShape::RisingPeakFalling => {
                (std::f64::consts::PI * progress).sin()
            }
            NarrativeShape::Flat => 0.5 + (rng.f64() - 0.5) * 0.05,
            NarrativeShape::Chaotic => rng.f64(),
        };

        // -- Desired trajectory --
        let desired_trajectory = match config.narrative_shape {
            NarrativeShape::RisingPeakFalling => {
                if progress < 0.4 {
                    Trajectory::Rising
                } else if progress < 0.6 {
                    Trajectory::Peak
                } else {
                    Trajectory::Falling
                }
            }
            NarrativeShape::Flat => Trajectory::Plateau,
            NarrativeShape::Chaotic => match rng.usize(5) {
                0 => Trajectory::Rising,
                1 => Trajectory::Falling,
                2 => Trajectory::Peak,
                3 => Trajectory::Valley,
                _ => Trajectory::Plateau,
            },
        };

        // -- Activity rate modulated by narrative shape --
        let activity_rate = match config.narrative_shape {
            NarrativeShape::RisingPeakFalling => {
                0.2 + 0.6 * (std::f64::consts::PI * progress).sin()
            }
            NarrativeShape::Flat => 0.3,
            NarrativeShape::Chaotic => 0.1 + rng.f64() * 0.6,
        };

        let active_count = ((config.character_count as f64 * activity_rate) as usize).max(1);

        // -- Build TickDelta --
        let mut advanced = Vec::new();
        let mut completed = Vec::new();
        let mut negated = Vec::new();
        let mut stalled = Vec::new();

        // Pattern advancements
        for _ in 0..active_count {
            let name = &pattern_names[rng.usize(pattern_names.len())];
            if !advanced.contains(name) {
                advanced.push(name.clone());
            }
        }

        // ~20% of advanced patterns complete
        let completion_count = (advanced.len() as f64 * 0.2).ceil() as usize;
        for name in advanced.iter().take(completion_count) {
            completed.push(name.clone());
        }

        // Thread open events (appear in `advanced` as `thread_N_open`)
        for i in 0..config.thread_count {
            if rng.f64() < 0.1 {
                let name = format!("thread_{}_open", i);
                if !advanced.contains(&name) {
                    advanced.push(name);
                }
            }
        }

        // Thread close events (appear in `completed` as `thread_N_close`)
        for i in 0..config.thread_count {
            if rng.f64() < 0.05 {
                let name = format!("thread_{}_close", i);
                if !completed.contains(&name) {
                    completed.push(name);
                }
            }
        }

        // Negations (rare)
        if rng.f64() < 0.1 {
            negated.push(pattern_names[rng.usize(pattern_names.len())].clone());
        }

        // Stalled patterns
        if rng.f64() < config.stall_rate {
            let stall_count = (config.character_count as f64 * 0.1).ceil() as usize;
            for _ in 0..stall_count.max(1) {
                let name = &pattern_names[rng.usize(pattern_names.len())];
                if !stalled.contains(name) {
                    stalled.push(name.clone());
                }
            }
        }

        // Payoff completions and plant staleness tracking
        for i in 0..config.plant_count {
            if rng.f64() < 0.03 {
                if !completed.contains(&plant_pairs[i].1) {
                    completed.push(plant_pairs[i].1.clone());
                }
                plant_payoff_completions[i] += 1;
            }
            if advanced.contains(&plant_pairs[i].0) {
                plant_ticks_since_advanced[i] = 0;
            } else {
                plant_ticks_since_advanced[i] += 1;
            }
        }

        let active_pm_count = config.character_count * 2;

        let delta = TickDelta {
            advanced,
            completed,
            negated,
            expired: Vec::new(),
            stalled,
            active_pm_count,
        };

        // -- Plant statuses --
        let plant_statuses: Vec<PlantStatus> = (0..config.plant_count)
            .map(|i| PlantStatus {
                plant_pattern: plant_pairs[i].0.clone(),
                payoff_pattern: plant_pairs[i].1.clone(),
                active_plants: 1, // always 1 active PM per plant
                payoff_completions: plant_payoff_completions[i],
                ticks_since_plant_advanced: plant_ticks_since_advanced[i],
                stale: plant_ticks_since_advanced[i] > 10,
            })
            .collect();

        // -- Event types for PivotDetector --
        let event_count = ((config.character_count as f64 * activity_rate).ceil() as usize).max(1);
        let mut tick_events = Vec::with_capacity(event_count);

        // Distribution shift based on progress to create meaningful JSD
        let shift = match config.narrative_shape {
            NarrativeShape::RisingPeakFalling => {
                (progress * config.event_diversity as f64 * 0.5) as usize
            }
            NarrativeShape::Flat => 0,
            NarrativeShape::Chaotic => rng.usize(config.event_diversity),
        };

        let half_diversity = (config.event_diversity / 2).max(1);
        for _ in 0..event_count {
            let idx = (rng.usize(half_diversity) + shift) % config.event_diversity;
            tick_events.push(event_vocab[idx].clone());
        }

        // -- Surprise values --
        let surprise = match config.narrative_shape {
            NarrativeShape::RisingPeakFalling => {
                (std::f64::consts::PI * 2.0 * progress).sin().abs() * 0.5
            }
            NarrativeShape::Flat => 0.1 + rng.f64() * 0.1,
            NarrativeShape::Chaotic => rng.f64(),
        };
        let sequential_surprise = surprise * (0.5 + rng.f64() * 0.5);

        ticks.push(NarrativeTick {
            tick,
            delta,
            plant_statuses,
            tension_value,
            desired_trajectory,
            event_types: tick_events,
            surprise,
            sequential_surprise,
        });
    }

    NarrativeTrace {
        ticks,
        thread_registrations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fabula_narratives::pivot::PivotDetector;
    use fabula_narratives::scorer::{assemble_signals, score, NarrativeWeights};
    use fabula_narratives::tension::TensionTracker;
    use fabula_narratives::thread::ThreadTracker;

    /// Validate that a generated trace exercises meaningful scoring signals.
    #[test]
    fn rising_peak_falling_trace_produces_meaningful_signals() {
        let config = NarrativeTraceConfig {
            tick_count: 500,
            character_count: 20,
            thread_count: 4,
            plant_count: 3,
            ..NarrativeTraceConfig::default()
        };
        let trace = generate_trace(&config);

        let mut thread_tracker = ThreadTracker::new();
        for (name, open_idx, close_idx) in &trace.thread_registrations {
            thread_tracker.register(name, *open_idx, *close_idx);
        }
        let mut tension_tracker = TensionTracker::new(20);
        let mut pivot_detector = PivotDetector::new();
        let weights = NarrativeWeights::default();

        let mut nonzero_scores = 0usize;
        let mut nonzero_pivots = 0usize;
        let mut positive_tension_fit = 0usize;
        let mut negative_tension_fit = 0usize;
        let mut any_filo_violation = false;
        let mut any_stalled = false;

        for tick in &trace.ticks {
            thread_tracker.observe_delta(&tick.delta);
            let filo_violations = thread_tracker.check_filo().len();
            if filo_violations > 0 {
                any_filo_violation = true;
            }

            tension_tracker.push(tick.tick, tick.tension_value);
            let trajectory = tension_tracker.trajectory();

            for et in &tick.event_types {
                pivot_detector.push(et);
            }
            let pivot_magnitude = pivot_detector.end_tick();
            if pivot_magnitude > 0.01 {
                nonzero_pivots += 1;
            }

            if !tick.delta.stalled.is_empty() {
                any_stalled = true;
            }

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
            if result.total.abs() > 0.01 {
                nonzero_scores += 1;
            }
            if signals.tension_fit > 0.0 {
                positive_tension_fit += 1;
            } else if signals.tension_fit < 0.0 {
                negative_tension_fit += 1;
            }
        }

        assert!(
            nonzero_scores > trace.ticks.len() / 2,
            "most ticks should produce nonzero scores, got {}/{}",
            nonzero_scores,
            trace.ticks.len()
        );
        assert!(
            nonzero_pivots > 10,
            "should have meaningful pivot shifts, got {}",
            nonzero_pivots
        );
        assert!(
            positive_tension_fit > 0,
            "should have some positive tension fit"
        );
        assert!(
            negative_tension_fit > 0 || positive_tension_fit > 0,
            "should have some tension fit signal"
        );
        assert!(any_stalled, "should have at least one stalled tick");
        // FILO violations depend on thread open/close ordering — may or may not
        // occur with seed 42. Just verify the tracker ran without panicking.
        let _ = any_filo_violation;
    }
}
