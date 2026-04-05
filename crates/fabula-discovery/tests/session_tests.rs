use fabula::interval::Interval;
use fabula::pattern::Pattern;
use fabula_discovery::{
    CandidateGenerator, DiscoverySession, PatternEvaluator, PatternFilter, ScoredPattern,
    SessionConfig, TraceCorpus,
};

/// A dummy generator that emits a fixed pattern each round.
struct FixedGenerator {
    pattern: Pattern<String, String>,
    feedback_count: usize,
}

impl CandidateGenerator for FixedGenerator {
    fn generate(&mut self, _corpus: &TraceCorpus, budget: usize) -> Vec<Pattern<String, String>> {
        vec![self.pattern.clone(); budget.min(1)]
    }

    fn feedback(&mut self, _scored: &[ScoredPattern<String, String>]) {
        self.feedback_count += 1;
    }

    fn name(&self) -> &str {
        "fixed"
    }
}

/// A dummy evaluator that always returns 0.8.
struct ConstantEvaluator;

impl PatternEvaluator for ConstantEvaluator {
    fn evaluate(&self, _pattern: &Pattern<String, String>, _corpus: &TraceCorpus) -> f64 {
        0.8
    }

    fn name(&self) -> &str {
        "constant"
    }
}

/// Accept everything.
struct AcceptAll;

impl PatternFilter for AcceptAll {
    fn accept(&self, _scored: &ScoredPattern<String, String>) -> bool {
        true
    }
}

fn make_corpus() -> TraceCorpus {
    TraceCorpus::new(vec![
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
            "b".into(),
            "betrays".into(),
            "a".into(),
            Interval {
                start: 3,
                end: None,
            },
        ),
    ])
}

fn make_pattern() -> Pattern<String, String> {
    use fabula::builder::PatternBuilder;
    PatternBuilder::new("test_pattern")
        .stage("e1", |s| s.edge_bind("e1", "trusts".to_string(), "target"))
        .build()
}

#[test]
fn session_runs_configured_rounds() {
    let corpus = make_corpus();
    let generator = FixedGenerator {
        pattern: make_pattern(),
        feedback_count: 0,
    };
    let config = SessionConfig {
        max_rounds: 3,
        candidates_per_round: 2,
    };

    let mut session = DiscoverySession::new(config);
    let result = session.run(
        &corpus,
        generator,
        vec![Box::new(ConstantEvaluator)],
        AcceptAll,
    );

    assert_eq!(result.rounds, 3);
    assert!(!result.accepted.is_empty());
}

#[test]
fn session_history_tracks_all_candidates() {
    let corpus = make_corpus();
    let generator = FixedGenerator {
        pattern: make_pattern(),
        feedback_count: 0,
    };
    let config = SessionConfig {
        max_rounds: 2,
        candidates_per_round: 1,
    };

    let mut session = DiscoverySession::new(config);
    let result = session.run(
        &corpus,
        generator,
        vec![Box::new(ConstantEvaluator)],
        AcceptAll,
    );

    // 2 rounds × 1 candidate = 2 total evaluated
    assert_eq!(result.all_scored.len(), 2);
}

/// A generator that returns patterns only on the first call, empty on subsequent calls.
struct OnceGenerator {
    pattern: Pattern<String, String>,
    generated: bool,
}

impl CandidateGenerator for OnceGenerator {
    fn generate(&mut self, _corpus: &TraceCorpus, budget: usize) -> Vec<Pattern<String, String>> {
        if self.generated {
            return Vec::new();
        }
        self.generated = true;
        vec![self.pattern.clone(); budget.min(1)]
    }

    fn feedback(&mut self, _scored: &[ScoredPattern<String, String>]) {}

    fn name(&self) -> &str {
        "once"
    }
}

#[test]
fn session_stops_early_when_generator_exhausted() {
    let corpus = make_corpus();
    let generator = OnceGenerator {
        pattern: make_pattern(),
        generated: false,
    };
    let config = SessionConfig {
        max_rounds: 5,
        candidates_per_round: 2,
    };

    let mut session = DiscoverySession::new(config);
    let result = session.run(
        &corpus,
        generator,
        vec![Box::new(ConstantEvaluator)],
        AcceptAll,
    );

    assert_eq!(
        result.rounds, 1,
        "Session should stop after 1 round when generator is exhausted"
    );
}

#[test]
fn threshold_filter_accepts_and_rejects() {
    use fabula_discovery::ThresholdFilter;
    use std::collections::HashMap;

    let corpus = make_corpus();

    // ConstantEvaluator always returns 0.8 with name "constant"
    // ThresholdFilter with threshold 0.5 (below 0.8) should accept
    let low_threshold = ThresholdFilter {
        threshold: 0.5,
        weights: HashMap::from([("constant".to_string(), 1.0)]),
    };

    let generator_accept = FixedGenerator {
        pattern: make_pattern(),
        feedback_count: 0,
    };

    let config = SessionConfig {
        max_rounds: 1,
        candidates_per_round: 1,
    };

    let mut session = DiscoverySession::new(config);
    let result = session.run(
        &corpus,
        generator_accept,
        vec![Box::new(ConstantEvaluator)],
        low_threshold,
    );
    assert!(
        !result.accepted.is_empty(),
        "ThresholdFilter with threshold below score should accept"
    );

    // ThresholdFilter with threshold 1.0 (above 0.8) should reject
    let high_threshold = ThresholdFilter {
        threshold: 1.0,
        weights: HashMap::from([("constant".to_string(), 1.0)]),
    };

    let generator_reject = FixedGenerator {
        pattern: make_pattern(),
        feedback_count: 0,
    };

    let config2 = SessionConfig {
        max_rounds: 1,
        candidates_per_round: 1,
    };

    let mut session2 = DiscoverySession::new(config2);
    let result2 = session2.run(
        &corpus,
        generator_reject,
        vec![Box::new(ConstantEvaluator)],
        high_threshold,
    );
    assert!(
        result2.accepted.is_empty(),
        "ThresholdFilter with threshold above score should reject"
    );
}
