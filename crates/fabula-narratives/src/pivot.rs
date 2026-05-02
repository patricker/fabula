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

use std::collections::HashMap;

/// Detects narrative pivots via event distribution shift.
///
/// Generic over a [`crate::distance::DistanceMetric`]. The default metric is
/// [`crate::distance::JensenShannon`], preserving the original Schulz (2024)
/// Pivot measure. Use [`PivotDetector::with_metric`] to supply a different
/// metric instance.
///
/// ```rust
/// use fabula_narratives::distance::JensenShannon;
/// use fabula_narratives::pivot::PivotDetector;
///
/// let mut pivot = PivotDetector::<JensenShannon>::new();
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
pub struct PivotDetector<M = crate::distance::JensenShannon>
where
    M: crate::distance::DistanceMetric + Default,
{
    /// Event counts for the current tick.
    current_counts: HashMap<String, u64>,
    /// Normalized distribution from the previous tick.
    prev_distribution: HashMap<String, f64>,
    /// Total events in current tick.
    current_total: u64,
    /// History of distance values.
    history: Vec<f64>,
    /// The distance metric used to compare consecutive distributions.
    metric: M,
}

impl<M> PivotDetector<M>
where
    M: crate::distance::DistanceMetric + Default,
{
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct a detector with a specific metric instance. Useful when
    /// the metric carries configuration; for stateless metrics like the
    /// built-ins, `PivotDetector::<MyMetric>::new()` is equivalent.
    pub fn with_metric(metric: M) -> Self {
        Self {
            current_counts: HashMap::new(),
            prev_distribution: HashMap::new(),
            current_total: 0,
            history: Vec::new(),
            metric,
        }
    }

    /// Record an event type for the current tick.
    pub fn push(&mut self, event_type: &str) {
        *self
            .current_counts
            .entry(event_type.to_string())
            .or_insert(0) += 1;
        self.current_total += 1;
    }

    /// End the current tick: compute the distance against the previous tick's
    /// distribution, save current as previous, clear accumulators.
    ///
    /// Returns the distance value. First tick returns `0.0` (no previous).
    /// Empty ticks return `0.0` and leave the previous distribution unchanged.
    pub fn end_tick(&mut self) -> f64 {
        if self.current_total == 0 {
            self.history.push(0.0);
            return 0.0;
        }

        let current_dist: HashMap<String, f64> = self
            .current_counts
            .iter()
            .map(|(k, &v)| (k.clone(), v as f64 / self.current_total as f64))
            .collect();

        let d = if self.prev_distribution.is_empty() {
            0.0
        } else {
            self.metric.distance(&self.prev_distribution, &current_dist)
        };

        self.history.push(d);
        self.prev_distribution = current_dist;
        self.current_counts.clear();
        self.current_total = 0;

        d
    }

    pub fn last_pivot(&self) -> f64 {
        self.history.last().copied().unwrap_or(0.0)
    }

    pub fn average_pivot(&self, window: usize) -> f64 {
        if self.history.is_empty() || window == 0 {
            return 0.0;
        }
        let start = self.history.len().saturating_sub(window);
        let slice = &self.history[start..];
        slice.iter().sum::<f64>() / slice.len() as f64
    }

    pub fn history(&self) -> &[f64] {
        &self.history
    }

    pub fn reset(&mut self) {
        self.current_counts.clear();
        self.prev_distribution.clear();
        self.current_total = 0;
        self.history.clear();
    }
}

/// Convenience alias for the common case: `PivotDetector` over
/// [`JensenShannon`](crate::distance::JensenShannon).
///
/// Equivalent to writing `PivotDetector<JensenShannon>` but reads cleaner
/// at call sites where the metric isn't otherwise constrained:
///
/// ```rust
/// use fabula_narratives::pivot::DefaultPivotDetector;
/// let mut p = DefaultPivotDetector::new();
/// p.push("trade");
/// p.end_tick();
/// ```
pub type DefaultPivotDetector = PivotDetector<crate::distance::JensenShannon>;


#[cfg(test)]
mod tests {
    use super::*;
    use crate::distance::{DistanceMetric, Hellinger, JensenShannon};

    #[test]
    fn identical_distributions_zero_jsd() {
        let mut p = PivotDetector::<JensenShannon>::new();
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
        let mut p = PivotDetector::<JensenShannon>::new();
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
        let mut p = PivotDetector::<JensenShannon>::new();
        p.push("test");
        assert_eq!(p.end_tick(), 0.0);
    }

    #[test]
    fn empty_tick_returns_zero() {
        let mut p = PivotDetector::<JensenShannon>::new();
        assert_eq!(p.end_tick(), 0.0);
    }

    #[test]
    fn partial_overlap_moderate_jsd() {
        let mut p = PivotDetector::<JensenShannon>::new();
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
        let mut p = PivotDetector::<JensenShannon>::new();
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
        let mut p = PivotDetector::<JensenShannon>::new();
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
        let jsd = JensenShannon.distance(&p, &q);
        assert!(
            jsd >= 0.0 && jsd <= 1.0,
            "JSD should be in [0, 1], got {}",
            jsd
        );
    }

    #[test]
    fn pivot_detector_with_custom_metric() {
        let mut p: PivotDetector<Hellinger> = PivotDetector::with_metric(Hellinger);
        p.push("trade");
        p.push("trade");
        p.end_tick(); // first tick — returns 0

        p.push("attack");
        p.push("attack");
        let d = p.end_tick();
        assert!(d > 0.5, "Hellinger should detect distribution shift; got {}", d);
        assert!(d <= 1.0, "Hellinger is bounded [0, 1]; got {}", d);
    }

    #[test]
    fn default_pivot_detector_uses_jsd() {
        // PivotDetector::<JensenShannon>::new() is the default — backward compat
        let mut p = PivotDetector::<JensenShannon>::new();
        p.push("a");
        p.end_tick();
        p.push("b");
        let d = p.end_tick();
        // Total mass shift from {a:1.0} to {b:1.0} = JSD = 1.0
        assert!((d - 1.0).abs() < 1e-9);
    }
}
