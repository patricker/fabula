//! End-to-end test for the "Detecting surprising events" guide.
//!
//! Verifies the score thresholds documented in section 6 of
//! docs/docs/guides/detecting-surprising-events.md are accurate for a
//! representative graph.

use fabula::causality::{event_causal_surprise, event_causal_surprise_batch};
use fabula_memory::MemGraph;
use std::collections::HashMap;

fn causal_labels() -> HashMap<String, f64> {
    let mut m = HashMap::new();
    m.insert("causes".to_string(), 1.0);
    m.insert("enables".to_string(), 0.6);
    m
}

#[test]
fn guide_clean_chain_is_low_surprise() {
    let mut g = MemGraph::new();
    g.add_ref("grudge", "causes", "betrayal", 2);
    let s = event_causal_surprise(&g, &"betrayal".to_string(), 5, &causal_labels());
    assert!(s < 0.25, "clean chain should be low surprise, got {}", s);
}

#[test]
fn guide_no_cause_is_max_surprise() {
    let g = MemGraph::new();
    let s = event_causal_surprise(&g, &"out_of_nowhere".to_string(), 5, &causal_labels());
    assert!((s - 1.0).abs() < 1e-9);
}

#[test]
fn guide_weak_cause_is_medium_surprise() {
    let mut g = MemGraph::new();
    g.add_ref("rumor", "enables", "betrayal", 2);
    let s = event_causal_surprise(&g, &"betrayal".to_string(), 5, &causal_labels());
    // weight 0.6, single predecessor, gap 0 → cleanliness = 0.6, surprise = 0.4.
    assert!((s - 0.4).abs() < 0.01, "weak cause → surprise ~0.4, got {}", s);
}

#[test]
fn guide_divergent_causes_produce_half_surprise() {
    // Two full-weight causes for the same effect: each path carries a
    // divergence penalty of 1, so cleanliness = 1.0 × 1 × 1/(1+1) = 0.5
    // and surprise = 0.5. Pins the divergence bucket advertised in the
    // guide's worked-examples block.
    let mut g = MemGraph::new();
    g.add_ref("cause_a", "causes", "effect", 1);
    g.add_ref("cause_b", "causes", "effect", 1);
    let s = event_causal_surprise(&g, &"effect".to_string(), 5, &causal_labels());
    assert!((s - 0.5).abs() < 0.01, "divergent causes → surprise ~0.5, got {}", s);
}

#[test]
fn guide_batch_call_works_on_tick_worth_of_events() {
    let mut g = MemGraph::new();
    g.add_ref("grudge", "causes", "betrayal", 2);
    g.add_ref("rumor", "enables", "suspicion", 1);

    let events = vec![
        "betrayal".to_string(),
        "suspicion".to_string(),
        "orphaned_event".to_string(),
    ];
    let scores = event_causal_surprise_batch(&g, &events, 5, &causal_labels());

    assert_eq!(scores.len(), 3);
    assert!(scores[0] < 0.25, "betrayal: clean");
    assert!((scores[1] - 0.4).abs() < 0.01, "suspicion: weak cause");
    assert!((scores[2] - 1.0).abs() < 1e-9, "orphan: max surprise");
}
