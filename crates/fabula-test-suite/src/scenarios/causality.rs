//! Causal pathfinding scenarios — run against MemGraph, PetGraph, GrafeoGraph.

use crate::TestGraph;
use fabula::causality::causal_paths;
use std::collections::HashMap;

fn labels() -> HashMap<String, f64> {
    [("causes".to_string(), 1.0)].into_iter().collect()
}

/// A single causal hop: A caused B. Query B, find path [A, B].
pub fn causality_single_hop<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_ref_edge("a", "causes", "b", 1);
    let paths = causal_paths(&g, &"b".to_string(), 3, &labels());
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].nodes, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(paths[0].edges.len(), 1);
}

/// A chain A -> B -> C. Query C, find path [A, B, C].
pub fn causality_multi_hop_chain<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_ref_edge("a", "causes", "b", 1);
    g.add_ref_edge("b", "causes", "c", 2);
    let paths = causal_paths(&g, &"c".to_string(), 5, &labels());
    let root_to_tip = paths
        .iter()
        .find(|p| p.nodes.len() == 3)
        .expect("expected a 3-node path");
    assert_eq!(
        root_to_tip.nodes,
        vec!["a".to_string(), "b".to_string(), "c".to_string()]
    );
}

/// No causal edges → no paths.
pub fn causality_no_causal_edges<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_ref_edge("a", "correlated_with", "b", 1);
    let paths = causal_paths(&g, &"b".to_string(), 3, &labels());
    assert!(paths.is_empty());
}

/// max_hops limits path length.
pub fn causality_max_hops_limit<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_ref_edge("a", "causes", "b", 1);
    g.add_ref_edge("b", "causes", "c", 2);
    g.add_ref_edge("c", "causes", "d", 3);
    let paths = causal_paths(&g, &"d".to_string(), 2, &labels());
    assert!(
        paths.iter().all(|p| p.edges.len() <= 2),
        "max_hops=2 should exclude the 3-edge chain"
    );
}

/// Paths are sorted by cleanliness descending.
pub fn causality_sorted_by_cleanliness<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_ref_edge("a", "causes", "target", 1);
    g.add_ref_edge("a", "causes", "m", 1);
    g.add_ref_edge("m", "causes", "target", 5); // longer gap
    let paths = causal_paths(&g, &"target".to_string(), 5, &labels());
    for w in paths.windows(2) {
        assert!(
            w[0].cleanliness >= w[1].cleanliness,
            "paths must be sorted descending by cleanliness"
        );
    }
}

/// An event with no causal predecessors scores maximum surprise.
pub fn causality_surprise_orphan_event<G: TestGraph>() {
    let g = G::new_graph();
    let s = fabula::causality::event_causal_surprise(
        &g,
        &"orphan".to_string(),
        3,
        &labels(),
    );
    assert!((s - 1.0).abs() < 1e-9, "got {}", s);
}

/// An event with a clean single-hop cause scores minimum surprise.
pub fn causality_surprise_clean_chain<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_ref_edge("cause", "causes", "effect", 1);
    let s = fabula::causality::event_causal_surprise(
        &g,
        &"effect".to_string(),
        3,
        &labels(),
    );
    assert!(s.abs() < 1e-9, "clean chain → surprise 0.0, got {}", s);
}

/// Batch scoring returns one value per event in parallel order.
pub fn causality_surprise_batch_parallel<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_ref_edge("cause", "causes", "effect", 1);
    let events = vec!["effect".to_string(), "unrelated".to_string()];
    let scores = fabula::causality::event_causal_surprise_batch(
        &g, &events, 3, &labels(),
    );
    assert_eq!(scores.len(), 2);
    assert!(scores[0] < 0.01, "effect should be unsurprising, got {}", scores[0]);
    assert!((scores[1] - 1.0).abs() < 1e-9, "unrelated should be maximally surprising, got {}", scores[1]);
}
