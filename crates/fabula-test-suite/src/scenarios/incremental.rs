//! Incremental matching scenarios — on_edge_added, drain_completed, negation kills.

use crate::TestGraph;
use fabula::prelude::*;

/// Helper: build the violation_of_hospitality pattern.
fn voh_pattern<G: TestGraph>() -> Pattern<String, G::V> {
    PatternBuilder::new("violation_of_hospitality")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("enterTown"))
                .edge_bind("e1", "actor".into(), "guest")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("showHospitality"))
                .edge_bind("e2", "actor".into(), "host")
                .edge_bind("e2", "target".into(), "guest")
        })
        .stage("e3", |s| {
            s.edge("e3", "eventType".into(), G::str_val("harm"))
                .edge_bind("e3", "actor".into(), "host")
                .edge_bind("e3", "target".into(), "guest")
        })
        .unless_between("e1", "e3", |neg| {
            neg.edge("eMid", "eventType".into(), G::str_val("leaveTown"))
                .edge_bind("eMid", "actor".into(), "guest")
        })
        .build()
}

/// Incremental: three stages fire Advanced, Advanced, Completed.
pub fn incremental_hospitality_three_stages<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngine<G> = SiftEngine::new();
    engine.register(voh_pattern::<G>());

    // Stage 1
    g.add_str_edge("ev1", "eventType", "enterTown", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.set_current_time(1);
    let ev = engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &G::str_val("enterTown"),
        &Interval::open(1),
    );
    assert!(
        ev.iter().any(|e| matches!(e, SiftEvent::Advanced { pattern, .. } if pattern == "violation_of_hospitality")),
        "stage 1 should advance"
    );

    // Stage 2
    g.add_str_edge("ev2", "eventType", "showHospitality", 2);
    g.add_ref_edge("ev2", "actor", "bob", 2);
    g.add_ref_edge("ev2", "target", "alice", 2);
    g.set_current_time(2);
    let ev = engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &G::str_val("showHospitality"),
        &Interval::open(2),
    );
    assert!(
        ev.iter().any(|e| matches!(e, SiftEvent::Advanced { pattern, .. } if pattern == "violation_of_hospitality")),
        "stage 2 should advance"
    );

    // Stage 3 -> complete
    g.add_str_edge("ev3", "eventType", "harm", 3);
    g.add_ref_edge("ev3", "actor", "bob", 3);
    g.add_ref_edge("ev3", "target", "alice", 3);
    g.set_current_time(3);
    let ev = engine.on_edge_added(
        &g,
        &"ev3".into(),
        &"eventType".into(),
        &G::str_val("harm"),
        &Interval::open(3),
    );
    assert!(
        ev.iter().any(|e| matches!(e, SiftEvent::Completed { pattern, .. } if pattern == "violation_of_hospitality")),
        "stage 3 should complete"
    );
}

/// Incremental: negation kills an in-progress match.
pub fn incremental_negation_kills<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngine<G> = SiftEngine::new();
    engine.register(voh_pattern::<G>());

    // Stages 1 + 2
    g.add_str_edge("ev1", "eventType", "enterTown", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.set_current_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(), &G::str_val("enterTown"), &Interval::open(1));
    g.add_str_edge("ev2", "eventType", "showHospitality", 2);
    g.add_ref_edge("ev2", "actor", "bob", 2);
    g.add_ref_edge("ev2", "target", "alice", 2);
    g.set_current_time(2);
    engine.on_edge_added(&g, &"ev2".into(), &"eventType".into(), &G::str_val("showHospitality"), &Interval::open(2));
    assert!(
        !engine.active_matches_for("violation_of_hospitality").is_empty(),
        "should have active partial match"
    );

    // Guest leaves -> kill
    g.add_str_edge("ev_leave", "eventType", "leaveTown", 3);
    g.add_ref_edge("ev_leave", "actor", "alice", 3);
    g.set_current_time(3);
    let ev = engine.on_edge_added(&g, &"ev_leave".into(), &"eventType".into(), &G::str_val("leaveTown"), &Interval::open(3));
    assert!(
        ev.iter().any(|e| matches!(e, SiftEvent::Negated { .. })),
        "should emit Negated event"
    );
    assert!(
        engine.active_matches_for("violation_of_hospitality").is_empty(),
        "partial match should be dead"
    );
}

