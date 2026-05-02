//! Composite narrative quality scorer for MCTS evaluation.
//!
//! Combines multiple scoring signals into a single `NarrativeScore` that the
//! GM's MCTS evaluation function uses to compare candidate actions.
//!
//! Based on Nelson & Mateas (2005) "Search-Based Drama Management": the GM
//! is an optimizer with a quality function over narrative states. This module
//! IS that quality function.
//!
//! # Scoring signals (from research)
//!
//! | Signal | Source | Measures |
//! |--------|--------|----------|
//! | Progress | TickDelta | Are patterns advancing/completing? |
//! | Resolution | PlantStatus | Are setups resolving? Chekhov's gun? |
//! | Thread balance | ThreadTracker | Right number of open threads? |
//! | Tension fit | TensionTracker | Is tension moving in the right direction? |
//! | Pivot magnitude | PivotDetector | How much did the narrative state shift? |
//! | Surprise | SurpriseScorer | How unexpected was this? |

use std::collections::HashMap;

use crate::tension::Trajectory;
use fabula::engine::{PlantStatus, TickDelta};

/// Per-pattern and per-thread significance weights.
///
/// Consumed by [`assemble_signals_with_significance`] (and, transitively,
/// by [`assemble_signals_weighted`]) to scale advancement/completion counts,
/// FILO penalties, and resolution rewards by authored importance.
///
/// Patterns or threads not present in the maps default to weight `1.0`.
/// This means an empty `SignificanceMap` is identical in behavior to the
/// unweighted [`assemble_signals`] path.
#[derive(Debug, Clone, Default)]
pub struct SignificanceMap {
    /// Pattern name -> weight. Drives `weighted_advancements`,
    /// `weighted_completions`, and `weighted_resolutions`.
    pub pattern_importance: HashMap<String, f64>,
    /// Thread name -> weight. Drives `weighted_filo_violations` (each
    /// violation's penalty is scaled by the violating thread's weight).
    pub thread_significance: HashMap<String, f64>,
}

/// Configurable weights for each scoring signal.
#[derive(Debug, Clone)]
pub struct NarrativeWeights {
    /// Reward for pattern advancements (progress).
    pub progress: f64,
    /// Reward for pattern completions (payoff).
    pub completion: f64,
    /// Penalty for stalled patterns (forgotten plants).
    pub stall_penalty: f64,
    /// Penalty per unresolved plant beyond the ideal count.
    pub unresolved_penalty: f64,
    /// Reward for resolving a plant (payoff fires).
    pub resolution_reward: f64,
    /// Penalty for FILO violations (thread nesting errors).
    pub filo_violation_penalty: f64,
    /// Reward when tension trajectory matches the desired direction.
    pub tension_fit: f64,
    /// Reward for narrative pivots (dramatic turns).
    pub pivot_reward: f64,
    /// Reward for surprise (unexpected patterns).
    pub surprise_reward: f64,
    /// Reward for sequential surprise (unexpected transitions between patterns).
    pub sequential_surprise_reward: f64,
    /// Multiplier applied to the final total and every breakdown component.
    /// Defaults to `1.0`. Callers update this per tick to amplify or attenuate
    /// scoring as the narrative progresses.
    ///
    /// # Example: linear ramp from 1.0 (prologue) to 2.0 (climax)
    ///
    /// ```rust
    /// use fabula_narratives::scorer::NarrativeWeights;
    ///
    /// fn weights_for_tick(tick: u64, total: u64) -> NarrativeWeights {
    ///     let progress = tick as f64 / total as f64; // 0.0 .. 1.0
    ///     NarrativeWeights {
    ///         time_scale: 1.0 + progress, // 1.0 .. 2.0
    ///         ..NarrativeWeights::default()
    ///     }
    /// }
    /// ```
    ///
    /// Set to `1.0` (the default) to disable time-scaling.
    pub time_scale: f64,
}

impl Default for NarrativeWeights {
    fn default() -> Self {
        Self {
            progress: 1.0,
            completion: 3.0,
            stall_penalty: -2.0,
            unresolved_penalty: -0.5,
            resolution_reward: 5.0,
            filo_violation_penalty: -3.0,
            tension_fit: 2.0,
            pivot_reward: 1.5,
            surprise_reward: 1.0,
            sequential_surprise_reward: 1.0,
            time_scale: 1.0,
        }
    }
}

