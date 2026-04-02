//! Statistical surprise scoring for pattern matches.
//!
//! Ranks matches by how unexpected they are relative to a baseline frequency.
//! Operates as a post-processing step — the engine finds matches, the scorer
//! ranks them. No engine modification required.
//!
//! # Phase 3.3a — Pattern-level scoring (Shannon surprise)
//!
//! Each pattern has a baseline expected frequency. Matches from patterns that
//! fire less often than expected score higher (more surprising).
//!
//! ```rust
//! use fabula::scoring::{SurpriseScorer, ScoredMatch};
//! use fabula::prelude::*;
//!
//! let mut scorer = SurpriseScorer::new();
//! // Pattern at index 0 is expected to match 10% of the time
//! scorer.set_baseline(0, 0.1);
//!
//! // After evaluation:
//! // let matches = engine.evaluate(&graph);
//! // scorer.observe(&matches, engine.patterns());
//! // let scored = scorer.score(&matches, engine.patterns());
//! ```
//!
//! # Phase 3.3b — Property-level scoring (StU) — planned
//!
//! Kreminski's "Select the Unexpected" (ICIDS 2022) scores individual matches
//! by the rarity of their *properties* (character traits, event types,
//! relationships). Two matches of the same pattern score differently if one
//! involves rare entities. Requires a property extractor with DataSource access.
//! See ROADMAP Phase 3.3b.

use crate::engine::{BoundValue, Match, SiftEvent};
use crate::pattern::Pattern;
use std::collections::HashMap;
use std::fmt::Debug;

