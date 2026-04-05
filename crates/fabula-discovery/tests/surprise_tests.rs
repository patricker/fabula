use fabula::builder::PatternBuilder;
use fabula::interval::Interval;
use fabula_discovery::evaluators::SurpriseEvaluator;
use fabula_discovery::{PatternEvaluator, TraceCorpus};

fn make_corpus() -> TraceCorpus {
    // 10 "trusts" edges, 2 "betrays" edges, 1 "trusts then betrays" co-occurrence
    // "betrays" is rare, so a pattern matching it should score higher
    let mut edges = Vec::new();
    for i in 0..10 {
        edges.push((
            format!("a{}", i),
            "trusts".into(),
            "b".into(),
            Interval {
                start: i * 10i64,
                end: Some(i * 10 + 5),
            },
        ));
    }
    edges.push((
        "x".into(),
        "betrays".into(),
        "b".into(),
        Interval {
            start: 50,
            end: Some(55),
        },
    ));
    edges.push((
        "y".into(),
        "betrays".into(),
        "b".into(),
        Interval {
            start: 70,
            end: Some(75),
        },
    ));
    TraceCorpus::new(edges)
}

#[test]
fn rare_label_scores_higher() {
    let corpus = make_corpus();
    let eval = SurpriseEvaluator;

    let common_pattern = PatternBuilder::<String, String>::new("common")
        .stage("e1", |s| s.edge_bind("e1", "trusts".to_string(), "target"))
        .build();

    let rare_pattern = PatternBuilder::<String, String>::new("rare")
        .stage("e1", |s| s.edge_bind("e1", "betrays".to_string(), "target"))
        .build();

    let common_score = eval.evaluate(&common_pattern, &corpus);
    let rare_score = eval.evaluate(&rare_pattern, &corpus);

    assert!(
        rare_score > common_score,
        "rare pattern ({}) should score higher than common pattern ({})",
        rare_score,
        common_score
    );
}

#[test]
fn empty_match_scores_zero() {
    let corpus = make_corpus();
    let eval = SurpriseEvaluator;

    let no_match = PatternBuilder::<String, String>::new("nonexistent")
        .stage("e1", |s| {
            s.edge_bind("e1", "unknown_label".to_string(), "target")
        })
        .build();

    let score = eval.evaluate(&no_match, &corpus);
    assert_eq!(score, 0.0, "no-match pattern should score 0");
}

#[test]
fn multi_stage_pattern_scores_nonzero() {
    // Create a corpus where two labels co-occur on shared nodes.
    // A two-stage pattern with those labels should score nonzero.
    let mut edges = Vec::new();
    for i in 0..5 {
        let t = i * 10i64;
        edges.push((
            "alice".into(),
            "trusts".into(),
            "bob".into(),
            Interval {
                start: t,
                end: Some(t + 5),
            },
        ));
        edges.push((
            "alice".into(),
            "betrays".into(),
            "bob".into(),
            Interval {
                start: t + 6,
                end: Some(t + 9),
            },
        ));
    }

    let corpus = TraceCorpus::new(edges);
    let eval = SurpriseEvaluator;

    let two_stage = PatternBuilder::<String, String>::new("two_stage")
        .stage("e1", |s| s.edge_bind("e1", "trusts".to_string(), "target"))
        .stage("e2", |s| s.edge_bind("e2", "betrays".to_string(), "target"))
        .build();

    let score = eval.evaluate(&two_stage, &corpus);
    assert!(
        score > 0.0,
        "Two-stage pattern with co-occurring labels should score nonzero, got {}",
        score
    );
}

#[test]
fn surprise_empty_corpus_returns_zero() {
    let corpus = TraceCorpus::new(vec![]);
    let eval = SurpriseEvaluator;

    let pattern = PatternBuilder::<String, String>::new("anything")
        .stage("e1", |s| s.edge_bind("e1", "trusts".to_string(), "target"))
        .build();

    let score = eval.evaluate(&pattern, &corpus);
    assert_eq!(
        score, 0.0,
        "Empty corpus should yield score 0.0, got {}",
        score
    );
}
