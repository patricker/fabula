use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

#[test]
fn metric_before_gap_in_range() {
    // Two stages matched via builder API with bounded intervals.
    // e1=[1,4), e2=[8,12). Before gap = 8-4 = 4. Bound [3,10] → match.
    use fabula::pattern::MetricGap;
    let mut g = MemGraph::new();
    g.add_edge_bounded("ev1", "eventType", MemValue::Str("crisis".into()), 1, 4);
    g.add_edge_bounded("ev2", "eventType", MemValue::Str("betrayal".into()), 8, 12);
    g.set_time(3); // scan at t=3 to find ev1

    let pattern = PatternBuilder::new("test")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("crisis".into()))
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), MemValue::Str("betrayal".into()))
        })
        .temporal_with_gap(
            "e1",
            AllenRelation::Before,
            "e2",
            MetricGap {
                min: Some(3.0),
                max: Some(10.0),
            },
        )
        .build();

    // Use incremental: add edges at their respective times
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);

    g.set_time(3);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("crisis".into()),
        &Interval::new(1, 4),
    );
    g.set_time(10);
    let events = engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &MemValue::Str("betrayal".into()),
        &Interval::new(8, 12),
    );

    let completed = events
        .iter()
        .filter(|e| matches!(e, SiftEvent::Completed { .. }))
        .count();
    assert_eq!(completed, 1, "gap=4 within [3,10] → match");
}

#[test]
fn metric_before_gap_too_far() {
    use fabula::pattern::MetricGap;
    let mut g = MemGraph::new();
    g.add_edge_bounded("ev1", "eventType", MemValue::Str("crisis".into()), 1, 4);
    g.add_edge_bounded("ev2", "eventType", MemValue::Str("betrayal".into()), 20, 25);

    let pattern = PatternBuilder::new("test")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("crisis".into()))
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), MemValue::Str("betrayal".into()))
        })
        .temporal_with_gap(
            "e1",
            AllenRelation::Before,
            "e2",
            MetricGap {
                min: Some(3.0),
                max: Some(10.0),
            },
        )
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    g.set_time(3);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("crisis".into()),
        &Interval::new(1, 4),
    );
    g.set_time(22);
    let events = engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &MemValue::Str("betrayal".into()),
        &Interval::new(20, 25),
    );

    let completed = events
        .iter()
        .filter(|e| matches!(e, SiftEvent::Completed { .. }))
        .count();
    assert_eq!(completed, 0, "gap=16 exceeds max=10 → no match");
}

#[test]
fn metric_before_gap_too_close() {
    use fabula::pattern::MetricGap;
    let mut g = MemGraph::new();
    g.add_edge_bounded("ev1", "eventType", MemValue::Str("crisis".into()), 1, 4);
    g.add_edge_bounded("ev2", "eventType", MemValue::Str("betrayal".into()), 5, 8);

    let pattern = PatternBuilder::new("test")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("crisis".into()))
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), MemValue::Str("betrayal".into()))
        })
        .temporal_with_gap(
            "e1",
            AllenRelation::Before,
            "e2",
            MetricGap {
                min: Some(3.0),
                max: Some(10.0),
            },
        )
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    g.set_time(3);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &MemValue::Str("crisis".into()),
        &Interval::new(1, 4),
    );
    g.set_time(6);
    let events = engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &MemValue::Str("betrayal".into()),
        &Interval::new(5, 8),
    );

    let completed = events
        .iter()
        .filter(|e| matches!(e, SiftEvent::Completed { .. }))
        .count();
    assert_eq!(completed, 0, "gap=1 below min=3 → no match");
}

#[test]
fn metric_open_ended_skips_gap_check() {
    // Open-ended intervals: Allen relation falls back to start comparison.
    // gap_for_relation returns None → metric check skipped → match allowed.
    use fabula::pattern::MetricGap;
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "crisis", 1);
    g.add_str("ev2", "eventType", "betrayal", 100);
    g.set_time(100);

    let pattern = PatternBuilder::new("test")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("crisis".into()))
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), MemValue::Str("betrayal".into()))
        })
        .temporal_with_gap(
            "e1",
            AllenRelation::Before,
            "e2",
            MetricGap {
                min: Some(3.0),
                max: Some(10.0),
            },
        )
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    // Open-ended intervals: gap can't be computed (no end point).
    // Metric check skipped. Allen Before fallback (start comparison) passes.
    assert_eq!(
        engine.evaluate(&g).len(),
        1,
        "open-ended intervals: metric check skipped, Allen fallback passes"
    );
}

// ===========================================================================
// 6. Type edge cases
// ===========================================================================

#[test]
fn memvalue_cross_variant_ordering() {
    // Derived PartialOrd orders by discriminant: Node < Str < Num < Bool
    assert!(MemValue::Node("z".into()) < MemValue::Str("a".into()));
    assert!(MemValue::Str("z".into()) < MemValue::Num(0.0));
    assert!(MemValue::Num(f64::MAX) < MemValue::Bool(false));
}

