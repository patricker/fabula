//! Verifies the multi-tag event modeling convention end-to-end:
//! - MemGraph::add_event(id, type, tags, time) attaches one eventType edge
//!   and one "tag" edge per tag value.
//! - A pattern with `e1.tag = "harm"` matches any event whose tag set
//!   contains "harm", regardless of eventType.

use fabula::engine::{evaluate_pattern, DefaultLetEvaluator};
use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};
use std::collections::HashSet;

fn extract_event_id(m: &Match<String, MemValue, i64>) -> String {
    // The stage anchor "e1" holds the matched event node id.
    match m.bindings.get("e1") {
        Some(BoundValue::Node(n)) => n.clone(),
        _ => panic!("expected e1 bound to a node, got {:?}", m.bindings),
    }
}

#[test]
fn add_event_attaches_eventtype_and_tag_edges() {
    let mut g = MemGraph::new();
    g.add_event("ev1", "attack", &["violent", "harm", "physical"], 1);
    g.set_time(2);

    // The eventType edge exists.
    let eventtype_edges = g.edges_from(&"ev1".to_string(), &"eventType".to_string(), &2);
    assert_eq!(eventtype_edges.len(), 1);
    assert_eq!(eventtype_edges[0].target, MemValue::Str("attack".into()));

    // Three tag edges exist, one per tag.
    let tag_edges = g.edges_from(&"ev1".to_string(), &"tag".to_string(), &2);
    let tag_values: HashSet<_> = tag_edges
        .iter()
        .filter_map(|e| match &e.target {
            MemValue::Str(s) => Some(s.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(tag_values.len(), 3);
    assert!(tag_values.contains("violent"));
    assert!(tag_values.contains("harm"));
    assert!(tag_values.contains("physical"));
}

#[test]
fn polymorphic_tag_pattern_matches_across_event_types() {
    let mut g = MemGraph::new();
    g.add_event("ev1", "attack", &["violent", "harm", "physical"], 1);
    g.add_event("ev2", "betrayal", &["harm", "social"], 5);
    g.add_event("ev3", "scolding", &["harm", "social"], 10);
    g.add_event("ev4", "compliment", &["social"], 15);
    g.set_time(20);

    // Pattern: stage with single clause `e1.tag = "harm"`.
    let pattern: Pattern<String, MemValue> = PatternBuilder::new("any_harm")
        .stage("e1", |s| {
            s.edge("e1", "tag".to_string(), MemValue::Str("harm".into()))
        })
        .build();

    let matches = evaluate_pattern(&g, &pattern, &DefaultLetEvaluator);

    let event_ids: HashSet<String> = matches.iter().map(extract_event_id).collect();
    assert_eq!(event_ids.len(), 3, "expected 3 matches; got {:?}", event_ids);
    assert!(event_ids.contains("ev1"));
    assert!(event_ids.contains("ev2"));
    assert!(event_ids.contains("ev3"));
    assert!(!event_ids.contains("ev4"));
}

#[test]
fn polymorphic_tag_disjunction_matches_any_listed_tag() {
    let mut g = MemGraph::new();
    g.add_event("ev1", "attack", &["violent", "harm"], 1);
    g.add_event("ev2", "trade", &["commercial"], 5);
    g.add_event("ev3", "duel", &["violent", "ritual"], 10);
    g.set_time(20);

    // Pattern: violent OR ritual via OneOf constraint.
    let pattern: Pattern<String, MemValue> = PatternBuilder::new("dramatic")
        .stage("e1", |s| {
            s.edge_constrained(
                "e1",
                "tag".to_string(),
                fabula::datasource::ValueConstraint::OneOf(vec![
                    MemValue::Str("violent".into()),
                    MemValue::Str("ritual".into()),
                ]),
            )
        })
        .build();

    let matches = evaluate_pattern(&g, &pattern, &DefaultLetEvaluator);
    let event_ids: HashSet<String> = matches.iter().map(extract_event_id).collect();
    assert!(event_ids.contains("ev1"));
    assert!(!event_ids.contains("ev2"));
    assert!(event_ids.contains("ev3"));
}
