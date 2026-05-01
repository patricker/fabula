//! Consistency scenarios -- verifying batch and incremental produce equivalent results,
//! and that drain_completed behaves correctly.

use crate::TestGraph;
use fabula::prelude::*;

/// Build the VoH pattern.
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

/// Feed events incrementally. Each event is a set of edges added to the graph
/// before the trigger edge (eventType) is passed to on_edge_added.
struct EventSpec<'a> {
    node: &'a str,
    event_type: &'a str,
    str_edges: Vec<(&'a str, &'a str)>,
    ref_edges: Vec<(&'a str, &'a str)>,
    time: i64,
}

fn feed_events_incremental<G: TestGraph>(
    g: &mut G,
    engine: &mut SiftEngineFor<G>,
    events: &[EventSpec<'_>],
) -> usize {
    let mut completed = 0;
    for ev in events {
        // Add all edges to the graph first
        g.add_str_edge(ev.node, "eventType", ev.event_type, ev.time);
        for &(label, val) in &ev.str_edges {
            g.add_str_edge(ev.node, label, val, ev.time);
        }
        for &(label, target) in &ev.ref_edges {
            g.add_ref_edge(ev.node, label, target, ev.time);
        }
        g.set_current_time(ev.time);
        // Trigger on the eventType edge
        let sift_events = engine.on_edge_added(
            g,
            &ev.node.into(),
            &"eventType".into(),
            &G::str_val(ev.event_type),
            &Interval::open(ev.time),
        );
        completed += sift_events
            .iter()
            .filter(|e| matches!(e, SiftEvent::Completed { .. }))
            .count();
    }
    completed
}

/// Batch and incremental produce the same result (0 matches) for a negated graph.
pub fn batch_incremental_negation_consistency<G: TestGraph>() {
    // Batch: hospitality + guest leaves -> 0 matches
    let mut g_batch = G::new_graph();
    g_batch.add_str_edge("ev1", "eventType", "enterTown", 1);
    g_batch.add_ref_edge("ev1", "actor", "alice", 1);
    g_batch.add_str_edge("ev2", "eventType", "showHospitality", 2);
    g_batch.add_ref_edge("ev2", "actor", "bob", 2);
    g_batch.add_ref_edge("ev2", "target", "alice", 2);
    g_batch.add_str_edge("ev_leave", "eventType", "leaveTown", 3);
    g_batch.add_ref_edge("ev_leave", "actor", "alice", 3);
    g_batch.add_str_edge("ev3", "eventType", "harm", 4);
    g_batch.add_ref_edge("ev3", "actor", "bob", 4);
    g_batch.add_ref_edge("ev3", "target", "alice", 4);
    g_batch.set_current_time(10);

    let mut engine_batch: SiftEngineFor<G> = SiftEngine::new(DefaultLetEvaluator);
    engine_batch.register(voh_pattern::<G>());
    let batch_count = engine_batch.evaluate(&g_batch).len();
    assert_eq!(batch_count, 0, "batch: negation should block");

    // Incremental: same events
    let mut g_inc = G::new_graph();
    let mut engine_inc: SiftEngineFor<G> = SiftEngine::new(DefaultLetEvaluator);
    engine_inc.register(voh_pattern::<G>());

    let events = vec![
        EventSpec {
            node: "ev1",
            event_type: "enterTown",
            str_edges: vec![],
            ref_edges: vec![("actor", "alice")],
            time: 1,
        },
        EventSpec {
            node: "ev2",
            event_type: "showHospitality",
            str_edges: vec![],
            ref_edges: vec![("actor", "bob"), ("target", "alice")],
            time: 2,
        },
        EventSpec {
            node: "ev_leave",
            event_type: "leaveTown",
            str_edges: vec![],
            ref_edges: vec![("actor", "alice")],
            time: 3,
        },
        EventSpec {
            node: "ev3",
            event_type: "harm",
            str_edges: vec![],
            ref_edges: vec![("actor", "bob"), ("target", "alice")],
            time: 4,
        },
    ];
    let inc_count = feed_events_incremental(&mut g_inc, &mut engine_inc, &events);
    assert_eq!(
        inc_count, batch_count,
        "incremental and batch should agree: both 0"
    );
}

/// Batch and incremental produce the same result (2 matches) for two independent violations.
pub fn batch_incremental_multi_match_consistency<G: TestGraph>() {
    // Two independent VoH instances: alice+bob and dave+charlie
    let mut g_batch = G::new_graph();
    g_batch.add_str_edge("ev1", "eventType", "enterTown", 1);
    g_batch.add_ref_edge("ev1", "actor", "alice", 1);
    g_batch.add_str_edge("ev2", "eventType", "showHospitality", 2);
    g_batch.add_ref_edge("ev2", "actor", "bob", 2);
    g_batch.add_ref_edge("ev2", "target", "alice", 2);
    g_batch.add_str_edge("ev3", "eventType", "harm", 3);
    g_batch.add_ref_edge("ev3", "actor", "bob", 3);
    g_batch.add_ref_edge("ev3", "target", "alice", 3);
    g_batch.add_str_edge("ev4", "eventType", "enterTown", 4);
    g_batch.add_ref_edge("ev4", "actor", "dave", 4);
    g_batch.add_str_edge("ev5", "eventType", "showHospitality", 5);
    g_batch.add_ref_edge("ev5", "actor", "charlie", 5);
    g_batch.add_ref_edge("ev5", "target", "dave", 5);
    g_batch.add_str_edge("ev6", "eventType", "harm", 6);
    g_batch.add_ref_edge("ev6", "actor", "charlie", 6);
    g_batch.add_ref_edge("ev6", "target", "dave", 6);
    g_batch.set_current_time(10);

    let mut engine_batch: SiftEngineFor<G> = SiftEngine::new(DefaultLetEvaluator);
    engine_batch.register(voh_pattern::<G>());
    let batch_count = engine_batch.evaluate(&g_batch).len();
    assert_eq!(batch_count, 2, "batch: should find 2 violations");

    // Incremental
    let mut g_inc = G::new_graph();
    let mut engine_inc: SiftEngineFor<G> = SiftEngine::new(DefaultLetEvaluator);
    engine_inc.register(voh_pattern::<G>());

    let events = vec![
        EventSpec {
            node: "ev1",
            event_type: "enterTown",
            str_edges: vec![],
            ref_edges: vec![("actor", "alice")],
            time: 1,
        },
        EventSpec {
            node: "ev2",
            event_type: "showHospitality",
            str_edges: vec![],
            ref_edges: vec![("actor", "bob"), ("target", "alice")],
            time: 2,
        },
        EventSpec {
            node: "ev3",
            event_type: "harm",
            str_edges: vec![],
            ref_edges: vec![("actor", "bob"), ("target", "alice")],
            time: 3,
        },
        EventSpec {
            node: "ev4",
            event_type: "enterTown",
            str_edges: vec![],
            ref_edges: vec![("actor", "dave")],
            time: 4,
        },
        EventSpec {
            node: "ev5",
            event_type: "showHospitality",
            str_edges: vec![],
            ref_edges: vec![("actor", "charlie"), ("target", "dave")],
            time: 5,
        },
        EventSpec {
            node: "ev6",
            event_type: "harm",
            str_edges: vec![],
            ref_edges: vec![("actor", "charlie"), ("target", "dave")],
            time: 6,
        },
    ];
    let inc_count = feed_events_incremental(&mut g_inc, &mut engine_inc, &events);
    assert_eq!(
        inc_count, batch_count,
        "incremental and batch should agree: both 2"
    );
}

/// drain_completed is idempotent -- second drain returns 0.
pub fn drain_completed_idempotent<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngineFor<G> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(
        PatternBuilder::new("find_harm")
            .stage("e", |s| s.edge("e", "eventType".into(), G::str_val("harm")))
            .build(),
    );

    g.add_str_edge("ev1", "eventType", "harm", 1);
    g.set_current_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &G::str_val("harm"),
        &Interval::open(1),
    );

    let first = engine.drain_completed();
    assert_eq!(first.len(), 1, "first drain should return 1 completed");

    let second = engine.drain_completed();
    assert_eq!(second.len(), 0, "second drain should return 0");
}

