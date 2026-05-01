//! Grafeo graph database adapter for fabula.
//!
//! Wraps a `grafeo::GrafeoDB` in-memory instance as a fabula `DataSource`.
//! Temporal intervals are stored as edge properties (`_valid_from`, `_valid_to`).
//! Edge labels map to Grafeo relationship types, and values are stored as
//! edge properties under the key `_value`.

use fabula::datasource::{DataSource, Edge, ValueConstraint};
use fabula::interval::Interval;
use grafeo::{GrafeoDB, NodeId, Value as GValue};
use grafeo_core::Edge as GEdge;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// A value in the Grafeo adapter.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum GrafeoValue {
    /// Reference to another node (by string ID).
    Node(String),
    /// String literal.
    Str(String),
    /// Numeric value.
    Num(f64),
    /// Boolean value.
    Bool(bool),
}

impl Hash for GrafeoValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            GrafeoValue::Node(s) => s.hash(state),
            GrafeoValue::Str(s) => s.hash(state),
            GrafeoValue::Num(n) => n.to_bits().hash(state),
            GrafeoValue::Bool(b) => b.hash(state),
        }
    }
}

impl fabula::expr::ArithmeticValue for GrafeoValue {
    fn try_add(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (GrafeoValue::Num(a), GrafeoValue::Num(b)) => Some(GrafeoValue::Num(a + b)),
            _ => None,
        }
    }
    fn try_sub(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (GrafeoValue::Num(a), GrafeoValue::Num(b)) => Some(GrafeoValue::Num(a - b)),
            _ => None,
        }
    }
    fn try_mul(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (GrafeoValue::Num(a), GrafeoValue::Num(b)) => Some(GrafeoValue::Num(a * b)),
            _ => None,
        }
    }
    fn try_div(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (GrafeoValue::Num(_), GrafeoValue::Num(b)) if *b == 0.0 => None,
            (GrafeoValue::Num(a), GrafeoValue::Num(b)) => Some(GrafeoValue::Num(a / b)),
            _ => None,
        }
    }
}

impl GrafeoValue {
    fn to_grafeo(&self) -> GValue {
        match self {
            GrafeoValue::Node(s) | GrafeoValue::Str(s) => GValue::String(s.clone().into()),
            GrafeoValue::Num(n) => GValue::Float64(*n),
            GrafeoValue::Bool(b) => GValue::Bool(*b),
        }
    }

    fn from_grafeo(val: &GValue, is_node_ref: bool) -> Option<Self> {
        match val {
            GValue::String(s) => {
                let s = s.to_string();
                Some(if is_node_ref {
                    GrafeoValue::Node(s)
                } else {
                    GrafeoValue::Str(s)
                })
            }
            GValue::Float64(f) => Some(GrafeoValue::Num(*f)),
            GValue::Int64(i) => Some(GrafeoValue::Num(*i as f64)),
            GValue::Bool(b) => Some(GrafeoValue::Bool(*b)),
            _ => None,
        }
    }
}

/// Extract interval from a Grafeo edge.
fn edge_interval(edge: &GEdge) -> Option<Interval<i64>> {
    let start = match edge.get_property("_valid_from")? {
        GValue::Int64(i) => *i,
        GValue::Float64(f) => *f as i64,
        _ => return None,
    };
    let end = edge.get_property("_valid_to").and_then(|v| match v {
        GValue::Int64(i) => Some(*i),
        GValue::Float64(f) => Some(*f as i64),
        _ => None,
    });
    Some(match end {
        Some(e) => Interval::new(start, e),
        None => Interval::open(start),
    })
}

/// Extract fabula value from a Grafeo edge.
fn edge_value(edge: &GEdge) -> Option<GrafeoValue> {
    let is_node_ref = edge
        .get_property("_is_node_ref")
        .and_then(|v| match v {
            GValue::Bool(b) => Some(*b),
            _ => None,
        })
        .unwrap_or(false);
    edge.get_property("_value")
        .and_then(|v| GrafeoValue::from_grafeo(v, is_node_ref))
}

/// A temporal graph backed by Grafeo's in-memory graph database.
pub struct GrafeoGraph {
    db: GrafeoDB,
    node_map: HashMap<String, NodeId>,
    current_time: i64,
}

impl GrafeoGraph {
    /// Create a new in-memory Grafeo-backed graph.
    pub fn new() -> Self {
        Self {
            db: GrafeoDB::new_in_memory(),
            node_map: HashMap::new(),
            current_time: 0,
        }
    }

    /// Set the current time.
    pub fn set_time(&mut self, t: i64) {
        self.current_time = t;
    }