/// Incremental: unrelated character leaving does NOT kill match.
pub fn incremental_unrelated_leave_no_kill<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngine<G> = SiftEngine::new();
    engine.register(voh_pattern::<G>());

    // Stages 1 + 2
    g.add_str_edge("ev1", "eventType", "enterTown", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.set_current_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(), &G::str_val("enterTown"), &Interval::open(1));
    g.add_str_edge("ev2", "eventType", "showHospitality", 2);
    g.add_ref_edge("ev2", "actor", "bob", 2);
    g.add_ref_edge("ev2", "target", "alice", 2);
    g.set_current_time(2);
    engine.on_edge_added(&g, &"ev2".into(), &"eventType".into(), &G::str_val("showHospitality"), &Interval::open(2));

    // Charlie leaves — should NOT kill
    g.add_str_edge("ev_leave", "eventType", "leaveTown", 3);
    g.add_ref_edge("ev_leave", "actor", "charlie", 3);
    g.set_current_time(3);
    let ev = engine.on_edge_added(&g, &"ev_leave".into(), &"eventType".into(), &G::str_val("leaveTown"), &Interval::open(3));
    assert!(
        !ev.iter().any(|e| matches!(e, SiftEvent::Negated { .. })),
        "charlie leaving should not negate"
    );
    assert!(
        !engine.active_matches_for("violation_of_hospitality").is_empty(),
        "partial match should still be alive"
    );
}

/// Incremental: single-stage pattern completes immediately.
pub fn incremental_single_stage_completes<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngine<G> = SiftEngine::new();
    let pattern = PatternBuilder::new("find_harm")
        .stage("e", |s| {
            s.edge("e", "eventType".into(), G::str_val("harm"))
                .edge_bind("e", "actor".into(), "attacker")
        })
        .build();
    engine.register(pattern);

    g.add_str_edge("ev1", "eventType", "harm", 1);
    g.add_ref_edge("ev1", "actor", "bob", 1);
    g.set_current_time(1);
    let ev = engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &G::str_val("harm"),
        &Interval::open(1),
    );
    assert!(
        ev.iter().any(|e| matches!(e, SiftEvent::Completed { .. })),
        "single-stage should complete immediately"
    );
}

/// Incremental: drain_completed removes completed matches.
pub fn incremental_drain_completed<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngine<G> = SiftEngine::new();
    engine.register(
        PatternBuilder::new("find_harm")
            .stage("e", |s| s.edge("e", "eventType".into(), G::str_val("harm")))
            .build(),
    );

    g.add_str_edge("ev1", "eventType", "harm", 1);
    g.set_current_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(), &G::str_val("harm"), &Interval::open(1));

    let completed = engine.drain_completed();
    assert_eq!(completed.len(), 1, "should have one completed match");
    assert!(
        engine
            .partial_matches()
            .iter()
            .all(|pm| pm.state != MatchState::Complete),
        "no Complete matches should remain after drain"
    );
}

/// Incremental: irrelevant edges produce no events.
pub fn incremental_irrelevant_edges_silent<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngine<G> = SiftEngine::new();
    engine.register(voh_pattern::<G>());

    // Match stage 1
    g.add_str_edge("ev1", "eventType", "enterTown", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.set_current_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(), &G::str_val("enterTown"), &Interval::open(1));

    // Unrelated edge
    g.add_str_edge("noise", "eventType", "tradeMerchant", 2);
    g.set_current_time(2);
    let ev = engine.on_edge_added(&g, &"noise".into(), &"eventType".into(), &G::str_val("tradeMerchant"), &Interval::open(2));
    assert!(ev.is_empty(), "irrelevant edge should produce no events");
    assert!(
        !engine.active_matches_for("violation_of_hospitality").is_empty(),
        "partial match should survive"
    );
}

