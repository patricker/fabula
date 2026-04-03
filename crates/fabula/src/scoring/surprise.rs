//! Pattern-level surprise scoring (Shannon surprise).
//!
//! Shannon surprise: `-log₂(observed / baseline)` with Laplace smoothing.
//! Standard information-theoretic self-information applied to pattern match frequencies.

use crate::engine::{BoundValue, Match, SiftEvent};
use crate::interval::Interval;
use crate::pattern::Pattern;
use std::collections::HashMap;
use std::fmt::Debug;

/// A match annotated with a surprise score.
#[derive(Debug, Clone)]
pub struct ScoredMatch<N: Debug, V: Debug, T: Debug + Clone> {
    /// The underlying match.
    pub pattern: String,
    /// Pattern index in the engine registry (if available).
    pub pattern_idx: Option<usize>,
    /// Variable bindings from the match.
    pub bindings: HashMap<String, BoundValue<N, V>>,
    /// Stage anchor variable -> matched time interval.
    pub intervals: HashMap<String, Interval<T>>,
    /// Surprise score in bits. Higher = more unexpected.
    /// Negative = pattern fires more often than baseline (less surprising).
    /// Uses Laplace smoothing to handle zero-observation cases.
    pub surprise: f64,
}

/// Pattern-level surprise scorer.
///
/// Tracks per-pattern match counts and computes Shannon surprise relative
/// to user-provided baseline frequencies.
///
/// Create alongside a `SiftEngine`, feed it match results via [`observe`]
/// or [`observe_events`], then call [`score`] to rank matches.
#[derive(Debug, Clone, Default)]
pub struct SurpriseScorer {
    /// Expected match probability per pattern index.
    baselines: HashMap<usize, f64>,
    /// Observed match count per pattern index.
    counts: HashMap<usize, u64>,
    /// Total observation rounds (each `observe` call = 1 round).
    total_rounds: u64,
}

impl SurpriseScorer {
    /// Create a new scorer with no baselines or observations.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the expected match frequency for a pattern (by registration index).
    ///
    /// `baseline` is a probability in (0, 1] — e.g., 0.1 means "expected to
    /// match in 10% of observation rounds."
    pub fn set_baseline(&mut self, pattern_idx: usize, baseline: f64) {
        assert!(baseline > 0.0 && baseline <= 1.0, "baseline must be in (0, 1], got {}", baseline);
        self.baselines.insert(pattern_idx, baseline);
    }

    /// Record one round of observations from batch evaluation results.
    ///
    /// Call this once per `evaluate()` call. Increments the round counter
    /// and counts each pattern that matched (at most once per pattern per round,
    /// so `p` stays in [0, 1] as a true probability).
    pub fn observe<N: Debug + PartialEq, V: Debug + PartialEq, T: Debug + Clone + PartialEq, L, VV>(
        &mut self,
        matches: &[Match<N, V, T>],
        patterns: &[Pattern<L, VV>],
    ) {
        self.total_rounds += 1;
        // Count each pattern at most once per round (probability, not rate)
        let mut seen_this_round = std::collections::HashSet::new();
        for m in matches {
            if let Some(idx) = patterns.iter().position(|p| p.name == m.pattern) {
                if seen_this_round.insert(idx) {
                    *self.counts.entry(idx).or_insert(0) += 1;
                }
            }
        }
    }

    /// Record observations from incremental matching events.
    ///
    /// Call this after each `on_edge_added()`. Only counts `Completed` events.
    /// Does NOT increment the round counter — call [`tick`] manually to
    /// mark observation boundaries in incremental mode.
    pub fn observe_events<N: Debug, V: Debug, L, VV>(
        &mut self,
        events: &[SiftEvent<N, V>],
        patterns: &[Pattern<L, VV>],
    ) {
        for event in events {
            if let SiftEvent::Completed { pattern, .. } = event {
                if let Some(idx) = patterns.iter().position(|p| p.name == *pattern) {
                    *self.counts.entry(idx).or_insert(0) += 1;
                }
            }
        }
    }

    /// Mark one observation round in incremental mode.
    ///
    /// Call this once per simulation tick (or however you define an
    /// observation boundary). Batch mode's [`observe`] does this automatically.
    pub fn tick(&mut self) {
        self.total_rounds += 1;
    }

