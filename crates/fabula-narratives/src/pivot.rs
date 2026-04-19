//! Event distribution shift detection (narrative pivot).
//!
//! Implements the **Pivot** measure from Schulz et al. (2024) "Narrative
//! Information Theory": JSD(s_t || s_{t-1}), the Jensen-Shannon divergence
//! between consecutive event-type distributions. High pivot = dramatic turn;
//! low pivot = continuation of current trajectory.
//!
//! The detector maintains a categorical distribution over event types,
//! updated each tick. JSD is symmetric and bounded in [0, 1] (when using
//! log base 2), making it directly comparable across ticks.

use std::collections::{HashMap, HashSet};

/// Detects narrative pivots via event distribution shift (JSD).
///
/// Feed event type strings each tick via [`push`], then call [`end_tick`]
/// to compute the JSD between this tick's distribution and the previous tick's.
///
/// ```rust
/// use fabula_narratives::pivot::PivotDetector;
///
/// let mut pivot = PivotDetector::new();
/// // Tick 1: mostly peaceful events
/// pivot.push("trade"); pivot.push("trade"); pivot.push("talk");
/// let _ = pivot.end_tick(); // first tick has no previous -- returns 0
///
/// // Tick 2: sudden violence
/// pivot.push("attack"); pivot.push("attack"); pivot.push("harm");
/// let jsd = pivot.end_tick();
/// assert!(jsd > 0.5, "dramatic shift should produce high JSD");
/// ```
#[derive(Debug, Clone, Default)]
pub struct PivotDetector {
    /// Event counts for the current tick.
    current_counts: HashMap<String, u64>,
    /// Normalized distribution from the previous tick.
    prev_distribution: HashMap<String, f64>,
    /// Total events in current tick.
    current_total: u64,
    /// History of JSD values.
    history: Vec<f64>,
}

impl PivotDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an event type for the current tick.
    pub fn push(&mut self, event_type: &str) {
        *self
            .current_counts
            .entry(event_type.to_string())
            .or_insert(0) += 1;
        self.current_total += 1;
    }

    /// End the current tick: compute JSD against previous tick's distribution,
    /// save current as previous, clear accumulators.
    ///
    /// Returns the JSD value in [0, 1]. First tick returns 0.0 (no previous).
    ///
    /// Empty ticks (no events pushed) return 0.0 and leave the previous
    /// distribution unchanged -- the next non-empty tick compares against the
    /// last non-empty tick's distribution.
    pub fn end_tick(&mut self) -> f64 {
        if self.current_total == 0 {
            // Empty tick -- no events, no pivot
            self.history.push(0.0);
            return 0.0;
        }

        // Normalize current counts to a distribution
        let current_dist: HashMap<String, f64> = self
            .current_counts
            .iter()
            .map(|(k, &v)| (k.clone(), v as f64 / self.current_total as f64))
            .collect();

        let jsd = if self.prev_distribution.is_empty() {
            0.0 // First tick -- no previous to compare
        } else {
            jensen_shannon_divergence(&self.prev_distribution, &current_dist)
        };

        self.history.push(jsd);
        self.prev_distribution = current_dist;
        self.current_counts.clear();
        self.current_total = 0;

        jsd
    }

    /// Most recent JSD value.
    pub fn last_pivot(&self) -> f64 {
        self.history.last().copied().unwrap_or(0.0)
    }

    /// Average pivot magnitude over the last N ticks.
    ///
    /// Returns 0.0 if the history is empty or window is 0.
    pub fn average_pivot(&self, window: usize) -> f64 {
        if self.history.is_empty() || window == 0 {
            return 0.0;
        }
        let start = self.history.len().saturating_sub(window);
        let slice = &self.history[start..];
        slice.iter().sum::<f64>() / slice.len() as f64
    }

    /// Full history of JSD values.
    pub fn history(&self) -> &[f64] {
        &self.history
    }

    /// Reset all state.
    pub fn reset(&mut self) {
        self.current_counts.clear();
        self.prev_distribution.clear();
        self.current_total = 0;
        self.history.clear();
    }
}