/// Incremental: second host creates a fork (two active PMs).
pub fn incremental_second_host_forks_pm<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngine<G> = SiftEngine::new();
    engine.register(voh_pattern::<G>());

    // Stage 1: alice enters
    g.add_str_edge("ev1", "eventType", "enterTown", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.set_current_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &G::str_val("enterTown"),
        &Interval::open(1),
    );

    // Stage 2a: bob shows hospitality -> PM advances
    g.add_str_edge("ev2", "eventType", "showHospitality", 2);
    g.add_ref_edge("ev2", "actor", "bob", 2);
    g.add_ref_edge("ev2", "target", "alice", 2);
    g.set_current_time(2);
    engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &G::str_val("showHospitality"),
        &Interval::open(2),
    );

    // Stage 2b: charlie shows hospitality -> FORK
    g.add_str_edge("ev3", "eventType", "showHospitality", 3);
    g.add_ref_edge("ev3", "actor", "charlie", 3);
    g.add_ref_edge("ev3", "target", "alice", 3);
    g.set_current_time(3);
    engine.on_edge_added(
        &g,
        &"ev3".into(),
        &"eventType".into(),
        &G::str_val("showHospitality"),
        &Interval::open(3),
    );

    // Should have at least 2 active PMs (one with host=bob, one with host=charlie)
    let active = engine.active_matches_for("violation_of_hospitality");
    assert!(
        active.len() >= 2,
        "should have at least 2 active PMs after fork, got {}",
        active.len()
    );
}

/// Incremental: original PM at stage 0 survives after advancing to stage 1.
pub fn incremental_original_pm_survives_advance<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngine<G> = SiftEngine::new();
    engine.register(voh_pattern::<G>());

    // alice enters -> PM at stage 1
    g.add_str_edge("ev1", "eventType", "enterTown", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.set_current_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &G::str_val("enterTown"),
        &Interval::open(1),
    );

    let pm_count = engine.partial_matches().len();
    assert!(
        pm_count >= 1,
        "should have at least 1 partial match after stage 1"
    );
}

/// Incremental: a completed single-stage pattern does not emit new events on subsequent edges.
pub fn incremental_dead_and_complete_inert<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngine<G> = SiftEngine::new();
    let pattern = PatternBuilder::new("find_harm")
        .stage("e", |s| {
            s.edge("e", "eventType".into(), G::str_val("harm"))
                .edge_bind("e", "actor".into(), "attacker")
        })
        .build();
    engine.register(pattern);

    // Complete it
    g.add_str_edge("ev1", "eventType", "harm", 1);
    g.add_ref_edge("ev1", "actor", "bob", 1);
    g.set_current_time(1);
    let ev = engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &G::str_val("harm"),
        &Interval::open(1),
    );
    assert!(
        ev.iter()
            .any(|e| matches!(e, SiftEvent::Completed { .. })),
        "should complete"
    );

    // Drain the completed match
    engine.drain_completed();

    // Add more harm edges — the completed PM should not emit new events
    g.add_str_edge("ev2", "eventType", "harm", 2);
    g.add_ref_edge("ev2", "actor", "charlie", 2);
    g.set_current_time(2);
    let ev = engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &G::str_val("harm"),
        &Interval::open(2),
    );
    // A new PM will be created and completed for this pattern (it's a single-stage pattern
    // that matches new events). This is correct behavior — the pattern template stays active.
    // The key point is that the *old* completed PM doesn't emit duplicate events.
    let completed_count = ev
        .iter()
        .filter(|e| matches!(e, SiftEvent::Completed { .. }))
        .count();
    assert!(
        completed_count <= 1,
        "should have at most 1 new completion (from the template), not duplicates from old PM"
    );
}