    /// Ensure a node exists, returning its Grafeo NodeId.
    pub fn ensure_node(&mut self, id: &str) -> NodeId {
        if let Some(&nid) = self.node_map.get(id) {
            return nid;
        }
        let session = self.db.session();
        let nid = session
            .create_node_with_props(&["Node"], [("_id", GValue::String(id.to_string().into()))]);
        self.node_map.insert(id.to_string(), nid);
        nid
    }

    /// Add a temporal edge.
    pub fn add_edge(&mut self, from: &str, label: &str, value: GrafeoValue, start: i64) {
        let from_nid = self.ensure_node(from);
        let to_nid = if let GrafeoValue::Node(ref target_id) = value {
            self.ensure_node(target_id)
        } else {
            let session = self.db.session();
            session.create_node(&["_Literal"])
        };

        let session = self.db.session();
        let eid = session.create_edge(from_nid, to_nid, label);
        self.db.set_edge_property(eid, "_value", value.to_grafeo());
        self.db
            .set_edge_property(eid, "_valid_from", GValue::Int64(start));
        self.db.set_edge_property(
            eid,
            "_is_node_ref",
            GValue::Bool(matches!(value, GrafeoValue::Node(_))),
        );
    }

    /// Add a temporal edge with a bounded interval `[start, end)`.
    pub fn add_edge_bounded(
        &mut self,
        from: &str,
        label: &str,
        value: GrafeoValue,
        start: i64,
        end: i64,
    ) {
        let from_nid = self.ensure_node(from);
        let to_nid = if let GrafeoValue::Node(ref target_id) = value {
            self.ensure_node(target_id)
        } else {
            let session = self.db.session();
            session.create_node(&["_Literal"])
        };

        let session = self.db.session();
        let eid = session.create_edge(from_nid, to_nid, label);
        self.db.set_edge_property(eid, "_value", value.to_grafeo());
        self.db
            .set_edge_property(eid, "_valid_from", GValue::Int64(start));
        self.db
            .set_edge_property(eid, "_valid_to", GValue::Int64(end));
        self.db.set_edge_property(
            eid,
            "_is_node_ref",
            GValue::Bool(matches!(value, GrafeoValue::Node(_))),
        );
    }

    /// Convenience: add a node-to-node edge.
    pub fn add_ref(&mut self, from: &str, label: &str, to: &str, start: i64) {
        self.add_edge(from, label, GrafeoValue::Node(to.to_string()), start);
    }

    /// Convenience: add a string-valued edge.
    pub fn add_str(&mut self, from: &str, label: &str, value: &str, start: i64) {
        self.add_edge(from, label, GrafeoValue::Str(value.to_string()), start);
    }

    /// Convenience: add a numeric-valued edge.
    pub fn add_num(&mut self, from: &str, label: &str, value: f64, start: i64) {
        self.add_edge(from, label, GrafeoValue::Num(value), start);
    }

    /// Collect edges from a node with a given label, optionally filtering by time.
    fn collect_edges(
        &self,
        node: &str,
        label: &str,
        at: Option<&i64>,
    ) -> Vec<Edge<String, GrafeoValue, i64>> {
        let Some(&nid) = self.node_map.get(node) else {
            return Vec::new();
        };
        let session = self.db.session();
        let neighbors = session.get_neighbors_outgoing_by_type(nid, label);
        let mut results = Vec::new();
        for (_, eid) in neighbors {
            let Some(ge) = session.get_edge(eid) else {
                continue;
            };
            let Some(interval) = edge_interval(&ge) else {
                continue;
            };
            let Some(value) = edge_value(&ge) else {
                continue;
            };
            if let Some(t) = at {
                if !interval.covers(t) {
                    continue;
                }
            }
            results.push(Edge {
                source: node.to_string(),
                target: value,
                interval,
            });
        }
        results
    }

    /// Scan all nodes for edges with a given label, optionally filtering by time + constraint.
    fn scan_edges(
        &self,
        label: &str,
        constraint: &ValueConstraint<GrafeoValue>,
        at: Option<&i64>,
    ) -> Vec<Edge<String, GrafeoValue, i64>> {
        let mut results = Vec::new();
        for (str_id, &nid) in &self.node_map {
            let session = self.db.session();
            let neighbors = session.get_neighbors_outgoing_by_type(nid, label);
            for (_, eid) in neighbors {
                let Some(ge) = session.get_edge(eid) else {
                    continue;
                };
                let Some(interval) = edge_interval(&ge) else {
                    continue;
                };
                let Some(value) = edge_value(&ge) else {
                    continue;
                };
                if let Some(t) = at {
                    if !interval.covers(t) {
                        continue;
                    }
                }
                if !constraint.matches(&value) {
                    continue;
                }
                results.push(Edge {
                    source: str_id.clone(),
                    target: value,
                    interval,
                });
            }
        }
        results
    }
}

impl Default for GrafeoGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl DataSource for GrafeoGraph {
    type N = String;
    type L = String;
    type V = GrafeoValue;
    type T = i64;

