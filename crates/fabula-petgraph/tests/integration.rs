//! Integration tests for the petgraph DataSource adapter.
//!
//! Proves that fabula's pattern matching works with a real third-party
//! graph library, not just the reference MemGraph.

use fabula::prelude::*;
use fabula_petgraph::{PetTemporalGraph, PetValue};

type Graph = PetTemporalGraph<String, String, PetValue<String>, i64>;

fn str_val(s: &str) -> PetValue<String> {
    PetValue::Str(s.to_string())
}

fn node_val(s: &str) -> PetValue<String> {
    PetValue::Node(s.to_string())
}

/// Build the hospitality scenario on a petgraph-backed graph.
fn hospitality_graph() -> Graph {
    let mut g = Graph::new(0);

    // Event 1: Alice enters town
    g.add_node("ev1".into());
    g.add_edge(
        "ev1".into(),
        "eventType".into(),
        str_val("enterTown"),
        Interval::open(1),
    );
    g.add_edge(
        "ev1".into(),
        "actor".into(),
        node_val("alice"),
        Interval::open(1),
    );

    // Event 2: Bob shows hospitality to Alice
    g.add_node("ev2".into());
    g.add_edge(
        "ev2".into(),
        "eventType".into(),
        str_val("showHospitality"),
        Interval::open(2),
    );
    g.add_edge(
        "ev2".into(),
        "actor".into(),
        node_val("bob"),
        Interval::open(2),
    );
    g.add_edge(
        "ev2".into(),
        "target".into(),
        node_val("alice"),
        Interval::open(2),
    );

    // Event 3: Bob harms Alice
    g.add_node("ev3".into());
    g.add_edge(
        "ev3".into(),
        "eventType".into(),
        str_val("harm"),
        Interval::open(3),
    );
    g.add_edge(
        "ev3".into(),
        "actor".into(),
        node_val("bob"),
        Interval::open(3),
    );
    g.add_edge(
        "ev3".into(),
        "target".into(),
        node_val("alice"),
        Interval::open(3),
    );

    // Characters
    g.add_node("alice".into());
    g.add_node("bob".into());

    g.set_time(10);
    g
}

fn violation_of_hospitality() -> Pattern<String, PetValue<String>> {
    PatternBuilder::new("violation_of_hospitality")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), str_val("enterTown"))
                .edge_bind("e1", "actor".into(), "guest")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), str_val("showHospitality"))
                .edge_bind("e2", "actor".into(), "host")
                .edge_bind("e2", "target".into(), "guest")
        })
        .stage("e3", |s| {
            s.edge("e3", "eventType".into(), str_val("harm"))
                .edge_bind("e3", "actor".into(), "host")
                .edge_bind("e3", "target".into(), "guest")
        })
        .unless_between("e1", "e3", |neg| {
            neg.edge("eMid", "eventType".into(), str_val("leaveTown"))
                .edge_bind("eMid", "actor".into(), "guest")
        })
        .build()
}

#[test]
fn petgraph_batch_hospitality() {
    let g = hospitality_graph();
    let mut engine: SiftEngineFor<Graph> = SiftEngine::new();
    engine.register(violation_of_hospitality());

    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1, "should find violation of hospitality");
    match &matches[0].bindings["guest"] {
        BoundValue::Node(n) => assert_eq!(n, "alice"),
        other => panic!("expected guest=alice, got {:?}", other),
    }
    match &matches[0].bindings["host"] {
        BoundValue::Node(n) => assert_eq!(n, "bob"),
        other => panic!("expected host=bob, got {:?}", other),
    }
}

#[test]
fn petgraph_batch_negation_blocks() {
    let mut g = hospitality_graph();
    // Alice leaves between events 1 and 3
    g.add_edge(
        "ev_leave".into(),
        "eventType".into(),
        str_val("leaveTown"),
        Interval::open(2),
    );
    g.add_edge(
        "ev_leave".into(),
        "actor".into(),
        node_val("alice"),
        Interval::open(2),
    );

    let mut engine: SiftEngineFor<Graph> = SiftEngine::new();
    engine.register(violation_of_hospitality());
    assert_eq!(engine.evaluate(&g).len(), 0, "guest left -- negation blocks");
}