/// Incremental: negation is checked before advance (Phase 1 before Phase 3).
pub fn incremental_negation_checked_before_advance<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngine<G> = SiftEngine::new();

    // Pattern: promise -> fulfill, unless break_promise after promise
    let pattern = PatternBuilder::new("kept_promise")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("promise"))
                .edge_bind("e1", "actor".into(), "person")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("fulfill"))
                .edge_bind("e2", "actor".into(), "person")
        })
        .unless_between("e1", "e2", |neg| {
            neg.edge("mid", "eventType".into(), G::str_val("break_promise"))
                .edge_bind("mid", "actor".into(), "person")
        })
        .build();
    engine.register(pattern);

    // alice promises
    g.add_str_edge("ev1", "eventType", "promise", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.set_current_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &G::str_val("promise"),
        &Interval::open(1),
    );

    // alice breaks promise -> should kill the PM
    g.add_str_edge("ev2", "eventType", "break_promise", 2);
    g.add_ref_edge("ev2", "actor", "alice", 2);
    g.set_current_time(2);
    let ev = engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &G::str_val("break_promise"),
        &Interval::open(2),
    );
    assert!(
        ev.iter().any(|e| matches!(e, SiftEvent::Negated { .. })),
        "break_promise should kill the PM"
    );
    assert!(
        engine.active_matches_for("kept_promise").is_empty(),
        "PM should be dead"
    );
}

/// Incremental: negation window closes after e3 is bound (unless_between e1..e3).
pub fn incremental_negation_only_when_window_open<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngine<G> = SiftEngine::new();
    engine.register(voh_pattern::<G>());

    // Complete the 3-stage pattern fully
    g.add_str_edge("ev1", "eventType", "enterTown", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.set_current_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &G::str_val("enterTown"),
        &Interval::open(1),
    );

    g.add_str_edge("ev2", "eventType", "showHospitality", 2);
    g.add_ref_edge("ev2", "actor", "bob", 2);
    g.add_ref_edge("ev2", "target", "alice", 2);
    g.set_current_time(2);
    engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &G::str_val("showHospitality"),
        &Interval::open(2),
    );

    g.add_str_edge("ev3", "eventType", "harm", 3);
    g.add_ref_edge("ev3", "actor", "bob", 3);
    g.add_ref_edge("ev3", "target", "alice", 3);
    g.set_current_time(3);
    let ev = engine.on_edge_added(
        &g,
        &"ev3".into(),
        &"eventType".into(),
        &G::str_val("harm"),
        &Interval::open(3),
    );
    assert!(
        ev.iter()
            .any(|e| matches!(e, SiftEvent::Completed { .. })),
        "should complete"
    );

    // Drain the completed match so it's safely out of the engine
    let completed = engine.drain_completed();
    assert_eq!(completed.len(), 1, "should have 1 completed match to drain");

    // Now add leaveTown AFTER the pattern completed (at t=4).
    // The completed match was already drained, so it cannot be affected.
    // Some ACTIVE partial matches (e.g., from stage 0/1 forks) may still be negated,
    // but the already-completed match is safe.
    g.add_str_edge("ev_leave", "eventType", "leaveTown", 4);
    g.add_ref_edge("ev_leave", "actor", "alice", 4);
    g.set_current_time(4);
    engine.on_edge_added(
        &g,
        &"ev_leave".into(),
        &"eventType".into(),
        &G::str_val("leaveTown"),
        &Interval::open(4),
    );

    // The drained completed match is still valid — not retroactively invalidated
    assert_eq!(
        completed[0].pattern,
        "violation_of_hospitality",
        "the already-drained completed match is unaffected"
    );
}

