//! Negation window scenarios -- unless_between, unless_after, unless_global.

use crate::TestGraph;
use fabula::prelude::*;

/// Batch: unless_after blocks match when negation event exists after anchor.
pub fn batch_unless_after_blocks<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "promise", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev2", "eventType", "break_promise", 2);
    g.add_ref_edge("ev2", "actor", "alice", 2);
    g.set_current_time(10);

    let pattern = PatternBuilder::new("broken_promise")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("promise"))
                .edge_bind("e1", "actor".into(), "person")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("break_promise"))
                .edge_bind("e2", "actor".into(), "person")
        })
        .unless_after("e1", |neg| {
            neg.edge("apology", "eventType".into(), G::str_val("apologize"))
                .edge_bind("apology", "actor".into(), "person")
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);

    // No apology -> should match
    assert_eq!(engine.evaluate(&g).len(), 1, "no apology -- should match");

    // Add apology after promise -> should NOT match
    g.add_str_edge("ev_apology", "eventType", "apologize", 3);
    g.add_ref_edge("ev_apology", "actor", "alice", 3);
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "apology exists -- should not match"
    );
}

/// Batch: unless_global blocks match when negation event exists anywhere.
pub fn batch_unless_global<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "betray", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev2", "eventType", "betray", 3);
    g.add_ref_edge("ev2", "actor", "alice", 3);
    g.set_current_time(10);

    let pattern = PatternBuilder::new("double_betrayal")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("betray"))
                .edge_bind("e1", "actor".into(), "char")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("betray"))
                .edge_bind("e2", "actor".into(), "char")
        })
        .unless_global(|neg| {
            neg.edge("mid", "eventType".into(), G::str_val("reconcile"))
                .edge_bind("mid", "actor".into(), "char")
        })
        .build();

    let mut engine: SiftEngineFor<G> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        1,
        "no reconciliation -- should match"
    );

    // Add reconciliation between betrayals -> no match
    g.add_str_edge("ev_rec", "eventType", "reconcile", 2);
    g.add_ref_edge("ev_rec", "actor", "alice", 2);
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "reconciliation blocks double_betrayal"
    );
}

/// Batch: double negation -- two unless_between windows. Either one blocks.
pub fn batch_double_negation_two_windows<G: TestGraph>() {
    // Pattern: e1 -> e2 -> e3, unless leave between e1..e2, unless betray between e2..e3
    let pattern = PatternBuilder::new("guarded_sequence")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("start"))
                .edge_bind("e1", "actor".into(), "person")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("middle"))
                .edge_bind("e2", "actor".into(), "person")
        })
        .stage("e3", |s| {
            s.edge("e3", "eventType".into(), G::str_val("end"))
                .edge_bind("e3", "actor".into(), "person")
        })
        .unless_between("e1", "e2", |neg| {
            neg.edge("n1", "eventType".into(), G::str_val("leave"))
                .edge_bind("n1", "actor".into(), "person")
        })
        .unless_between("e2", "e3", |neg| {
            neg.edge("n2", "eventType".into(), G::str_val("betray"))
                .edge_bind("n2", "actor".into(), "person")
        })
        .build();

    // Case 1: clean sequence -> match
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "start", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev2", "eventType", "middle", 3);
    g.add_ref_edge("ev2", "actor", "alice", 3);
    g.add_str_edge("ev3", "eventType", "end", 5);
    g.add_ref_edge("ev3", "actor", "alice", 5);
    g.set_current_time(10);

    let mut engine: SiftEngineFor<G> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    assert_eq!(engine.evaluate(&g).len(), 1, "clean sequence should match");

    // Case 2: leave between e1..e2 -> no match
    g.add_str_edge("ev_leave", "eventType", "leave", 2);
    g.add_ref_edge("ev_leave", "actor", "alice", 2);
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "leave between e1..e2 should block"
    );

    // Reset: no leave, but betray between e2..e3 -> no match
    let mut g2 = G::new_graph();
    g2.add_str_edge("ev1", "eventType", "start", 1);
    g2.add_ref_edge("ev1", "actor", "alice", 1);
    g2.add_str_edge("ev2", "eventType", "middle", 3);
    g2.add_ref_edge("ev2", "actor", "alice", 3);
    g2.add_str_edge("ev_betray", "eventType", "betray", 4);
    g2.add_ref_edge("ev_betray", "actor", "alice", 4);
    g2.add_str_edge("ev3", "eventType", "end", 5);
    g2.add_ref_edge("ev3", "actor", "alice", 5);
    g2.set_current_time(10);

    let pattern2 = PatternBuilder::new("guarded_sequence")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("start"))
                .edge_bind("e1", "actor".into(), "person")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("middle"))
                .edge_bind("e2", "actor".into(), "person")
        })
        .stage("e3", |s| {
            s.edge("e3", "eventType".into(), G::str_val("end"))
                .edge_bind("e3", "actor".into(), "person")
        })
        .unless_between("e1", "e2", |neg| {
            neg.edge("n1", "eventType".into(), G::str_val("leave"))
                .edge_bind("n1", "actor".into(), "person")
        })
        .unless_between("e2", "e3", |neg| {
            neg.edge("n2", "eventType".into(), G::str_val("betray"))
                .edge_bind("n2", "actor".into(), "person")
        })
        .build();

    let mut engine2: SiftEngineFor<G> = SiftEngine::new(DefaultLetEvaluator);
    engine2.register(pattern2);
    assert_eq!(
        engine2.evaluate(&g2).len(),
        0,
        "betray between e2..e3 should block"
    );
}

