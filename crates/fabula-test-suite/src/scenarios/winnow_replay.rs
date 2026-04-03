//! Winnow reference replay scenarios.
//!
//! These tests replicate the canonical 7-step sequence from Winnow's test suite
//! and verify multi-pattern interaction.

use crate::TestGraph;
use fabula::prelude::*;

/// Build the VoH pattern used in the 7-step replay.
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

/// Incremental: the canonical Winnow 7-step sequence.
///
/// 1. enter(alice, t=1) -> PM advances (stage 1)
/// 2. hospitality(bob->alice, t=2) -> PM advances (stage 2)
/// 3. hospitality(charlie->alice, t=3) -> FORK: second PM at stage 2
/// 4. enter(dave, t=4) -> new PM for dave at stage 1
/// 5. leave(alice, t=5) -> KILLS all alice-related PMs (negation)
/// 6. harm(bob->alice, t=6) -> no completion (alice PMs dead)
/// 7. harm(charlie->alice, t=7) -> no completion (alice PMs dead)
pub fn incremental_winnow_7step_sequence<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(voh_pattern::<G>());

    // Step 1: enter(alice, t=1) -> PM advances to stage 1
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
        ev.iter()
            .any(|e| matches!(e, SiftEvent::Advanced { pattern, .. } if pattern == "violation_of_hospitality")),
        "step 1: alice enters -> should advance"
    );

    // Step 2: hospitality(bob->alice, t=2) -> PM advances to stage 2
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
        ev.iter()
            .any(|e| matches!(e, SiftEvent::Advanced { pattern, .. } if pattern == "violation_of_hospitality")),
        "step 2: bob shows hospitality -> should advance"
    );

    // Step 3: hospitality(charlie->alice, t=3) -> FORK: second PM with host=charlie
    g.add_str_edge("ev3", "eventType", "showHospitality", 3);
    g.add_ref_edge("ev3", "actor", "charlie", 3);
    g.add_ref_edge("ev3", "target", "alice", 3);
    g.set_current_time(3);
    let ev = engine.on_edge_added(
        &g,
        &"ev3".into(),
        &"eventType".into(),
        &G::str_val("showHospitality"),
        &Interval::open(3),
    );
    assert!(
        ev.iter()
            .any(|e| matches!(e, SiftEvent::Advanced { pattern, .. } if pattern == "violation_of_hospitality")),
        "step 3: charlie shows hospitality -> should fork/advance"
    );

    // Step 4: enter(dave, t=4) -> new PM for dave at stage 1
    g.add_str_edge("ev4", "eventType", "enterTown", 4);
    g.add_ref_edge("ev4", "actor", "dave", 4);
    g.set_current_time(4);
    let ev = engine.on_edge_added(
        &g,
        &"ev4".into(),
        &"eventType".into(),
        &G::str_val("enterTown"),
        &Interval::open(4),
    );
    assert!(
        ev.iter()
            .any(|e| matches!(e, SiftEvent::Advanced { pattern, .. } if pattern == "violation_of_hospitality")),
        "step 4: dave enters -> should create new PM"
    );

    // Step 5: leave(alice, t=5) -> KILLS all alice-related PMs
    g.add_str_edge("ev_leave", "eventType", "leaveTown", 5);
    g.add_ref_edge("ev_leave", "actor", "alice", 5);
    g.set_current_time(5);
    let ev = engine.on_edge_added(
        &g,
        &"ev_leave".into(),
        &"eventType".into(),
        &G::str_val("leaveTown"),
        &Interval::open(5),
    );
    // Should emit Negated events for alice's PMs
    let negated_count = ev
        .iter()
        .filter(|e| matches!(e, SiftEvent::Negated { .. }))
        .count();
    assert!(
        negated_count > 0,
        "step 5: alice leaves -> should negate alice PMs, got {} negated",
        negated_count
    );
    // No completions at this step
    assert!(
        !ev.iter()
            .any(|e| matches!(e, SiftEvent::Completed { .. })),
        "step 5: should have no completions"
    );

    // Step 6: harm(bob->alice, t=6) -> no completion (alice PMs dead)
    g.add_str_edge("ev6", "eventType", "harm", 6);
    g.add_ref_edge("ev6", "actor", "bob", 6);
    g.add_ref_edge("ev6", "target", "alice", 6);
    g.set_current_time(6);
    let ev = engine.on_edge_added(
        &g,
        &"ev6".into(),
        &"eventType".into(),
        &G::str_val("harm"),
        &Interval::open(6),
    );
    assert!(
        !ev.iter()
            .any(|e| matches!(e, SiftEvent::Completed { .. })),
        "step 6: bob harms alice -> should NOT complete (alice PMs are dead)"
    );

    // Step 7: harm(charlie->alice, t=7) -> no completion (alice PMs dead)
    g.add_str_edge("ev7", "eventType", "harm", 7);
    g.add_ref_edge("ev7", "actor", "charlie", 7);
    g.add_ref_edge("ev7", "target", "alice", 7);
    g.set_current_time(7);
    let ev = engine.on_edge_added(
        &g,
        &"ev7".into(),
        &"eventType".into(),
        &G::str_val("harm"),
        &Interval::open(7),
    );
    assert!(
        !ev.iter()
            .any(|e| matches!(e, SiftEvent::Completed { .. })),
        "step 7: charlie harms alice -> should NOT complete (alice PMs are dead)"
    );
}