    fn edges_from(
        &self,
        node: &String,
        label: &String,
        at: &i64,
    ) -> Vec<Edge<String, GrafeoValue, i64>> {
        self.collect_edges(node, label, Some(at))
    }

    fn scan(
        &self,
        label: &String,
        constraint: &ValueConstraint<GrafeoValue>,
        at: &i64,
    ) -> Vec<Edge<String, GrafeoValue, i64>> {
        self.scan_edges(label, constraint, Some(at))
    }

    fn edges_from_any_time(
        &self,
        node: &String,
        label: &String,
    ) -> Vec<Edge<String, GrafeoValue, i64>> {
        self.collect_edges(node, label, None)
    }

    fn scan_any_time(
        &self,
        label: &String,
        constraint: &ValueConstraint<GrafeoValue>,
    ) -> Vec<Edge<String, GrafeoValue, i64>> {
        self.scan_edges(label, constraint, None)
    }

    fn now(&self) -> i64 {
        self.current_time
    }

    fn value_as_node(&self, value: &GrafeoValue) -> Option<String> {
        match value {
            GrafeoValue::Node(id) => Some(id.clone()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fabula::prelude::*;

    #[test]
    fn grafeo_basic_batch() {
        let mut g = GrafeoGraph::new();
        g.add_str("ev1", "eventType", "enterTown", 1);
        g.add_ref("ev1", "actor", "alice", 1);
        g.add_str("ev2", "eventType", "showHospitality", 2);
        g.add_ref("ev2", "actor", "bob", 2);
        g.add_ref("ev2", "target", "alice", 2);
        g.add_str("ev3", "eventType", "harm", 3);
        g.add_ref("ev3", "actor", "bob", 3);
        g.add_ref("ev3", "target", "alice", 3);
        g.set_time(10);

        let pattern = PatternBuilder::new("violation")
            .stage("e1", |s| {
                s.edge(
                    "e1",
                    "eventType".into(),
                    GrafeoValue::Str("enterTown".into()),
                )
                .edge_bind("e1", "actor".into(), "guest")
            })
            .stage("e2", |s| {
                s.edge(
                    "e2",
                    "eventType".into(),
                    GrafeoValue::Str("showHospitality".into()),
                )
                .edge_bind("e2", "actor".into(), "host")
                .edge_bind("e2", "target".into(), "guest")
            })
            .stage("e3", |s| {
                s.edge("e3", "eventType".into(), GrafeoValue::Str("harm".into()))
                    .edge_bind("e3", "actor".into(), "host")
                    .edge_bind("e3", "target".into(), "guest")
            })
            .build();

        let mut engine: SiftEngineFor<GrafeoGraph> = SiftEngine::new();
        engine.register(pattern);
        let matches = engine.evaluate(&g);
        assert_eq!(
            matches.len(),
            1,
            "should find violation of hospitality on Grafeo"
        );
        match &matches[0].bindings["guest"] {
            BoundValue::Node(n) => assert_eq!(n, "alice"),
            other => panic!("expected guest=alice, got {:?}", other),
        }
    }

    #[test]
    fn grafeo_incremental() {
        let mut g = GrafeoGraph::new();
        let mut engine: SiftEngineFor<GrafeoGraph> = SiftEngine::new();

        let pattern = PatternBuilder::new("find_harm")
            .stage("e", |s| {
                s.edge("e", "eventType".into(), GrafeoValue::Str("harm".into()))
                    .edge_bind("e", "actor".into(), "attacker")
            })
            .build();
        engine.register(pattern);

        g.add_str("ev1", "eventType", "harm", 1);
        g.add_ref("ev1", "actor", "bob", 1);
        g.set_time(1);
        let ev = engine.on_edge_added(
            &g,
            &"ev1".into(),
            &"eventType".into(),
            &GrafeoValue::Str("harm".into()),
            &Interval::open(1),
        );
        assert!(
            ev.iter().any(|e| matches!(e, SiftEvent::Completed { .. })),
            "single-stage should complete on Grafeo"
        );
    }
}

#[cfg(test)]
mod arith_tests {
    use super::*;
    use fabula::expr::ArithmeticValue;

    #[test]
    fn grafeo_num_plus_num() {
        assert_eq!(
            GrafeoValue::Num(2.0).try_add(&GrafeoValue::Num(3.0)),
            Some(GrafeoValue::Num(5.0))
        );
    }

    #[test]
    fn grafeo_div_zero_is_none() {
        assert_eq!(GrafeoValue::Num(1.0).try_div(&GrafeoValue::Num(0.0)), None);
    }

    #[test]
    fn grafeo_node_arith_is_none() {
        assert_eq!(
            GrafeoValue::Node("a".into()).try_sub(&GrafeoValue::Node("b".into())),
            None
        );
    }
}
