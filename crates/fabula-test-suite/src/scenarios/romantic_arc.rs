//! Romantic arc -- three-stage tag-based pattern from Winnow.
//!
//! Two negative romantic events then one positive, all by the same character.

use crate::TestGraph;
use fabula::prelude::*;

fn romantic_pattern<G: TestGraph>() -> Pattern<String, G::V> {
    PatternBuilder::new("romantic_arc")
        .stage("e1", |s| {
            s.edge("e1", "tag".into(), G::str_val("negative"))
                .edge("e1", "tag".into(), G::str_val("romantic"))
                .edge_bind("e1", "actor".into(), "char")
        })
        .stage("e2", |s| {
            s.edge("e2", "tag".into(), G::str_val("negative"))
                .edge("e2", "tag".into(), G::str_val("romantic"))
                .edge_bind("e2", "actor".into(), "char")
        })
        .stage("e3", |s| {
            s.edge("e3", "tag".into(), G::str_val("positive"))
                .edge("e3", "tag".into(), G::str_val("romantic"))
                .edge_bind("e3", "actor".into(), "char")
        })
        .build()
}

/// Batch: romantic arc matches when same character has neg, neg, pos.
pub fn batch_romantic_arc_matches<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("r1", "tag", "negative", 1);
    g.add_str_edge("r1", "tag", "romantic", 1);
    g.add_ref_edge("r1", "actor", "mira", 1);
    g.add_str_edge("r2", "tag", "negative", 2);
    g.add_str_edge("r2", "tag", "romantic", 2);
    g.add_ref_edge("r2", "actor", "mira", 2);
    g.add_str_edge("r3", "tag", "positive", 3);
    g.add_str_edge("r3", "tag", "romantic", 3);
    g.add_ref_edge("r3", "actor", "mira", 3);
    g.set_current_time(10);

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(romantic_pattern::<G>());
    let matches = engine.evaluate(&g);
    assert_eq!(matches.len(), 1);
    assert!(G::is_node_eq(&matches[0].bindings["char"], "mira"));
}

/// Batch: inline not_edge in stage 1 -- event WITH tag=major -> no match, without -> match.
pub fn batch_romantic_arc_inline_negation<G: TestGraph>() {
    // Pattern: stage 1 has not_edge for tag=major
    let pattern = PatternBuilder::new("romantic_arc_no_major")
        .stage("e1", |s| {
            s.edge("e1", "tag".into(), G::str_val("negative"))
                .edge("e1", "tag".into(), G::str_val("romantic"))
                .edge_bind("e1", "actor".into(), "char")
                .not_edge("e1", "tag".into(), G::str_val("major"))
        })
        .stage("e2", |s| {
            s.edge("e2", "tag".into(), G::str_val("negative"))
                .edge("e2", "tag".into(), G::str_val("romantic"))
                .edge_bind("e2", "actor".into(), "char")
        })
        .stage("e3", |s| {
            s.edge("e3", "tag".into(), G::str_val("positive"))
                .edge("e3", "tag".into(), G::str_val("romantic"))
                .edge_bind("e3", "actor".into(), "char")
        })
        .build();

    // Case 1: r1 has tag=major -> no match (negated clause fires)
    let mut g = G::new_graph();
    g.add_str_edge("r1", "tag", "negative", 1);
    g.add_str_edge("r1", "tag", "romantic", 1);
    g.add_str_edge("r1", "tag", "major", 1);
    g.add_ref_edge("r1", "actor", "mira", 1);
    g.add_str_edge("r2", "tag", "negative", 2);
    g.add_str_edge("r2", "tag", "romantic", 2);
    g.add_ref_edge("r2", "actor", "mira", 2);
    g.add_str_edge("r3", "tag", "positive", 3);
    g.add_str_edge("r3", "tag", "romantic", 3);
    g.add_ref_edge("r3", "actor", "mira", 3);
    g.set_current_time(10);

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "r1 has tag=major -> negated clause blocks stage 1"
    );

    // Case 2: no tag=major -> match
    let pattern2 = PatternBuilder::new("romantic_arc_no_major")
        .stage("e1", |s| {
            s.edge("e1", "tag".into(), G::str_val("negative"))
                .edge("e1", "tag".into(), G::str_val("romantic"))
                .edge_bind("e1", "actor".into(), "char")
                .not_edge("e1", "tag".into(), G::str_val("major"))
        })
        .stage("e2", |s| {
            s.edge("e2", "tag".into(), G::str_val("negative"))
                .edge("e2", "tag".into(), G::str_val("romantic"))
                .edge_bind("e2", "actor".into(), "char")
        })
        .stage("e3", |s| {
            s.edge("e3", "tag".into(), G::str_val("positive"))
                .edge("e3", "tag".into(), G::str_val("romantic"))
                .edge_bind("e3", "actor".into(), "char")
        })
        .build();

    let mut g2 = G::new_graph();
    g2.add_str_edge("r1", "tag", "negative", 1);
    g2.add_str_edge("r1", "tag", "romantic", 1);
    g2.add_ref_edge("r1", "actor", "mira", 1);
    g2.add_str_edge("r2", "tag", "negative", 2);
    g2.add_str_edge("r2", "tag", "romantic", 2);
    g2.add_ref_edge("r2", "actor", "mira", 2);
    g2.add_str_edge("r3", "tag", "positive", 3);
    g2.add_str_edge("r3", "tag", "romantic", 3);
    g2.add_ref_edge("r3", "actor", "mira", 3);
    g2.set_current_time(10);

    let mut engine2: SiftEngineFor<G> = SiftEngine::new();
    engine2.register(pattern2);
    assert_eq!(
        engine2.evaluate(&g2).len(),
        1,
        "no tag=major -> should match"
    );
}