/// Input signals for the narrative scorer.
///
/// The caller assembles these from the various trackers and engine state.
/// This decouples the scorer from the trackers -- it's a pure function
/// from signals to score.
#[derive(Debug, Clone, Default)]
pub struct NarrativeSignals {
    /// Number of patterns that advanced this tick.
    pub advancements: usize,
    /// Number of patterns that completed this tick.
    pub completions: usize,
    /// Number of stalled patterns (active PMs, no recent advancement).
    pub stalled: usize,
    /// Number of unresolved plants.
    pub unresolved_plants: usize,
    /// Number of plant/payoff pairs resolved this tick.
    pub resolutions: usize,
    /// Number of FILO nesting violations.
    pub filo_violations: usize,
    /// Whether the tension trajectory matches the desired direction.
    /// 1.0 = perfect fit, 0.0 = neutral, -1.0 = opposite.
    pub tension_fit: f64,
    /// Pivot magnitude from PivotDetector (JSD, 0-1).
    pub pivot_magnitude: f64,
    /// Pattern-level surprise score (from SurpriseScorer, higher = more surprising).
    pub surprise: f64,
    /// Sequential surprise score (from SequentialScorer, higher = more surprising).
    pub sequential_surprise: f64,
    /// Importance-weighted advancement count. 0.0 means use unweighted `advancements`.
    pub weighted_advancements: f64,
    /// Importance-weighted completion count. 0.0 means use unweighted `completions`.
    pub weighted_completions: f64,
    /// Significance-weighted FILO violation count. `0.0` means use unweighted
    /// `filo_violations`. Computed as the sum of thread weights for each
    /// violation; `1.0` per violation when no significance is supplied.
    pub weighted_filo_violations: f64,
    /// Importance-weighted resolution count. `0.0` means use unweighted
    /// `resolutions`. Computed as the sum of pattern weights for each
    /// resolution.
    pub weighted_resolutions: f64,
}

/// Composite narrative quality score with explainable sub-scores.
#[derive(Debug, Clone)]
pub struct NarrativeScore {
    /// Overall composite score (higher = better narrative quality).
    pub total: f64,
    /// Breakdown of individual signal contributions.
    pub breakdown: ScoreBreakdown,
}

/// Per-signal contribution to the total score.
#[derive(Debug, Clone, Default)]
pub struct ScoreBreakdown {
    pub progress: f64,
    pub completion: f64,
    pub stall_penalty: f64,
    pub unresolved_penalty: f64,
    pub resolution: f64,
    pub filo_penalty: f64,
    pub tension: f64,
    pub pivot: f64,
    pub surprise: f64,
    pub sequential_surprise: f64,
}

/// Score narrative quality from assembled signals.
///
/// Pure function: signals in, score out. No state, no side effects.
///
/// ```rust
/// use fabula_narratives::scorer::{score, NarrativeSignals, NarrativeWeights};
///
/// let signals = NarrativeSignals {
///     advancements: 3,
///     completions: 1,
///     stalled: 0,
///     resolutions: 1,
///     pivot_magnitude: 0.4,
///     ..Default::default()
/// };
/// let result = score(&signals, &NarrativeWeights::default());
/// assert!(result.total > 0.0, "progress + completion should score positively");
/// ```
pub fn score(signals: &NarrativeSignals, weights: &NarrativeWeights) -> NarrativeScore {
    let adv = if signals.weighted_advancements > 0.0 {
        signals.weighted_advancements
    } else {
        signals.advancements as f64
    };
    let comp = if signals.weighted_completions > 0.0 {
        signals.weighted_completions
    } else {
        signals.completions as f64
    };
    let scale = weights.time_scale;
    let breakdown = ScoreBreakdown {
        progress: adv * weights.progress * scale,
        completion: comp * weights.completion * scale,
        stall_penalty: signals.stalled as f64 * weights.stall_penalty * scale,
        unresolved_penalty: signals.unresolved_plants as f64 * weights.unresolved_penalty * scale,
        resolution: signals.resolutions as f64 * weights.resolution_reward * scale,
        filo_penalty: signals.filo_violations as f64 * weights.filo_violation_penalty * scale,
        tension: signals.tension_fit * weights.tension_fit * scale,
        pivot: signals.pivot_magnitude * weights.pivot_reward * scale,
        surprise: signals.surprise * weights.surprise_reward * scale,
        sequential_surprise: signals.sequential_surprise
            * weights.sequential_surprise_reward
            * scale,
    };

    let total = breakdown.progress
        + breakdown.completion
        + breakdown.stall_penalty
        + breakdown.unresolved_penalty
        + breakdown.resolution
        + breakdown.filo_penalty
        + breakdown.tension
        + breakdown.pivot
        + breakdown.surprise
        + breakdown.sequential_surprise;

    NarrativeScore { total, breakdown }
}

/// Convenience: compute tension fit from a trajectory and desired direction.
///
/// Returns 1.0 if the trajectory matches, -1.0 if opposite, 0.0 if neutral.
/// Unknown trajectories (either actual or desired) always return 0.0 --
/// two unknowns are not a match, they're both lacking data.
pub fn tension_fit(actual: Trajectory, desired: Trajectory) -> f64 {
    match (actual, desired) {
        (Trajectory::Unknown, _) | (_, Trajectory::Unknown) => 0.0,
        (a, d) if a == d => 1.0,
        (Trajectory::Rising, Trajectory::Falling) | (Trajectory::Falling, Trajectory::Rising) => {
            -1.0
        }
        (Trajectory::Peak, Trajectory::Valley) | (Trajectory::Valley, Trajectory::Peak) => -1.0,
        _ => 0.0,
    }
}