/// A match annotated with a surprise score.
#[derive(Debug, Clone)]
pub struct ScoredMatch<N: Debug, V: Debug> {
    /// The underlying match.
    pub pattern: String,
    /// Variable bindings from the match.
    pub bindings: HashMap<String, BoundValue<N, V>>,
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
    pub fn observe<N: Debug, V: Debug, L, VV>(
        &mut self,
        matches: &[Match<N, V>],
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
    pub fn score<N: Debug + Clone, V: Debug + Clone, L, VV>(
        &self,
        matches: &[Match<N, V>],
        patterns: &[Pattern<L, VV>],
    ) -> Vec<ScoredMatch<N, V>> {
        matches
            .iter()
            .map(|m| {
                let idx = patterns.iter().position(|p| p.name == m.pattern);
                let surprise = idx
                    .and_then(|i| self.surprise_for(i))
                    .unwrap_or(0.0);
                ScoredMatch {
                    pattern: m.pattern.clone(),
                    bindings: m.bindings.clone(),
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

// ---------------------------------------------------------------------------
// Phase 3.3b — Property-level scoring (StU)
// ---------------------------------------------------------------------------

/// A match annotated with property-level (StU) surprise score.
///
/// The `stu_score` is the mean of per-property frequencies for this match.
/// **Lower = more surprising** (the match contains rarer properties).
/// `property_frequencies` shows which properties contributed and their
/// individual frequencies, sorted ascending (rarest first) for explainability.
#[derive(Debug, Clone)]
pub struct StuScoredMatch<N: Debug, V: Debug> {
    /// Pattern name.
    pub pattern: String,
    /// Variable bindings from the match.
    pub bindings: HashMap<String, BoundValue<N, V>>,
    /// Per-property frequencies, sorted ascending (rarest first).
    /// Each entry is `(property_string, frequency)`.
    pub property_frequencies: Vec<(String, f64)>,
    /// StU score: mean of property frequencies. Lower = more surprising.
    pub stu_score: f64,
}

/// Internal frequency table for a single pattern.
#[derive(Debug, Clone, Default)]
struct PropertyTable {
    /// How many matches have been observed for this pattern.
    total_matches: u64,
    /// How many matches contained each property.
    property_counts: HashMap<String, u64>,
}

/// Property-level surprise scorer using the StU heuristic.
///
/// Kreminski's "Select the Unexpected" (ICIDS 2022) scores individual matches
/// by the empirical frequency of their *properties*. A match involving rare
/// properties (unusual traits, uncommon factions, surprising relationships)
/// scores lower (= more surprising).
///
/// **The scorer only does frequency math.** Property extraction is the caller's
/// responsibility — this struct never touches a graph or DataSource.
///
/// # Property extraction guidance
///
/// Properties should be **categorical attributes**, not entity identifiers.
/// Emit `"actor_faction=rebels"` rather than `"actor=char_147"`. Entity IDs
/// have near-uniform frequency in rich simulations, making all matches score
/// identically (the "everything is rare" failure mode).
///
/// Good properties: traits, factions, relationship types, event categories,
/// emotional states, location types.
///
/// # Example
///
/// ```rust
/// use fabula::scoring::StuScorer;
/// use fabula::prelude::*;
///
/// let mut stu = StuScorer::new();
///
/// // Observe properties for completed matches (user extracts these)
/// stu.observe_one("betrayal", &["actor_trait=ambitious", "target_role=king"]);
/// stu.observe_one("betrayal", &["actor_trait=loyal", "target_role=merchant"]);
/// stu.observe_one("betrayal", &["actor_trait=ambitious", "target_role=merchant"]);
///
/// // Score a new match
/// let freq = stu.property_frequency("betrayal", "actor_trait=ambitious");
/// assert!(freq.is_some()); // 2 out of 3 matches had this property
/// ```
#[derive(Debug, Clone, Default)]
pub struct StuScorer {
    /// Per-pattern property frequency tables.
    tables: HashMap<String, PropertyTable>,
}

impl StuScorer {
    /// Create a new empty scorer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record properties for a single match. Call once per completed match.
    ///
    /// `pattern` is the pattern name. `properties` is the list of property
    /// strings extracted by the caller's domain-specific extractor.
    pub fn observe_one(&mut self, pattern: &str, properties: &[impl AsRef<str>]) {
        let table = self.tables.entry(pattern.to_string()).or_default();
        table.total_matches += 1;
        // Count each property at most once per match (presence, not multiplicity)
        let mut seen = std::collections::HashSet::new();
        for prop in properties {
            if seen.insert(prop.as_ref().to_string()) {
                *table
                    .property_counts
                    .entry(prop.as_ref().to_string())
                    .or_insert(0) += 1;
            }
        }
    }

    /// Record properties for a batch of matches.
    ///
    /// Each entry is `(pattern_name, properties)`.
    pub fn observe_batch(&mut self, observations: &[(&str, &[String])]) {
        for (pattern, props) in observations {
            self.observe_one(pattern, props);
        }
    }

    /// Compute the smoothed frequency of a property within a pattern's matches.
    ///
    /// Uses Laplace smoothing: `(count + 1) / (total_matches + V)` where
    /// V is the vocabulary size (number of distinct properties seen for this pattern).
    /// Returns `None` if the pattern has never been observed.
    pub fn property_frequency(&self, pattern: &str, property: &str) -> Option<f64> {
        let table = self.tables.get(pattern)?;
        let count = table.property_counts.get(property).copied().unwrap_or(0);
        let vocab_size = table.property_counts.len() as f64;
        Some((count as f64 + 1.0) / (table.total_matches as f64 + vocab_size))
    }

    /// Score a set of matches given their pre-extracted properties.
    ///
    /// Returns one `StuScoredMatch` per input. Score = mean of per-property
    /// Laplace-smoothed frequencies. **Lower = more surprising.**
    ///
    /// Matches whose pattern has not been observed get `stu_score = 1.0`
    /// (maximally unsurprising — no data to distinguish).
    pub fn score<N: Debug + Clone, V: Debug + Clone>(
        &self,
        matches_with_props: &[(Match<N, V>, Vec<String>)],
    ) -> Vec<StuScoredMatch<N, V>> {
        matches_with_props
            .iter()
            .map(|(m, props)| {
                let table = self.tables.get(&m.pattern);

                if props.is_empty() || table.is_none() {
                    return StuScoredMatch {
                        pattern: m.pattern.clone(),
                        bindings: m.bindings.clone(),
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

                let mut prop_freqs: Vec<(String, f64)> = unique_props
                    .iter()
                    .map(|prop| {
                        let count = table.property_counts.get(prop.as_str()).copied().unwrap_or(0);
                        let freq =
                            (count as f64 + 1.0) / (table.total_matches as f64 + vocab_size);
                        (prop.to_string(), freq)
                    })
                    .collect();

                // Sort ascending — rarest properties first (for explainability)
                prop_freqs.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

                let stu_score =
                    prop_freqs.iter().map(|(_, f)| f).sum::<f64>() / prop_freqs.len() as f64;

                StuScoredMatch {
                    pattern: m.pattern.clone(),
                    bindings: m.bindings.clone(),
                    property_frequencies: prop_freqs,
                    stu_score,
                }
            })
            .collect()
    }

    /// Total matches observed for a pattern.
    pub fn match_count(&self, pattern: &str) -> u64 {
        self.tables
            .get(pattern)
            .map(|t| t.total_matches)
            .unwrap_or(0)
    }

    /// Number of distinct properties seen for a pattern.
    pub fn vocabulary_size(&self, pattern: &str) -> usize {
        self.tables
            .get(pattern)
            .map(|t| t.property_counts.len())
            .unwrap_or(0)
    }

    /// Reset all observations.
    pub fn reset(&mut self) {
        self.tables.clear();
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

    fn dummy_match(name: &str) -> Match<String, String> {
        Match {
            pattern: name.to_string(),
            bindings: HashMap::new(),
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
        let no_matches: Vec<Match<String, String>> = vec![];
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

    #[test]
    fn stu_rare_properties_score_lower() {
        let mut stu = StuScorer::new();

        // Observe 10 betrayal matches. "ambitious" appears in 2, "loyal" in 8.
        for i in 0..10 {
            let props = if i < 2 {
                vec!["actor_trait=ambitious".to_string()]
            } else {
                vec!["actor_trait=loyal".to_string()]
            };
            stu.observe_one("betrayal", &props);
        }

        let m_ambitious = (
            dummy_match("betrayal"),
            vec!["actor_trait=ambitious".to_string()],
        );
        let m_loyal = (
            dummy_match("betrayal"),
            vec!["actor_trait=loyal".to_string()],
        );

        let scored = stu.score(&[m_ambitious, m_loyal]);
        assert!(
            scored[0].stu_score < scored[1].stu_score,
            "ambitious ({:.3}) should score lower (more surprising) than loyal ({:.3})",
            scored[0].stu_score,
            scored[1].stu_score
        );
    }

    #[test]
    fn stu_laplace_smoothing_for_novel_property() {
        let mut stu = StuScorer::new();

        // Observe 10 matches with "trait=brave"
        for _ in 0..10 {
            stu.observe_one("test", &["trait=brave"]);
        }

        // Novel property never seen before
        let freq = stu.property_frequency("test", "trait=cowardly");
        assert!(freq.is_some());
        let f = freq.unwrap();
        // Laplace: (0+1)/(10+1) = 1/11 ≈ 0.091 (V=1 distinct property)
        assert!(f > 0.0, "novel property should have non-zero frequency: {}", f);
        assert!(f < 0.2, "novel property should have low frequency: {}", f);
    }

    #[test]
    fn stu_empty_properties_get_default_score() {
        let stu = StuScorer::new();

        let m = (dummy_match("test"), vec![]);
        let scored = stu.score(&[m]);
        assert_eq!(scored[0].stu_score, 1.0, "empty properties = maximally unsurprising");
    }

    #[test]
    fn stu_unobserved_pattern_gets_default_score() {
        let stu = StuScorer::new();

        let m = (
            dummy_match("unknown"),
            vec!["some_prop".to_string()],
        );
        let scored = stu.score(&[m]);
        assert_eq!(scored[0].stu_score, 1.0, "unobserved pattern = no data");
    }

    #[test]
    fn stu_property_frequencies_sorted_ascending() {
        let mut stu = StuScorer::new();

        for i in 0..10 {
            let mut props = vec!["common=yes".to_string()]; // appears 10/10
            if i < 3 {
                props.push("rare=yes".to_string()); // appears 3/10
            }
            stu.observe_one("test", &props);
        }

        let m = (
            dummy_match("test"),
            vec!["rare=yes".to_string(), "common=yes".to_string()],
        );
        let scored = stu.score(&[m]);
        let pf = &scored[0].property_frequencies;

        assert_eq!(pf.len(), 2);
        assert!(
            pf[0].1 <= pf[1].1,
            "properties should be sorted ascending: {:?}",
            pf
        );
        assert!(pf[0].0.contains("rare"), "rarest property should be first");
    }

    #[test]
    fn stu_observe_batch() {
        let mut stu = StuScorer::new();

        let p1_props = vec!["a".to_string(), "b".to_string()];
        let p2_props = vec!["c".to_string()];
        let batch: Vec<(&str, &[String])> = vec![
            ("p1", &p1_props),
            ("p2", &p2_props),
        ];
        stu.observe_batch(&batch);

        assert_eq!(stu.match_count("p1"), 1);
        assert_eq!(stu.match_count("p2"), 1);
        assert_eq!(stu.vocabulary_size("p1"), 2);
    }

    #[test]
    fn stu_deduplicates_properties_per_match() {
        let mut stu = StuScorer::new();

        // Same property listed twice in one match
        stu.observe_one("test", &["dup", "dup", "dup"]);
        assert_eq!(stu.match_count("test"), 1);

        // Property should be counted once, not three times
        let freq = stu.property_frequency("test", "dup").unwrap();
        // Laplace: (1+1)/(1+1) = 1.0
        assert!(
            (freq - 1.0).abs() < 0.01,
            "duplicated property should count once: {}",
            freq
        );
    }

    #[test]
    fn stu_reset_clears_all() {
        let mut stu = StuScorer::new();
        stu.observe_one("test", &["a", "b"]);
        assert_eq!(stu.match_count("test"), 1);

        stu.reset();
        assert_eq!(stu.match_count("test"), 0);
        assert_eq!(stu.vocabulary_size("test"), 0);
    }
}
