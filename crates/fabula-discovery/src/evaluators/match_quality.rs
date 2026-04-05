use crate::corpus::TraceCorpus;
use crate::traits::PatternEvaluator;
use fabula::engine::evaluate_pattern;
use fabula::pattern::Pattern;
use fabula_memory::{MemGraph, MemValue};

/// Scores patterns by match quality.
///
/// Builds a MemGraph from the corpus, runs `evaluate_pattern` against it,
/// and scores based on match count and specificity (stages x constraints).
/// A pattern with moderate match count and high specificity scores best.
#[derive(Debug, Clone, Copy, Default)]
pub struct MatchQualityEvaluator;

impl PatternEvaluator for MatchQualityEvaluator {
    fn evaluate(&self, pattern: &Pattern<String, String>, corpus: &TraceCorpus) -> f64 {
        let graph = corpus_to_memgraph(corpus);

        // Convert Pattern<String, String> to Pattern<String, MemValue> for MemGraph
        let mem_pattern = pattern.map_types(|l| l.clone(), |v| MemValue::Node(v.clone()));

        let matches = evaluate_pattern(&graph, &mem_pattern);

        let match_count = matches.len();
        if match_count == 0 {
            return 0.0;
        }

        // Specificity: more stages and more clauses = more specific pattern
        let total_clauses: usize = pattern.stages.iter().map(|s| s.clauses.len()).sum();
        let specificity = (pattern.stages.len() as f64) + (total_clauses as f64 * 0.5);

        // Sweet spot scoring: penalize both too few and too many matches
        // Peak at ~5-20 matches for a typical corpus
        let corpus_size = corpus.len() as f64;
        let match_ratio = match_count as f64 / corpus_size;
        let match_quality = if match_ratio > 0.5 {
            // Too general — matches more than half the corpus
            0.5 / match_ratio
        } else {
            // Good — specific enough to be interesting
            (match_count as f64).ln().max(0.0)
        };

        match_quality * specificity
    }

    fn name(&self) -> &str {
        "match_quality"
    }
}

/// Convert a TraceCorpus to a MemGraph for pattern evaluation.
fn corpus_to_memgraph(corpus: &TraceCorpus) -> MemGraph {
    let mut graph = MemGraph::new();
    let (_, max_t) = corpus.time_range();
    graph.set_time(max_t + 1);

    for edge in corpus.edges() {
        let value = MemValue::Node(edge.target.clone());
        if let Some(end) = edge.interval.end {
            graph.add_edge_bounded(&edge.source, &edge.label, value, edge.interval.start, end);
        } else {
            graph.add_edge(&edge.source, &edge.label, value, edge.interval.start);
        }
    }

    graph
}
