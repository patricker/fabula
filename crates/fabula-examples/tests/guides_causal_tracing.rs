//! End-to-end test for the "Tracing causal chains" guide.
//!
//! Verifies the code snippets in docs/docs/guides/tracing-causal-chains.md
//! actually compile and produce the expected behavior.

use fabula::causality::causal_paths;
use fabula_memory::MemGraph;
use std::collections::HashMap;

#[test]
fn guide_example_produces_sorted_paths() {
    let mut causal_labels: HashMap<String, f64> = HashMap::new();
    causal_labels.insert("causes".to_string(), 1.0);
    causal_labels.insert("enables".to_string(), 0.6);
    causal_labels.insert("escalates".to_string(), 0.8);

    let mut graph = MemGraph::new();
    graph.add_ref("insult_event", "causes", "grudge_event", 1);
    graph.add_ref("grudge_event", "causes", "betrayal_event", 2);
    graph.add_ref("failed_negotiation", "enables", "insult_event", 0);
    graph.add_ref("old_debt", "enables", "strained_alliance", -5);
    graph.add_ref("strained_alliance", "enables", "rival_encounter", -1);
    graph.add_ref("rival_encounter", "escalates", "betrayal_event", 2);

    let paths = causal_paths(&graph, &"betrayal_event".to_string(), 5, &causal_labels);

    // Every depth from 1 hop (proximate cause) to 3 hops (full root-cause chain)
    // is emitted for each of the two causal branches: six paths total.
    assert_eq!(paths.len(), 6, "expected 6 paths across 2 branches × 3 depths");

    for w in paths.windows(2) {
        assert!(w[0].cleanliness >= w[1].cleanliness);
    }

    // Top path is the proximate cause: grudge_event directly causes betrayal.
    let top = &paths[0];
    assert_eq!(top.nodes, vec!["grudge_event".to_string(), "betrayal_event".to_string()]);
    assert!((top.cleanliness - 0.500).abs() < 0.01, "top cleanliness ~0.500, got {}", top.cleanliness);

    // The full three-hop chain from the highest-weight branch is also present.
    let full_chain = paths
        .iter()
        .find(|p| p.nodes.len() == 4 && p.nodes[0] == "failed_negotiation")
        .expect("full insult→grudge→betrayal chain should be emitted");
    assert!((full_chain.cleanliness - 0.425).abs() < 0.01);
}