/// Batch: multiple patterns registered, only VoH matches the graph.
pub fn batch_winnow_multi_pattern<G: TestGraph>() {
    let mut g = G::new_graph();
    // Build a graph that satisfies VoH only
    g.add_str_edge("ev1", "eventType", "enterTown", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev2", "eventType", "showHospitality", 2);
    g.add_ref_edge("ev2", "actor", "bob", 2);
    g.add_ref_edge("ev2", "target", "alice", 2);
    g.add_str_edge("ev3", "eventType", "harm", 3);
    g.add_ref_edge("ev3", "actor", "bob", 3);
    g.add_ref_edge("ev3", "target", "alice", 3);
    g.set_current_time(10);

    let mut engine: SiftEngineFor<G> = SiftEngine::new();

    // Register VoH
    engine.register(voh_pattern::<G>());

    // Register romantic arc (needs neg, neg, pos romantic tags -- not in graph)
    engine.register(
        PatternBuilder::new("romantic_arc")
            .stage("e1", |s| {
                s.edge("e1", "tag".into(), G::str_val("negative"))
                    .edge("e1", "tag".into(), G::str_val("romantic"))
                    .edge_bind("e1", "actor".into(), "char")
            })
            .stage("e2", |s| {
                s.edge("e2", "tag".into(), G::str_val("negative"))
                    .edge("e2", "tag".into(), G::str_val("romantic"))
                    .edge_bind("e2", "actor".into(), "char")
            })
            .stage("e3", |s| {
                s.edge("e3", "tag".into(), G::str_val("positive"))
                    .edge("e3", "tag".into(), G::str_val("romantic"))
                    .edge_bind("e3", "actor".into(), "char")
            })
            .build(),
    );

    // Register two-betrayals (needs betray events + impulsive trait -- not in graph)
    engine.register(
        PatternBuilder::new("two_impulsive_betrayals")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), G::str_val("betray"))
                    .edge_bind("e1", "actor".into(), "char")
                    .edge("char", "trait".into(), G::str_val("impulsive"))
            })
            .stage("e2", |s| {
                s.edge("e2", "eventType".into(), G::str_val("betray"))
                    .edge_bind("e2", "actor".into(), "char")
            })
            .build(),
    );

    let matches = engine.evaluate(&g);
    assert_eq!(
        matches.len(),
        1,
        "only VoH should match, got {}",
        matches.len()
    );
    assert_eq!(matches[0].pattern, "violation_of_hospitality");
}
