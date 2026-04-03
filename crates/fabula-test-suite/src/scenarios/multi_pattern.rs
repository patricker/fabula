//! Multiple pattern scenarios — two patterns registered simultaneously.

use crate::TestGraph;
use fabula::prelude::*;

/// Batch: four patterns registered, only VoH matches the graph data.
pub fn multi_pattern_all_four_winnow<G: TestGraph>() {
    let mut g = G::new_graph();
    // VoH data
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

    // Pattern 1: VoH
    engine.register(
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
            .build(),
    );

    // Pattern 2: romantic_arc (won't match — no romantic tags)
    engine.register(
        PatternBuilder::new("romantic_arc")
            .stage("e1", |s| {
                s.edge("e1", "tag".into(), G::str_val("negative"))
                    .edge("e1", "tag".into(), G::str_val("romantic"))
            })
            .stage("e2", |s| {
                s.edge("e2", "tag".into(), G::str_val("negative"))
                    .edge("e2", "tag".into(), G::str_val("romantic"))
            })
            .stage("e3", |s| {
                s.edge("e3", "tag".into(), G::str_val("positive"))
                    .edge("e3", "tag".into(), G::str_val("romantic"))
            })
            .build(),
    );

    // Pattern 3: two betrayals (won't match — no betray events)
    engine.register(
        PatternBuilder::new("two_betrayals")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), G::str_val("betray"))
            })
            .stage("e2", |s| {
                s.edge("e2", "eventType".into(), G::str_val("betray"))
            })
            .build(),
    );

    // Pattern 4: hypocrisy-like (won't match — no preach events)
    engine.register(
        PatternBuilder::new("hypocrisy")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), G::str_val("preach"))
            })
            .stage("e2", |s| {
                s.edge("e2", "eventType".into(), G::str_val("violate"))
            })
            .build(),
    );

    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1, "only VoH should match");
    assert_eq!(matches[0].pattern, "violation_of_hospitality");
}

/// Batch: two single-stage patterns that both match the same eventType.
pub fn multi_pattern_shared_events<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "harm", 1);
    g.add_ref_edge("ev1", "actor", "bob", 1);
    g.set_current_time(10);

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    // Pattern 1: find any harm
    engine.register(
        PatternBuilder::new("any_harm")
            .stage("e", |s| s.edge("e", "eventType".into(), G::str_val("harm")))
            .build(),
    );
    // Pattern 2: find harm with actor binding
    engine.register(
        PatternBuilder::new("harm_with_actor")
            .stage("e", |s| {
                s.edge("e", "eventType".into(), G::str_val("harm"))
                    .edge_bind("e", "actor".into(), "attacker")
            })
            .build(),
    );

    let matches = engine.evaluate(&g);
    assert_eq!(
        matches.len(),
        2,
        "both patterns should match the same harm event"
    );
    let pattern_names: Vec<&str> = matches.iter().map(|m| m.pattern.as_str()).collect();
    assert!(pattern_names.contains(&"any_harm"));
    assert!(pattern_names.contains(&"harm_with_actor"));
}

/// Two patterns fire independently from the same edge event.
pub fn incremental_multiple_patterns_fire<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngineFor<G> = SiftEngine::new();

    // Pattern 1: full hospitality (3 stages)
    engine.register(
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
            .build(),
    );

    // Pattern 2: any enterTown (single stage — completes immediately)
    engine.register(
        PatternBuilder::new("any_enter")
            .stage("e", |s| {
                s.edge("e", "eventType".into(), G::str_val("enterTown"))
            })
            .build(),
    );

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
        ev.iter().any(|e| matches!(e, SiftEvent::Completed { pattern, .. } if pattern == "any_enter")),
        "any_enter should complete"
    );
    assert!(
        ev.iter().any(|e| matches!(e, SiftEvent::Advanced { pattern, .. } if pattern == "violation_of_hospitality")),
        "violation_of_hospitality should advance"
    );
}
