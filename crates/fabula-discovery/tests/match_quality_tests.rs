use fabula::builder::PatternBuilder;
use fabula::interval::Interval;
use fabula_discovery::evaluators::MatchQualityEvaluator;
use fabula_discovery::{PatternEvaluator, TraceCorpus};

#[test]
fn match_quality_nonmatching_scores_zero() {
    let corpus = TraceCorpus::new(vec![
        (
            "alice".into(),
            "trusts".into(),
            "bob".into(),
            Interval {
                start: 1i64,
                end: None,
            },
        ),
        (
            "bob".into(),
            "helps".into(),
            "alice".into(),
            Interval {
                start: 3,
                end: None,
            },
        ),
    ]);

    let eval = MatchQualityEvaluator;

    // Pattern with a label that doesn't exist in the corpus
    let pattern = PatternBuilder::<String, String>::new("nonexistent")
        .stage("e1", |s| {
            s.edge_bind("e1", "unknown_label".to_string(), "target")
        })
        .build();

    let score = eval.evaluate(&pattern, &corpus);
    assert_eq!(
        score, 0.0,
        "Pattern with nonexistent label should score 0.0"
    );
}

#[test]
fn match_quality_specific_scores_higher_than_general() {
    // Build a corpus with 10 "trusts" edges and 3 "betrays" edges.
    // The "trusts" pattern is overly general (match_ratio > 0.5),
    // so it should score lower than "betrays" which is more specific.
    // Use open-ended intervals so edges remain visible at evaluation time.
    let mut edges = Vec::new();
    for i in 0..10 {
        edges.push((
            format!("a{}", i),
            "trusts".into(),
            "b".into(),
            Interval {
                start: i * 10i64,
                end: None,
            },
        ));
    }
    for i in 0..3 {
        edges.push((
            format!("x{}", i),
            "betrays".into(),
            "b".into(),
            Interval {
                start: 100 + i * 10i64,
                end: None,
            },
        ));
    }

    let corpus = TraceCorpus::new(edges);
    let eval = MatchQualityEvaluator;

    let general_pattern = PatternBuilder::<String, String>::new("general")
        .stage("e1", |s| s.edge_bind("e1", "trusts".to_string(), "target"))
        .build();

    let specific_pattern = PatternBuilder::<String, String>::new("specific")
        .stage("e1", |s| s.edge_bind("e1", "betrays".to_string(), "target"))
        .build();

    let general_score = eval.evaluate(&general_pattern, &corpus);
    let specific_score = eval.evaluate(&specific_pattern, &corpus);

    assert!(
        specific_score > general_score,
        "Specific 'betrays' pattern ({}) should score higher than overly general 'trusts' pattern ({})",
        specific_score,
        general_score
    );
}
