//! Verify SiftEngine::pattern_importance_map() exposes registered patterns'
//! importance values for use with fabula_narratives::assemble_signals_weighted.

use fabula::engine::DefaultLetEvaluator;
use fabula::prelude::*;

#[test]
fn empty_engine_returns_empty_map() {
    let engine: SiftEngine<String, String, String, i64, DefaultLetEvaluator> =
        SiftEngine::new(DefaultLetEvaluator);
    let map = engine.pattern_importance_map();
    assert!(map.is_empty());
}

#[test]
fn map_contains_registered_patterns_with_default_importance() {
    let mut engine: SiftEngine<String, String, String, i64, DefaultLetEvaluator> =
        SiftEngine::new(DefaultLetEvaluator);

    let p1: Pattern<String, String> = PatternBuilder::new("alpha")
        .stage("e1", |s| s.edge("e1", "label".to_string(), "value".to_string()))
        .build();
    engine.register(p1);

    let map = engine.pattern_importance_map();
    assert_eq!(map.len(), 1);
    assert_eq!(map.get("alpha").copied(), Some(1.0)); // default
}

#[test]
fn map_reflects_explicit_importance_values() {
    let mut engine: SiftEngine<String, String, String, i64, DefaultLetEvaluator> =
        SiftEngine::new(DefaultLetEvaluator);

    let climax: Pattern<String, String> = PatternBuilder::new("climax")
        .stage("e1", |s| s.edge("e1", "l".to_string(), "v".to_string()))
        .importance(10.0)
        .build();
    let side_quest: Pattern<String, String> = PatternBuilder::new("side_quest")
        .stage("e1", |s| s.edge("e1", "l".to_string(), "v".to_string()))
        .importance(0.5)
        .build();

    engine.register(climax);
    engine.register(side_quest);

    let map = engine.pattern_importance_map();
    assert_eq!(map.len(), 2);
    assert_eq!(map.get("climax").copied(), Some(10.0));
    assert_eq!(map.get("side_quest").copied(), Some(0.5));
}

#[test]
fn map_excludes_private_patterns() {
    let mut engine: SiftEngine<String, String, String, i64, DefaultLetEvaluator> =
        SiftEngine::new(DefaultLetEvaluator);

    let public: Pattern<String, String> = PatternBuilder::new("public")
        .stage("e1", |s| s.edge("e1", "l".to_string(), "v".to_string()))
        .build();
    let mut private_pat: Pattern<String, String> = PatternBuilder::new("private")
        .stage("e1", |s| s.edge("e1", "l".to_string(), "v".to_string()))
        .build();
    private_pat.private = true;

    engine.register(public);
    engine.register(private_pat);

    let map = engine.pattern_importance_map();
    assert_eq!(map.len(), 1);
    assert!(map.contains_key("public"));
    assert!(!map.contains_key("private"));
}
