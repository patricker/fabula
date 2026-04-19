use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

#[test]
fn kill_pms_involving_removes_matching_pms() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    // Two-stage pattern: alice does "start" then "end"
    engine.register(
        PatternBuilder::new("quest")
            .stage("e1", |s| {
                s.edge("e1", "type".into(), MemValue::Str("start".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("end".into()))
            })
            .build(),
    );

    // Feed stage 1 for alice -- creates 1 active PM
    g.add_str("alice", "type", "start", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"alice".into(),
        &"type".into(),
        &MemValue::Str("start".into()),
        &Interval::open(1),
    );

    let active = engine
        .partial_matches()
        .iter()
        .filter(|pm| pm.state == MatchState::Active)
        .count();
    assert_eq!(active, 1, "should have 1 active PM after stage 1");

    // Kill PMs involving alice
    let killed = engine.kill_pms_involving(&"alice".into());
    assert_eq!(killed, 1, "should kill 1 PM");

    let active = engine
        .partial_matches()
        .iter()
        .filter(|pm| pm.state == MatchState::Active)
        .count();
    assert_eq!(active, 0, "should have 0 active PMs after kill");
}

#[test]
fn kill_pms_involving_spares_unrelated_pms() {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    engine.register(
        PatternBuilder::new("quest")
            .stage("e1", |s| {
                s.edge("e1", "type".into(), MemValue::Str("start".into()))
            })
            .stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("end".into()))
            })
            .build(),
    );

    // Feed stage 1 for alice
    g.add_str("alice", "type", "start", 1);
    g.set_time(1);
    engine.on_edge_added(
        &g,
        &"alice".into(),
        &"type".into(),
        &MemValue::Str("start".into()),
        &Interval::open(1),
    );

    // Feed stage 1 for bob
    g.add_str("bob", "type", "start", 2);
    g.set_time(2);
    engine.on_edge_added(
        &g,
        &"bob".into(),
        &"type".into(),
        &MemValue::Str("start".into()),
        &Interval::open(2),
    );

    let active = engine
        .partial_matches()
        .iter()
        .filter(|pm| pm.state == MatchState::Active)
        .count();
    assert_eq!(active, 2, "should have 2 active PMs");

    // Kill only alice
    let killed = engine.kill_pms_involving(&"alice".into());
    assert_eq!(killed, 1, "should kill 1 PM (alice)");

    let active = engine
        .partial_matches()
        .iter()
        .filter(|pm| pm.state == MatchState::Active)
        .count();
    assert_eq!(active, 1, "should have 1 active PM (bob)");

    // Verify remaining PM is bob's
    let remaining = &engine.partial_matches()[0];
    assert!(
        remaining.bindings.values().any(|bv| match bv {
            BoundValue::Node(n) => n == "bob",
            _ => false,
        }),
        "remaining PM should be bob's"
    );
}

#[test]
fn kill_pms_involving_returns_zero_when_no_match() {
    let engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    // No PMs exist at all
    let killed = engine.clone().kill_pms_involving(&"nobody".into());
    assert_eq!(killed, 0, "should return 0 when no PMs exist");
}
