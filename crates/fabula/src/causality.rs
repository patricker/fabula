//! Retrospective causal pathfinding over temporal graphs.
//!
//! Given an effect node in the graph and a set of causal edge labels, finds
//! all paths of length up to `max_hops` leading to the effect, ordered by a
//! cleanliness score that combines edge weights, time gaps, and path
//! divergence.
//!
//! # Design
//!
//! Causality is represented as explicit edges in the graph (e.g., an edge
//! labeled `"causes"` from event A to event B means A caused B). Callers
//! mark which labels are causal and with what weight. Edges not in the
//! causal labels map are ignored.
//!
//! Based on ROADMAP Phase 6.1.

/// A traced causal chain leading to an effect.
#[derive(Debug, Clone)]
pub struct CausalPath<N, V, T> {
    /// Nodes along the path, ordered from root cause to effect (inclusive at both ends).
    pub nodes: Vec<N>,
    /// Edges traversed, one fewer than `nodes.len()`.
    pub edges: Vec<CausalEdge<V, T>>,
    /// Cleanliness score in `[0.0, 1.0]`, higher is better.
    pub cleanliness: f64,
    /// Confidence estimate derived from edge weights.
    pub confidence: f64,
}

/// An edge in a traced causal path.
#[derive(Debug, Clone)]
pub struct CausalEdge<V, T> {
    /// The edge's target value (as it appeared in the graph).
    pub value: V,
    /// The edge's start time.
    pub time: T,
    /// The weight this edge contributed (from the causal labels map).
    pub weight: f64,
}

/// Compute a cleanliness score for a path.
///
/// `weights` contains one entry per edge in the path (length == path.edges.len()).
/// `total_gap` is the sum of temporal gaps between consecutive nodes as f64.
/// `branches_skipped` is the number of causal predecessors the BFS declined to
/// follow at any node along the path (measures divergence).
///
/// Returns a score in `[0.0, 1.0]`. Empty weights → `0.0`.
#[allow(dead_code)]
pub(crate) fn cleanliness_score(
    weights: &[f64],
    total_gap: f64,
    branches_skipped: usize,
) -> f64 {
    if weights.is_empty() {
        return 0.0;
    }
    let mean_weight = weights.iter().sum::<f64>() / weights.len() as f64;
    // Gap penalty saturates at 0.5 using an exponential approach.
    // gap = 0 → penalty = 0; gap → ∞ → penalty → 0.5.
    let gap_penalty = 0.5 * (1.0 - (-total_gap / 50.0).exp());
    let divergence_factor = 1.0 / (1.0 + branches_skipped as f64);
    (mean_weight * (1.0 - gap_penalty) * divergence_factor).clamp(0.0, 1.0)
}

#[cfg(test)]
mod cleanliness_tests {
    use super::*;

    #[test]
    fn perfect_path_all_unit_weights_zero_gap() {
        // 1.0 weights, no time span, no branches → 1.0
        let score = cleanliness_score(&[1.0, 1.0, 1.0], 0.0, 0);
        assert!((score - 1.0).abs() < 1e-9, "got {}", score);
    }

    #[test]
    fn mean_weights_reduce_cleanliness() {
        // 0.5 mean, no gap, no branches → 0.5
        let score = cleanliness_score(&[0.5, 0.5], 0.0, 0);
        assert!((score - 0.5).abs() < 1e-9, "got {}", score);
    }

    #[test]
    fn longer_gap_reduces_cleanliness() {
        let tight = cleanliness_score(&[1.0, 1.0], 1.0, 0);
        let loose = cleanliness_score(&[1.0, 1.0], 100.0, 0);
        assert!(tight > loose);
    }

    #[test]
    fn gap_penalty_saturates_at_half() {
        // Very large gap should reduce score by at most 50%
        let huge_gap = cleanliness_score(&[1.0, 1.0], 1e9, 0);
        assert!(huge_gap >= 0.5 - 1e-9, "got {}", huge_gap);
    }

    #[test]
    fn divergence_reduces_cleanliness() {
        let focused = cleanliness_score(&[1.0], 0.0, 0);
        let noisy = cleanliness_score(&[1.0], 0.0, 5);
        assert!(focused > noisy);
    }

    #[test]
    fn empty_weights_returns_zero() {
        assert_eq!(cleanliness_score(&[], 0.0, 0), 0.0);
    }
}
