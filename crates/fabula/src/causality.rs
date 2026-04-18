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

use crate::datasource::{DataSource, ValueConstraint};
use crate::interval::NumericTime;
use std::collections::HashMap;

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
/// `divergent_branches` is the total count of sibling causes the path
/// "walked past" — at each node on the path, every additional causal
/// predecessor (beyond the one followed into this path) contributes one.
/// It measures how much of a fork the causal graph had along this chain;
/// the BFS follows all siblings in separate paths, but higher divergence
/// means this chain is one of many plausible explanations rather than
/// the only one.
///
/// Returns a score in `[0.0, 1.0]`. Empty weights → `0.0`.
pub fn cleanliness_score(
    weights: &[f64],
    total_gap: f64,
    divergent_branches: usize,
) -> f64 {
    if weights.is_empty() {
        return 0.0;
    }
    let mean_weight = weights.iter().sum::<f64>() / weights.len() as f64;
    // Gap penalty saturates at 0.5 using an exponential approach.
    // gap = 0 → penalty = 0; gap → ∞ → penalty → 0.5.
    let gap_penalty = 0.5 * (1.0 - (-total_gap / 50.0).exp());
    let divergence_factor = 1.0 / (1.0 + divergent_branches as f64);
    (mean_weight * (1.0 - gap_penalty) * divergence_factor).clamp(0.0, 1.0)
}

/// Collect all causal predecessors of `target` across all causal labels.
/// Returns `(source_node, value, time, weight)` tuples.
type PredecessorEntry<N, V, T> = (N, V, T, f64);

fn causal_predecessors<DS: DataSource>(
    ds: &DS,
    target: &DS::N,
    causal_labels: &HashMap<DS::L, f64>,
) -> Vec<PredecessorEntry<DS::N, DS::V, DS::T>> {
    let mut out = Vec::new();
    for (label, weight) in causal_labels {
        for edge in ds.scan_any_time(label, &ValueConstraint::Any) {
            if let Some(tgt_node) = ds.value_as_node(&edge.target) {
                if &tgt_node == target {
                    let edge_time = edge.interval.start.clone();
                    out.push((edge.source, edge.target, edge_time, *weight));
                }
            }
        }
    }
    out
}