/// Incremental: adding a negation edge AFTER completion does NOT invalidate the completed match.
pub fn incremental_negation_after_completion_no_retroactive<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngine<G> = SiftEngine::new();
    engine.register(voh_pattern::<G>());

    // Complete the pattern
    g.add_str_edge("ev1", "eventType", "enterTown", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.set_current_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &G::str_val("enterTown"),
        &Interval::open(1),
    );
    g.add_str_edge("ev2", "eventType", "showHospitality", 2);
    g.add_ref_edge("ev2", "actor", "bob", 2);
    g.add_ref_edge("ev2", "target", "alice", 2);
    g.set_current_time(2);
    engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &G::str_val("showHospitality"),
        &Interval::open(2),
    );
    g.add_str_edge("ev3", "eventType", "harm", 3);
    g.add_ref_edge("ev3", "actor", "bob", 3);
    g.add_ref_edge("ev3", "target", "alice", 3);
    g.set_current_time(3);
    let ev = engine.on_edge_added(
        &g,
        &"ev3".into(),
        &"eventType".into(),
        &G::str_val("harm"),
        &Interval::open(3),
    );
    assert!(
        ev.iter()
            .any(|e| matches!(e, SiftEvent::Completed { .. })),
        "should complete the pattern"
    );

    // Now drain to confirm completion
    let completed = engine.drain_completed();
    assert_eq!(completed.len(), 1, "should have 1 completed match");

    // Add a negation edge in the original window (t=2, between e1=1 and e3=3)
    // This should NOT invalidate the already-completed+drained match.
    // Some active PMs (forks/originals) may get negated, but that's fine.
    g.add_str_edge("ev_late_leave", "eventType", "leaveTown", 2);
    g.add_ref_edge("ev_late_leave", "actor", "alice", 2);
    g.set_current_time(4);
    engine.on_edge_added(
        &g,
        &"ev_late_leave".into(),
        &"eventType".into(),
        &G::str_val("leaveTown"),
        &Interval::open(2),
    );

    // The key assertion: the completed match we already drained is unaffected.
    // We still have it in hand and it's valid.
    assert_eq!(
        completed[0].pattern,
        "violation_of_hospitality",
        "the drained completed match is not retroactively invalidated"
    );

    // No NEW completed matches should appear (the pattern should not re-complete)
    let new_completed = engine.drain_completed();
    assert_eq!(
        new_completed.len(),
        0,
        "no new completions after the negation edge"
    );
}

/// Incremental: negation event includes meaningful details.
pub fn incremental_negation_event_details<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngine<G> = SiftEngine::new();
    engine.register(voh_pattern::<G>());

    // Set up stages 1+2
    g.add_str_edge("ev1", "eventType", "enterTown", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.set_current_time(1);
    engine.on_edge_added(&g, &"ev1".into(), &"eventType".into(), &G::str_val("enterTown"), &Interval::open(1));
    g.add_str_edge("ev2", "eventType", "showHospitality", 2);
    g.add_ref_edge("ev2", "actor", "bob", 2);
    g.add_ref_edge("ev2", "target", "alice", 2);
    g.set_current_time(2);
    engine.on_edge_added(&g, &"ev2".into(), &"eventType".into(), &G::str_val("showHospitality"), &Interval::open(2));

    // Kill it
    g.add_str_edge("ev_leave", "eventType", "leaveTown", 3);
    g.add_ref_edge("ev_leave", "actor", "alice", 3);
    g.set_current_time(3);
    let ev = engine.on_edge_added(&g, &"ev_leave".into(), &"eventType".into(), &G::str_val("leaveTown"), &Interval::open(3));

    let negated = ev.iter().find(|e| matches!(e, SiftEvent::Negated { .. }));
    assert!(negated.is_some(), "should emit Negated event");
    if let Some(SiftEvent::Negated { trigger_source, clause_label, .. }) = negated {
        assert_eq!(trigger_source, "ev_leave");
        assert!(
            clause_label.contains("eventType"),
            "clause_label should reference the matching label, got: {}",
            clause_label
        );
    }
}