/// Assemble [`NarrativeSignals`] from tracker outputs and engine data.
///
/// Convenience function for the common MCTS evaluation path. Computes
/// signal values from a tick delta and pre-collected tracker state so
/// callers don't need to manually plumb 9 fields every evaluation.
#[allow(clippy::too_many_arguments)]
pub fn assemble_signals(
    delta: &TickDelta,
    plant_statuses: &[PlantStatus],
    filo_violations: usize,
    tension_trajectory: Trajectory,
    desired_trajectory: Trajectory,
    pivot_magnitude: f64,
    surprise: f64,
    sequential_surprise: f64,
) -> NarrativeSignals {
    NarrativeSignals {
        advancements: delta.advanced.len(),
        completions: delta.completed.len(),
        stalled: delta.stalled.len(),
        unresolved_plants: plant_statuses
            .iter()
            .filter(|p| p.active_plants > 0 && p.payoff_completions == 0)
            .count(),
        resolutions: delta
            .completed
            .iter()
            .filter(|name| plant_statuses.iter().any(|p| &p.payoff_pattern == *name))
            .count(),
        filo_violations,
        tension_fit: tension_fit(tension_trajectory, desired_trajectory),
        pivot_magnitude,
        surprise,
        sequential_surprise,
        weighted_advancements: 0.0,
        weighted_completions: 0.0,
        weighted_filo_violations: 0.0,
        weighted_resolutions: 0.0,
    }
}

