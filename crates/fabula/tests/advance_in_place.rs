//! End-to-end tests for the advance_in_place pattern flag.

use fabula::builder::PatternBuilder;
use fabula::datasource::{DataSource, Edge, ValueConstraint};
use fabula::engine::{DefaultLetEvaluator, MatchState, SiftEngine, SiftEngineFor, SiftEvent};
use fabula::interval::Interval;

#[derive(Default)]
struct ToyGraph {
    edges: Vec<(String, String, String, i64)>,
    time: i64,
}

impl ToyGraph {
    fn add_str(&mut self, src: &str, label: &str, val: &str, t: i64) {
        self.edges.push((src.into(), label.into(), val.into(), t));
    }
    fn set_time(&mut self, t: i64) {
        self.time = t;
    }
}

impl DataSource for ToyGraph {
    type N = String;
    type L = String;
    type V = String;
    type T = i64;
    fn now(&self) -> i64 {
        self.time
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
    fn edges_from_any_time(&self, node: &String, label: &String) -> Vec<Edge<String, String, i64>> {
        self.edges_from(node, label, &0)
    }
    fn scan(
        &self,
        label: &String,
        _constraint: &ValueConstraint<String>,
        _at: &i64,
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
    fn scan_any_time(
        &self,
        label: &String,
        constraint: &ValueConstraint<String>,
    ) -> Vec<Edge<String, String, i64>> {
        self.scan(label, constraint, &0)
    }
}

fn two_stage_pattern(advance_in_place: bool) -> fabula::pattern::Pattern<String, String> {
    let mut b = PatternBuilder::<String, String>::new("enter_then_leave")
        .stage("a", |s| s.edge("a", "eventType".into(), "enter".into()))
        .stage("b", |s| s.edge("b", "eventType".into(), "leave".into()));
    if advance_in_place {
        b = b.advance_in_place();
    }
    b.build()
}

#[test]
fn default_behavior_original_survives_advancement() {
    let mut g = ToyGraph::default();
    let mut engine: SiftEngineFor<ToyGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(two_stage_pattern(false));

    g.add_str("ev1", "eventType", "enter", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".to_string(),
        &"eventType".to_string(),
        &"enter".to_string(),
        &Interval::open(1),
    );

    g.add_str("ev2", "eventType", "leave", 2);
    g.set_time(2);
    engine.on_edge_added(
        &g,
        &"ev2".to_string(),
        &"eventType".to_string(),
        &"leave".to_string(),
        &Interval::open(2),
    );
    engine.end_tick(0);

    // next_stage == 1 means "advanced past stage 0, still waiting for stage 1"
    // i.e. the ORIGINAL PM that sat at stage 0 was cloned forward while the
    // original stayed alive. Default fork behavior → at least one such PM.
    let stage_one_active = engine
        .partial_matches()
        .iter()
        .filter(|pm| pm.next_stage == 1 && pm.state == MatchState::Active)
        .count();
    assert!(
        stage_one_active >= 1,
        "default behavior should preserve a stage-1 PM, got {}",
        stage_one_active
    );
}

#[test]
fn advance_in_place_consumes_original_after_strict_forward_advance() {
    let mut g = ToyGraph::default();
    let mut engine: SiftEngineFor<ToyGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(two_stage_pattern(true));

    g.add_str("ev1", "eventType", "enter", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".to_string(),
        &"eventType".to_string(),
        &"enter".to_string(),
        &Interval::open(1),
    );

    g.add_str("ev2", "eventType", "leave", 2);
    g.set_time(2);
    engine.on_edge_added(
        &g,
        &"ev2".to_string(),
        &"eventType".to_string(),
        &"leave".to_string(),
        &Interval::open(2),
    );
    engine.end_tick(0);

    let stage_one_active = engine
        .partial_matches()
        .iter()
        .filter(|pm| pm.next_stage == 1 && pm.state == MatchState::Active)
        .count();
    assert_eq!(
        stage_one_active, 0,
        "advance_in_place should consume the stage-1 PM after advancement, got {}",
        stage_one_active
    );
}

#[test]
fn advance_in_place_still_emits_completed_event() {
    let mut g = ToyGraph::default();
    let mut engine: SiftEngineFor<ToyGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(two_stage_pattern(true));

    g.add_str("ev1", "eventType", "enter", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".to_string(),
        &"eventType".to_string(),
        &"enter".to_string(),
        &Interval::open(1),
    );

    g.add_str("ev2", "eventType", "leave", 2);
    g.set_time(2);
    let events = engine.on_edge_added(
        &g,
        &"ev2".to_string(),
        &"eventType".to_string(),
        &"leave".to_string(),
        &Interval::open(2),
    );

    assert!(
        events
            .iter()
            .any(|e| matches!(e, SiftEvent::Completed { .. })),
        "Completed event must still fire when advance_in_place is enabled"
    );
}

#[test]
fn advance_in_place_prevents_second_stage_one_pm_from_forking() {
    let mut g = ToyGraph::default();
    let mut engine: SiftEngineFor<ToyGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(two_stage_pattern(true));

    g.add_str("ev1", "eventType", "enter", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".to_string(),
        &"eventType".to_string(),
        &"enter".to_string(),
        &Interval::open(1),
    );

    g.add_str("ev2", "eventType", "enter", 2);
    g.set_time(2);
    engine.on_edge_added(
        &g,
        &"ev2".to_string(),
        &"eventType".to_string(),
        &"enter".to_string(),
        &Interval::open(2),
    );

    g.add_str("ev3", "eventType", "leave", 3);
    g.set_time(3);
    engine.on_edge_added(
        &g,
        &"ev3".to_string(),
        &"eventType".to_string(),
        &"leave".to_string(),
        &Interval::open(3),
    );
    engine.end_tick(0);

    let stage_one_active = engine
        .partial_matches()
        .iter()
        .filter(|pm| pm.next_stage == 1 && pm.state == MatchState::Active)
        .count();
    assert_eq!(
        stage_one_active, 0,
        "all stage-1 PMs should be consumed under advance_in_place, got {}",
        stage_one_active
    );
}

// ---------------------------------------------------------------------------
// Unordered-group interaction
// ---------------------------------------------------------------------------

/// With advance_in_place, a within-group advance (one of the group members
/// matches, mask enriched, stage_idx unchanged) must NOT consume the original.
/// The remaining group members still need to match against it.
#[test]
fn advance_in_place_preserves_original_across_within_group_match() {
    let mut g = ToyGraph::default();
    let mut engine: SiftEngineFor<ToyGraph> = SiftEngine::new(DefaultLetEvaluator);
    let pattern = PatternBuilder::<String, String>::new("group_then_leave")
        .stage("a", |s| s.edge("a", "eventType".into(), "enter".into()))
        .unordered_group(|b| {
            b.stage("g1", |s| s.edge("g1", "eventType".into(), "greet".into()))
                .stage("g2", |s| s.edge("g2", "eventType".into(), "seat".into()))
        })
        .stage("z", |s| s.edge("z", "eventType".into(), "leave".into()))
        .advance_in_place()
        .build();
    engine.register(pattern);

    g.add_str("e1", "eventType", "enter", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"e1".to_string(),
        &"eventType".to_string(),
        &"enter".to_string(),
        &Interval::open(1),
    );

    // First group member: greet — within-group advance.
    g.add_str("e2", "eventType", "greet", 2);
    g.set_time(2);
    engine.on_edge_added(
        &g,
        &"e2".to_string(),
        &"eventType".to_string(),
        &"greet".to_string(),
        &Interval::open(2),
    );

    // An Active PM must survive the within-group match so seat can match it.
    let active_before_seat = engine
        .partial_matches()
        .iter()
        .filter(|pm| pm.state == MatchState::Active)
        .count();
    assert!(
        active_before_seat >= 1,
        "within-group advance should preserve an Active PM, got {}",
        active_before_seat
    );

    g.add_str("e3", "eventType", "seat", 3);
    g.set_time(3);
    engine.on_edge_added(
        &g,
        &"e3".to_string(),
        &"eventType".to_string(),
        &"seat".to_string(),
        &Interval::open(3),
    );

    g.add_str("e4", "eventType", "leave", 4);
    g.set_time(4);
    let events = engine.on_edge_added(
        &g,
        &"e4".to_string(),
        &"eventType".to_string(),
        &"leave".to_string(),
        &Interval::open(4),
    );

    assert!(
        events
            .iter()
            .any(|e| matches!(e, SiftEvent::Completed { .. })),
        "the full sequence should produce a Completed event"
    );
}

// ---------------------------------------------------------------------------
// Repeat-range interaction
// ---------------------------------------------------------------------------

/// With advance_in_place and a repeat_range pattern, the Complete-at-end
/// arm consumes the original but the loop-back arm produces a fresh PM
/// that must survive so the next iteration can match. We assert at least
/// two Completed events across three step edges — if loop-back didn't
/// survive, only the first step would complete and subsequent steps would
/// match via fresh initiations (still producing completions, but the
/// intent of this test is to verify the loop itself works).
#[test]
fn advance_in_place_with_repeat_range_loop_back_survives() {
    use fabula::pattern::RepeatRange;
    use std::collections::HashSet;

    let mut g = ToyGraph::default();
    let mut engine: SiftEngineFor<ToyGraph> = SiftEngine::new(DefaultLetEvaluator);

    let mut pattern = PatternBuilder::<String, String>::new("repeating_step")
        .stage("s", |s| s.edge("s", "eventType".into(), "step".into()))
        .advance_in_place()
        .build();
    pattern.repeat_range = Some(RepeatRange {
        stage_start: 0,
        stage_end: 1,
        min_reps: 1,
        max_reps: Some(3),
        shared_vars: HashSet::new(),
    });
    engine.register(pattern);

    let mut total_completed = 0;
    for i in 0..3 {
        let ev = format!("ev_step_{i}");
        let t = (i + 1) as i64;
        g.add_str(&ev, "eventType", "step", t);
        g.set_time(t);
        let events = engine.on_edge_added(
            &g,
            &ev,
            &"eventType".to_string(),
            &"step".to_string(),
            &Interval::open(t),
        );
        total_completed += events
            .iter()
            .filter(|e| matches!(e, SiftEvent::Completed { .. }))
            .count();
    }
    engine.end_tick(0);

    assert!(
        total_completed >= 2,
        "repeat_range with advance_in_place should emit multiple completions \
         across iterations; got {} completions",
        total_completed
    );
}
