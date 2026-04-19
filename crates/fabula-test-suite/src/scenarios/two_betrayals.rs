//! Two impulsive betrayals pattern -- character with trait "impulsive" betrays twice.
//!
//! Pattern: stage1 (eventType=betray, actor=?char, ?char has trait=impulsive),
//! stage2 (eventType=betray, actor=?char), unless_global (any action by ?char between).

use crate::TestGraph;
use fabula::prelude::*;

/// Build the two-impulsive-betrayals pattern.
fn two_betrayals_pattern<G: TestGraph>() -> Pattern<String, G::V> {
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
        .unless_global(|neg| {
            neg.edge("mid", "eventType".into(), G::str_val("reconcile"))
                .edge_bind("mid", "actor".into(), "char")
        })
        .build()
}

/// Batch: two betrayals by an impulsive character match.
pub fn batch_two_betrayals_match<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("alice", "trait", "impulsive", 0);
    g.add_str_edge("ev1", "eventType", "betray", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev2", "eventType", "betray", 3);
    g.add_ref_edge("ev2", "actor", "alice", 3);
    g.set_current_time(10);

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(two_betrayals_pattern::<G>());
    let matches = engine.evaluate(&g);
    assert_eq!(
        matches.len(),
        1,
        "should match two betrayals by impulsive character"
    );
    assert!(G::is_node_eq(&matches[0].bindings["char"], "alice"));
}

/// Batch: intervening reconcile blocks via unless_global.
pub fn batch_two_betrayals_intervening_blocks<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("alice", "trait", "impulsive", 0);
    g.add_str_edge("ev1", "eventType", "betray", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev_mid", "eventType", "reconcile", 2);
    g.add_ref_edge("ev_mid", "actor", "alice", 2);
    g.add_str_edge("ev2", "eventType", "betray", 3);
    g.add_ref_edge("ev2", "actor", "alice", 3);
    g.set_current_time(10);

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(two_betrayals_pattern::<G>());
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "reconcile between betrayals should block"
    );
}

/// Batch: other actor's reconcile does not block.
pub fn batch_two_betrayals_other_actor_doesnt_block<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("alice", "trait", "impulsive", 0);
    g.add_str_edge("ev1", "eventType", "betray", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev_mid", "eventType", "reconcile", 2);
    g.add_ref_edge("ev_mid", "actor", "bob", 2);
    g.add_str_edge("ev2", "eventType", "betray", 3);
    g.add_ref_edge("ev2", "actor", "alice", 3);
    g.set_current_time(10);

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(two_betrayals_pattern::<G>());
    assert_eq!(
        engine.evaluate(&g).len(),
        1,
        "bob's reconcile should not block alice's pattern"
    );
}

/// Batch: non-impulsive character does not match.
pub fn batch_two_betrayals_non_impulsive_no_match<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("alice", "trait", "cautious", 0);
    g.add_str_edge("ev1", "eventType", "betray", 1);
    g.add_ref_edge("ev1", "actor", "alice", 1);
    g.add_str_edge("ev2", "eventType", "betray", 3);
    g.add_ref_edge("ev2", "actor", "alice", 3);
    g.set_current_time(10);

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(two_betrayals_pattern::<G>());
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "cautious character should not match impulsive pattern"
    );
}
