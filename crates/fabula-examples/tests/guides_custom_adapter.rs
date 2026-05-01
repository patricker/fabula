use fabula::datasource::{DataSource, Edge, ValueConstraint};
use fabula::interval::Interval;
use fabula::prelude::*;
use std::fmt;
use std::hash::{Hash, Hasher};

// #region value_type
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum MyValue {
    NodeRef(String),
    Text(String),
    Number(f64),
    Flag(bool),
}

impl Hash for MyValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            MyValue::NodeRef(s) => s.hash(state),
            MyValue::Text(s) => s.hash(state),
            MyValue::Number(n) => n.to_bits().hash(state),
            MyValue::Flag(b) => b.hash(state),
        }
    }
}

impl fmt::Display for MyValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MyValue::NodeRef(id) => write!(f, "@{}", id),
            MyValue::Text(s) => write!(f, "\"{}\"", s),
            MyValue::Number(n) => write!(f, "{}", n),
            MyValue::Flag(b) => write!(f, "{}", b),
        }
    }
}

// `ArithmeticValue` is NOT required by the engine API itself — fabula
// dispatches let-binding evaluation through a separate `LetEvaluator`.
// Implementing it here means consumers can use the built-in
// `DefaultLetEvaluator` (which requires `V: ArithmeticValue`). If your
// `V` type is foreign (e.g., from another crate you don't own and the
// orphan rule blocks adding this impl), you can instead supply a custom
// `LetEvaluator` impl on a fresh evaluator type, or use `NoLetEvaluator`
// for let-free patterns. Adapters with non-numeric values can return
// `None` for unsupported operand combinations.
impl fabula::expr::ArithmeticValue for MyValue {
    fn try_add(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (MyValue::Number(a), MyValue::Number(b)) => Some(MyValue::Number(a + b)),
            _ => None,
        }
    }
    fn try_sub(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (MyValue::Number(a), MyValue::Number(b)) => Some(MyValue::Number(a - b)),
            _ => None,
        }
    }
    fn try_mul(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (MyValue::Number(a), MyValue::Number(b)) => Some(MyValue::Number(a * b)),
            _ => None,
        }
    }
    fn try_div(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (MyValue::Number(_), MyValue::Number(b)) if *b == 0.0 => None,
            (MyValue::Number(a), MyValue::Number(b)) => Some(MyValue::Number(a / b)),
            _ => None,
        }
    }
}
// #endregion

// #region graph_struct
struct StoredEdge {
    source: String,
    label: String,
    target: MyValue,
    interval: Interval<i64>,
}

pub struct MyGraph {
    edges: Vec<StoredEdge>,
    current_time: i64,
}

impl Default for MyGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl MyGraph {
    pub fn new() -> Self {
        Self {
            edges: Vec::new(),
            current_time: 0,
        }
    }

    pub fn set_time(&mut self, t: i64) {
        self.current_time = t;
    }

    pub fn add_edge(&mut self, source: &str, label: &str, target: MyValue, start: i64) {
        self.edges.push(StoredEdge {
            source: source.to_string(),
            label: label.to_string(),
            target,
            interval: Interval::open(start),
        });
    }

    pub fn add_edge_bounded(
        &mut self,
        source: &str,
        label: &str,
        target: MyValue,
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
}
// #endregion

// #region impl_datasource
impl DataSource for MyGraph {
    type N = String;
    type L = String;
    type V = MyValue;
    type T = i64;

    fn edges_from(
        &self,
        node: &String,
        label: &String,
        at: &i64,
    ) -> Vec<Edge<String, MyValue, i64>> {
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
        constraint: &ValueConstraint<MyValue>,
        at: &i64,
    ) -> Vec<Edge<String, MyValue, i64>> {
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
    ) -> Vec<Edge<String, MyValue, i64>> {
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
        constraint: &ValueConstraint<MyValue>,
    ) -> Vec<Edge<String, MyValue, i64>> {
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

    fn value_as_node(&self, value: &MyValue) -> Option<String> {
        match value {
            MyValue::NodeRef(id) => Some(id.clone()),
            _ => None,
        }
    }
}
// #endregion

#[test]
fn smoke_test() {
    // #region smoke_test
    let mut g = MyGraph::new();
    g.add_edge("ev1", "eventType", MyValue::Text("hello".into()), 1);
    g.add_edge("ev1", "actor", MyValue::NodeRef("alice".into()), 1);
    g.set_time(10);

    let pattern = PatternBuilder::new("greeting")
        .stage("e", |s| {
            s.edge("e", "eventType".into(), MyValue::Text("hello".into()))
                .edge_bind("e", "actor".into(), "who")
        })
        .build();

    let mut engine: SiftEngine<String, String, MyValue, i64, DefaultLetEvaluator> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    let matches = engine.evaluate(&g);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].bindings["who"], BoundValue::Node("alice".into()));
    // #endregion
}