/// Batch: negation with multi-clause body -- only blocks when ALL clauses match.
pub fn batch_negation_multi_clause_body<G: TestGraph>() {
    // Pattern: promise -> fulfill, unless (eventType=leave AND actor=?person) between
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
            neg.edge("mid", "eventType".into(), G::str_val("leave"))
                .edge_bind("mid", "actor".into(), "person")
        })
        .build();

    // Case 1: different person leaves -> negation should NOT fire (actor doesn't match)
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "promise", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev_leave", "eventType", "leave", 2);
    g.add_ref_edge("ev_leave", "actor", "bob", 2); // bob, not alice
    g.add_str_edge("ev2", "eventType", "fulfill", 3);
    g.add_ref_edge("ev2", "actor", "alice", 3);
    g.set_current_time(10);

    let mut engine: SiftEngineFor<G> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        1,
        "bob leaving should not block alice's promise"
    );

    // Case 2: alice leaves -> negation fires
    let pattern2 = PatternBuilder::new("kept_promise")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("promise"))
                .edge_bind("e1", "actor".into(), "person")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("fulfill"))
                .edge_bind("e2", "actor".into(), "person")
        })
        .unless_between("e1", "e2", |neg| {
            neg.edge("mid", "eventType".into(), G::str_val("leave"))
                .edge_bind("mid", "actor".into(), "person")
        })
        .build();

    let mut g2 = G::new_graph();
    g2.add_str_edge("ev1", "eventType", "promise", 1);
    g2.add_ref_edge("ev1", "actor", "alice", 1);
    g2.add_str_edge("ev_leave", "eventType", "leave", 2);
    g2.add_ref_edge("ev_leave", "actor", "alice", 2); // alice
    g2.add_str_edge("ev2", "eventType", "fulfill", 3);
    g2.add_ref_edge("ev2", "actor", "alice", 3);
    g2.set_current_time(10);

    let mut engine2: SiftEngineFor<G> = SiftEngine::new(DefaultLetEvaluator);
    engine2.register(pattern2);
    assert_eq!(
        engine2.evaluate(&g2).len(),
        0,
        "alice leaving should block alice's promise"
    );
}

/// Batch: negation event at exact same timestamp as window start.
/// Per B4 fix (exclusive start), it's outside the window.
pub fn batch_negation_at_boundary_exclusive<G: TestGraph>() {
    // Pattern: e1 -> e2, unless leave between e1..e2
    let pattern = PatternBuilder::new("boundary_test")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), G::str_val("start"))
                .edge_bind("e1", "actor".into(), "person")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), G::str_val("end"))
                .edge_bind("e2", "actor".into(), "person")
        })
        .unless_between("e1", "e2", |neg| {
            neg.edge("mid", "eventType".into(), G::str_val("leave"))
                .edge_bind("mid", "actor".into(), "person")
        })
        .build();

    // Negation event at t=1 (same as e1 at t=1) -- window is (1, 3) exclusive start
    let mut g = G::new_graph();
    g.add_str_edge("ev1", "eventType", "start", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev_leave", "eventType", "leave", 1); // same timestamp as e1
    g.add_ref_edge("ev_leave", "actor", "alice", 1);
    g.add_str_edge("ev2", "eventType", "end", 3);
    g.add_ref_edge("ev2", "actor", "alice", 3);
    g.set_current_time(10);

    let mut engine: SiftEngineFor<G> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        1,
        "negation at exact start boundary should NOT block (exclusive start, B4 fix)"
    );
}
