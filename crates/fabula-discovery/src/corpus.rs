use fabula::interval::{AllenRelation, Interval};
use std::collections::{HashMap, HashSet};

/// A single edge in a trace corpus.
#[derive(Debug, Clone)]
pub struct TraceEdge {
    pub source: String,
    pub label: String,
    pub target: String,
    pub interval: Interval<i64>,
}

/// An indexed log of edges for pattern discovery.
///
/// Built from a simulation's edge history. Provides indexed access
/// by label, by node, and by time range for efficient mining.
#[derive(Debug, Clone)]
pub struct TraceCorpus {
    edges: Vec<TraceEdge>,
    by_label: HashMap<String, Vec<usize>>,
    by_source: HashMap<String, Vec<usize>>,
    by_target: HashMap<String, Vec<usize>>,
}

impl TraceCorpus {
    /// Build a corpus from a list of (source, label, target, interval) tuples.
    pub fn new(raw: Vec<(String, String, String, Interval<i64>)>) -> Self {
        let edges: Vec<TraceEdge> = raw
            .into_iter()
            .map(|(source, label, target, interval)| TraceEdge {
                source,
                label,
                target,
                interval,
            })
            .collect();

        let mut by_label: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_source: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_target: HashMap<String, Vec<usize>> = HashMap::new();

        for (i, e) in edges.iter().enumerate() {
            by_label.entry(e.label.clone()).or_default().push(i);
            by_source.entry(e.source.clone()).or_default().push(i);
            by_target.entry(e.target.clone()).or_default().push(i);
        }

        Self {
            edges,
            by_label,
            by_source,
            by_target,
        }
    }

    /// Number of edges in the corpus.
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    /// Returns true if the corpus has no edges.
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// All edges in insertion order.
    pub fn edges(&self) -> &[TraceEdge] {
        &self.edges
    }

    /// All distinct labels in the corpus.
    pub fn labels(&self) -> HashSet<&str> {
        self.by_label.keys().map(|s| s.as_str()).collect()
    }

    /// All distinct nodes (sources and targets) in the corpus.
    pub fn nodes(&self) -> HashSet<&str> {
        let mut nodes: HashSet<&str> = HashSet::new();
        for e in &self.edges {
            nodes.insert(&e.source);
            nodes.insert(&e.target);
        }
        nodes
    }

    /// Edges with a given label.
    pub fn edges_with_label(&self, label: &str) -> Vec<&TraceEdge> {
        self.by_label
            .get(label)
            .map(|indices| indices.iter().map(|&i| &self.edges[i]).collect())
            .unwrap_or_default()
    }

    /// Edges originating from a given node.
    pub fn edges_from_node(&self, node: &str) -> Vec<&TraceEdge> {
        self.by_source
            .get(node)
            .map(|indices| indices.iter().map(|&i| &self.edges[i]).collect())
            .unwrap_or_default()
    }

    /// Edges targeting a given node.
    pub fn edges_to_node(&self, node: &str) -> Vec<&TraceEdge> {
        self.by_target
            .get(node)
            .map(|indices| indices.iter().map(|&i| &self.edges[i]).collect())
            .unwrap_or_default()
    }

    /// (min_start, max_end) across all edges.
    /// For open-ended intervals, uses start as the end bound.
    pub fn time_range(&self) -> (i64, i64) {
        let min = self
            .edges
            .iter()
            .map(|e| e.interval.start)
            .min()
            .unwrap_or(0);
        let max = self
            .edges
            .iter()
            .map(|e| e.interval.end.unwrap_or(e.interval.start))
            .max()
            .unwrap_or(0);
        (min, max)
    }

    /// Split into two corpora at time `t`.
    /// Edges with `start < t` go to the first corpus; the rest to the second.
    pub fn split_at(&self, t: &i64) -> (Self, Self) {
        let (before, after): (Vec<_>, Vec<_>) = self
            .edges
            .iter()
            .cloned()
            .map(|e| (e.source, e.label, e.target, e.interval))
            .partition(|(_, _, _, iv)| iv.start < *t);

        (Self::new(before), Self::new(after))
    }

    /// All ordered pairs of distinct labels.
    pub fn label_pairs(&self) -> Vec<(&str, &str)> {
        let mut labels: Vec<&str> = self.labels().into_iter().collect();
        labels.sort();
        let mut pairs = Vec::new();
        for &a in &labels {
            for &b in &labels {
                if a != b {
                    pairs.push((a, b));
                }
            }
        }
        pairs
    }

    /// For a pair of labels, find all instances where edges share a node
    /// (source of one matches source or target of the other) and compute
    /// the Allen relation between their intervals.
    pub fn pairwise_relations(&self, label_a: &str, label_b: &str) -> Vec<PairwiseHit> {
        let edges_a = self.edges_with_label(label_a);
        let edges_b = self.edges_with_label(label_b);
        let mut hits = Vec::new();

        for a in &edges_a {
            for b in &edges_b {
                // Check for shared node (source-source, source-target, target-source)
                let shared = if a.source == b.source {
                    Some(SharedNode::Source(a.source.clone()))
                } else if a.source == b.target {
                    Some(SharedNode::SourceTarget(a.source.clone()))
                } else if a.target == b.source {
                    Some(SharedNode::TargetSource(a.target.clone()))
                } else if a.target == b.target {
                    Some(SharedNode::Target(a.target.clone()))
                } else {
                    None
                };

                if let Some(shared_node) = shared {
                    if let Some(relation) = a.interval.relation(&b.interval) {
                        hits.push(PairwiseHit {
                            shared_node,
                            relation,
                        });
                    }
                }
            }
        }

        hits
    }
}

/// How two edges share a node.
#[derive(Debug, Clone, PartialEq)]
pub enum SharedNode {
    /// Both edges have the same source.
    Source(String),
    /// Edge A's source equals edge B's target.
    SourceTarget(String),
    /// Edge A's target equals edge B's source.
    TargetSource(String),
    /// Both edges have the same target.
    Target(String),
}

/// A co-occurrence of two edges sharing a node with a computed Allen relation.
#[derive(Debug, Clone)]
pub struct PairwiseHit {
    pub shared_node: SharedNode,
    pub relation: AllenRelation,
}
