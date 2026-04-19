//! Violation of hospitality -- the canonical multi-stage pattern from Winnow.
//!
//! Guest enters town -> host shows hospitality -> host harms guest.
//! Unless guest leaves between entry and harm.

use crate::TestGraph;
use fabula::prelude::*;

/// Build the violation_of_hospitality pattern for any adapter.
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

/// Build the standard hospitality graph.
fn hospitality_graph<G: TestGraph>() -> G {
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "enterTown", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev2", "eventType", "showHospitality", 2);
    g.add_ref_edge("ev2", "actor", "bob", 2);
    g.add_ref_edge("ev2", "target", "alice", 2);
    g.add_str_edge("ev3", "eventType", "harm", 3);
    g.add_ref_edge("ev3", "actor", "bob", 3);
    g.add_ref_edge("ev3", "target", "alice", 3);
    g.set_current_time(10);
    g
}

/// Batch: hospitality violation fires when all three stages match.
pub fn batch_hospitality_matches<G: TestGraph>() {
    let g = hospitality_graph::<G>();
    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(voh_pattern::<G>());
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1, "should find exactly one violation");
    assert_eq!(matches[0].pattern, "violation_of_hospitality");
    assert!(
        G::is_node_eq(&matches[0].bindings["guest"], "alice"),
        "guest should be alice"
    );
    assert!(
        G::is_node_eq(&matches[0].bindings["host"], "bob"),
        "host should be bob"
    );
}

/// Batch: guest leaving between entry and harm negates the match.
pub fn batch_hospitality_negated_when_guest_leaves<G: TestGraph>() {
    let mut g = hospitality_graph::<G>();
    g.add_str_edge("ev_leave", "eventType", "leaveTown", 2);
    g.add_ref_edge("ev_leave", "actor", "alice", 2);
    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(voh_pattern::<G>());
    assert_eq!(engine.evaluate(&g).len(), 0, "guest left -- should negate");
}

/// Batch: an unrelated character leaving does NOT negate.
pub fn batch_hospitality_unrelated_leave<G: TestGraph>() {
    let mut g = hospitality_graph::<G>();
    g.add_str_edge("ev_leave", "eventType", "leaveTown", 2);
    g.add_ref_edge("ev_leave", "actor", "charlie", 2);
    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(voh_pattern::<G>());
    assert_eq!(
        engine.evaluate(&g).len(),
        1,
        "charlie leaving should not negate alice's violation"
    );
}

/// Batch: two guests enter, both receive hospitality from bob, bob harms both -> 2 matches.
pub fn batch_hospitality_two_guests<G: TestGraph>() {
    let mut g = G::new_graph();
    // alice enters
    g.add_str_edge("ev1", "eventType", "enterTown", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    // dave enters
    g.add_str_edge("ev2", "eventType", "enterTown", 2);
    g.add_ref_edge("ev2", "actor", "dave", 2);
    // bob shows hospitality to alice
    g.add_str_edge("ev3", "eventType", "showHospitality", 3);
    g.add_ref_edge("ev3", "actor", "bob", 3);
    g.add_ref_edge("ev3", "target", "alice", 3);
    // bob shows hospitality to dave
    g.add_str_edge("ev4", "eventType", "showHospitality", 4);
    g.add_ref_edge("ev4", "actor", "bob", 4);
    g.add_ref_edge("ev4", "target", "dave", 4);
    // bob harms alice
    g.add_str_edge("ev5", "eventType", "harm", 5);
    g.add_ref_edge("ev5", "actor", "bob", 5);
    g.add_ref_edge("ev5", "target", "alice", 5);
    // bob harms dave
    g.add_str_edge("ev6", "eventType", "harm", 6);
    g.add_ref_edge("ev6", "actor", "bob", 6);
    g.add_ref_edge("ev6", "target", "dave", 6);
    g.set_current_time(10);

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(voh_pattern::<G>());
    assert_eq!(
        engine.evaluate(&g).len(),
        2,
        "should find two violations (alice + dave)"
    );
}

/// Batch: enter + harm but no showHospitality -> 0 matches (missing middle stage).
pub fn batch_hospitality_missing_middle<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "enterTown", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev3", "eventType", "harm", 3);
    g.add_ref_edge("ev3", "actor", "bob", 3);
    g.add_ref_edge("ev3", "target", "alice", 3);
    g.set_current_time(10);

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(voh_pattern::<G>());
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "missing showHospitality -> no match"
    );
}

/// Batch: VoH with host property check (communalism). Match only when property exists.
pub fn batch_hospitality_host_property_check<G: TestGraph>() {
    // Pattern with property check on host
    let pattern_with_prop = PatternBuilder::new("voh_with_communalism")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("enterTown"))
                .edge_bind("e1", "actor".into(), "guest")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("showHospitality"))
                .edge_bind("e2", "actor".into(), "host")
                .edge_bind("e2", "target".into(), "guest")
                .edge("host", "value".into(), G::str_val("communalism"))
        })
        .stage("e3", |s| {
            s.edge("e3", "eventType".into(), G::str_val("harm"))
                .edge_bind("e3", "actor".into(), "host")
                .edge_bind("e3", "target".into(), "guest")
        })
        .build();

    // Case 1: bob has value=communalism -> match
    let mut g = hospitality_graph::<G>();
    g.add_str_edge("bob", "value", "communalism", 0);

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern_with_prop);
    assert_eq!(
        engine.evaluate(&g).len(),
        1,
        "bob has communalism -> should match"
    );

    // Case 2: bob does NOT have value=communalism -> no match
    let g2 = hospitality_graph::<G>();
    let pattern_with_prop2 = PatternBuilder::new("voh_with_communalism")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("enterTown"))
                .edge_bind("e1", "actor".into(), "guest")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("showHospitality"))
                .edge_bind("e2", "actor".into(), "host")
                .edge_bind("e2", "target".into(), "guest")
                .edge("host", "value".into(), G::str_val("communalism"))
        })
        .stage("e3", |s| {
            s.edge("e3", "eventType".into(), G::str_val("harm"))
                .edge_bind("e3", "actor".into(), "host")
                .edge_bind("e3", "target".into(), "guest")
        })
        .build();

    let mut engine2: SiftEngineFor<G> = SiftEngine::new();
    engine2.register(pattern_with_prop2);
    assert_eq!(
        engine2.evaluate(&g2).len(),
        0,
        "bob lacks communalism -> no match"
    );
}
