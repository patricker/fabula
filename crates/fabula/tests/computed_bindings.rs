//! Integration tests for `Stage::let_bindings` evaluation in batch and
//! incremental modes.
//!
//! `fabula` core has zero external dependencies, so this test defines a
//! minimal DataSource (`TestGraph`) with a numeric `V` type
//! (`TestVal`) that implements `ArithmeticValue`.

use fabula::builder::PatternBuilder;
use fabula::datasource::{DataSource, Edge, ValueConstraint};
use fabula::engine::{DefaultLetEvaluator, evaluate_pattern, SiftEngine, SiftEvent};
use fabula::expr::{ArithmeticValue, BinOp, Expr};
use fabula::interval::Interval;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, PartialEq, PartialOrd)]
enum TestVal {
    Str(String),
    Num(f64),
}

impl Hash for TestVal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            TestVal::Str(s) => s.hash(state),
            TestVal::Num(n) => n.to_bits().hash(state),
        }
    }
}

impl ArithmeticValue for TestVal {
    fn try_add(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (TestVal::Num(a), TestVal::Num(b)) => Some(TestVal::Num(a + b)),
            _ => None,
        }
    }
    fn try_sub(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (TestVal::Num(a), TestVal::Num(b)) => Some(TestVal::Num(a - b)),
            _ => None,
        }
    }
    fn try_mul(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (TestVal::Num(a), TestVal::Num(b)) => Some(TestVal::Num(a * b)),
            _ => None,
        }
    }
    fn try_div(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (TestVal::Num(_), TestVal::Num(b)) if *b == 0.0 => None,
            (TestVal::Num(a), TestVal::Num(b)) => Some(TestVal::Num(a / b)),
            _ => None,
        }
    }
}

#[derive(Default)]
struct TestGraph {
    edges: Vec<(String, String, TestVal, i64)>,
    time: i64,
}

impl TestGraph {
    fn add(&mut self, src: &str, label: &str, val: TestVal, t: i64) {
        self.edges.push((src.into(), label.into(), val, t));
    }
    fn set_time(&mut self, t: i64) {
        self.time = t;
    }
}

impl DataSource for TestGraph {
    type N = String;
    type L = String;
    type V = TestVal;
    type T = i64;
    fn now(&self) -> i64 {
        self.time
    }
    fn value_as_node(&self, _v: &TestVal) -> Option<String> {
        None
    }
    fn edges_from(
        &self,
        node: &String,
        label: &String,
        _at: &i64,
    ) -> Vec<Edge<String, TestVal, i64>> {
        self.edges
            .iter()
            .filter(|(s, l, _, _)| s == node && l == label)
            .map(|(s, _, v, t)| Edge {
                source: s.clone(),
                target: v.clone(),
                interval: Interval::open(*t),
            })
            .collect()
    }
    fn edges_from_any_time(
        &self,
        node: &String,
        label: &String,
    ) -> Vec<Edge<String, TestVal, i64>> {
        self.edges_from(node, label, &0)
    }
    fn scan(
        &self,
        label: &String,
        _constraint: &ValueConstraint<TestVal>,
        _at: &i64,
    ) -> Vec<Edge<String, TestVal, i64>> {
        self.edges
            .iter()
            .filter(|(_, l, _, _)| l == label)
            .map(|(s, _, v, t)| Edge {
                source: s.clone(),
                target: v.clone(),
                interval: Interval::open(*t),
            })
            .collect()
    }
    fn scan_any_time(
        &self,
        label: &String,
        constraint: &ValueConstraint<TestVal>,
    ) -> Vec<Edge<String, TestVal, i64>> {
        self.scan(label, constraint, &0)
    }
}

fn graph_with_two_pulses() -> TestGraph {
    let mut g = TestGraph::default();
    g.add("e1", "type", TestVal::Str("world".into()), 1);
    g.add("e1", "pulse_count", TestVal::Num(3.0), 1);
    g.add("e2", "type", TestVal::Str("world".into()), 5);
    g.add("e2", "pulse_count", TestVal::Num(8.0), 5);
    g.set_time(10);
    g
}

#[test]
fn let_in_batch_evaluation_matches_deadline() {
    let g = graph_with_two_pulses();

    // Stage 1 binds ?ts from e1's pulse_count (=3), then `let deadline = ts + 5`
    // (=8). Stage 2 requires e2's pulse_count == ?deadline. e2's pulse_count is
    // 8, so the match should succeed.
    let p = PatternBuilder::<String, TestVal>::new("deadline_match")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), TestVal::Str("world".into()))
                .edge_bind("e1", "pulse_count".into(), "ts")
                .let_binding(
                    "deadline",
                    Expr::bin(BinOp::Add, Expr::var("ts"), Expr::lit(TestVal::Num(5.0))),
                )
        })
        .stage("e2", |s| {
            s.edge("e2", "type".into(), TestVal::Str("world".into()))
                .edge_eq_var("e2", "pulse_count".into(), "deadline")
        })
        .build();

    let matches = evaluate_pattern(&g, &p, &DefaultLetEvaluator);
    assert_eq!(
        matches.len(),
        1,
        "expected one match where pulse_count = ts + 5"
    );
}

#[test]
fn let_with_unbound_var_yields_no_match() {
    let g = graph_with_two_pulses();
    let p = PatternBuilder::<String, TestVal>::new("nope")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), TestVal::Str("world".into()))
                .let_binding(
                    "x",
                    Expr::bin(BinOp::Add, Expr::var("ghost"), Expr::lit(TestVal::Num(1.0))),
                )
        })
        .build();
    let matches = evaluate_pattern(&g, &p, &DefaultLetEvaluator);
    assert_eq!(matches.len(), 0);
}

