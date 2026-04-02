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

use crate::tension::Trajectory;

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
        }
    }
}

/// Input signals for the narrative scorer.
///
/// The caller assembles these from the various trackers and engine state.
/// This decouples the scorer from the trackers — it's a pure function
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
    pub stall: f64,
    pub unresolved: f64,
    pub resolution: f64,
    pub filo: f64,
    pub tension: f64,
    pub pivot: f64,
    pub surprise: f64,
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
    let breakdown = ScoreBreakdown {
        progress: signals.advancements as f64 * weights.progress,
        completion: signals.completions as f64 * weights.completion,
        stall: signals.stalled as f64 * weights.stall_penalty,
        unresolved: signals.unresolved_plants as f64 * weights.unresolved_penalty,
        resolution: signals.resolutions as f64 * weights.resolution_reward,
        filo: signals.filo_violations as f64 * weights.filo_violation_penalty,
        tension: signals.tension_fit * weights.tension_fit,
        pivot: signals.pivot_magnitude * weights.pivot_reward,
        surprise: signals.surprise * weights.surprise_reward,
    };

    let total = breakdown.progress
        + breakdown.completion
        + breakdown.stall
        + breakdown.unresolved
        + breakdown.resolution
        + breakdown.filo
        + breakdown.tension
        + breakdown.pivot
        + breakdown.surprise;

    NarrativeScore { total, breakdown }
}

/// Convenience: compute tension fit from a trajectory and desired direction.
///
/// Returns 1.0 if the trajectory matches, -1.0 if opposite, 0.0 if neutral.
pub fn tension_fit(actual: Trajectory, desired: Trajectory) -> f64 {
    match (actual, desired) {
        (a, d) if a == d => 1.0,
        (Trajectory::Rising, Trajectory::Falling) | (Trajectory::Falling, Trajectory::Rising) => -1.0,
        (Trajectory::Peak, Trajectory::Valley) | (Trajectory::Valley, Trajectory::Peak) => -1.0,
        _ => 0.0,
    }
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
        assert!(result.total < 0.0, "stalled patterns should produce negative score");
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
}
