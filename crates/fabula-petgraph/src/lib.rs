//! Petgraph adapter for fabula -- wraps `petgraph::StableGraph` with temporal edges.
//!
//! Petgraph has no native temporal support, so this adapter stores intervals
//! as part of the edge weight. Queries filter by time at evaluation.

use fabula::datasource::{DataSource, Edge, ValueConstraint};
use fabula::interval::Interval;
use petgraph::stable_graph::{NodeIndex, StableGraph};
use petgraph::Direction;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

/// A temporal edge weight -- bundles a label, value, and validity interval
/// onto a petgraph edge.
#[derive(Debug, Clone)]
pub struct TemporalEdge<L, V, T> {
    pub label: L,
    pub value: V,
    pub interval: Interval<T>,
}

/// A temporal graph backed by `petgraph::StableGraph`.
///
/// Nodes are identified by `N`, edges carry `TemporalEdge<L, V, T>`.
/// A `HashMap<N, NodeIndex>` provides reverse lookup from user IDs to
/// petgraph's internal `NodeIndex`.
pub struct PetTemporalGraph<N, L, V, T> {
    graph: StableGraph<N, TemporalEdge<L, V, T>>,
    node_index: HashMap<N, NodeIndex>,
    current_time: T,
}

impl<N, L, V, T> PetTemporalGraph<N, L, V, T>
where
    N: Eq + Hash + Clone + Debug,
    T: Clone,
{
    /// Create a new empty graph with the given initial time.
    pub fn new(initial_time: T) -> Self {
        Self {
            graph: StableGraph::new(),
            node_index: HashMap::new(),
            current_time: initial_time,
        }
    }

    /// Set the current time.
    pub fn set_time(&mut self, t: T) {
        self.current_time = t;
    }

    /// Add a node (or return existing index if already present).
    pub fn add_node(&mut self, id: N) -> NodeIndex {
        if let Some(&idx) = self.node_index.get(&id) {
            idx
        } else {
            let idx = self.graph.add_node(id.clone());
            self.node_index.insert(id, idx);
            idx
        }
    }

    /// Add a temporal edge between two nodes.
    pub fn add_edge(&mut self, from: N, label: L, value: V, interval: Interval<T>) {
        let from_idx = self.add_node(from);
        let edge = TemporalEdge {
            label,
            value,
            interval,
        };
        // We need a "to" node for petgraph. Use from_idx as a self-loop
        // since we store the target in the edge value, not as a graph edge target.
        // This is a design choice: petgraph edges connect NodeIndex pairs,
        // but fabula's model has edges carrying values (which may or may not be nodes).
        self.graph.add_edge(from_idx, from_idx, edge);
    }

    /// Add a temporal edge with a node-valued target (also ensures target node exists).
    pub fn add_edge_to_node(&mut self, from: N, label: L, to: N, interval: Interval<T>)
    where
        V: From<NodeRef<N>>,
    {
        self.add_node(to.clone());
        let value = V::from(NodeRef(to));
        self.add_edge(from, label, value, interval);
    }

    /// Add a temporal edge with a bounded interval `[start, end)`.
    pub fn add_edge_bounded(&mut self, from: N, label: L, value: V, start: T, end: T)
    where
        T: Ord,
    {
        let from_idx = self.add_node(from);
        let edge = TemporalEdge {
            label,
            value,
            interval: Interval::new(start, end),
        };
        self.graph.add_edge(from_idx, from_idx, edge);
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Number of edges.
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }
}

/// Wrapper to distinguish node references from other values.
/// Used with `From<NodeRef<N>>` to convert node IDs into values.
#[derive(Debug, Clone)]
pub struct NodeRef<N>(pub N);

/// A value type for the petgraph adapter that can hold node references or literals.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum PetValue<N: Debug + Clone + PartialOrd> {
    /// Reference to another node.
    Node(N),
    /// String literal.
    Str(String),
    /// Numeric value.
    Num(f64),
    /// Boolean value.
    Bool(bool),
}

impl<N: Debug + Clone + PartialOrd + Hash> Hash for PetValue<N> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            PetValue::Node(n) => n.hash(state),
            PetValue::Str(s) => s.hash(state),
            PetValue::Num(n) => n.to_bits().hash(state),
            PetValue::Bool(b) => b.hash(state),
        }
    }
}

impl<N: Debug + Clone + PartialOrd> From<NodeRef<N>> for PetValue<N> {
    fn from(r: NodeRef<N>) -> Self {
        PetValue::Node(r.0)
    }
}

impl<N, L, T> DataSource for PetTemporalGraph<N, L, PetValue<N>, T>
where
    N: Eq + Hash + Clone + Debug + PartialOrd,
    L: Eq + Hash + Clone + Debug,
    T: Ord + Clone + Debug + Hash,
{
    type N = N;
    type L = L;
    type V = PetValue<N>;
    type T = T;

    fn edges_from(&self, node: &N, label: &L, at: &T) -> Vec<Edge<N, PetValue<N>, T>> {
        let Some(&idx) = self.node_index.get(node) else {
            return Vec::new();
        };
        self.graph
            .edges_directed(idx, Direction::Outgoing)
            .filter(|e| &e.weight().label == label && e.weight().interval.covers(at))
            .map(|e| Edge {
                source: node.clone(),
                target: e.weight().value.clone(),
                interval: e.weight().interval.clone(),
            })
            .collect()
    }

    fn scan(
        &self,
        label: &L,
        constraint: &ValueConstraint<PetValue<N>>,
        at: &T,
    ) -> Vec<Edge<N, PetValue<N>, T>> {
        let mut results = Vec::new();
        for idx in self.graph.node_indices() {
            let node_id = &self.graph[idx];
            for e in self.graph.edges_directed(idx, Direction::Outgoing) {
                let w = e.weight();
                if &w.label == label && constraint.matches(&w.value) && w.interval.covers(at) {
                    results.push(Edge {
                        source: node_id.clone(),
                        target: w.value.clone(),
                        interval: w.interval.clone(),
                    });
                }
            }
        }
        results
    }

    fn edges_from_any_time(&self, node: &N, label: &L) -> Vec<Edge<N, PetValue<N>, T>> {
        let Some(&idx) = self.node_index.get(node) else {
            return Vec::new();
        };
        self.graph
            .edges_directed(idx, Direction::Outgoing)
            .filter(|e| &e.weight().label == label)
            .map(|e| Edge {
                source: node.clone(),
                target: e.weight().value.clone(),
                interval: e.weight().interval.clone(),
            })
            .collect()
    }

    fn scan_any_time(
        &self,
        label: &L,
        constraint: &ValueConstraint<PetValue<N>>,
    ) -> Vec<Edge<N, PetValue<N>, T>> {
        let mut results = Vec::new();
        for idx in self.graph.node_indices() {
            let node_id = &self.graph[idx];
            for e in self.graph.edges_directed(idx, Direction::Outgoing) {
                let w = e.weight();
                if &w.label == label && constraint.matches(&w.value) {
                    results.push(Edge {
                        source: node_id.clone(),
                        target: w.value.clone(),
                        interval: w.interval.clone(),
                    });
                }
            }
        }
        results
    }

    fn now(&self) -> T {
        self.current_time.clone()
    }

    fn value_as_node(&self, value: &PetValue<N>) -> Option<N> {
        match value {
            PetValue::Node(n) => Some(n.clone()),
            _ => None,
        }
    }
}
