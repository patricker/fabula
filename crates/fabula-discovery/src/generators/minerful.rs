use crate::corpus::{SharedNode, TraceCorpus};
use crate::score::ScoredPattern;
use crate::traits::CandidateGenerator;
use fabula::interval::AllenRelation;
use fabula::pattern::{Clause, Pattern, Stage, Target, TemporalConstraint, Var};
use std::collections::HashMap;

/// Configuration for the MINERful-adapted generator.
#[derive(Debug, Clone)]
pub struct MinerfulConfig {
    /// Minimum fraction of corpus edges that must participate in a pattern.
    pub min_support: f64,
    /// Minimum fraction of co-occurrences where the Allen relation holds.
    pub min_confidence: f64,
}

impl Default for MinerfulConfig {
    fn default() -> Self {
        Self {
            min_support: 0.1,
            min_confidence: 0.5,
        }
    }
}

/// MINERful-adapted constraint miner.
///
/// Discovers two-stage patterns by:
/// 1. Finding all label pairs that share nodes in the corpus
/// 2. Computing Allen relation distributions for each pair
/// 3. Emitting patterns for pairs exceeding support/confidence thresholds
///
/// Based on Di Ciccio & Mecella's MINERful (2015), adapted for
/// Allen interval algebra over temporal graphs.
///
/// This is a **single-pass** miner: it generates all candidates on the first
/// call to [`CandidateGenerator::generate`] and returns empty on subsequent calls.
/// In a multi-round [`DiscoverySession`](crate::DiscoverySession), the session
/// will terminate early after round 1.
pub struct MinerfulGenerator {
    config: MinerfulConfig,
    round: usize,
    generated: bool,
}

impl MinerfulGenerator {
    /// Create a generator with the given support/confidence thresholds.
    pub fn new(config: MinerfulConfig) -> Self {
        Self {
            config,
            round: 0,
            generated: false,
        }
    }
}

/// Statistics for a label pair + Allen relation combination.
#[derive(Debug)]
struct PairStats {
    label_a: String,
    label_b: String,
    relation: AllenRelation,
    /// How many instances of this (label_a, label_b, relation) triple exist.
    count: usize,
    /// Total co-occurrences of (label_a, label_b) regardless of relation.
    total_pair: usize,
    /// Most common shared node type for this pair.
    shared_node_example: SharedNode,
}

impl CandidateGenerator for MinerfulGenerator {
    fn generate(&mut self, corpus: &TraceCorpus, budget: usize) -> Vec<Pattern<String, String>> {
        if self.generated {
            return Vec::new();
        }
        self.generated = true;
        self.round += 1;

        let total_edges = corpus.len() as f64;
        if total_edges == 0.0 {
            return Vec::new();
        }

        // Phase 1: Compute pairwise Allen relation statistics
        let mut pair_stats: Vec<PairStats> = Vec::new();

        for (label_a, label_b) in corpus.label_pairs() {
            let hits = corpus.pairwise_relations(label_a, label_b);
            if hits.is_empty() {
                continue;
            }

            // Count by Allen relation
            let mut by_relation: HashMap<AllenRelation, (usize, SharedNode)> = HashMap::new();
            for hit in &hits {
                let entry = by_relation
                    .entry(hit.relation)
                    .or_insert((0, hit.shared_node.clone()));
                entry.0 += 1;
            }

            let total_pair = hits.len();
            let support = total_pair as f64 / total_edges;

            if support < self.config.min_support {
                continue;
            }

            for (relation, (count, shared_node)) in by_relation {
                let confidence = count as f64 / total_pair as f64;
                if confidence >= self.config.min_confidence {
                    pair_stats.push(PairStats {
                        label_a: label_a.to_string(),
                        label_b: label_b.to_string(),
                        relation,
                        count,
                        total_pair,
                        shared_node_example: shared_node,
                    });
                }
            }
        }

        // Phase 2: Sort by interest factor (count * confidence) and take top budget
        pair_stats.sort_by(|a, b| {
            let interest_a = a.count as f64 * (a.count as f64 / a.total_pair as f64);
            let interest_b = b.count as f64 * (b.count as f64 / b.total_pair as f64);
            interest_b
                .partial_cmp(&interest_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        pair_stats.truncate(budget);

        // Phase 3: Convert to Pattern objects
        pair_stats
            .into_iter()
            .enumerate()
            .map(|(i, stats)| build_pattern(i, self.round, &stats))
            .collect()
    }

    fn feedback(&mut self, _scored: &[ScoredPattern<String, String>]) {
        // MINERful is a single-pass miner -- feedback doesn't change its behavior.
        // Future: could adjust support/confidence thresholds based on acceptance rates.
    }

    fn name(&self) -> &str {
        "minerful"
    }
}

fn build_pattern(idx: usize, round: usize, stats: &PairStats) -> Pattern<String, String> {
    let name = format!(
        "discovered_r{}_{}_{}_{}",
        round,
        idx,
        stats.label_a.replace(' ', "_"),
        stats.label_b.replace(' ', "_"),
    );

    // Determine variable bindings from shared node type
    let (source_a, target_a, source_b, target_b) = match &stats.shared_node_example {
        SharedNode::Source(_) => {
            // Both edges share the same source
            ("actor", "target_a", "actor", "target_b")
        }
        SharedNode::SourceTarget(_) => {
            // Edge A's source = Edge B's target
            ("actor", "target_a", "source_b", "actor")
        }
        SharedNode::TargetSource(_) => {
            // Edge A's target = Edge B's source
            ("source_a", "actor", "actor", "target_b")
        }
        SharedNode::Target(_) => {
            // Both edges share the same target
            ("source_a", "actor", "source_b", "actor")
        }
    };

    let stage_a = Stage {
        anchor: Var::new("e1"),
        clauses: vec![Clause {
            source: Var::new(source_a),
            label: stats.label_a.clone(),
            target: Target::Bind(Var::new(target_a)),
            negated: false,
        }],
        let_bindings: Vec::new(),
    };

    let stage_b = Stage {
        anchor: Var::new("e2"),
        clauses: vec![Clause {
            source: Var::new(source_b),
            label: stats.label_b.clone(),
            target: Target::Bind(Var::new(target_b)),
            negated: false,
        }],
        let_bindings: Vec::new(),
    };

    Pattern {
        name,
        stages: vec![stage_a, stage_b],
        temporal: vec![TemporalConstraint {
            left: Var::new("e1"),
            relation: stats.relation,
            right: Var::new("e2"),
            gap: None,
        }],
        negations: Vec::new(),
        group: None,
        metadata: HashMap::new(),
        deadline_ticks: None,
        inactivity_threshold: None,
        repeat_range: None,
        unordered_groups: Vec::new(),
        private: false,
        importance: 1.0,
        advance_in_place: false,
    }
}
