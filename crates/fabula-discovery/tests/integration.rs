//! End-to-end: build corpus -> run discovery session -> emit DSL -> parse DSL

use fabula::interval::Interval;
use fabula_discovery::evaluators::{MatchQualityEvaluator, SurpriseEvaluator};
use fabula_discovery::generators::{MinerfulConfig, MinerfulGenerator};
use fabula_discovery::{
    pattern_to_dsl, DiscoverySession, PatternFilter, ScoredPattern, SessionConfig, TraceCorpus,
};

/// Accept patterns with any positive composite score.
struct AcceptPositive;
impl PatternFilter for AcceptPositive {
    fn accept(&self, scored: &ScoredPattern<String, String>) -> bool {
        scored.score.scores.values().any(|&v| v > 0.0)
    }
}

fn make_rich_corpus() -> TraceCorpus {
    let mut edges = Vec::new();

    // Repeated pattern: trust -> betray (5 instances, same entities)
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

    // Repeated pattern: meet -> ally (3 instances, different entities)
    for i in 0..3 {
        let t = 100 + i * 20i64;
        let src = format!("char_{}", i);
        edges.push((
            src.clone(),
            "meets".into(),
            "hero".into(),
            Interval {
                start: t,
                end: Some(t + 3),
            },
        ));
        edges.push((
            src,
            "allies_with".into(),
            "hero".into(),
            Interval {
                start: t + 5,
                end: Some(t + 8),
            },
        ));
    }

    // Noise
    for i in 0..8 {
        edges.push((
            format!("npc_{}", i),
            "wanders".into(),
            "town".into(),
            Interval {
                start: (i * 5) as i64,
                end: Some((i * 5 + 2) as i64),
            },
        ));
    }

    TraceCorpus::new(edges)
}

#[test]
fn end_to_end_discovery_and_emission() {
    let corpus = make_rich_corpus();

    let generator = MinerfulGenerator::new(MinerfulConfig {
        min_support: 0.1,
        min_confidence: 0.3,
    });

    let config = SessionConfig {
        max_rounds: 1,
        candidates_per_round: 10,
    };

    let mut session = DiscoverySession::new(config);
    let result = session.run(
        &corpus,
        generator,
        vec![Box::new(SurpriseEvaluator), Box::new(MatchQualityEvaluator)],
        AcceptPositive,
    );

    assert!(
        !result.accepted.is_empty(),
        "Should discover at least one pattern"
    );

    // Every accepted pattern should emit valid DSL
    for scored in &result.accepted {
        let dsl = pattern_to_dsl(&scored.pattern);
        let parsed = fabula_dsl::parse_document(&dsl);
        assert!(
            parsed.is_ok(),
            "Pattern '{}' emitted invalid DSL:\n{}\nError: {}",
            scored.pattern.name,
            dsl,
            parsed.unwrap_err()
        );
    }

    // Session history should be complete
    assert!(result.all_scored.len() >= result.accepted.len());
    assert!(result.rounds == 1);
}

#[test]
fn discovered_patterns_match_corpus() {
    // Use the same rich corpus (bounded intervals) so MINERful can compute
    // Allen relations. For MemGraph evaluation, add edges as open-ended so
    // they remain visible at ds.now() -- evaluate_pattern is a snapshot query.
    let corpus = make_rich_corpus();

    let generator = MinerfulGenerator::new(MinerfulConfig {
        min_support: 0.1,
        min_confidence: 0.3,
    });

    let config = SessionConfig {
        max_rounds: 1,
        candidates_per_round: 10,
    };

    let mut session = DiscoverySession::new(config);
    let result = session.run(
        &corpus,
        generator,
        vec![Box::new(SurpriseEvaluator)],
        AcceptPositive,
    );

    assert!(
        !result.accepted.is_empty(),
        "Should discover at least one pattern"
    );

    // Build a MemGraph with open-ended intervals so all edges are visible
    // at evaluation time (evaluate_pattern uses ds.now() as the snapshot).
    let graph = {
        use fabula_memory::{MemGraph, MemValue};
        let mut g = MemGraph::new();
        let (_, max_t) = corpus.time_range();
        g.set_time(max_t);
        for edge in corpus.edges() {
            let value = MemValue::Node(edge.target.clone());
            // Use open-ended intervals so edges remain visible at now()
            g.add_edge(&edge.source, &edge.label, value, edge.interval.start);
        }
        g
    };

    // At least one discovered pattern should actually produce matches
    // when evaluated against the MemGraph
    let any_matches = result.accepted.iter().any(|scored| {
        use fabula_memory::MemValue;
        // Convert Pattern<String, String> to Pattern<String, MemValue> for MemGraph
        let mem_pattern = scored
            .pattern
            .map_types(|l| l.clone(), |v| MemValue::Node(v.clone()));
        let matches = fabula::engine::evaluate_pattern(&graph, &mem_pattern, &fabula::engine::DefaultLetEvaluator);
        !matches.is_empty()
    });

    assert!(
        any_matches,
        "At least one discovered pattern should match the corpus"
    );
}