/// drain_completed interleaved -- complete A, drain, complete B, drain.
pub fn drain_completed_interleaved<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngineFor<G> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(
        PatternBuilder::new("find_harm")
            .stage("e", |s| s.edge("e", "eventType".into(), G::str_val("harm")))
            .build(),
    );
    engine.register(
        PatternBuilder::new("find_betray")
            .stage("e", |s| {
                s.edge("e", "eventType".into(), G::str_val("betray"))
            })
            .build(),
    );

    // Complete pattern A
    g.add_str_edge("ev1", "eventType", "harm", 1);
    g.set_current_time(1);
    engine.on_edge_added(
        &g,
        &"ev1".into(),
        &"eventType".into(),
        &G::str_val("harm"),
        &Interval::open(1),
    );

    let first = engine.drain_completed();
    assert_eq!(first.len(), 1, "first drain: 1 (harm)");
    assert_eq!(first[0].pattern, "find_harm");

    // Complete pattern B
    g.add_str_edge("ev2", "eventType", "betray", 2);
    g.set_current_time(2);
    engine.on_edge_added(
        &g,
        &"ev2".into(),
        &"eventType".into(),
        &G::str_val("betray"),
        &Interval::open(2),
    );

    let second = engine.drain_completed();
    assert_eq!(second.len(), 1, "second drain: 1 (betray)");
    assert_eq!(second[0].pattern, "find_betray");
}
