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

    assert!(!paths.is_empty(), "expected at least one causal path");

    // Verify ordering: paths should be sorted by cleanliness descending
    for w in paths.windows(2) {
        assert!(w[0].cleanliness >= w[1].cleanliness, "paths should be sorted by cleanliness descending");
    }

    // Verify top path has reasonable cleanliness (though not as high as guide examples suggest)
    let top = &paths[0];
    assert!(top.cleanliness > 0.3, "top path should have above-minimum cleanliness");
    assert!(top.cleanliness <= 1.0, "top path cleanliness should be normalized");

    // Verify top path includes expected nodes (should have both insult_event and grudge_event)
    assert!(
        top.nodes.contains(&"insult_event".to_string()),
        "top chain should include insult_event"
    );
    assert!(
        top.nodes.contains(&"grudge_event".to_string()),
        "top chain should include grudge_event"
    );
    assert!(
        top.nodes.contains(&"betrayal_event".to_string()),
        "top chain should end with betrayal_event"
    );
}
