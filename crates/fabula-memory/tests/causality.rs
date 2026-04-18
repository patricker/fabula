use fabula::causality::causal_paths;
use fabula_memory::MemGraph;
use std::collections::HashMap;

fn labels() -> HashMap<String, f64> {
    [("causes".to_string(), 1.0)].into_iter().collect()
}

#[test]
fn memgraph_single_hop() {
    let mut g = MemGraph::new();
    g.add_ref("a", "causes", "b", 1);
    let paths = causal_paths(&g, &"b".to_string(), 3, &labels());
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].nodes, vec!["a".to_string(), "b".to_string()]);
    assert!(paths[0].cleanliness > 0.9);
}

#[test]
fn memgraph_two_paths_sorted_by_cleanliness() {
    // Two routes to 'c': a -> c (direct, weight 1.0) and a -> b -> c (longer)
    let mut g = MemGraph::new();
    g.add_ref("a", "causes", "c", 1);
    g.add_ref("a", "causes", "b", 1);
    g.add_ref("b", "causes", "c", 2);
    let paths = causal_paths(&g, &"c".to_string(), 5, &labels());
    assert!(paths.len() >= 2);
    for w in paths.windows(2) {
        assert!(w[0].cleanliness >= w[1].cleanliness);
    }
}

#[test]
fn memgraph_weighted_labels() {
    let mut labels = HashMap::new();
    labels.insert("strongly_causes".to_string(), 1.0);
    labels.insert("weakly_suggests".to_string(), 0.3);

    let mut g = MemGraph::new();
    g.add_ref("a", "strongly_causes", "c", 1);
    g.add_ref("a2", "weakly_suggests", "c", 1);
    let paths = causal_paths(&g, &"c".to_string(), 3, &labels);
    assert_eq!(paths.len(), 2);
    let strong = paths.iter().find(|p| p.nodes[0] == "a").unwrap();
    let weak = paths.iter().find(|p| p.nodes[0] == "a2").unwrap();
    assert!(strong.cleanliness > weak.cleanliness);
}

#[test]
fn memgraph_respects_temporal_order() {
    // a -> b at time 5, b -> c at time 3 — out of order, should not form a chain
    let mut g = MemGraph::new();
    g.add_ref("a", "causes", "b", 5);
    g.add_ref("b", "causes", "c", 3);
    let paths = causal_paths(&g, &"c".to_string(), 5, &labels());
    // Only b → c should appear as a valid chain; a → b → c fails temporal validation
    assert!(
        paths.iter().all(|p| p.nodes[0] != "a"),
        "path from 'a' should not exist due to temporal violation"
    );
}

#[test]
fn memgraph_cycle_does_not_loop() {
    let mut g = MemGraph::new();
    g.add_ref("a", "causes", "b", 1);
    g.add_ref("b", "causes", "a", 2);
    // Finite traversal despite the cycle
    let paths = causal_paths(&g, &"b".to_string(), 10, &labels());
    assert!(paths.len() < 100, "cycle should not produce explosion");
}

#[test]
fn memgraph_confidence_is_weakest_link() {
    // Mixed-weight chain: confidence should be the minimum weight, not the mean.
    let mut labels = HashMap::new();
    labels.insert("strongly_causes".to_string(), 1.0);
    labels.insert("weakly_suggests".to_string(), 0.3);

    let mut g = MemGraph::new();
    g.add_ref("a", "strongly_causes", "b", 1);
    g.add_ref("b", "weakly_suggests", "c", 2);
    let paths = causal_paths(&g, &"c".to_string(), 5, &labels);
    let full = paths
        .iter()
        .find(|p| p.nodes.len() == 3)
        .expect("expected the a->b->c chain");
    // Weakest link is 0.3 (weakly_suggests), not the mean of 0.65.
    assert!((full.confidence - 0.3).abs() < 1e-9, "got {}", full.confidence);
}

#[test]
fn memgraph_empty_graph_empty_result() {
    let g = MemGraph::new();
    let paths = causal_paths(&g, &"anything".to_string(), 3, &labels());
    assert!(paths.is_empty());
}

#[test]
fn datasource_predecessors_default_impl() {
    // Exercise the DataSource::predecessors default implementation directly
    // (via MemGraph, which doesn't override it) to confirm it returns the
    // correct set of reverse edges.
    use fabula::datasource::DataSource;

    let mut g = MemGraph::new();
    g.add_ref("a", "causes", "target", 1);
    g.add_ref("b", "causes", "target", 2);
    g.add_ref("c", "causes", "unrelated", 3);
    g.add_ref("d", "enables", "target", 4);

    let preds = g.predecessors(&"target".to_string(), &"causes".to_string());
    assert_eq!(preds.len(), 2, "two edges labeled 'causes' point at 'target'");
    let sources: Vec<String> = preds.into_iter().map(|e| e.source).collect();
    assert!(sources.contains(&"a".to_string()));
    assert!(sources.contains(&"b".to_string()));

    // Different label returns only that label's edges.
    let enables = g.predecessors(&"target".to_string(), &"enables".to_string());
    assert_eq!(enables.len(), 1);
    assert_eq!(enables[0].source, "d");

    // Node with no incoming edges returns empty.
    let empty = g.predecessors(&"nobody".to_string(), &"causes".to_string());
    assert!(empty.is_empty());
}

#[test]
fn memgraph_surprise_no_cause() {
    let g = MemGraph::new();
    let s = fabula::causality::event_causal_surprise(
        &g,
        &"orphan_event".to_string(),
        3,
        &labels(),
    );
    assert!((s - 1.0).abs() < 1e-9);
}

#[test]
fn memgraph_surprise_clean_chain_is_low() {
    let mut g = MemGraph::new();
    g.add_ref("cause", "causes", "effect", 1);
    let s = fabula::causality::event_causal_surprise(
        &g,
        &"effect".to_string(),
        3,
        &labels(),
    );
    // Single pred, weight 1.0, no gap, no divergence → cleanliness 1.0, surprise 0.0.
    assert!(s.abs() < 1e-9, "got {}", s);
}

#[test]
fn memgraph_surprise_batch_matches_individual_calls() {
    let mut g = MemGraph::new();
    g.add_ref("a", "causes", "b", 1);
    let events = vec![
        "b".to_string(),
        "unrelated".to_string(),
    ];
    let batch = fabula::causality::event_causal_surprise_batch(
        &g, &events, 3, &labels(),
    );
    let individual: Vec<f64> = events
        .iter()
        .map(|e| fabula::causality::event_causal_surprise(&g, e, 3, &labels()))
        .collect();
    assert_eq!(batch, individual);
}
