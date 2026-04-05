use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

// ── Recipe 1: Repeated behavior by the same actor ──────────────────────────

fn recipe1_pattern() -> Pattern<String, MemValue> {
    // #region r1_pattern
    PatternBuilder::<String, MemValue>::new("two_impulsive_betrayals")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("betray".into()))
                .edge_bind("e1", "actor".into(), "char")
                .edge("char", "trait".into(), MemValue::Str("impulsive".into()))
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), MemValue::Str("betray".into()))
                .edge_bind("e2", "actor".into(), "char")
        })
        .unless_global(|neg| {
            neg.edge("mid", "eventType".into(), MemValue::Str("reconcile".into()))
                .edge_bind("mid", "actor".into(), "char")
        })
        .build()
    // #endregion
}

#[test]
fn recipe1_matching() {
    let pattern = recipe1_pattern();

    // #region r1_matching
    let mut g = MemGraph::new();
    g.add_str("alice", "trait", "impulsive", 0);
    g.add_str("ev1", "eventType", "betray", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.add_str("ev2", "eventType", "betray", 3);
    g.add_ref("ev2", "actor", "alice", 3);
    g.set_time(10);

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].bindings["char"], BoundValue::Node("alice".into()));
    // #endregion
}

#[test]
fn recipe1_non_matching() {
    let pattern = recipe1_pattern();

    // #region r1_non_matching
    let mut g = MemGraph::new();
    g.add_str("alice", "trait", "cautious", 0); // not impulsive
    g.add_str("ev1", "eventType", "betray", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.add_str("ev2", "eventType", "betray", 3);
    g.add_ref("ev2", "actor", "alice", 3);
    g.set_time(10);
    // #endregion

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 0);
}

// ── Recipe 2: Violation with exception (negation between) ──────────────────

fn recipe2_pattern() -> Pattern<String, MemValue> {
    // #region r2_pattern
    PatternBuilder::<String, MemValue>::new("broken_promise")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("promise".into()))
                .edge_bind("e1", "actor".into(), "person")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), MemValue::Str("break_promise".into()))
                .edge_bind("e2", "actor".into(), "person")
        })
        .unless_between("e1", "e2", |neg| {
            neg.edge("apology", "eventType".into(), MemValue::Str("apologize".into()))
                .edge_bind("apology", "actor".into(), "person")
        })
        .build()
    // #endregion
}

#[test]
fn recipe2_matching() {
    let pattern = recipe2_pattern();

    // #region r2_matching
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "promise", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.add_str("ev2", "eventType", "break_promise", 3);
    g.add_ref("ev2", "actor", "alice", 3);
    g.set_time(10);
    // No apology between t=1 and t=3 -> match
    // #endregion

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1);
}

#[test]
fn recipe2_non_matching() {
    let pattern = recipe2_pattern();

    // #region r2_non_matching
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "promise", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.add_str("ev_apology", "eventType", "apologize", 2);
    g.add_ref("ev_apology", "actor", "alice", 2); // apology at t=2
    g.add_str("ev2", "eventType", "break_promise", 3);
    g.add_ref("ev2", "actor", "alice", 3);
    g.set_time(10);
    // Apology at t=2 is between e1 (t=1) and e2 (t=3) -> negated
    // #endregion

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 0);
}

// ── Recipe 3: Numeric threshold (edge_constrained) ─────────────────────────

#[test]
fn recipe3_matching() {
    // #region r3_pattern
    let pattern = PatternBuilder::<String, MemValue>::new("low_loyalty")
        .stage("e", |s| {
            s.edge("e", "eventType".into(), MemValue::Str("loyalty_check".into()))
                .edge_constrained(
                    "e",
                    "loyalty".into(),
                    ValueConstraint::Lt(MemValue::Num(0.5)),
                )
        })
        .build();
    // #endregion

    // #region r3_matching
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "loyalty_check", 1);
    g.add_num("ev1", "loyalty", 0.3, 1); // 0.3 < 0.5
    g.set_time(10);
    // #endregion

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1);
}

#[test]
fn recipe3_non_matching() {
    let pattern = PatternBuilder::<String, MemValue>::new("low_loyalty")
        .stage("e", |s| {
            s.edge("e", "eventType".into(), MemValue::Str("loyalty_check".into()))
                .edge_constrained(
                    "e",
                    "loyalty".into(),
                    ValueConstraint::Lt(MemValue::Num(0.5)),
                )
        })
        .build();

    // #region r3_non_matching
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "loyalty_check", 1);
    g.add_num("ev1", "loyalty", 0.8, 1); // 0.8 is NOT < 0.5
    g.set_time(10);
    // #endregion

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 0);
}

