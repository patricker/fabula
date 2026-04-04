//! Property-level surprise scoring (Select the Unexpected).
//!
//! Kreminski, Dickinson, Wardrip-Fruin, Mateas (2022) "Select the Unexpected:
//! A Statistical Heuristic for Story Sifting" (ICIDS 2022). Scores individual
//! matches by the mean empirical frequency of their *properties*.

use crate::engine::{BoundValue, Match};
use crate::interval::Interval;
use std::collections::HashMap;
use std::fmt::Debug;

/// A match annotated with property-level (StU) surprise score.
///
/// The `stu_score` is the mean of per-property frequencies for this match.
/// **Lower = more surprising** (the match contains rarer properties).
/// `property_frequencies` shows which properties contributed and their
/// individual frequencies, sorted ascending (rarest first) for explainability.
#[derive(Debug, Clone)]
pub struct StuScoredMatch<N: Debug, V: Debug, T: Debug + Clone> {
    /// Pattern name.
    pub pattern: String,
    /// Pattern index in the engine registry (if available).
    pub pattern_idx: Option<usize>,
    /// Variable bindings from the match.
    pub bindings: HashMap<String, BoundValue<N, V>>,
    /// Stage anchor variable -> matched time interval.
    pub intervals: HashMap<String, Interval<T>>,
    /// Metadata from the matched pattern.
    pub metadata: HashMap<String, String>,
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
    #[allow(clippy::type_complexity)]
    pub fn score<
        N: Debug + Clone + PartialEq,
        V: Debug + Clone + PartialEq,
        T: Debug + Clone + PartialEq,
    >(
        &self,
        matches_with_props: &[(Match<N, V, T>, Vec<String>)],
    ) -> Vec<StuScoredMatch<N, V, T>> {
        matches_with_props
            .iter()
            .map(|(m, props)| {
                let table = self.tables.get(&m.pattern);

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

                let mut prop_freqs: Vec<(String, f64)> = unique_props
                    .iter()
                    .map(|prop| {
                        let count = table
                            .property_counts
                            .get(prop.as_str())
                            .copied()
                            .unwrap_or(0);
                        let freq = (count as f64 + 1.0) / (table.total_matches as f64 + vocab_size);
                        (prop.to_string(), freq)
                    })
                    .collect();

                // Sort ascending — rarest properties first (for explainability)
                prop_freqs
                    .sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

                let stu_score =
                    prop_freqs.iter().map(|(_, f)| f).sum::<f64>() / prop_freqs.len() as f64;

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
    use crate::engine::Match;

    fn dummy_match(name: &str) -> Match<String, String, i64> {
        Match {
            pattern: name.to_string(),
            pattern_idx: None,
            bindings: HashMap::new(),
            intervals: HashMap::new(),
            metadata: HashMap::new(),
        }
    }

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
        assert!(
            f > 0.0,
            "novel property should have non-zero frequency: {}",
            f
        );
        assert!(f < 0.2, "novel property should have low frequency: {}", f);
    }

    #[test]
    fn stu_empty_properties_get_default_score() {
        let stu = StuScorer::new();

        let m = (dummy_match("test"), vec![]);
        let scored = stu.score(&[m]);
        assert_eq!(
            scored[0].stu_score, 1.0,
            "empty properties = maximally unsurprising"
        );
    }

    #[test]
    fn stu_unobserved_pattern_gets_default_score() {
        let stu = StuScorer::new();

        let m = (dummy_match("unknown"), vec!["some_prop".to_string()]);
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
        let batch: Vec<(&str, &[String])> = vec![("p1", &p1_props), ("p2", &p2_props)];
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
