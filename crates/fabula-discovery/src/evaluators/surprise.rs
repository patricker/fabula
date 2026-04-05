use crate::corpus::TraceCorpus;
use crate::traits::PatternEvaluator;
use fabula::pattern::Pattern;

/// Scores patterns by statistical surprise.
///
/// Uses an interest factor inspired by MINERful:
/// `match_count / expected_count_under_independence`.
///
/// A pattern that matches more often than random label co-occurrence
/// would predict gets a high score. A pattern that matches less than
/// expected (or not at all) gets a low score.
#[derive(Debug, Clone, Copy, Default)]
pub struct SurpriseEvaluator;

impl PatternEvaluator for SurpriseEvaluator {
    fn evaluate(&self, pattern: &Pattern<String, String>, corpus: &TraceCorpus) -> f64 {
        if corpus.is_empty() || pattern.stages.is_empty() {
            return 0.0;
        }

        // Count how many edges in the corpus match each stage's first clause label
        let total = corpus.len() as f64;
        let mut label_freqs: Vec<f64> = Vec::new();

        for stage in &pattern.stages {
            if let Some(clause) = stage.clauses.first() {
                let count = corpus.edges_with_label(&clause.label).len() as f64;
                label_freqs.push(count / total);
            }
        }

        if label_freqs.is_empty() {
            return 0.0;
        }

        // Expected co-occurrence under independence: product of individual frequencies
        let expected_freq: f64 = label_freqs.iter().product();
        if expected_freq == 0.0 {
            return 0.0;
        }

        // Actual match count — use a simple heuristic based on shared-node co-occurrence
        // For single-stage patterns, observed = label frequency
        // For multi-stage, count pairwise co-occurrences sharing a node
        let observed_freq = if pattern.stages.len() == 1 {
            label_freqs[0]
        } else {
            // Count instances where all stage labels co-occur on a shared node
            let first_label = &pattern.stages[0].clauses[0].label;
            let mut co_occurrence = 0usize;

            for edge_a in corpus.edges_with_label(first_label) {
                let mut matches_all = true;
                for stage in pattern.stages.iter().skip(1) {
                    if let Some(clause) = stage.clauses.first() {
                        let has_match = corpus.edges_with_label(&clause.label).iter().any(|e| {
                            e.source == edge_a.source
                                || e.target == edge_a.source
                                || e.source == edge_a.target
                                || e.target == edge_a.target
                        });
                        if !has_match {
                            matches_all = false;
                            break;
                        }
                    }
                }
                if matches_all {
                    co_occurrence += 1;
                }
            }

            co_occurrence as f64 / total
        };

        if observed_freq == 0.0 {
            return 0.0;
        }

        // Interest factor: observed / expected
        // Values > 1.0 mean "more frequent than chance"
        // Take log to compress the scale, add 1 to avoid negative scores for rare-but-present
        let interest = observed_freq / expected_freq;

        // Score: combine interest with rarity
        // Rare labels that co-occur = high surprise
        // Common labels that co-occur = low surprise
        let rarity = -label_freqs.iter().map(|f| f.ln()).sum::<f64>() / label_freqs.len() as f64;

        interest * rarity
    }

    fn name(&self) -> &str {
        "surprise"
    }
}
