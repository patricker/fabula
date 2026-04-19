//! Value disjunction (OneOf) scenarios -- matching edges against a set of values.

use crate::TestGraph;
use fabula::prelude::*;

/// Batch: OneOf matches any listed value -- "attack" and "betray" match, "trade" does not.
pub fn batch_one_of_matches_any<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "attack", 1);
    g.add_str_edge("ev2", "eventType", "trade", 2);
    g.add_str_edge("ev3", "eventType", "betray", 3);
    g.set_current_time(10);

    let pattern = PatternBuilder::new("harm_event")
        .stage("e", |s| {
            s.edge_one_of(
                "e",
                "eventType".into(),
                vec![
                    G::str_val("attack"),
                    G::str_val("betray"),
                    G::str_val("steal"),
                ],
            )
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(
        matches.len(),
        2,
        "attack and betray should match OneOf, trade should not"
    );
}

/// Batch: OneOf rejects all when no listed values are present.
pub fn batch_one_of_rejects_unlisted<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "trade", 1);
    g.add_str_edge("ev2", "eventType", "observe", 2);
    g.set_current_time(10);

    let pattern = PatternBuilder::new("harm_event")
        .stage("e", |s| {
            s.edge_one_of(
                "e",
                "eventType".into(),
                vec![
                    G::str_val("attack"),
                    G::str_val("betray"),
                    G::str_val("steal"),
                ],
            )
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(
        matches.len(),
        0,
        "trade and observe are not in OneOf list, should get 0 matches"
    );
}

/// Batch: OneOf with variable join across two stages -- only Alice matches
/// because she both attacks and apologizes.
pub fn batch_one_of_with_variable_join<G: TestGraph>() {
    let mut g = G::new_graph();

    // Alice attacks at t=1
    g.add_str_edge("ev1", "eventType", "attack", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);

    // Alice apologizes at t=2
    g.add_str_edge("ev2", "eventType", "apologize", 2);
    g.add_ref_edge("ev2", "actor", "alice", 2);

    // Bob betrays at t=3
    g.add_str_edge("ev3", "eventType", "betray", 3);
    g.add_ref_edge("ev3", "actor", "bob", 3);

    // Bob does NOT apologize
    g.set_current_time(10);

    let pattern = PatternBuilder::new("harm_then_sorry")
        .stage("e1", |s| {
            s.edge_one_of(
                "e1",
                "eventType".into(),
                vec![
                    G::str_val("attack"),
                    G::str_val("betray"),
                    G::str_val("steal"),
                ],
            )
            .edge_bind("e1", "actor".into(), "person")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("apologize"))
                .edge_bind("e2", "actor".into(), "person")
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(
        matches.len(),
        1,
        "only Alice has a harm event followed by an apology"
    );
}

/// Incremental: OneOf advances and completes via on_edge_added.
pub fn incremental_one_of_advances<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngineFor<G> = SiftEngine::new();

    let pattern = PatternBuilder::new("harm_event")
        .stage("e", |s| {
            s.edge_one_of(
                "e",
                "eventType".into(),
                vec![
                    G::str_val("attack"),
                    G::str_val("betray"),
                    G::str_val("steal"),
                ],
            )
        })
        .build();
    engine.register(pattern);

    // Feed "trade" -- should NOT produce any events
    g.add_str_edge("ev1", "eventType", "trade", 1);
    g.set_current_time(1);
    let ev = engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &G::str_val("trade"),
        &Interval::open(1),
    );
    assert!(
        ev.is_empty(),
        "trade is not in OneOf list, no events expected"
    );

    // Feed "attack" -- single-stage pattern should complete immediately
    g.add_str_edge("ev2", "eventType", "attack", 2);
    g.set_current_time(2);
    let ev = engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &G::str_val("attack"),
        &Interval::open(2),
    );
    assert!(
        ev.iter()
            .any(|e| matches!(e, SiftEvent::Completed { pattern, .. } if pattern == "harm_event")),
        "attack is in OneOf list, single-stage pattern should complete"
    );
}
