//! Sequential surprise scoring (bigram transitions).
//!
//! Scores pattern completions by how surprising they are given the *previous*
//! pattern that completed. Uses a bigram model: `P(B | A)` from observed
//! transition frequencies, scored as `-log₂(P(B | A))`.
//!
//! A common betrayal after a rare alliance is surprising; a common betrayal
//! after another common betrayal is not.

use std::collections::HashMap;

/// Per-predecessor transition counts.
#[derive(Debug, Clone, Default)]
struct BigramRow {
    /// Total transitions observed from this predecessor.
    total: u64,
    /// Count of transitions to each successor pattern.
    successors: HashMap<String, u64>,
}

/// Sequential surprise scorer using bigram pattern transitions.
///
/// Tracks which pattern completed after which, and scores transitions by
/// their conditional surprise: `-log₂(P(current | previous))`.
///
/// # Example
///
/// ```rust
/// use fabula::scoring::SequentialScorer;
///
/// let mut seq = SequentialScorer::new();
/// seq.observe_transition("alliance", "betrayal");
/// seq.observe_transition("alliance", "betrayal");
/// seq.observe_transition("alliance", "trade");
///
/// // betrayal after alliance: common (2/3)
/// let common = seq.score_transition("alliance", "betrayal");
/// // trade after alliance: rarer (1/3)
/// let rare = seq.score_transition("alliance", "trade");
/// assert!(rare > common, "rarer transition should be more surprising");
/// ```
#[derive(Debug, Clone, Default)]
pub struct SequentialScorer {
    rows: HashMap<String, BigramRow>,
}

impl SequentialScorer {
    /// Create a new empty scorer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a transition: `prev` pattern completed, then `current` completed.
    pub fn observe_transition(&mut self, prev: &str, current: &str) {
        let row = self.rows.entry(prev.to_string()).or_default();
        row.total += 1;
        *row.successors.entry(current.to_string()).or_insert(0) += 1;
    }

    /// Laplace-smoothed transition probability `P(current | prev)`.
    ///
    /// Returns `None` if `prev` has never been observed as a predecessor.
    /// Uses Laplace smoothing: `(count + 1) / (total + V)` where V is
    /// the number of distinct successors seen after `prev`.
    pub fn transition_probability(&self, prev: &str, current: &str) -> Option<f64> {
        let row = self.rows.get(prev)?;
        let count = row.successors.get(current).copied().unwrap_or(0);
        let vocab = row.successors.len() as f64;
        Some((count as f64 + 1.0) / (row.total as f64 + vocab))
    }

    /// Sequential surprise in bits: `-log₂(P(current | prev))`.
    ///
    /// **Higher = more surprising.** Returns `0.0` if `prev` has never been
    /// observed (no data to judge surprise).
    pub fn score_transition(&self, prev: &str, current: &str) -> f64 {
        match self.transition_probability(prev, current) {
            Some(p) => -p.log2(),
            None => 0.0,
        }
    }

    /// Total transitions observed from a predecessor.
    pub fn total_transitions_from(&self, prev: &str) -> u64 {
        self.rows.get(prev).map(|r| r.total).unwrap_or(0)
    }

    /// Number of distinct successors observed after a predecessor.
    pub fn vocabulary_size(&self, prev: &str) -> usize {
        self.rows.get(prev).map(|r| r.successors.len()).unwrap_or(0)
    }

    /// Reset all observations.
    pub fn reset(&mut self) {
        self.rows.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rare_transition_scores_higher() {
        let mut seq = SequentialScorer::new();
        for _ in 0..9 {
            seq.observe_transition("a", "b"); // common
        }
        seq.observe_transition("a", "c"); // rare

        let common = seq.score_transition("a", "b");
        let rare = seq.score_transition("a", "c");
        assert!(
            rare > common,
            "rare ({:.3}) should score higher than common ({:.3})",
            rare,
            common
        );
    }

    #[test]
    fn novel_successor_nonzero_via_laplace() {
        let mut seq = SequentialScorer::new();
        seq.observe_transition("a", "b");

        // "c" never observed after "a" -- Laplace gives nonzero
        let p = seq.transition_probability("a", "c").unwrap();
        assert!(p > 0.0, "novel successor should have nonzero probability");
        assert!(p <= 0.5, "novel successor should be low probability: {}", p);
    }

    #[test]
    fn unseen_predecessor_returns_zero() {
        let seq = SequentialScorer::new();
        assert_eq!(seq.score_transition("unknown", "anything"), 0.0);
        assert!(seq.transition_probability("unknown", "anything").is_none());
    }

    #[test]
    fn reset_clears_state() {
        let mut seq = SequentialScorer::new();
        seq.observe_transition("a", "b");
        assert_eq!(seq.total_transitions_from("a"), 1);

        seq.reset();
        assert_eq!(seq.total_transitions_from("a"), 0);
        assert_eq!(seq.vocabulary_size("a"), 0);
    }

    #[test]
    fn vocabulary_tracking() {
        let mut seq = SequentialScorer::new();
        seq.observe_transition("a", "b");
        seq.observe_transition("a", "c");
        seq.observe_transition("a", "b"); // duplicate successor
        assert_eq!(seq.vocabulary_size("a"), 2);
        assert_eq!(seq.total_transitions_from("a"), 3);
    }
}