/// Batch: 3 negative + 1 positive romantic events -> multiple matches from different combinations.
pub fn batch_romantic_arc_combinatorial<G: TestGraph>() {
    let mut g = G::new_graph();
    // Three negative romantic events
    g.add_str_edge("r1", "tag", "negative", 1);
    g.add_str_edge("r1", "tag", "romantic", 1);
    g.add_ref_edge("r1", "actor", "mira", 1);
    g.add_str_edge("r2", "tag", "negative", 2);
    g.add_str_edge("r2", "tag", "romantic", 2);
    g.add_ref_edge("r2", "actor", "mira", 2);
    g.add_str_edge("r3", "tag", "negative", 3);
    g.add_str_edge("r3", "tag", "romantic", 3);
    g.add_ref_edge("r3", "actor", "mira", 3);
    // One positive romantic event
    g.add_str_edge("r4", "tag", "positive", 4);
    g.add_str_edge("r4", "tag", "romantic", 4);
    g.add_ref_edge("r4", "actor", "mira", 4);
    g.set_current_time(10);

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(romantic_pattern::<G>());
    let matches = engine.evaluate(&g);
    // Combinations of 2 negatives from {r1,r2,r3} + r4 positive:
    // (r1,r2,r4), (r1,r3,r4), (r2,r3,r4) = 3 matches
    assert!(
        matches.len() >= 2,
        "should have multiple matches from combinatorial negatives, got {}",
        matches.len()
    );
}

/// Batch: different characters across stages means no match.
pub fn batch_romantic_arc_different_characters<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("r1", "tag", "negative", 1);
    g.add_str_edge("r1", "tag", "romantic", 1);
    g.add_ref_edge("r1", "actor", "mira", 1);
    g.add_str_edge("r2", "tag", "negative", 2);
    g.add_str_edge("r2", "tag", "romantic", 2);
    g.add_ref_edge("r2", "actor", "kaelen", 2); // different character
    g.add_str_edge("r3", "tag", "positive", 3);
    g.add_str_edge("r3", "tag", "romantic", 3);
    g.add_ref_edge("r3", "actor", "mira", 3);
    g.set_current_time(10);

    let mut engine: SiftEngineFor<G> = SiftEngine::new();
    engine.register(romantic_pattern::<G>());
    assert_eq!(
        engine.evaluate(&g).len(),
        0,
        "different actors should not match"
    );
}
