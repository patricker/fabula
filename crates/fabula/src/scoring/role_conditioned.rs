//! Role-conditioned variant of [`StuScorer`](super::stu::StuScorer).
//!
//! Conditions per-property frequency on an entity-role attribute. Solves the
//! "villain doing villain things" problem: a villain's harmful actions have
//! high marginal surprise but low conditional surprise given their role.
//!
//! Observations and lookups are keyed on `(pattern, role)` instead of just
//! `pattern`. Math, aggregation modes, and Laplace smoothing are identical
//! to [`StuScorer`].

use crate::engine::Match;
use crate::scoring::stu::{PropertyTable, StuAggregation, StuScoredMatch};
use std::collections::HashMap;
use std::fmt::Debug;

/// Surprise scorer that conditions on entity role.
///
/// Tracks per-property frequencies keyed on `(pattern, role)` instead of
/// just `pattern`. See the [module-level docs](self) for the use case.
#[derive(Debug, Clone)]
pub struct RoleConditionedStuScorer {
    /// `(pattern, role) -> PropertyTable`.
    tables: HashMap<(String, String), PropertyTable>,
    /// How to combine per-property frequencies into a single score.
    aggregation: StuAggregation,
}

impl Default for RoleConditionedStuScorer {
    fn default() -> Self {
        Self {
            tables: HashMap::new(),
            aggregation: StuAggregation::ArithmeticMean,
        }
    }
}

impl RoleConditionedStuScorer {
    /// Create a new empty scorer with arithmetic mean aggregation (default).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the aggregation strategy. Default is `ArithmeticMean`.
    pub fn with_aggregation(mut self, aggregation: StuAggregation) -> Self {
        self.aggregation = aggregation;
        self
    }

    /// Record properties for a single match in the context of a role.
    /// Call once per completed match. Properties are deduplicated within
    /// a single observation (presence, not multiplicity).
    pub fn observe_one(&mut self, pattern: &str, role: &str, properties: &[impl AsRef<str>]) {
        let key = (pattern.to_string(), role.to_string());
        let table = self.tables.entry(key).or_default();
        table.total_matches += 1;
        let mut seen = std::collections::HashSet::new();
        for prop in properties {
            let s = prop.as_ref().to_string();
            if seen.insert(s.clone()) {
                *table.property_counts.entry(s).or_insert(0) += 1;
            }
        }
    }

    /// Smoothed conditional frequency `P(property | pattern, role)`.
    /// Uses Laplace smoothing identically to [`StuScorer::property_frequency`]:
    /// `(count + 1) / (total_matches + V)` where V is the vocabulary size
    /// for this `(pattern, role)` pair. Returns `None` if the pair has never
    /// been observed.
    pub fn property_frequency(
        &self,
        pattern: &str,
        role: &str,
        property: &str,
    ) -> Option<f64> {
        let key = (pattern.to_string(), role.to_string());
        let table = self.tables.get(&key)?;
        let count = table.property_counts.get(property).copied().unwrap_or(0);
        let vocab_size = table.property_counts.len() as f64;
        Some((count as f64 + 1.0) / (table.total_matches as f64 + vocab_size))
    }

    /// Number of observations recorded for a `(pattern, role)` pair.
    pub fn match_count(&self, pattern: &str, role: &str) -> u64 {
        self.tables
            .get(&(pattern.to_string(), role.to_string()))
            .map(|t| t.total_matches)
            .unwrap_or(0)
    }

    /// Reset all training data.
    pub fn reset(&mut self) {
        self.tables.clear();
    }

