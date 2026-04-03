//! Golden tests for pattern composition operators.

use crate::TestGraph;
use fabula::compose;
use fabula::prelude::*;

// ---------------------------------------------------------------------------
// Sequence: setup then payoff, shared character binding
// ---------------------------------------------------------------------------

/// A character promises, then fulfills. Shared "char" binding ensures same person.
pub fn batch_sequence_shared_binding<G: TestGraph>() {
    let setup = PatternBuilder::<String, G::V>::new("setup")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("promise"))
                .edge_bind("e1", "actor".into(), "char")
        })
        .build();

    let payoff = PatternBuilder::<String, G::V>::new("payoff")
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("fulfill"))
                .edge_bind("e2", "actor".into(), "char")
        })
        .build();

    let arc = compose::sequence("promise_kept", &setup, &payoff, &["char"]);

    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "promise", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev2", "eventType", "fulfill", 5);
    g.add_ref_edge("ev2", "actor", "alice", 5);
    g.set_current_time(10);

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(arc);

    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].pattern, "promise_kept");
    assert!(G::is_node_eq(&matches[0].bindings["char"], "alice"));
}

/// Sequence doesn't match when different characters do setup vs payoff.
pub fn batch_sequence_different_actors_no_match<G: TestGraph>() {
    let setup = PatternBuilder::<String, G::V>::new("setup")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("promise"))
                .edge_bind("e1", "actor".into(), "char")
        })
        .build();

    let payoff = PatternBuilder::<String, G::V>::new("payoff")
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("fulfill"))
                .edge_bind("e2", "actor".into(), "char")
        })
        .build();

    let arc = compose::sequence("promise_kept", &setup, &payoff, &["char"]);

    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "promise", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev2", "eventType", "fulfill", 5);
    g.add_ref_edge("ev2", "actor", "bob", 5); // different actor
    g.set_current_time(10);

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(arc);

    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 0);
}

// ---------------------------------------------------------------------------
// Sequence with negation: setup's negation carries into composed pattern
// ---------------------------------------------------------------------------

/// Promise kept — unless cancelled in between.
pub fn batch_sequence_with_negation<G: TestGraph>() {
    let setup = PatternBuilder::<String, G::V>::new("setup")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("promise"))
                .edge_bind("e1", "actor".into(), "char")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("fulfill"))
                .edge_bind("e2", "actor".into(), "char")
        })
        .unless_between("e1", "e2", |neg| {
            neg.edge("cancel_ev", "eventType".into(), G::str_val("cancel"))
                .edge_bind("cancel_ev", "actor".into(), "char")
        })
        .build();

    // Compose with a second pattern (reaction)
    let reaction = PatternBuilder::<String, G::V>::new("reaction")
        .stage("e3", |s| {
            s.edge("e3", "eventType".into(), G::str_val("celebrate"))
                .edge_bind("e3", "actor".into(), "char")
        })
        .build();

    let arc = compose::sequence("promise_then_celebrate", &setup, &reaction, &["char"]);

    // Graph WITH cancel — should not match
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "promise", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev_cancel", "eventType", "cancel", 3);
    g.add_ref_edge("ev_cancel", "actor", "alice", 3);
    g.add_str_edge("ev2", "eventType", "fulfill", 5);
    g.add_ref_edge("ev2", "actor", "alice", 5);
    g.add_str_edge("ev3", "eventType", "celebrate", 7);
    g.add_ref_edge("ev3", "actor", "alice", 7);
    g.set_current_time(10);

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(arc);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 0, "cancel should block the match");
}

// ---------------------------------------------------------------------------
// Choice: exclusive — first to complete kills others
// ---------------------------------------------------------------------------