/// Find all causal paths leading to an effect, sorted by cleanliness descending.
///
/// Walks backward from `effect` through edges matching `causal_labels`. A path
/// may contain up to `max_hops` edges. Temporal ordering is validated: each
/// predecessor edge's time must be strictly less than the successor edge's time.
///
/// ```
/// # use fabula::causality::causal_paths;
/// # use std::collections::HashMap;
/// # fn example<DS: fabula::datasource::DataSource<N = String, L = String, V = String, T = i64>>(ds: &DS) {
/// let mut labels: HashMap<String, f64> = HashMap::new();
/// labels.insert("causes".to_string(), 1.0);
/// let paths = causal_paths(ds, &"final_event".to_string(), 5, &labels);
/// for p in paths {
///     println!("cleanliness={:.3}, {} hops", p.cleanliness, p.edges.len());
/// }
/// # }
/// ```
pub fn causal_paths<DS: DataSource>(
    ds: &DS,
    effect: &DS::N,
    max_hops: usize,
    causal_labels: &HashMap<DS::L, f64>,
) -> Vec<CausalPath<DS::N, DS::V, DS::T>>
where
    DS::T: NumericTime,
{
    type WorkItem<N, V, T> = (Vec<N>, Vec<CausalEdge<V, T>>, usize);

    let mut completed: Vec<CausalPath<DS::N, DS::V, DS::T>> = Vec::new();
    let mut worklist: Vec<WorkItem<DS::N, DS::V, DS::T>> =
        vec![(vec![effect.clone()], Vec::new(), 0)];

    while let Some((nodes_rev, edges_rev, divergent_branches)) = worklist.pop() {
        if edges_rev.len() >= max_hops {
            if !edges_rev.is_empty() {
                finalize_path(nodes_rev, edges_rev, divergent_branches, &mut completed);
            }
            continue;
        }

        let current = nodes_rev.last().expect("nodes_rev never empty").clone();
        let preds = causal_predecessors(ds, &current, causal_labels);
        let pred_count = preds.len();

        if preds.is_empty() {
            if !edges_rev.is_empty() {
                finalize_path(nodes_rev, edges_rev, divergent_branches, &mut completed);
            }
            continue;
        }

        for (pred_node, pred_value, pred_time, weight) in preds {
            if nodes_rev.contains(&pred_node) {
                continue;
            }
            if let Some(last_edge) = edges_rev.last() {
                if pred_time >= last_edge.time {
                    continue;
                }
            }

            let mut new_nodes = nodes_rev.clone();
            new_nodes.push(pred_node);
            let mut new_edges = edges_rev.clone();
            new_edges.push(CausalEdge {
                value: pred_value,
                time: pred_time,
                weight,
            });
            let new_branches = divergent_branches + pred_count - 1;
            worklist.push((new_nodes, new_edges, new_branches));
        }

        // Also emit the current path as a valid explanation — shorter paths
        // typically score higher (smaller gap, less divergence) and represent
        // the "proximate cause" view. Longer extensions explored above become
        // additional paths in the result set.
        if !edges_rev.is_empty() {
            finalize_path(nodes_rev, edges_rev, divergent_branches, &mut completed);
        }
    }

    completed.sort_by(|a, b| {
        b.cleanliness
            .partial_cmp(&a.cleanliness)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    completed
}

fn finalize_path<N, V, T>(
    nodes_rev: Vec<N>,
    edges_rev: Vec<CausalEdge<V, T>>,
    divergent_branches: usize,
    out: &mut Vec<CausalPath<N, V, T>>,
) where
    T: NumericTime,
{
    let mut nodes = nodes_rev;
    nodes.reverse();
    let mut edges = edges_rev;
    edges.reverse();

    let total_gap: f64 = if edges.len() < 2 {
        0.0
    } else {
        edges
            .windows(2)
            .map(|w| (w[1].time.as_f64() - w[0].time.as_f64()).abs())
            .sum()
    };

    let weights: Vec<f64> = edges.iter().map(|e| e.weight).collect();
    let cleanliness = cleanliness_score(&weights, total_gap, divergent_branches);
    // Confidence = weakest-link: a chain is only as strong as its least-certain edge.
    // This is a distinct signal from cleanliness (which uses the mean).
    let confidence = weights.iter().copied().reduce(f64::min).unwrap_or(0.0);

    out.push(CausalPath {
        nodes,
        edges,
        cleanliness,
        confidence,
    });
}

#[cfg(test)]
mod bfs_tests {
    use super::*;
    use crate::datasource::{DataSource, Edge, ValueConstraint};
    use crate::interval::Interval;
    use std::collections::HashMap;

    // Minimal inline datasource for unit tests — avoids pulling in fabula-memory.
    #[derive(Default)]
    struct ToyGraph {
        edges: Vec<(String, String, String, i64)>, // (src, label, target, time)
    }
    impl ToyGraph {
        fn add(&mut self, src: &str, label: &str, tgt: &str, t: i64) {
            self.edges
                .push((src.into(), label.into(), tgt.into(), t));
        }
    }
    impl DataSource for ToyGraph {
        type N = String;
        type L = String;
        type V = String;
        type T = i64;
        fn now(&self) -> i64 {
            100
        }
        fn value_as_node(&self, v: &String) -> Option<String> {
            Some(v.clone())
        }
        fn edges_from(
            &self,
            node: &String,
            label: &String,
            _at: &i64,
        ) -> Vec<Edge<String, String, i64>> {
            self.edges_from_any_time(node, label)
        }
        fn edges_from_any_time(
            &self,
            node: &String,
            label: &String,
        ) -> Vec<Edge<String, String, i64>> {
            self.edges
                .iter()
                .filter(|(s, l, _, _)| s == node && l == label)
                .map(|(s, _, t, time)| Edge {
                    source: s.clone(),
                    target: t.clone(),
                    interval: Interval::open(*time),
                })
                .collect()
        }
        fn scan(
            &self,
            _label: &String,
            _constraint: &ValueConstraint<String>,
            _at: &i64,
        ) -> Vec<Edge<String, String, i64>> {
            vec![]
        }
        fn scan_any_time(
            &self,
            label: &String,
            _constraint: &ValueConstraint<String>,
        ) -> Vec<Edge<String, String, i64>> {
            self.edges
                .iter()
                .filter(|(_, l, _, _)| l == label)
                .map(|(s, _, t, time)| Edge {
                    source: s.clone(),
                    target: t.clone(),
                    interval: Interval::open(*time),
                })
                .collect()
        }
    }

    fn causal_labels() -> HashMap<String, f64> {
        [("causes".to_string(), 1.0)].into_iter().collect()
    }

    #[test]
    fn single_hop_cause() {
        let mut g = ToyGraph::default();
        g.add("a", "causes", "b", 1);
        let paths = causal_paths(&g, &"b".to_string(), 3, &causal_labels());
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].nodes, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn multi_hop_chain() {
        let mut g = ToyGraph::default();
        g.add("a", "causes", "b", 1);
        g.add("b", "causes", "c", 2);
        let paths = causal_paths(&g, &"c".to_string(), 5, &causal_labels());
        let long_path = paths.iter().find(|p| p.nodes.len() == 3).expect("need chain");
        assert_eq!(
            long_path.nodes,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn no_causal_edges_returns_empty() {
        let mut g = ToyGraph::default();
        g.add("a", "unrelated", "b", 1);
        let paths = causal_paths(&g, &"b".to_string(), 3, &causal_labels());
        assert!(paths.is_empty());
    }

    #[test]
    fn max_hops_limits_search() {
        let mut g = ToyGraph::default();
        g.add("a", "causes", "b", 1);
        g.add("b", "causes", "c", 2);
        g.add("c", "causes", "d", 3);
        let paths = causal_paths(&g, &"d".to_string(), 2, &causal_labels());
        assert!(paths.iter().all(|p| p.edges.len() <= 2));
    }
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
