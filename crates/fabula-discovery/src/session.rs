use crate::corpus::TraceCorpus;
use crate::score::{PatternScore, ScoredPattern};
use crate::traits::{CandidateGenerator, PatternEvaluator, PatternFilter};

/// Configuration for a discovery session.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Maximum number of generate-evaluate rounds.
    pub max_rounds: usize,
    /// How many candidates to request per round.
    pub candidates_per_round: usize,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            max_rounds: 10,
            candidates_per_round: 50,
        }
    }
}

/// The result of a completed discovery session.
#[derive(Debug, Clone)]
pub struct SessionHistory {
    /// How many rounds actually ran.
    pub rounds: usize,
    /// All candidates that were evaluated, in order.
    pub all_scored: Vec<ScoredPattern<String, String>>,
    /// Candidates that passed the filter.
    pub accepted: Vec<ScoredPattern<String, String>>,
}

/// Orchestrates the generate-evaluate loop with configurable budgets.
pub struct DiscoverySession {
    config: SessionConfig,
}

impl DiscoverySession {
    /// Create a session with the given configuration.
    pub fn new(config: SessionConfig) -> Self {
        Self { config }
    }

    /// Run the full discovery loop.
    ///
    /// 1. Generator proposes candidates
    /// 2. Evaluators score each candidate
    /// 3. Filter decides which to keep
    /// 4. Scored results feed back to the generator
    /// 5. Repeat for `max_rounds`
    pub fn run(
        &mut self,
        corpus: &TraceCorpus,
        mut generator: impl CandidateGenerator,
        evaluators: Vec<Box<dyn PatternEvaluator>>,
        filter: impl PatternFilter,
    ) -> SessionHistory {
        let mut all_scored = Vec::new();
        let mut accepted = Vec::new();
        let mut rounds_run = 0;

        for round in 0..self.config.max_rounds {
            let candidates = generator.generate(corpus, self.config.candidates_per_round);

            if candidates.is_empty() {
                break;
            }

            rounds_run += 1;
            let mut round_scored = Vec::new();
            for pattern in candidates {
                let mut scores = std::collections::HashMap::new();
                for evaluator in &evaluators {
                    let value = evaluator.evaluate(&pattern, corpus);
                    scores.insert(evaluator.name().to_string(), value);
                }

                let scored = ScoredPattern {
                    pattern,
                    score: PatternScore {
                        scores,
                        round,
                        generator: generator.name().to_string(),
                    },
                };

                if filter.accept(&scored) {
                    accepted.push(scored.clone());
                }

                round_scored.push(scored);
            }

            generator.feedback(&round_scored);
            all_scored.extend(round_scored);
        }

        SessionHistory {
            rounds: rounds_run,
            all_scored,
            accepted,
        }
    }
}