    /// Score a batch of matches, each with its associated role and
    /// pre-extracted property list. Returns one `StuScoredMatch` per input.
    ///
    /// Score interpretation depends on the aggregation mode: **lower = more
    /// surprising** for `ArithmeticMean`, `GeometricMean`, and `Min`;
    /// **higher = more surprising** for `TfIdf`.
    ///
    /// Matches whose `(pattern, role)` pair has not been observed get
    /// `stu_score = 1.0` (maximally unsurprising — no data to distinguish).
    #[allow(clippy::type_complexity)]
    pub fn score<
        N: Debug + Clone + PartialEq,
        V: Debug + Clone + PartialEq,
        T: Debug + Clone + PartialEq,
    >(
        &self,
        matches_with_role_and_props: &[(Match<N, V, T>, String, Vec<String>)],
    ) -> Vec<StuScoredMatch<N, V, T>> {
        matches_with_role_and_props
            .iter()
            .map(|(m, role, props)| {
                let key = (m.pattern.clone(), role.clone());
                let table = self.tables.get(&key);

                if props.is_empty() || table.is_none() {
                    return StuScoredMatch {
                        pattern: m.pattern.clone(),
                        pattern_idx: m.pattern_idx,
                        bindings: m.bindings.clone(),
                        intervals: m.intervals.clone(),
                        metadata: m.metadata.clone(),
                        property_frequencies: Vec::new(),
                        stu_score: 1.0,
                    };
                }

                let table = table.unwrap();
                let vocab_size = table.property_counts.len() as f64;

                // Deduplicate properties (consistent with observe_one)
                let unique_props: Vec<&String> = {
                    let mut seen = std::collections::HashSet::new();
                    props.iter().filter(|p| seen.insert(p.as_str())).collect()
                };

                let prop_freqs: Vec<(String, f64)> = unique_props
                    .iter()
                    .map(|prop| {
                        let count = table
                            .property_counts
                            .get(prop.as_str())
                            .copied()
                            .unwrap_or(0);
                        let freq =
                            (count as f64 + 1.0) / (table.total_matches as f64 + vocab_size);
                        (prop.to_string(), freq)
                    })
                    .collect();

                let stu_score = aggregate(&self.aggregation, &prop_freqs);

                StuScoredMatch {
                    pattern: m.pattern.clone(),
                    pattern_idx: m.pattern_idx,
                    bindings: m.bindings.clone(),
                    intervals: m.intervals.clone(),
                    metadata: m.metadata.clone(),
                    property_frequencies: prop_freqs,
                    stu_score,
                }
            })
            .collect()
    }
}

