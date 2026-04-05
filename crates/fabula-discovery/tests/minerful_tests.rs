use fabula::interval::Interval;
use fabula_discovery::generators::{MinerfulConfig, MinerfulGenerator};
use fabula_discovery::{CandidateGenerator, TraceCorpus};

fn make_corpus_with_clear_pattern() -> TraceCorpus {
    // Create a corpus where "trusts" is consistently followed by "betrays"
    // for the same pair of entities. This should be discoverable.
    let mut edges = Vec::new();

    // 5 instances of trusts-then-betrays for the same entity pair
    for i in 0..5 {
        let t = i * 20i64;
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
                start: t + 10,
                end: Some(t + 15),
            },
        ));
    }

    // Some noise: unrelated edges
    for i in 0..3 {
        edges.push((
            "carol".into(),
            "helps".into(),
            "dave".into(),
            Interval {
                start: (i * 30) as i64,
                end: Some((i * 30 + 5) as i64),
            },
        ));
    }

    TraceCorpus::new(edges)
}

#[test]
fn discovers_pairwise_constraints() {
    let corpus = make_corpus_with_clear_pattern();
    let mut gen = MinerfulGenerator::new(MinerfulConfig {
        min_support: 0.3,
        min_confidence: 0.5,
    });

    let candidates = gen.generate(&corpus, 10);

    // Should discover at least one pattern involving "trusts" and "betrays"
    let has_trust_betray = candidates.iter().any(|p| {
        let labels: Vec<&str> = p
            .stages
            .iter()
            .flat_map(|s| s.clauses.iter().map(|c| c.label.as_str()))
            .collect();
        labels.contains(&"trusts") && labels.contains(&"betrays")
    });

    assert!(
        has_trust_betray,
        "Should discover trusts-betrays pattern. Found {} candidates: {:?}",
        candidates.len(),
        candidates.iter().map(|p| &p.name).collect::<Vec<_>>()
    );
}

#[test]
fn respects_budget() {
    let corpus = make_corpus_with_clear_pattern();
    let mut gen = MinerfulGenerator::new(MinerfulConfig {
        min_support: 0.1,
        min_confidence: 0.1,
    });

    let candidates = gen.generate(&corpus, 3);
    assert!(
        candidates.len() <= 3,
        "Should respect budget of 3, got {}",
        candidates.len()
    );
}

#[test]
fn generated_patterns_have_temporal_constraints() {
    let corpus = make_corpus_with_clear_pattern();
    let mut gen = MinerfulGenerator::new(MinerfulConfig {
        min_support: 0.3,
        min_confidence: 0.5,
    });

    let candidates = gen.generate(&corpus, 10);

    for pattern in &candidates {
        if pattern.stages.len() > 1 {
            assert!(
                !pattern.temporal.is_empty(),
                "Multi-stage pattern '{}' should have temporal constraints",
                pattern.name
            );
        }
    }
}

#[test]
fn minerful_is_single_pass() {
    let corpus = make_corpus_with_clear_pattern();
    let mut gen = MinerfulGenerator::new(MinerfulConfig {
        min_support: 0.1,
        min_confidence: 0.1,
    });

    let first = gen.generate(&corpus, 10);
    assert!(!first.is_empty(), "First call should produce patterns");

    let second = gen.generate(&corpus, 10);
    assert!(
        second.is_empty(),
        "Second call to generate should return empty (single-pass miner)"
    );
}

#[test]
fn minerful_empty_corpus() {
    let corpus = TraceCorpus::new(vec![]);
    let mut gen = MinerfulGenerator::new(MinerfulConfig {
        min_support: 0.1,
        min_confidence: 0.1,
    });

    let candidates = gen.generate(&corpus, 10);
    assert!(
        candidates.is_empty(),
        "Empty corpus should produce no patterns"
    );
}

#[test]
fn minerful_single_label_corpus() {
    // A corpus with only one distinct label — no label pairs to mine
    let edges = vec![
        (
            "a".into(),
            "trusts".into(),
            "b".into(),
            Interval {
                start: 1i64,
                end: Some(5),
            },
        ),
        (
            "c".into(),
            "trusts".into(),
            "d".into(),
            Interval {
                start: 10,
                end: Some(15),
            },
        ),
    ];

    let corpus = TraceCorpus::new(edges);
    let mut gen = MinerfulGenerator::new(MinerfulConfig {
        min_support: 0.1,
        min_confidence: 0.1,
    });

    let candidates = gen.generate(&corpus, 10);
    assert!(
        candidates.is_empty(),
        "Single-label corpus should produce no patterns (no label pairs), got {}",
        candidates.len()
    );
}
