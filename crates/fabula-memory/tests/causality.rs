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