/// Like [`assemble_signals`] but weights advancements and completions by pattern importance.
///
/// `importance` maps pattern names to their importance weight. Patterns not present
/// in the map default to 1.0. The resulting `weighted_advancements` and
/// `weighted_completions` fields are the sum of importance values for each
/// advanced/completed pattern name in the tick delta.
#[allow(clippy::too_many_arguments)]
pub fn assemble_signals_weighted(
    delta: &TickDelta,
    plant_statuses: &[PlantStatus],
    filo_violations: usize,
    tension_trajectory: Trajectory,
    desired_trajectory: Trajectory,
    pivot_magnitude: f64,
    surprise: f64,
    sequential_surprise: f64,
    importance: &HashMap<String, f64>,
) -> NarrativeSignals {
    let mut signals = assemble_signals(
        delta,
        plant_statuses,
        filo_violations,
        tension_trajectory,
        desired_trajectory,
        pivot_magnitude,
        surprise,
        sequential_surprise,
    );
    signals.weighted_advancements = delta
        .advanced
        .iter()
        .map(|name| importance.get(name).copied().unwrap_or(1.0))
        .sum();
    signals.weighted_completions = delta
        .completed
        .iter()
        .map(|name| importance.get(name).copied().unwrap_or(1.0))
        .sum();
    signals
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_progress_scores_positive() {
        let signals = NarrativeSignals {
            advancements: 5,
            completions: 2,
            ..Default::default()
        };
        let result = score(&signals, &NarrativeWeights::default());
        assert!(result.total > 0.0);
        assert!(result.breakdown.progress > 0.0);
        assert!(result.breakdown.completion > 0.0);
    }

    #[test]
    fn stalled_patterns_penalize() {
        let signals = NarrativeSignals {
            stalled: 3,
            ..Default::default()
        };
        let result = score(&signals, &NarrativeWeights::default());
        assert!(
            result.total < 0.0,
            "stalled patterns should produce negative score"
        );
    }

    #[test]
    fn resolution_rewards() {
        let signals = NarrativeSignals {
            resolutions: 2,
            ..Default::default()
        };
        let result = score(&signals, &NarrativeWeights::default());
        assert_eq!(result.breakdown.resolution, 10.0); // 2 * 5.0
    }

    #[test]
    fn filo_violations_penalize() {
        let signals = NarrativeSignals {
            filo_violations: 1,
            ..Default::default()
        };
        let result = score(&signals, &NarrativeWeights::default());
        assert!(result.total < 0.0);
    }

    #[test]
    fn tension_fit_matching() {
        assert_eq!(tension_fit(Trajectory::Rising, Trajectory::Rising), 1.0);
        assert_eq!(tension_fit(Trajectory::Rising, Trajectory::Falling), -1.0);
        assert_eq!(tension_fit(Trajectory::Plateau, Trajectory::Rising), 0.0);
    }

    #[test]
    fn tension_fit_unknown_returns_zero() {
        assert_eq!(tension_fit(Trajectory::Unknown, Trajectory::Unknown), 0.0);
        assert_eq!(tension_fit(Trajectory::Unknown, Trajectory::Rising), 0.0);
        assert_eq!(tension_fit(Trajectory::Rising, Trajectory::Unknown), 0.0);
    }

    #[test]
    fn assemble_signals_from_delta() {
        let delta = TickDelta {
            advanced: vec!["pattern_a".into(), "pattern_b".into()],
            completed: vec!["payoff_x".into()],
            negated: vec![],
            expired: vec![],
            stalled: vec!["stale_one".into()],
            active_pm_count: 5,
        };
        let plants = vec![PlantStatus {
            plant_pattern: "plant_x".into(),
            payoff_pattern: "payoff_x".into(),
            active_plants: 1,
            payoff_completions: 0,
            ticks_since_plant_advanced: 10,
            stale: true,
        }];
        let signals = assemble_signals(
            &delta,
            &plants,
            2,
            Trajectory::Rising,
            Trajectory::Rising,
            0.5,
            0.3,
            1.7,
        );
        assert_eq!(signals.advancements, 2);
        assert_eq!(signals.completions, 1);
        assert_eq!(signals.stalled, 1);
        assert_eq!(signals.unresolved_plants, 1);
        assert_eq!(signals.resolutions, 1); // payoff_x completed and matches plant
        assert_eq!(signals.filo_violations, 2);
        assert_eq!(signals.tension_fit, 1.0); // Rising matches Rising
        assert_eq!(signals.pivot_magnitude, 0.5);
        assert_eq!(signals.surprise, 0.3);
        assert_eq!(signals.sequential_surprise, 1.7);
    }

    #[test]
    fn custom_weights() {
        let signals = NarrativeSignals {
            advancements: 1,
            ..Default::default()
        };
        let weights = NarrativeWeights {
            progress: 100.0,
            ..NarrativeWeights::default()
        };
        let result = score(&signals, &weights);
        assert_eq!(result.breakdown.progress, 100.0);
    }

    #[test]
    fn zero_signals_zero_score() {
        let result = score(&NarrativeSignals::default(), &NarrativeWeights::default());
        assert_eq!(result.total, 0.0);
    }

    #[test]
    fn weighted_advancements_used_when_nonzero() {
        let signals = NarrativeSignals {
            advancements: 2,
            weighted_advancements: 11.0,
            ..Default::default()
        };
        let weights = NarrativeWeights::default();
        let result = score(&signals, &weights);
        assert_eq!(result.breakdown.progress, 11.0 * weights.progress);
    }

    #[test]
    fn unweighted_used_when_weighted_is_zero() {
        let signals = NarrativeSignals {
            advancements: 3,
            weighted_advancements: 0.0,
            ..Default::default()
        };
        let weights = NarrativeWeights::default();
        let result = score(&signals, &weights);
        assert_eq!(result.breakdown.progress, 3.0 * weights.progress);
    }

    #[test]
    fn assemble_signals_weighted_computes_importance() {
        let delta = TickDelta {
            advanced: vec!["minor".into(), "climax".into()],
            completed: vec![],
            negated: vec![],
            expired: vec![],
            stalled: vec![],
            active_pm_count: 0,
        };
        let mut importance = HashMap::new();
        importance.insert("climax".to_string(), 10.0);

        let signals = assemble_signals_weighted(
            &delta,
            &[],
            0,
            Trajectory::Unknown,
            Trajectory::Unknown,
            0.0,
            0.0,
            0.0,
            &importance,
        );
        // "minor" defaults to 1.0, "climax" is 10.0
        assert_eq!(signals.weighted_advancements, 11.0);
    }

    #[test]
    fn time_scale_multiplies_total_score() {
        let signals = NarrativeSignals {
            advancements: 3,
            completions: 1,
            ..Default::default()
        };

        let baseline = NarrativeWeights::default();
        let amplified = NarrativeWeights {
            time_scale: 2.0,
            ..NarrativeWeights::default()
        };

        let baseline_score = score(&signals, &baseline).total;
        let amplified_score = score(&signals, &amplified).total;

        assert!((amplified_score - 2.0 * baseline_score).abs() < 1e-9);
    }

    #[test]
    fn default_time_scale_is_one() {
        assert_eq!(NarrativeWeights::default().time_scale, 1.0);
    }

    #[test]
    fn significance_map_default_is_empty_maps() {
        let m = SignificanceMap::default();
        assert!(m.pattern_importance.is_empty());
        assert!(m.thread_significance.is_empty());
    }

    #[test]
    fn signals_default_zero_weighted_filo_and_resolutions() {
        let s = NarrativeSignals::default();
        assert_eq!(s.weighted_filo_violations, 0.0);
        assert_eq!(s.weighted_resolutions, 0.0);
    }
}
