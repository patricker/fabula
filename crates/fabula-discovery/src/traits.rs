//! # Design Note: Synchronous Traits
//!
//! All traits are synchronous. For LLM-based evaluators or generators
//! that call async APIs, implementors should use `tokio::runtime::Runtime::block_on`
//! or equivalent. Making these traits async would require `async-trait` and
//! complicate the common case. This may change in a future version.

use crate::corpus::TraceCorpus;
use crate::score::ScoredPattern;
use fabula::pattern::Pattern;

/// Proposes candidate patterns. Receives scored feedback to guide the next round.
///
/// Generators maintain internal state across rounds -- a population of
/// high-scoring patterns, frequency tables, or conversation history with an LLM.
pub trait CandidateGenerator {
    /// Propose up to `budget` candidate patterns from the corpus.
    fn generate(&mut self, corpus: &TraceCorpus, budget: usize) -> Vec<Pattern<String, String>>;

    /// Receive scored results from the previous round.
    /// The generator can use these to guide future proposals.
    fn feedback(&mut self, scored: &[ScoredPattern<String, String>]);

    /// Human-readable name for this generator (used in score metadata).
    fn name(&self) -> &str;
}

/// Scores a candidate pattern against a corpus.
///
/// Multiple evaluators can run on the same candidate. Each returns a named
/// score that is aggregated into a [`PatternScore`](crate::PatternScore).
pub trait PatternEvaluator {
    /// Score a candidate pattern.
    fn evaluate(&self, pattern: &Pattern<String, String>, corpus: &TraceCorpus) -> f64;

    /// Human-readable name for this evaluator (used as score key).
    fn name(&self) -> &str;
}

/// Decides whether a scored pattern is worth keeping.
pub trait PatternFilter {
    /// Returns true if the pattern should be kept.
    fn accept(&self, scored: &ScoredPattern<String, String>) -> bool;
}

/// Accepts patterns whose composite score meets or exceeds a threshold.
pub struct ThresholdFilter {
    /// Minimum composite score to accept.
    pub threshold: f64,
    /// Per-evaluator weights for composite scoring. Missing evaluators default to weight 1.0.
    pub weights: std::collections::HashMap<String, f64>,
}

impl PatternFilter for ThresholdFilter {
    fn accept(&self, scored: &ScoredPattern<String, String>) -> bool {
        scored.score.composite(&self.weights) >= self.threshold
    }
}