#[test]
fn petgraph_incremental_three_stages() {
    let mut g = Graph::new(0);
    let mut engine: SiftEngineFor<Graph> = SiftEngine::new();
    engine.register(violation_of_hospitality());

    // Stage 1
    g.add_node("alice".into());
    g.add_node("ev1".into());
    g.add_edge(
        "ev1".into(),
        "eventType".into(),
        str_val("enterTown"),
        Interval::open(1),
    );
    g.add_edge(
        "ev1".into(),
        "actor".into(),
        node_val("alice"),
        Interval::open(1),
    );
    g.set_time(1);
    let ev = engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &str_val("enterTown"),
        &Interval::open(1),
    );
    assert!(ev.iter().any(|e| matches!(e, SiftEvent::Advanced { .. })));

    // Stage 2
    g.add_node("bob".into());
    g.add_node("ev2".into());
    g.add_edge(
        "ev2".into(),
        "eventType".into(),
        str_val("showHospitality"),
        Interval::open(2),
    );
    g.add_edge(
        "ev2".into(),
        "actor".into(),
        node_val("bob"),
        Interval::open(2),
    );
    g.add_edge(
        "ev2".into(),
        "target".into(),
        node_val("alice"),
        Interval::open(2),
    );
    g.set_time(2);
    let ev = engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &str_val("showHospitality"),
        &Interval::open(2),
    );
    assert!(ev.iter().any(|e| matches!(e, SiftEvent::Advanced { .. })));

    // Stage 3 → complete
    g.add_node("ev3".into());
    g.add_edge(
        "ev3".into(),
        "eventType".into(),
        str_val("harm"),
        Interval::open(3),
    );
    g.add_edge(
        "ev3".into(),
        "actor".into(),
        node_val("bob"),
        Interval::open(3),
    );
    g.add_edge(
        "ev3".into(),
        "target".into(),
        node_val("alice"),
        Interval::open(3),
    );
    g.set_time(3);
    let ev = engine.on_edge_added(
        &g,
        &"ev3".into(),
        &"eventType".into(),
        &str_val("harm"),
        &Interval::open(3),
    );
    assert!(ev.iter().any(|e| matches!(e, SiftEvent::Completed { .. })));
}

#[test]
fn petgraph_single_stage_immediate() {
    let mut g = Graph::new(0);
    let mut engine: SiftEngineFor<Graph> = SiftEngine::new();

    let pattern = PatternBuilder::new("find_harm")
        .stage("e", |s| {
            s.edge("e", "eventType".into(), str_val("harm")).edge_bind(
                "e",
                "actor".into(),
                "attacker",
            )
        })
        .build();
    engine.register(pattern);

    g.add_node("ev1".into());
    g.add_node("bob".into());
    g.add_edge(
        "ev1".into(),
        "eventType".into(),
        str_val("harm"),
        Interval::open(1),
    );
    g.add_edge(
        "ev1".into(),
        "actor".into(),
        node_val("bob"),
        Interval::open(1),
    );
    g.set_time(1);
    let ev = engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &str_val("harm"),
        &Interval::open(1),
    );
    assert!(ev.iter().any(|e| matches!(e, SiftEvent::Completed { .. })));
}

#[test]
fn petgraph_why_not_on_empty() {
    let g = Graph::new(0);
    let mut engine: SiftEngineFor<Graph> = SiftEngine::new();
    engine.register(violation_of_hospitality());

    let analysis = engine.why_not(&g, "violation_of_hospitality").unwrap();
    assert!(!analysis.stages.is_empty());
    match analysis.stages[0].status {
        StageStatus::Unmatched => {}
        ref other => panic!("expected Unmatched, got {:?}", other),
    }
}