    /// Compute surprise scores for a set of matches.
    ///
    /// Returns one `ScoredMatch` per input match, annotated with the pattern's
    /// current surprise score. Patterns without a baseline get score 0.0.
    pub fn score<N: Debug + Clone + PartialEq, V: Debug + Clone + PartialEq, T: Debug + Clone + PartialEq, L, VV>(
        &self,
        matches: &[Match<N, V, T>],
        patterns: &[Pattern<L, VV>],
    ) -> Vec<ScoredMatch<N, V, T>> {
        matches
            .iter()
            .map(|m| {
                let idx = patterns.iter().position(|p| p.name == m.pattern);
                let surprise = idx
                    .and_then(|i| self.surprise_for(i))
                    .unwrap_or(0.0);
                ScoredMatch {
                    pattern: m.pattern.clone(),
                    pattern_idx: m.pattern_idx,
                    bindings: m.bindings.clone(),
                    intervals: m.intervals.clone(),
                    surprise,
                }
            })
            .collect()
    }

    /// Get the current surprise score for a pattern (by index).
    ///
    /// Returns `None` if no baseline is set for this pattern.
    /// Uses Laplace smoothing: `p = (count + 1) / (rounds + 1)`.
    pub fn surprise_for(&self, pattern_idx: usize) -> Option<f64> {
        let baseline = *self.baselines.get(&pattern_idx)?;
        let count = self.counts.get(&pattern_idx).copied().unwrap_or(0);
        let p = (count as f64 + 1.0) / (self.total_rounds as f64 + 1.0);
        Some(-(p / baseline).log2())
    }

    /// Reset all observation counts and rounds. Baselines are preserved.
    pub fn reset_counts(&mut self) {
        self.counts.clear();
        self.total_rounds = 0;
    }

    /// Total observation rounds recorded.
    pub fn total_rounds(&self) -> u64 {
        self.total_rounds
    }