#[test]
fn memvalue_nan_comparisons() {
    let nan = MemValue::Num(f64::NAN);

    let lt = ValueConstraint::Lt(MemValue::Num(5.0));
    assert!(!lt.matches(&nan), "NaN is not less than 5.0");

    let gt = ValueConstraint::Gt(MemValue::Num(5.0));
    assert!(!gt.matches(&nan), "NaN is not greater than 5.0");

    let eq = ValueConstraint::Eq(MemValue::Num(f64::NAN));
    assert!(!eq.matches(&nan), "NaN != NaN");

    let between = ValueConstraint::Between(MemValue::Num(0.0), MemValue::Num(10.0));
    assert!(!between.matches(&nan), "NaN is not between 0 and 10");
}

#[test]
fn between_reversed_bounds_never_matches() {
    let c = ValueConstraint::Between(20, 10); // lo > hi
    assert!(!c.matches(&15), "nothing can be >= 20 AND <= 10");
    assert!(!c.matches(&10));
    assert!(!c.matches(&20));
    assert!(!c.matches(&0));
    assert!(!c.matches(&100));
}

#[test]
fn between_equal_bounds_matches_only_exact() {
    let c = ValueConstraint::Between(5, 5);
    assert!(c.matches(&5), "value == lo == hi should match");
    assert!(!c.matches(&4));
    assert!(!c.matches(&6));
}

#[test]
fn cross_variant_constraint_between() {
    // Between(Num(0.0), Num(10.0)) tested against Str("5")
    // Str is a different variant, so Num(0.0) < Str("5") due to discriminant ordering
    // and Str("5") > Num(10.0) for the same reason. So >= lo is true, <= hi is false.
    let c = ValueConstraint::Between(MemValue::Num(0.0), MemValue::Num(10.0));
    assert!(
        !c.matches(&MemValue::Str("5".into())),
        "cross-variant Between comparison: Str is not between two Nums"
    );
}

// ===========================================================================
// 7. Negation edge cases
// ===========================================================================

#[test]
fn unless_global_single_stage_works() {
    // B7 fix: unless_global on single-stage now uses open-ended window.
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "crime", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    // This pardon should block the crime pattern
    g.add_str("ev2", "eventType", "pardon", 2);
    g.add_ref("ev2", "actor", "alice", 2);
    g.set_time(10);

    let pattern = PatternBuilder::new("unpardoned_crime")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("crime".into()))
                .edge_bind("e1", "actor".into(), "criminal")
        })
        .unless_global(|neg| {
            neg.edge("p", "eventType".into(), MemValue::Str("pardon".into()))
                .edge_bind("p", "actor".into(), "criminal")
        })
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    // B7 fixed: unless_global on single-stage uses open-ended window → pardon blocks
    assert_eq!(
        matches.len(),
        0,
        "pardon should block the crime via unless_global"
    );
}

#[test]
fn negation_before_window_start_does_not_block() {
    let mut g = MemGraph::new();
    g.add_str("ev0", "eventType", "pardon", 0); // BEFORE the crime
    g.add_ref("ev0", "actor", "alice", 0);
    g.add_str("ev1", "eventType", "crime", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.add_str("ev2", "eventType", "crime", 3);
    g.add_ref("ev2", "actor", "alice", 3);
    g.set_time(10);

    let pattern = PatternBuilder::new("double_crime")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("crime".into()))
                .edge_bind("e1", "actor".into(), "criminal")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), MemValue::Str("crime".into()))
                .edge_bind("e2", "actor".into(), "criminal")
        })
        .unless_between("e1", "e2", |neg| {
            neg.edge("p", "eventType".into(), MemValue::Str("pardon".into()))
                .edge_bind("p", "actor".into(), "criminal")
        })
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    // Pardon at t=0 is before e1 at t=1, so it's outside the window
    assert_eq!(
        engine.evaluate(&g).len(),
        1,
        "negation event before window start should not block"
    );
}

#[test]
fn negation_at_exact_window_boundary() {
    // B4 fix: negation window is exclusive on start (matching Winnow's < semantics).
    // An event at the exact same timestamp as the window start is NOT in the window.
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "crime", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.add_str("ev_pardon", "eventType", "pardon", 1); // same time as crime (window start)
    g.add_ref("ev_pardon", "actor", "alice", 1);
    g.add_str("ev2", "eventType", "crime", 3);
    g.add_ref("ev2", "actor", "alice", 3);
    g.set_time(10);

    let pattern = PatternBuilder::new("double_crime")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("crime".into()))
                .edge_bind("e1", "actor".into(), "criminal")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), MemValue::Str("crime".into()))
                .edge_bind("e2", "actor".into(), "criminal")
        })
        .unless_between("e1", "e2", |neg| {
            neg.edge("p", "eventType".into(), MemValue::Str("pardon".into()))
                .edge_bind("p", "actor".into(), "criminal")
        })
        .build();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    // Pardon at t=1, window start is t=1. With exclusive start (>), 1 > 1 is false.
    // Pardon is NOT in the window → match succeeds.
    assert_eq!(
        matches.len(),
        1,
        "event at exact window start is outside exclusive window -- match succeeds"
    );

    // But a pardon at t=2 (strictly between 1 and 3) DOES block:
    g.add_str("ev_pardon2", "eventType", "pardon", 2);
    g.add_ref("ev_pardon2", "actor", "alice", 2);
    let matches2 = engine.evaluate(&g);
    assert_eq!(
        matches2.len(),
        0,
        "pardon at t=2 is strictly between 1 and 3 -- blocks match"
    );
}