// ── Recipe 4: Overlapping events (explicit Allen constraint) ───────────────

#[test]
fn recipe4_matching() {
    // #region r4_pattern
    let pattern = PatternBuilder::<String, MemValue>::new("sortie_during_siege")
        .stage("siege", |s| {
            s.edge("siege", "eventType".into(), MemValue::Str("siege".into()))
        })
        .stage("sortie", |s| {
            s.edge("sortie", "eventType".into(), MemValue::Str("sortie".into()))
        })
        .temporal("sortie", AllenRelation::During, "siege")
        .build();
    // #endregion

    // #region r4_matching
    let mut g = MemGraph::new();
    g.add_edge_bounded("ev_siege", "eventType", MemValue::Str("siege".into()), 1, 100);
    g.add_edge_bounded("ev_sortie", "eventType", MemValue::Str("sortie".into()), 3, 5);
    g.set_time(4); // Both intervals active at t=4
    // sortie [3, 5) is During siege [1, 100) -> match
    // #endregion

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1);
}

#[test]
fn recipe4_non_matching() {
    let pattern = PatternBuilder::<String, MemValue>::new("sortie_during_siege")
        .stage("siege", |s| {
            s.edge("siege", "eventType".into(), MemValue::Str("siege".into()))
        })
        .stage("sortie", |s| {
            s.edge("sortie", "eventType".into(), MemValue::Str("sortie".into()))
        })
        .temporal("sortie", AllenRelation::During, "siege")
        .build();

    // #region r4_non_matching
    let mut g = MemGraph::new();
    g.add_edge_bounded("ev_siege", "eventType", MemValue::Str("siege".into()), 1, 4);
    g.add_edge_bounded("ev_sortie", "eventType", MemValue::Str("sortie".into()), 3, 7);
    g.set_time(3);
    // sortie [3, 7) is NOT During siege [1, 4) -- sortie extends past siege
    // The Allen relation here is OverlappedBy, not During
    // #endregion

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 0);
}

// ── Recipe 5: Absence detection (unless_after) ─────────────────────────────

fn recipe5_pattern() -> Pattern<String, MemValue> {
    // #region r5_pattern
    PatternBuilder::<String, MemValue>::new("unfulfilled_promise")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("promise".into()))
                .edge_bind("e1", "actor".into(), "person")
        })
        .unless_after("e1", |neg| {
            neg.edge("fulfillment", "eventType".into(), MemValue::Str("fulfill".into()))
                .edge_bind("fulfillment", "actor".into(), "person")
        })
        .build()
    // #endregion
}

#[test]
fn recipe5_matching() {
    let pattern = recipe5_pattern();

    // #region r5_matching
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "promise", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.set_time(10);
    // No fulfill event by alice after t=1 -> match
    // #endregion

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1);
}

#[test]
fn recipe5_non_matching() {
    let pattern = recipe5_pattern();

    // #region r5_non_matching
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "promise", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.add_str("ev2", "eventType", "fulfill", 5);
    g.add_ref("ev2", "actor", "alice", 5);
    g.set_time(10);
    // fulfill by alice at t=5, which is after promise at t=1 -> negated
    // #endregion

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 0);
}

// ── Recipe 6: Multi-clause negation (all clauses must match) ───────────────

fn recipe6_pattern() -> Pattern<String, MemValue> {
    // #region r6_pattern
    PatternBuilder::<String, MemValue>::new("kept_promise")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("promise".into()))
                .edge_bind("e1", "actor".into(), "person")
        })
        .stage("e2", |s| {
            s.edge("e2", "eventType".into(), MemValue::Str("fulfill".into()))
                .edge_bind("e2", "actor".into(), "person")
        })
        .unless_between("e1", "e2", |neg| {
            neg.edge("mid", "eventType".into(), MemValue::Str("leave".into()))
                .edge_bind("mid", "actor".into(), "person")
        })
        .build()
    // #endregion
}

#[test]
fn recipe6_matching() {
    let pattern = recipe6_pattern();

    // #region r6_matching
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "promise", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.add_str("ev_leave", "eventType", "leave", 2);
    g.add_ref("ev_leave", "actor", "bob", 2); // bob leaves, not alice
    g.add_str("ev2", "eventType", "fulfill", 3);
    g.add_ref("ev2", "actor", "alice", 3);
    g.set_time(10);
    // bob's leave does not block alice's pattern -> match
    // #endregion

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1);
}