#[test]
fn let_division_by_zero_yields_no_match() {
    let g = graph_with_two_pulses();
    let p = PatternBuilder::<String, TestVal>::new("divzero")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), TestVal::Str("world".into()))
                .edge_bind("e1", "pulse_count".into(), "ts")
                .let_binding(
                    "x",
                    Expr::bin(BinOp::Div, Expr::var("ts"), Expr::lit(TestVal::Num(0.0))),
                )
        })
        .build();
    let matches = evaluate_pattern(&g, &p, &DefaultLetEvaluator);
    assert_eq!(matches.len(), 0);
}

#[test]
fn let_in_incremental_evaluation_matches_deadline() {
    let mut g = TestGraph::default();
    let p = PatternBuilder::<String, TestVal>::new("deadline_match")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), TestVal::Str("world".into()))
                .edge_bind("e1", "pulse_count".into(), "ts")
                .let_binding(
                    "deadline",
                    Expr::bin(BinOp::Add, Expr::var("ts"), Expr::lit(TestVal::Num(5.0))),
                )
        })
        .stage("e2", |s| {
            s.edge("e2", "type".into(), TestVal::Str("world".into()))
                .edge_eq_var("e2", "pulse_count".into(), "deadline")
        })
        .build();

    let mut engine = SiftEngine::<String, String, TestVal, i64, DefaultLetEvaluator>::new(DefaultLetEvaluator);
    engine.register(p);

    let interval_e1: Interval<i64> = Interval::open(1);
    let interval_e2: Interval<i64> = Interval::open(5);

    // For each event, add the secondary clause edge first so it's visible
    // when the trigger arrives. The trigger for both stages is `type=world`.
    g.add("e1", "pulse_count", TestVal::Num(3.0), 1);
    let _ = engine.on_edge_added(
        &g,
        &"e1".to_string(),
        &"pulse_count".to_string(),
        &TestVal::Num(3.0),
        &interval_e1,
    );
    g.add("e1", "type", TestVal::Str("world".into()), 1);
    let _ = engine.on_edge_added(
        &g,
        &"e1".to_string(),
        &"type".to_string(),
        &TestVal::Str("world".into()),
        &interval_e1,
    );
    g.add("e2", "pulse_count", TestVal::Num(8.0), 5);
    let _ = engine.on_edge_added(
        &g,
        &"e2".to_string(),
        &"pulse_count".to_string(),
        &TestVal::Num(8.0),
        &interval_e2,
    );
    g.add("e2", "type", TestVal::Str("world".into()), 5);
    let evs = engine.on_edge_added(
        &g,
        &"e2".to_string(),
        &"type".to_string(),
        &TestVal::Str("world".into()),
        &interval_e2,
    );

    let completed = evs
        .iter()
        .filter(|e| matches!(e, SiftEvent::Completed { .. }))
        .count();
    assert_eq!(completed, 1, "expected one Completed event");
}

/// Regression: a `let` inside a `repeat_range` looping segment must re-evaluate
/// each iteration. Before the fix, the prior iteration's let result lingered in
/// the loop_bindings map, tripping the shadow-check in eval_stage_lets and
/// killing the next-iteration advancement.
#[test]
fn let_inside_repeat_range_reevaluates_each_iteration() {
    use fabula::compose::repeat_range;

    let sub = PatternBuilder::<String, TestVal>::new("step")
        .stage("e", |s| {
            s.edge_bind("e", "v".into(), "raw").let_binding(
                "doubled",
                Expr::bin(BinOp::Mul, Expr::var("raw"), Expr::lit(TestVal::Num(2.0))),
            )
        })
        .build();

    let looped = repeat_range("looped", &sub, 2, Some(3), &[]);

    let mut g = TestGraph::default();
    // Three matchable events; pattern should complete after 2 and 3 iterations.
    g.add("e1", "v", TestVal::Num(1.0), 1);
    g.add("e2", "v", TestVal::Num(2.0), 2);
    g.add("e3", "v", TestVal::Num(3.0), 3);

    let mut engine = SiftEngine::<String, String, TestVal, i64, DefaultLetEvaluator>::new(DefaultLetEvaluator);
    engine.register(looped);

    for (id, t) in [("e1", 1i64), ("e2", 2), ("e3", 3)] {
        let interval = Interval::open(t);
        let _ = engine.on_edge_added(
            &g,
            &id.to_string(),
            &"v".to_string(),
            &TestVal::Num(t as f64),
            &interval,
        );
    }

    let completed: Vec<_> = engine
        .drain_completed()
        .into_iter()
        .filter(|m| m.pattern == "looped")
        .collect();
    // Expected completions (e1 prologue, then looping):
    //   1. e1+e2 advanced to rep=2 (e2 as iter-1 last_e -- wait, rep=1
    //      transitions to rep=2 only by a SECOND last_e match, so this
    //      completion's bindings show iter-2's last_e). Trace:
    //   2. e1+e3 at rep=2 (e1 prologue, e2 then e3 as last_e iterations)
    //   3. e1+e3 at rep=3 (continuing to a third iteration -- this one only
    //      happens if the PM survives across iterations)
    //   4. e2+e3 at rep=2 (e2 prologue PM advancing once)
    //
    // Without the fix, completion #3 is missing because the stale let value
    // from iter-2 trips the shadow check on iter-3's stage match.
    assert!(
        completed.len() >= 4,
        "expected at least 4 completions (iter-3 of e1-prologue PM proves \
         loop_bindings correctly clears let names), got {}",
        completed.len()
    );
}
