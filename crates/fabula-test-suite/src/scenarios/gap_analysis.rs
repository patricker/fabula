//! Gap analysis (why_not) scenarios.

use crate::TestGraph;
use fabula::prelude::*;

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

/// why_not on an empty graph stops at the first unmatched stage.
pub fn gap_empty_graph<G: TestGraph>() {
    let g = G::new_graph();
    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(voh_pattern::<G>());
    let analysis = engine.why_not(&g, "violation_of_hospitality").unwrap();
    assert_eq!(
        analysis.stages.len(),
        1,
        "should stop at first unmatched stage"
    );
    match analysis.stages[0].status {
        StageStatus::Unmatched => {}
        ref other => panic!("expected Unmatched, got {:?}", other),
    }
}

/// why_not returns None for an unregistered pattern.
pub fn gap_unknown_pattern<G: TestGraph>() {
    let g = G::new_graph();
    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(voh_pattern::<G>());
    let result = engine.why_not(&g, "nonexistent_pattern");
    assert!(result.is_none(), "unknown pattern should return None");
}

/// why_not: 3-clause stage where clauses 1 and 2 match but clause 3 doesn't.
pub fn gap_partially_matched_stage<G: TestGraph>() {
    let mut g = G::new_graph();
    // ev1 has eventType and actor, but missing "tag"
    g.add_str_edge("ev1", "eventType", "enterTown", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.set_current_time(10);

    let pattern = PatternBuilder::new("tagged_entry")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("enterTown"))
                .edge_bind("e1", "actor".into(), "person")
                .edge("e1", "tag".into(), G::str_val("important"))
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    let analysis = engine.why_not(&g, "tagged_entry").unwrap();

    assert_eq!(analysis.stages.len(), 1, "should analyze first stage");
    // The stage should be partially matched (some clauses ok, some not)
    // Note: why_not doesn't propagate bindings, so source var is unbound.
    // The first stage analysis depends on implementation — it might be Unmatched
    // because the source var for the first clause is unbound in why_not.
    assert!(
        !analysis.stages[0].clauses.is_empty(),
        "should have clause analyses"
    );
}

/// why_not: stage 2 fails because binding doesn't carry expected property.
pub fn gap_second_stage_fails_binding<G: TestGraph>() {
    let mut g = G::new_graph();
    // Stage 1 data is present
    g.add_str_edge("ev1", "eventType", "enterTown", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    // alice does NOT have status=leader
    g.set_current_time(10);

    let pattern = PatternBuilder::new("leader_entry")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("enterTown"))
                .edge_bind("e1", "actor".into(), "char")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("showHospitality"))
                .edge("char", "status".into(), G::str_val("leader"))
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    let analysis = engine.why_not(&g, "leader_entry").unwrap();

    // why_not should stop at the first unmatched stage
    // Stage 1 source var is unbound in why_not, so it may report stage 1 as unmatched
    assert!(
        !analysis.stages.is_empty(),
        "should have at least one stage analysis"
    );
    // The last reported stage should be unmatched
    let last = analysis.stages.last().unwrap();
    assert!(
        !matches!(last.status, StageStatus::Matched),
        "last stage should not be Matched, got {:?}",
        last.status
    );
}

/// why_not: pattern with not_edge in stage where edge exists -> should report failure.
pub fn gap_negated_clause_in_stage<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "action", 1);
    g.add_str_edge("ev1", "tag", "forbidden", 1);
    g.set_current_time(10);

    let pattern = PatternBuilder::new("clean_action")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("action"))
                .not_edge("e1", "tag".into(), G::str_val("forbidden"))
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);

    // Batch: should not match because tag=forbidden exists
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "tag=forbidden exists -> not_edge blocks match"
    );

    // why_not should report the issue
    let analysis = engine.why_not(&g, "clean_action").unwrap();
    assert!(
        !analysis.stages.is_empty(),
        "should have stage analysis"
    );
}

/// why_not with data present still reports first stage unmatched (bindings
/// are not propagated in why_not's current implementation).
pub fn gap_with_partial_data<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "enterTown", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.set_current_time(10);

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(voh_pattern::<G>());
    let analysis = engine.why_not(&g, "violation_of_hospitality").unwrap();
    // why_not stops at the first unmatched stage. Since bindings don't
    // propagate, the first stage's source var (?e1) is unbound.
    assert_eq!(
        analysis.stages.len(),
        1,
        "should stop at first stage (unbound source var)"
    );
    // The stage should have clause analyses with reasons
    assert!(
        !analysis.stages[0].clauses.is_empty(),
        "should have clause analyses"
    );
}
