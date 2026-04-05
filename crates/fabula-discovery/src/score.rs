use fabula::pattern::Pattern;
use std::collections::HashMap;

/// Per-evaluator scores for a candidate pattern.
#[derive(Debug, Clone, Default)]
pub struct PatternScore {
    /// Named scores from each evaluator that ran.
    pub scores: HashMap<String, f64>,
    /// Which round of the session produced this.
    pub round: usize,
    /// Which generator produced the candidate.
    pub generator: String,
}

impl PatternScore {
    /// Weighted composite score across all evaluators.
    ///
    /// Evaluators not present in `weights` default to weight 1.0.
    pub fn composite(&self, weights: &HashMap<String, f64>) -> f64 {
        self.scores
            .iter()
            .map(|(name, &value)| {
                let w = weights.get(name).copied().unwrap_or(1.0);
                w * value
            })
            .sum()
    }
}

/// A candidate pattern paired with its evaluation scores.
#[derive(Debug, Clone)]
pub struct ScoredPattern<L, V> {
    /// The candidate pattern.
    pub pattern: Pattern<L, V>,
    /// Evaluation scores from all evaluators.
    pub score: PatternScore,
}
