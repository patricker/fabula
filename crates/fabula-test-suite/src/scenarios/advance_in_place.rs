//! Generic scenarios for Pattern.advance_in_place, run against MemGraph,
//! PetGraph, and GrafeoGraph via the golden_tests! macro.

use crate::TestGraph;
use fabula::builder::PatternBuilder;
use fabula::engine::{MatchState, SiftEngine, SiftEngineFor, SiftEvent};
use fabula::interval::Interval;

fn two_stage_pattern<G: TestGraph>(
    advance_in_place: bool,
) -> fabula::pattern::Pattern<String, G::V> {
    let mut b = PatternBuilder::<String, G::V>::new("enter_then_leave")
        .stage("a", |s| s.edge("a", "eventType".into(), G::str_val("enter")))
        .stage("b", |s| s.edge("b", "eventType".into(), G::str_val("leave")));
    if advance_in_place {
        b = b.advance_in_place();
    }
    b.build()
}

/// Without the flag, the stage-1 PM survives after a leave arrives.
pub fn advance_in_place_default_preserves_original<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(two_stage_pattern::<G>(false));

    g.add_str_edge("ev1", "eventType", "enter", 1);
    g.set_current_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &G::str_val("enter"),
        &Interval::open(1),
    );

    g.add_str_edge("ev2", "eventType", "leave", 2);
    g.set_current_time(2);
    engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &G::str_val("leave"),
        &Interval::open(2),
    );
    engine.end_tick(0);

    let stage_one_active = engine
        .partial_matches()
        .iter()
        .filter(|pm| pm.next_stage == 1 && pm.state == MatchState::Active)
        .count();
    assert!(
        stage_one_active >= 1,
        "default behavior keeps the original stage-1 PM alive, got {}",
        stage_one_active
    );
}

/// With the flag, the stage-1 PM is consumed after strict-forward advancement.
pub fn advance_in_place_consumes_original<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(two_stage_pattern::<G>(true));

    g.add_str_edge("ev1", "eventType", "enter", 1);
    g.set_current_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &G::str_val("enter"),
        &Interval::open(1),
    );

    g.add_str_edge("ev2", "eventType", "leave", 2);
    g.set_current_time(2);
    engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &G::str_val("leave"),
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
        "advance_in_place consumes the stage-1 PM, got {}",
        stage_one_active
    );
}

/// Even with the flag, Completed events must still fire.
pub fn advance_in_place_still_emits_completed<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(two_stage_pattern::<G>(true));

    g.add_str_edge("ev1", "eventType", "enter", 1);
    g.set_current_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &G::str_val("enter"),
        &Interval::open(1),
    );

    g.add_str_edge("ev2", "eventType", "leave", 2);
    g.set_current_time(2);
    let events = engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &G::str_val("leave"),
        &Interval::open(2),
    );

    assert!(
        events
            .iter()
            .any(|e| matches!(e, SiftEvent::Completed { .. })),
        "advance_in_place still emits Completed"
    );
}

/// Crowd: two enters then one leave. With flag, no stage-1 residue.
pub fn advance_in_place_crowded_no_residue<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(two_stage_pattern::<G>(true));

    for (ev, t) in [("ev1", 1_i64), ("ev2", 2)].iter() {
        g.add_str_edge(ev, "eventType", "enter", *t);
        g.set_current_time(*t);
        engine.on_edge_added(
            &g,
            &(*ev).into(),
            &"eventType".into(),
            &G::str_val("enter"),
            &Interval::open(*t),
        );
    }

    g.add_str_edge("ev3", "eventType", "leave", 3);
    g.set_current_time(3);
    engine.on_edge_added(
        &g,
        &"ev3".into(),
        &"eventType".into(),
        &G::str_val("leave"),
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
        "advance_in_place crowded: no stage-1 residue, got {}",
        stage_one_active
    );
}
