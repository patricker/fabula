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
#[derive(Clone)]
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
        self.add_edge(
            source,
            label,
            MemValue::Node(target_node.to_string()),
            start,
        );
    }

    /// Convenience: add a node-to-string edge.
    pub fn add_str(&mut self, source: &str, label: &str, value: &str, start: i64) {
        self.add_edge(source, label, MemValue::Str(value.to_string()), start);
    }

    /// Convenience: add a node-to-number edge.
    pub fn add_num(&mut self, source: &str, label: &str, value: f64, start: i64) {
        self.add_edge(source, label, MemValue::Num(value), start);
    }

    /// Close an open-ended interval on the first matching edge.
    ///
    /// Finds the most recent open-ended edge from `source` with the given
    /// `label` and sets its end to `at`. Returns `true` if an edge was
    /// closed, `false` if no open-ended match was found.
    pub fn end_edge(&mut self, source: &str, label: &str, at: i64) -> bool {
        // Search in reverse to close the most recent matching open edge
        for edge in self.edges.iter_mut().rev() {
            if edge.source == source && edge.label == label && edge.interval.end.is_none() {
                edge.interval.end = Some(at);
                return true;
            }
        }
        false
    }

    /// End the current open-ended edge (if any) and insert a new one.
    ///
    /// Equivalent to `end_edge(source, label, at)` followed by
    /// `add_edge(source, label, value, at)`. Useful for updating state:
    /// close the old value and record the new one at the same timestamp.
    pub fn upsert_edge(&mut self, source: &str, label: &str, value: MemValue, at: i64) {
        self.end_edge(source, label, at);
        self.add_edge(source, label, value, at);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_edge_closes_open_interval() {
        let mut g = MemGraph::new();
        g.add_str("alice", "mood", "happy", 1);
        g.set_time(5);

        // Edge is visible at time 5 (open-ended)
        assert_eq!(g.edges_from(&"alice".into(), &"mood".into(), &5).len(), 1);

        // Close it at time 3
        assert!(g.end_edge("alice", "mood", 3));

        // Now it's bounded [1, 3) — visible at time 2 but not at time 3
        assert_eq!(g.edges_from(&"alice".into(), &"mood".into(), &2).len(), 1);
        assert_eq!(g.edges_from(&"alice".into(), &"mood".into(), &3).len(), 0);
    }

    #[test]
    fn end_edge_returns_false_when_no_open_edge() {
        let mut g = MemGraph::new();
        g.add_edge_bounded("alice", "mood", MemValue::Str("happy".into()), 1, 3);

        // No open-ended edge to close
        assert!(!g.end_edge("alice", "mood", 5));
    }

    #[test]
    fn end_edge_closes_most_recent() {
        let mut g = MemGraph::new();
        g.add_str("alice", "mood", "happy", 1);
        g.add_str("alice", "mood", "sad", 5);

        // Should close "sad" (most recent), not "happy"
        assert!(g.end_edge("alice", "mood", 8));

        // "happy" still open (visible at time 10)
        let edges = g.edges_from(&"alice".into(), &"mood".into(), &10);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].target, MemValue::Str("happy".into()));
    }

    #[test]
    fn upsert_edge_closes_and_inserts() {
        let mut g = MemGraph::new();
        g.add_str("alice", "mood", "happy", 1);
        g.set_time(5);

        // Upsert: close "happy", insert "sad"
        g.upsert_edge("alice", "mood", MemValue::Str("sad".into()), 5);

        // At time 3: only "happy" visible
        let at3 = g.edges_from(&"alice".into(), &"mood".into(), &3);
        assert_eq!(at3.len(), 1);
        assert_eq!(at3[0].target, MemValue::Str("happy".into()));

        // At time 5: only "sad" visible (happy ended at 5, sad starts at 5)
        let at5 = g.edges_from(&"alice".into(), &"mood".into(), &5);
        assert_eq!(at5.len(), 1);
        assert_eq!(at5[0].target, MemValue::Str("sad".into()));

        // Total edges: 2 (the closed one + the new one)
        assert_eq!(g.edge_count(), 2);
    }
}