#[test]
fn recipe6_non_matching() {
    let pattern = recipe6_pattern();

    // #region r6_non_matching
    let mut g = MemGraph::new();
    g.add_str("ev1", "eventType", "promise", 1);
    g.add_ref("ev1", "actor", "alice", 1);
    g.add_str("ev_leave", "eventType", "leave", 2);
    g.add_ref("ev_leave", "actor", "alice", 2); // alice leaves
    g.add_str("ev2", "eventType", "fulfill", 3);
    g.add_ref("ev2", "actor", "alice", 3);
    g.set_time(10);
    // alice's leave at t=2 is between t=1 and t=3, all clauses match -> negated
    // #endregion

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 0);
}

// ── Recipe 7: Cross-stage value comparison (escalation) ────────────────────

#[test]
fn recipe7_escalating_price() {
    // #region r7_pattern
    let pattern = PatternBuilder::new("escalating_price")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), MemValue::Str("order".into()))
                .edge_bind("e1", "price".into(), "base_price")
        })
        .stage("e2", |s| {
            s.edge("e2", "type".into(), MemValue::Str("order".into()))
                .edge_gt_var("e2", "price".into(), "base_price")
        })
        .build();
    // #endregion

    let mut g = MemGraph::new();
    g.add_str("o1", "type", "order", 1);
    g.add_num("o1", "price", 10.0, 1);
    g.add_str("o2", "type", "order", 2);
    g.add_num("o2", "price", 15.0, 2);
    g.set_time(10);

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1);
}

#[test]
fn recipe7_range_check() {
    // #region r7_range_check
    let pattern = PatternBuilder::new("in_range_reading")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), MemValue::Str("bounds".into()))
                .edge_bind("e1", "low".into(), "low")
                .edge_bind("e1", "high".into(), "high")
        })
        .stage("e2", |s| {
            s.edge("e2", "type".into(), MemValue::Str("reading".into()))
                .edge_gt_var("e2", "value".into(), "low")
                .edge_lt_var("e2", "value".into(), "high")
        })
        .build();
    // #endregion

    let mut g = MemGraph::new();
    g.add_str("b1", "type", "bounds", 1);
    g.add_num("b1", "low", 10.0, 1);
    g.add_num("b1", "high", 50.0, 1);
    g.add_str("r1", "type", "reading", 2);
    g.add_num("r1", "value", 30.0, 2);
    g.set_time(10);

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1);
}

// ── Recipe 8: Threshold detection with repeat range ────────────────────────

#[test]
fn recipe8_brute_force() {
    // #region r8_pattern
    let attempt = PatternBuilder::new("login_fail")
        .stage("e", |s| {
            s.edge("e", "type".into(), MemValue::Str("login_fail".into()))
                .edge_bind("e", "account".into(), "account")
        })
        .build();

    let pattern = fabula::compose::repeat_range("brute_force", &attempt, 3, None, &["account"]);
    // #endregion

    let mut g = MemGraph::new();
    for i in 1..=4 {
        g.add_str(&format!("f{i}"), "type", "login_fail", i);
        g.add_ref(&format!("f{i}"), "account", "admin", i);
    }
    g.set_time(10);

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert!(!matches.is_empty(), "should match with 4 failures (>= 3)");
}

// ── Recipe 9: Concurrent signals (unordered stages) ────────────────────────

#[test]
fn recipe9_concurrent_signals() {
    // #region r9_pattern
    let pattern = PatternBuilder::<String, MemValue>::new("multi_signal_shutdown")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), MemValue::Str("alarm".into()))
                .edge_bind("e1", "sensor".into(), "sensor")
        })
        .unordered_group(|g| {
            g.stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("temperature_spike".into()))
                    .edge_bind("e2", "sensor".into(), "sensor")
            })
            .stage("e3", |s| {
                s.edge("e3", "type".into(), MemValue::Str("pressure_drop".into()))
                    .edge_bind("e3", "sensor".into(), "sensor")
            })
        })
        .stage("e4", |s| {
            s.edge("e4", "type".into(), MemValue::Str("shutdown".into()))
                .edge_bind("e4", "sensor".into(), "sensor")
        })
        .build();
    // #endregion

    let mut g = MemGraph::new();
    g.add_str("ev1", "type", "alarm", 1);
    g.add_ref("ev1", "sensor", "s1", 1);
    // Pressure drop before temperature spike (reversed order)
    g.add_str("ev3", "type", "pressure_drop", 2);
    g.add_ref("ev3", "sensor", "s1", 2);
    g.add_str("ev2", "type", "temperature_spike", 3);
    g.add_ref("ev2", "sensor", "s1", 3);
    g.add_str("ev4", "type", "shutdown", 4);
    g.add_ref("ev4", "sensor", "s1", 4);
    g.set_time(10);

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1);
}
