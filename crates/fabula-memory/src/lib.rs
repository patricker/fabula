//! A simple in-memory temporal graph implementation of [`DataSource`].
//!
//! Useful for testing fabula without a backing database. Not intended for
//! production use — it's a linear scan over all edges.

use fabula::datasource::{DataSource, Edge, ValueConstraint};
use fabula::interval::Interval;
use std::fmt;
use std::hash::{Hash, Hasher};

/// A value in the in-memory graph — can be a node reference, string, number, or boolean.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum MemValue {
    /// Reference to another node.
    Node(String),
    /// String literal.
    Str(String),
    /// Numeric value.
    Num(f64),
    /// Boolean value.
    Bool(bool),
}

impl Hash for MemValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            MemValue::Node(s) => s.hash(state),
            MemValue::Str(s) => s.hash(state),
            MemValue::Num(n) => n.to_bits().hash(state),
            MemValue::Bool(b) => b.hash(state),
        }
    }
}

impl fmt::Display for MemValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemValue::Node(id) => write!(f, "@{}", id),
            MemValue::Str(s) => write!(f, "\"{}\"", s),
            MemValue::Num(n) => write!(f, "{}", n),
            MemValue::Bool(b) => write!(f, "{}", b),
        }
    }
}

/// A stored edge in the in-memory graph.
#[derive(Debug, Clone)]
struct StoredEdge {
    source: String,
    label: String,
    target: MemValue,
    interval: Interval<i64>,
}

/// A simple in-memory temporal graph.
///
/// Stores edges as `(source, label, target, interval)` tuples.
/// Queries are linear scans — fine for testing, not for production.
pub struct MemGraph {
    edges: Vec<StoredEdge>,
    current_time: i64,
}

impl MemGraph {
    /// Create a new empty graph.
    pub fn new() -> Self {
        Self {
            edges: Vec::new(),
            current_time: 0,
        }
    }

    /// Set the current time.
    pub fn set_time(&mut self, t: i64) {
        self.current_time = t;
    }

    /// Add an edge with an open-ended interval starting at `start`.
    pub fn add_edge(&mut self, source: &str, label: &str, target: MemValue, start: i64) {
        self.edges.push(StoredEdge {
            source: source.to_string(),
            label: label.to_string(),
            target,
            interval: Interval::open(start),
        });
    }

    /// Add an edge with a bounded interval `[start, end)`.
    pub fn add_edge_bounded(
        &mut self,
        source: &str,
        label: &str,
        target: MemValue,
        start: i64,
        end: i64,
    ) {
        self.edges.push(StoredEdge {
            source: source.to_string(),
            label: label.to_string(),
            target,
            interval: Interval::new(start, end),
        });
    }

    /// Convenience: add a node-to-node edge.
    pub fn add_ref(&mut self, source: &str, label: &str, target_node: &str, start: i64) {
        self.add_edge(source, label, MemValue::Node(target_node.to_string()), start);
    }

    /// Convenience: add a node-to-string edge.
    pub fn add_str(&mut self, source: &str, label: &str, value: &str, start: i64) {
        self.add_edge(source, label, MemValue::Str(value.to_string()), start);
    }

    /// Convenience: add a node-to-number edge.
    pub fn add_num(&mut self, source: &str, label: &str, value: f64, start: i64) {
        self.add_edge(source, label, MemValue::Num(value), start);
    }

    /// Total number of edges.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

impl Default for MemGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl DataSource for MemGraph {
    type N = String;
    type L = String;
    type V = MemValue;
    type T = i64;

    fn edges_from(
        &self,
        node: &String,
        label: &String,
        at: &i64,
    ) -> Vec<Edge<String, MemValue, i64>> {
        self.edges
            .iter()
            .filter(|e| &e.source == node && &e.label == label && e.interval.covers(at))
            .map(|e| Edge {
                source: e.source.clone(),
                target: e.target.clone(),
                interval: e.interval.clone(),
            })
            .collect()
    }

    fn scan(
        &self,
        label: &String,
        constraint: &ValueConstraint<MemValue>,
        at: &i64,
    ) -> Vec<Edge<String, MemValue, i64>> {
        self.edges
            .iter()
            .filter(|e| &e.label == label && constraint.matches(&e.target) && e.interval.covers(at))
            .map(|e| Edge {
                source: e.source.clone(),
                target: e.target.clone(),
                interval: e.interval.clone(),
            })
            .collect()
    }

    fn edges_from_any_time(
        &self,
        node: &String,
        label: &String,
    ) -> Vec<Edge<String, MemValue, i64>> {
        self.edges
            .iter()
            .filter(|e| &e.source == node && &e.label == label)
            .map(|e| Edge {
                source: e.source.clone(),
                target: e.target.clone(),
                interval: e.interval.clone(),
            })
            .collect()
    }

    fn scan_any_time(
        &self,
        label: &String,
        constraint: &ValueConstraint<MemValue>,
    ) -> Vec<Edge<String, MemValue, i64>> {
        self.edges
            .iter()
            .filter(|e| &e.label == label && constraint.matches(&e.target))
            .map(|e| Edge {
                source: e.source.clone(),
                target: e.target.clone(),
                interval: e.interval.clone(),
            })
            .collect()
    }

    fn now(&self) -> i64 {
        self.current_time
    }

    fn value_as_node(&self, value: &MemValue) -> Option<String> {
        match value {
            MemValue::Node(id) => Some(id.clone()),
            _ => None,
        }
    }
}
