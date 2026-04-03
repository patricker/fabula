//! Allen temporal relation scenarios — explicit temporal constraints on bounded intervals.

use crate::TestGraph;
use fabula::prelude::*;

/// Batch: explicit Before constraint using open-ended intervals.
/// For open-ended intervals, the engine falls back to checking start < start
/// when the relation is Before or Meets.
pub fn batch_explicit_before_constraint<G: TestGraph>() {
    // Case 1: e1 starts at 1, e2 starts at 5 -> Before fallback holds -> match
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "start", 1);
    g.add_str_edge("ev2", "eventType", "finish", 5);
    g.set_current_time(10);

    let pattern = PatternBuilder::new("before_pattern")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("start"))
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("finish"))
        })
        .temporal("e1", AllenRelation::Before, "e2")
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        1,
        "e1 at t=1 before e2 at t=5 should match"
    );

    // Case 2: reverse order -> no match
    let mut g2 = G::new_graph();
    g2.add_str_edge("ev1", "eventType", "start", 5);
    g2.add_str_edge("ev2", "eventType", "finish", 1);
    g2.set_current_time(10);

    let pattern2 = PatternBuilder::new("before_pattern")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("start"))
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("finish"))
        })
        .temporal("e1", AllenRelation::Before, "e2")
        .build();

    let mut engine2: SiftEngineFor<G> = SiftEngine::new();
    engine2.register(pattern2);
    assert_eq!(
        engine2.evaluate(&g2).len(),
        0,
        "e1 at t=5, e2 at t=1 -> temporal order violated -> no match"
    );
}

/// Batch: explicit During constraint with bounded intervals that overlap at query time.
/// The outer event starts first (satisfying implicit stage ordering),
/// and the inner event is contained within it (During relation).
/// outer=[1,100), inner=[3,5). Stage order: outer first, inner second.
/// Then we assert inner During outer via explicit temporal constraint.
pub fn batch_explicit_during_constraint<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge_bounded("ev_outer", "eventType", "outer", 1, 100);
    g.add_str_edge_bounded("ev_inner", "eventType", "inner", 3, 5);
    g.set_current_time(4); // Both intervals are active at t=4

    let pattern = PatternBuilder::new("during_pattern")
        .stage("e_outer", |s| {
            s.edge("e_outer", "eventType".into(), G::str_val("outer"))
        })
        .stage("e_inner", |s| {
            s.edge("e_inner", "eventType".into(), G::str_val("inner"))
        })
        .temporal("e_inner", AllenRelation::During, "e_outer")
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        1,
        "inner=[3,5) during outer=[1,100) should match"
    );
}

/// Batch: explicit Overlaps constraint with bounded intervals active at query time.
/// e1=[1,6) overlaps e2=[3,100). Query at t=4 where both are active.
pub fn batch_explicit_overlaps_constraint<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge_bounded("ev1", "eventType", "early", 1, 6);
    g.add_str_edge_bounded("ev2", "eventType", "late", 3, 100);
    g.set_current_time(4); // Both intervals are active at t=4

    let pattern = PatternBuilder::new("overlaps_pattern")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("early"))
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("late"))
        })
        .temporal("e1", AllenRelation::Overlaps, "e2")
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        1,
        "e1=[1,6) overlaps e2=[3,100) should match"
    );
}