/// Jensen-Shannon Divergence between two categorical distributions.
/// JSD(P || Q) = 0.5 * KL(P || M) + 0.5 * KL(Q || M), where M = (P + Q) / 2.
/// Uses log base 2, so result is in [0, 1].
fn jensen_shannon_divergence(p: &HashMap<String, f64>, q: &HashMap<String, f64>) -> f64 {
    let all_keys: HashSet<&String> = p.keys().chain(q.keys()).collect();

    let mut jsd = 0.0;
    for key in all_keys {
        let p_val = p.get(key).copied().unwrap_or(0.0);
        let q_val = q.get(key).copied().unwrap_or(0.0);
        let m_val = (p_val + q_val) / 2.0;

        if m_val > 0.0 {
            if p_val > 0.0 {
                jsd += 0.5 * p_val * (p_val / m_val).log2();
            }
            if q_val > 0.0 {
                jsd += 0.5 * q_val * (q_val / m_val).log2();
            }
        }
    }

    // Clamp: floating-point rounding can produce tiny negative values
    jsd.max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_distributions_zero_jsd() {
        let mut p = PivotDetector::new();
        p.push("trade");
        p.push("talk");
        p.end_tick();
        p.push("trade");
        p.push("talk");
        let jsd = p.end_tick();
        assert!(
            jsd.abs() < 0.001,
            "identical distributions should have JSD ≈ 0, got {}",
            jsd
        );
    }

    #[test]
    fn completely_different_distributions_high_jsd() {
        let mut p = PivotDetector::new();
        p.push("peace");
        p.push("peace");
        p.end_tick();
        p.push("war");
        p.push("war");
        let jsd = p.end_tick();
        assert!(
            jsd > 0.9,
            "completely different distributions should have high JSD, got {}",
            jsd
        );
    }

    #[test]
    fn first_tick_returns_zero() {
        let mut p = PivotDetector::new();
        p.push("test");
        assert_eq!(p.end_tick(), 0.0);
    }

    #[test]
    fn empty_tick_returns_zero() {
        let mut p = PivotDetector::new();
        assert_eq!(p.end_tick(), 0.0);
    }

    #[test]
    fn partial_overlap_moderate_jsd() {
        let mut p = PivotDetector::new();
        // Tick 1: mostly trade
        p.push("trade");
        p.push("trade");
        p.push("talk");
        p.end_tick();
        // Tick 2: mix of trade and attack
        p.push("trade");
        p.push("attack");
        p.push("attack");
        let jsd = p.end_tick();
        assert!(
            jsd > 0.1 && jsd < 0.9,
            "partial overlap should give moderate JSD, got {}",
            jsd
        );
    }

    #[test]
    fn average_pivot_over_window() {
        let mut p = PivotDetector::new();
        // 5 ticks of identical events
        for _ in 0..5 {
            p.push("same");
            p.end_tick();
        }
        assert!(
            p.average_pivot(5) < 0.01,
            "stable events should have low average pivot"
        );
    }

    #[test]
    fn average_pivot_zero_window_returns_zero() {
        let mut p = PivotDetector::new();
        p.push("test");
        p.end_tick();
        // window=0 should return 0.0, not NaN
        let avg = p.average_pivot(0);
        assert_eq!(avg, 0.0);
    }

    #[test]
    fn jsd_is_bounded_zero_one() {
        let p: HashMap<String, f64> = [("a".into(), 1.0)].into();
        let q: HashMap<String, f64> = [("b".into(), 1.0)].into();
        let jsd = jensen_shannon_divergence(&p, &q);
        assert!(
            jsd >= 0.0 && jsd <= 1.0,
            "JSD should be in [0, 1], got {}",
            jsd
        );
    }
}