/// Three crisis types, exclusive. Only the first to complete survives.
pub fn incremental_choice_exclusive<G: TestGraph>() {
    let war = PatternBuilder::<String, G::V>::new("war")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("war"))
        })
        .build();

    let famine = PatternBuilder::<String, G::V>::new("famine")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("famine"))
        })
        .build();

    let plague = PatternBuilder::<String, G::V>::new("plague")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("plague"))
        })
        .build();

    let crises = compose::choice("crisis", &[&war, &famine, &plague], true);

    let mut g = G::new_graph();
    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    for c in crises {
        engine.register(c);
    }

    // Famine hits first
    g.add_str_edge("ev1", "eventType", "famine", 1);
    g.set_current_time(1);
    let events = engine.on_edge_added(
        &g,
        &"ev1".to_string(),
        &"eventType".to_string(),
        &G::str_val("famine"),
        &Interval::open(1),
    );

    let completed: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, SiftEvent::Completed { .. }))
        .collect();
    assert_eq!(completed.len(), 1);

    // Now war hits — but it's still a single-stage pattern, so it completes immediately
    // However, the group kill from famine's completion already happened in the previous call.
    // War's completion is independent (single-stage patterns complete on initiation).
    // The exclusive group kills PMs, not completed matches — single-stage patterns
    // don't have active PMs to kill.
    // For multi-stage exclusive choice, see the next test.
}

/// Multi-stage exclusive choice: first to complete kills the others' PMs.
pub fn incremental_choice_exclusive_multistage<G: TestGraph>() {
    let path_a = PatternBuilder::<String, G::V>::new("path_a")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("start_a"))
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("end_a"))
        })
        .build();

    let path_b = PatternBuilder::<String, G::V>::new("path_b")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("start_b"))
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("end_b"))
        })
        .build();

    let paths = compose::choice("path", &[&path_a, &path_b], true);

    let mut g = G::new_graph();
    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    for p in paths {
        engine.register(p);
    }

    // Both paths start
    g.add_str_edge("ev1", "eventType", "start_a", 1);
    g.add_str_edge("ev2", "eventType", "start_b", 2);
    g.set_current_time(2);
    engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(), &G::str_val("start_a"), &Interval::open(1));
    engine.on_edge_added(&g, &"ev2".into(), &"eventType".into(), &G::str_val("start_b"), &Interval::open(2));

    // Both should have active PMs
    let active = engine.partial_matches().iter().filter(|pm| pm.state == MatchState::Active).count();
    assert_eq!(active, 2, "both paths should have active PMs");

    // Path A completes
    g.add_str_edge("ev3", "eventType", "end_a", 5);
    g.set_current_time(5);
    let events = engine.on_edge_added(&g, &"ev3".into(), &"eventType".into(), &G::str_val("end_a"), &Interval::open(5));

    let completed: Vec<_> = events.iter().filter(|e| matches!(e, SiftEvent::Completed { .. })).collect();
    assert_eq!(completed.len(), 1);

    // Path B's PM should be killed by the exclusive group
    let active_after = engine.partial_matches().iter().filter(|pm| pm.state == MatchState::Active).count();
    assert_eq!(active_after, 0, "path_b's PM should be killed by exclusive group");
}

// ---------------------------------------------------------------------------
// Repeat: same offender three times
// ---------------------------------------------------------------------------

/// Three offenses by the same character.
pub fn batch_repeat_shared_binding<G: TestGraph>() {
    let offense = PatternBuilder::<String, G::V>::new("offense")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("offense"))
                .edge_bind("e1", "actor".into(), "offender")
        })
        .build();

    let escalation = compose::repeat("three_strikes", &offense, 3, &["offender"]);
    assert_eq!(escalation.stages.len(), 3);

    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "offense", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev2", "eventType", "offense", 5);
    g.add_ref_edge("ev2", "actor", "alice", 5);
    g.add_str_edge("ev3", "eventType", "offense", 10);
    g.add_ref_edge("ev3", "actor", "alice", 10);
    g.set_current_time(15);

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(escalation);

    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].pattern, "three_strikes");
    assert!(G::is_node_eq(&matches[0].bindings["offender"], "alice"));
}

/// Repeat doesn't match when different actors commit the offenses.
pub fn batch_repeat_different_actors_no_match<G: TestGraph>() {
    let offense = PatternBuilder::<String, G::V>::new("offense")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("offense"))
                .edge_bind("e1", "actor".into(), "offender")
        })
        .build();

    let escalation = compose::repeat("three_strikes", &offense, 3, &["offender"]);

    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "offense", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev2", "eventType", "offense", 5);
    g.add_ref_edge("ev2", "actor", "bob", 5); // different actor
    g.add_str_edge("ev3", "eventType", "offense", 10);
    g.add_ref_edge("ev3", "actor", "alice", 10);
    g.set_current_time(15);

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(escalation);

    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 0);
}