/// Combine per-property frequencies into a single score per the aggregation
/// strategy. Mirrors [`StuScorer`]'s aggregation math (same formulas, same
/// polarity conventions) but without cold-start attenuation — the
/// `RoleConditionedStuScorer` omits attenuation so scores are directly
/// comparable to the trained frequencies.
///
/// **Lower = more surprising** for `ArithmeticMean`, `GeometricMean`, `Min`.
/// **Higher = more surprising** for `TfIdf`.
fn aggregate(aggregation: &StuAggregation, prop_freqs: &[(String, f64)]) -> f64 {
    if prop_freqs.is_empty() {
        return 1.0;
    }
    let k = prop_freqs.len() as f64;
    match aggregation {
        StuAggregation::ArithmeticMean => {
            prop_freqs.iter().map(|(_, f)| f).sum::<f64>() / k
        }
        StuAggregation::GeometricMean => {
            (prop_freqs.iter().map(|(_, f)| f.ln()).sum::<f64>() / k).exp()
        }
        StuAggregation::Min => prop_freqs
            .iter()
            .map(|(_, f)| *f)
            .fold(f64::INFINITY, f64::min),
        StuAggregation::TfIdf => {
            // Higher = more surprising. Sum of -log2(freq) per property.
            // Matches StuScorer's log-base semantics.
            prop_freqs.iter().map(|(_, f)| -f.log2()).sum::<f64>()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_match(pattern: &str) -> Match<String, String, i64> {
        Match {
            pattern: pattern.to_string(),
            pattern_idx: None,
            bindings: std::collections::HashMap::new(),
            intervals: std::collections::HashMap::new(),
            metadata: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn role_conditioned_recovers_per_role_distribution() {
        let mut scorer = RoleConditionedStuScorer::new();

        for _ in 0..95 {
            scorer.observe_one("betrayal", "villain", &["weapon=knife"]);
        }
        for _ in 0..5 {
            scorer.observe_one("betrayal", "villain", &["weapon=words"]);
        }
        for _ in 0..5 {
            scorer.observe_one("betrayal", "hero", &["weapon=knife"]);
        }
        for _ in 0..95 {
            scorer.observe_one("betrayal", "hero", &["weapon=words"]);
        }

        let villain_knife = scorer
            .property_frequency("betrayal", "villain", "weapon=knife")
            .unwrap();
        let hero_knife = scorer
            .property_frequency("betrayal", "hero", "weapon=knife")
            .unwrap();

        assert!(
            villain_knife > 0.9,
            "villain knife should be common: {}",
            villain_knife
        );
        assert!(
            hero_knife < 0.1,
            "hero knife should be rare: {}",
            hero_knife
        );
        assert!(
            villain_knife > hero_knife * 5.0,
            "villain knife ({}) should be much more common than hero knife ({})",
            villain_knife,
            hero_knife
        );
    }

    #[test]
    fn unknown_pattern_role_pair_returns_none() {
        let mut scorer = RoleConditionedStuScorer::new();
        scorer.observe_one("betrayal", "villain", &["weapon=knife"]);

        assert_eq!(
            scorer.property_frequency("betrayal", "hero", "weapon=knife"),
            None,
            "unknown role for known pattern returns None"
        );
        assert_eq!(
            scorer.property_frequency("rescue", "villain", "weapon=knife"),
            None,
            "unknown pattern returns None"
        );
    }

    #[test]
    fn match_count_tracks_per_role() {
        let mut scorer = RoleConditionedStuScorer::new();
        scorer.observe_one("betrayal", "villain", &["weapon=knife"]);
        scorer.observe_one("betrayal", "villain", &["weapon=words"]);
        scorer.observe_one("betrayal", "hero", &["weapon=words"]);

        assert_eq!(scorer.match_count("betrayal", "villain"), 2);
        assert_eq!(scorer.match_count("betrayal", "hero"), 1);
        assert_eq!(scorer.match_count("betrayal", "unknown"), 0);
        assert_eq!(scorer.match_count("rescue", "villain"), 0);
    }

    #[test]
    fn properties_deduplicated_within_a_single_observation() {
        let mut scorer = RoleConditionedStuScorer::new();
        scorer.observe_one("p", "r", &["x", "x", "x", "y"]);

        // After one observation with duplicate "x", the count for "x"
        // should be 1 (presence semantics), not 3.
        assert_eq!(scorer.match_count("p", "r"), 1);
        // Frequency of "x" with vocab=2 (x, y), counts: x=1: (1+1)/(1+2) = 2/3
        let fx = scorer.property_frequency("p", "r", "x").unwrap();
        assert!((fx - (2.0 / 3.0)).abs() < 1e-9, "got {}", fx);
    }

    // -----------------------------------------------------------------------
    // Aggregation and score() tests (Task 3)
    // -----------------------------------------------------------------------

    #[test]
    fn score_uses_role_conditioned_frequencies() {
        let mut scorer = RoleConditionedStuScorer::new();
        // Villains: knife is common, words is rare.
        for _ in 0..95 {
            scorer.observe_one("betrayal", "villain", &["weapon=knife"]);
        }
        for _ in 0..5 {
            scorer.observe_one("betrayal", "villain", &["weapon=words"]);
        }
        // Heroes: knife is rare.
        for _ in 0..5 {
            scorer.observe_one("betrayal", "hero", &["weapon=knife"]);
        }
        for _ in 0..95 {
            scorer.observe_one("betrayal", "hero", &["weapon=words"]);
        }

        let villain_match = fake_match("betrayal");
        let hero_match = fake_match("betrayal");
        let scored = scorer.score(&[
            (villain_match, "villain".to_string(), vec!["weapon=knife".to_string()]),
            (hero_match, "hero".to_string(), vec!["weapon=knife".to_string()]),
        ]);

        assert_eq!(scored.len(), 2);
        // ArithmeticMean: lower = more surprising. Hero knife should score
        // much lower than villain knife.
        assert!(
            scored[0].stu_score > scored[1].stu_score * 5.0,
            "expected villain knife (common 95%) to score much higher than hero knife (rare 5%); got villain={}, hero={}",
            scored[0].stu_score, scored[1].stu_score
        );
    }

    #[test]
    fn score_unknown_role_yields_neutral_one() {
        let mut scorer = RoleConditionedStuScorer::new();
        scorer.observe_one("betrayal", "villain", &["weapon=knife"]);

        let m = fake_match("betrayal");
        let scored = scorer.score(&[(m, "stranger".to_string(), vec!["weapon=knife".to_string()])]);
        assert_eq!(scored.len(), 1);
        assert!(
            (scored[0].stu_score - 1.0).abs() < 1e-9,
            "unknown (pattern, role) yields neutral score 1.0; got {}",
            scored[0].stu_score
        );
    }

    #[test]
    fn with_aggregation_changes_score_combination() {
        let mut scorer = RoleConditionedStuScorer::new().with_aggregation(StuAggregation::Min);
        scorer.observe_one("p", "r", &["common"]);
        scorer.observe_one("p", "r", &["common"]);
        scorer.observe_one("p", "r", &["common", "rare"]);

        let m = fake_match("p");
        let scored = scorer.score(&[(
            m,
            "r".to_string(),
            vec!["common".to_string(), "rare".to_string()],
        )]);

        // Min aggregation = freq of the rarest property.
        // common: 3/3 + 1, vocab=2 → 4/5 = 0.8
        // rare:   1/3 + 1, vocab=2 → 2/5 = 0.4
        // Min = 0.4
        assert!(
            (scored[0].stu_score - 0.4).abs() < 1e-9,
            "Min aggregation should pick the rare property's freq; got {}",
            scored[0].stu_score
        );
    }
}
