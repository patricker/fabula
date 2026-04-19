use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

fn build_graph() -> MemGraph {
    // #region graph_setup
    let mut graph = MemGraph::new();

    // Alice logs in from New York at time 1.
    graph.add_str("login1", "type", "login", 1);
    graph.add_ref("login1", "user", "alice", 1);
    graph.add_str("login1", "location", "new_york", 1);

    // Alice logs in from Tokyo at time 3 -- no logout in between.
    graph.add_str("login2", "type", "login", 3);
    graph.add_ref("login2", "user", "alice", 3);
    graph.add_str("login2", "location", "tokyo", 3);

    // Bob logs in from London at time 2.
    graph.add_str("login3", "type", "login", 2);
    graph.add_ref("login3", "user", "bob", 2);
    graph.add_str("login3", "location", "london", 2);

    // Bob logs out at time 4.
    graph.add_str("logout1", "type", "logout", 4);
    graph.add_ref("logout1", "user", "bob", 4);

    // Bob logs in from Paris at time 5 -- but he logged out first, so this is fine.
    graph.add_str("login4", "type", "login", 5);
    graph.add_ref("login4", "user", "bob", 5);
    graph.add_str("login4", "location", "paris", 5);

    graph.set_time(10);
    // #endregion
    graph
}

fn build_pattern() -> Pattern<String, MemValue> {
    // #region build_pattern
    let pattern = PatternBuilder::<String, MemValue>::new("suspicious_login")
        // Stage 1: A user logs in from some location.
        .stage("login_a", |s| {
            s.edge("login_a", "type".into(), MemValue::Str("login".into()))
                .edge_bind("login_a", "user".into(), "user")
                .edge_bind("login_a", "location".into(), "loc_a")
        })
        // Stage 2: The same user logs in from a different location.
        .stage("login_b", |s| {
            s.edge("login_b", "type".into(), MemValue::Str("login".into()))
                .edge_bind("login_b", "user".into(), "user")
                .edge_bind("login_b", "location".into(), "loc_b")
        })
        // Negation: there must be no logout by that user between the two logins.
        .unless_between("login_a", "login_b", |neg| {
            neg.edge("logout_evt", "type".into(), MemValue::Str("logout".into()))
                .edge_bind("logout_evt", "user".into(), "user")
        })
        .build();
    // #endregion
    pattern
}

#[test]
fn batch_evaluation() {
    let graph = build_graph();
    let pattern = build_pattern();

    // #region batch_eval
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);

    let matches = engine.evaluate(&graph);

    println!("\n=== Batch results: {} match(es) ===", matches.len());
    for m in &matches {
        println!("  Pattern: {}", m.pattern);
        for (var, val) in &m.bindings {
            println!("    {} = {:?}", var, val);
        }
    }
    // #endregion

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].pattern, "suspicious_login");
}

#[test]
fn incremental_evaluation() {
    let pattern = build_pattern();

    // #region incremental_eval
    let mut inc_engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    inc_engine.register(pattern);

    // Replay the edges into a fresh graph, one event at a time.
    let mut inc_graph = MemGraph::new();
    inc_graph.set_time(10);

    // Helper: add one event (a bundle of edges) and report what happened.
    let add_event = |graph: &mut MemGraph,
                     engine: &mut SiftEngineFor<MemGraph>,
                     id: &str,
                     typ: &str,
                     user: &str,
                     extra_label: &str,
                     extra_val: &str,
                     t: i64| {
        graph.add_str(id, "type", typ, t);
        graph.add_ref(id, "user", user, t);
        let interval = Interval::open(t);

        // Notify the engine about each edge.
        let mut events = Vec::new();
        events.extend(engine.on_edge_added(
            graph,
            &id.to_string(),
            &"type".to_string(),
            &MemValue::Str(typ.into()),
            &interval,
        ));
        events.extend(engine.on_edge_added(
            graph,
            &id.to_string(),
            &"user".to_string(),
            &MemValue::Node(user.into()),
            &interval,
        ));

        if !extra_label.is_empty() {
            graph.add_str(id, extra_label, extra_val, t);
            events.extend(engine.on_edge_added(
                graph,
                &id.to_string(),
                &extra_label.to_string(),
                &MemValue::Str(extra_val.into()),
                &interval,
            ));
        }

        for evt in &events {
            println!("  {:?}", evt);
        }
    };

    println!("\n=== Incremental replay ===");

    println!("\n-- Alice logs in from New York (t=1) --");
    add_event(
        &mut inc_graph,
        &mut inc_engine,
        "login1",
        "login",
        "alice",
        "location",
        "new_york",
        1,
    );

    println!("\n-- Bob logs in from London (t=2) --");
    add_event(
        &mut inc_graph,
        &mut inc_engine,
        "login3",
        "login",
        "bob",
        "location",
        "london",
        2,
    );

    println!("\n-- Alice logs in from Tokyo (t=3) --");
    add_event(
        &mut inc_graph,
        &mut inc_engine,
        "login2",
        "login",
        "alice",
        "location",
        "tokyo",
        3,
    );

    println!("\n-- Bob logs out (t=4) --");
    add_event(
        &mut inc_graph,
        &mut inc_engine,
        "logout1",
        "logout",
        "bob",
        "",
        "",
        4,
    );

    println!("\n-- Bob logs in from Paris (t=5) --");
    add_event(
        &mut inc_graph,
        &mut inc_engine,
        "login4",
        "login",
        "bob",
        "location",
        "paris",
        5,
    );
    // #endregion
}