    /// Observed match count for a pattern.
    pub fn count_for(&self, pattern_idx: usize) -> u64 {
        self.counts.get(&pattern_idx).copied().unwrap_or(0)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::PatternBuilder;

    fn dummy_pattern(name: &str) -> Pattern<String, String> {
        PatternBuilder::<String, String>::new(name)
            .stage("e", |s| s.edge("e", "type".into(), "x".into()))
            .build()
    }

    fn dummy_match(name: &str) -> Match<String, String, i64> {
        Match {
            pattern: name.to_string(),
            pattern_idx: None,
            bindings: HashMap::new(),
            intervals: HashMap::new(),
        }
    }

    #[test]
    fn surprise_high_for_rare_pattern() {
        let patterns = vec![dummy_pattern("common"), dummy_pattern("rare")];
        let mut scorer = SurpriseScorer::new();
        scorer.set_baseline(0, 0.5);
        scorer.set_baseline(1, 0.5);

        // Simulate 10 rounds: common fires every time, rare fires once
        for i in 0..10 {
            let mut matches = vec![dummy_match("common")];
            if i == 5 {
                matches.push(dummy_match("rare"));
            }
            scorer.observe(&matches, &patterns);
        }

        let rare_surprise = scorer.surprise_for(1).unwrap();
        let common_surprise = scorer.surprise_for(0).unwrap();
        assert!(
            rare_surprise > common_surprise,
            "rare ({:.2}) should be more surprising than common ({:.2})",
            rare_surprise,
            common_surprise
        );
    }

    #[test]
    fn surprise_near_zero_at_baseline() {
        let patterns = vec![dummy_pattern("normal")];
        let mut scorer = SurpriseScorer::new();
        scorer.set_baseline(0, 0.5);

        // Pattern fires 5 out of 10 rounds = 50%, matching baseline
        for i in 0..10 {
            let matches = if i % 2 == 0 {
                vec![dummy_match("normal")]
            } else {
                vec![]
            };
            scorer.observe(&matches, &patterns);
        }

        let surprise = scorer.surprise_for(0).unwrap();
        // With Laplace smoothing: p = (5+1)/(10+1) = 0.545, baseline = 0.5
        // surprise = -log2(0.545/0.5) = -log2(1.09) ≈ -0.12
        // Close to zero — slightly negative because observed slightly above baseline
        assert!(
            surprise.abs() < 0.5,
            "surprise should be near zero, got {:.2}",
            surprise
        );
    }

    #[test]
    fn surprise_negative_for_common_pattern() {
        let patterns = vec![dummy_pattern("frequent")];
        let mut scorer = SurpriseScorer::new();
        scorer.set_baseline(0, 0.1); // expect 10%

        // Pattern fires every round = 100%, way above baseline
        for _ in 0..10 {
            scorer.observe(&[dummy_match("frequent")], &patterns);
        }

        let surprise = scorer.surprise_for(0).unwrap();
        assert!(
            surprise < 0.0,
            "over-represented pattern should have negative surprise, got {:.2}",
            surprise
        );
    }

    #[test]
    fn surprise_high_for_never_matched() {
        let patterns = vec![dummy_pattern("ghost")];
        let mut scorer = SurpriseScorer::new();
        scorer.set_baseline(0, 0.5);

        // 20 rounds, never matches
        let no_matches: Vec<Match<String, String, i64>> = vec![];
        for _ in 0..20 {
            scorer.observe(&no_matches, &patterns);
        }

        let surprise = scorer.surprise_for(0).unwrap();
        // p = (0+1)/(20+1) = 0.048, baseline = 0.5
        // surprise = -log2(0.048/0.5) = -log2(0.095) ≈ 3.4 bits
        assert!(
            surprise > 2.0,
            "never-matched pattern should have high surprise, got {:.2}",
            surprise
        );
    }

    #[test]
    fn observe_events_counts_completions_only() {
        let patterns = vec![dummy_pattern("test")];
        let mut scorer = SurpriseScorer::new();
        scorer.set_baseline(0, 0.5);

        let events: Vec<SiftEvent<String, String>> = vec![
            SiftEvent::Advanced {
                pattern: "test".into(),
                match_id: 0,
                stage_index: 0,
            },
            SiftEvent::Completed {
                pattern: "test".into(),
                match_id: 1,
                bindings: HashMap::new(),
            },
            SiftEvent::Negated {
                pattern: "test".into(),
                match_id: 2,
                clause_label: "x".into(),
                trigger_source: "src".into(),
            },
        ];

        scorer.observe_events(&events, &patterns);
        assert_eq!(scorer.count_for(0), 1, "only Completed should be counted");
    }

    #[test]
    fn score_returns_scored_matches() {
        let patterns = vec![dummy_pattern("a"), dummy_pattern("b")];
        let mut scorer = SurpriseScorer::new();
        scorer.set_baseline(0, 0.5);
        scorer.set_baseline(1, 0.5);

        // a fires 9 times, b fires 1 time
        for i in 0..10 {
            let mut matches = vec![dummy_match("a")];
            if i == 0 {
                matches.push(dummy_match("b"));
            }
            scorer.observe(&matches, &patterns);
        }

        let to_score = vec![dummy_match("a"), dummy_match("b")];
        let scored = scorer.score(&to_score, &patterns);

        assert_eq!(scored.len(), 2);
        assert!(scored[1].surprise > scored[0].surprise, "b should be more surprising than a");
    }

    #[test]
    fn no_baseline_returns_zero_surprise() {
        let patterns = vec![dummy_pattern("unscored")];
        let scorer = SurpriseScorer::new();
        // No baseline set

        let scored = scorer.score(&[dummy_match("unscored")], &patterns);
        assert_eq!(scored[0].surprise, 0.0);
    }

    #[test]
    fn reset_clears_counts_preserves_baselines() {
        let patterns = vec![dummy_pattern("test")];
        let mut scorer = SurpriseScorer::new();
        scorer.set_baseline(0, 0.5);

        for _ in 0..5 {
            scorer.observe(&[dummy_match("test")], &patterns);
        }
        assert_eq!(scorer.count_for(0), 5);
        assert_eq!(scorer.total_rounds(), 5);

        scorer.reset_counts();
        assert_eq!(scorer.count_for(0), 0);
        assert_eq!(scorer.total_rounds(), 0);
        // Baseline preserved
        assert!(scorer.surprise_for(0).is_some());
    }

    #[test]
    fn tick_increments_rounds() {
        let mut scorer = SurpriseScorer::new();
        assert_eq!(scorer.total_rounds(), 0);
        scorer.tick();
        scorer.tick();
        scorer.tick();
        assert_eq!(scorer.total_rounds(), 3);
    }

    // ---- StU (property-level) tests ----
}
