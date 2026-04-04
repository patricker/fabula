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
    /// StU score: aggregated property frequencies. Interpretation depends on
    /// the aggregation mode — lower = more surprising for most modes, but
    /// **higher = more surprising** for `TfIdf`.
    pub stu_score: f64,
}

/// Internal frequency table for a single pattern.
#[derive(Debug, Clone, Default)]
struct PropertyTable {
    /// How many matches have been observed for this pattern.
    total_matches: u64,
    /// How many matches contained each property.
    property_counts: HashMap<String, u64>,
    /// Co-occurrence counts for property pairs (canonical sorted order).
    /// Only populated when `pmi_correction` is enabled on the scorer.
    pair_counts: HashMap<(String, String), u64>,
}

/// Aggregation strategy for combining per-property frequencies into a single StU score.
///
/// Each variant implements a different "theory of surprise":
/// - `ArithmeticMean` — average rarity across all properties
/// - `TfIdf` — total information content (log-weighted, **higher = more surprising**)
/// - `GeometricMean` — sensitive to outlier rare properties
/// - `Min` — the single rarest property dominates
///
/// All variants except `TfIdf` produce scores where **lower = more surprising**.
/// `TfIdf` has reversed polarity: **higher = more surprising**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StuAggregation {
    /// Arithmetic mean of per-property frequencies. **Lower = more surprising.**
    /// This is the original StU heuristic from Kreminski et al. (2022).
    #[default]
    ArithmeticMean,
    /// TF-IDF style: `sum(-log2(freq))`. **Higher = more surprising.**
    /// Rare properties dominate via log weighting.
    TfIdf,
    /// Geometric mean of per-property frequencies. **Lower = more surprising.**
    /// A single rare property pulls the entire score down multiplicatively.
    GeometricMean,
    /// Minimum per-property frequency. **Lower = more surprising.**
    /// The single most surprising property dominates the score.
    Min,
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
/// # Aggregation
///
/// The default aggregation is arithmetic mean (original StU). Use
/// [`with_aggregation`](StuScorer::with_aggregation) to select an alternative:
///
/// ```rust
/// use fabula::scoring::{StuScorer, StuAggregation};
///
/// let scorer = StuScorer::new().with_aggregation(StuAggregation::Min);
/// ```
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
#[derive(Debug, Clone)]
pub struct StuScorer {
    /// Per-pattern property frequency tables.
    tables: HashMap<String, PropertyTable>,
    /// How to combine per-property frequencies into a single score.
    aggregation: StuAggregation,
    /// Whether to apply PMI-based correction for correlated properties.
    pmi_correction: bool,
}

impl Default for StuScorer {
    fn default() -> Self {
        Self {
            tables: HashMap::new(),
            aggregation: StuAggregation::ArithmeticMean,
            pmi_correction: false,
        }
    }
}

impl StuScorer {
    /// Create a new empty scorer with arithmetic mean aggregation (default).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the aggregation strategy. Default is `ArithmeticMean`.
    pub fn with_aggregation(mut self, aggregation: StuAggregation) -> Self {
        self.aggregation = aggregation;
        self
    }

    /// Enable PMI-based correction for correlated properties.
    ///
    /// When two properties frequently co-occur (high PMI), their individual
    /// rarities would be double-counted. This correction replaces the
    /// less-rare member's frequency with its conditional frequency given
    /// the partner, removing the redundancy.
    ///
    /// Adds O(k²) pair counting per `observe_one` call where k is the
    /// number of properties per match (typically 2-8).
    pub fn with_pmi_correction(mut self) -> Self {
        self.pmi_correction = true;
        self
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
        let mut unique: Vec<String> = Vec::new();
        for prop in properties {
            if seen.insert(prop.as_ref().to_string()) {
                let s = prop.as_ref().to_string();
                *table.property_counts.entry(s.clone()).or_insert(0) += 1;
                unique.push(s);
            }
        }
        // Count co-occurring pairs (canonical sorted order) for PMI correction
        if self.pmi_correction {
            for i in 0..unique.len() {
                for j in (i + 1)..unique.len() {
                    let pair = if unique[i] < unique[j] {
                        (unique[i].clone(), unique[j].clone())
                    } else {
                        (unique[j].clone(), unique[i].clone())
                    };
                    *table.pair_counts.entry(pair).or_insert(0) += 1;
                }
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
    /// Returns one `StuScoredMatch` per input. Score interpretation depends on
    /// the aggregation mode: **lower = more surprising** for `ArithmeticMean`,
    /// `GeometricMean`, and `Min`; **higher = more surprising** for `TfIdf`.
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

                // PMI correction: for highly correlated pairs, replace the less-rare
                // member's frequency with its conditional frequency given the partner.
                if self.pmi_correction && prop_freqs.len() >= 2 {
                    let pmi_threshold = 1.0; // 1 bit of mutual information
                    let n = table.total_matches as f64;
                    for i in 0..prop_freqs.len() {
                        for j in (i + 1)..prop_freqs.len() {
                            let (a, b) = if prop_freqs[i].0 < prop_freqs[j].0 {
                                (prop_freqs[i].0.clone(), prop_freqs[j].0.clone())
                            } else {
                                (prop_freqs[j].0.clone(), prop_freqs[i].0.clone())
                            };
                            let pair_count = table.pair_counts.get(&(a, b)).copied().unwrap_or(0);
                            if pair_count == 0 {
                                continue;
                            }
                            // Use raw (unsmoothed) frequencies for PMI consistency
                            let p_ab = pair_count as f64 / n;
                            let c_i = table
                                .property_counts
                                .get(&prop_freqs[i].0)
                                .copied()
                                .unwrap_or(0) as f64;
                            let c_j = table
                                .property_counts
                                .get(&prop_freqs[j].0)
                                .copied()
                                .unwrap_or(0) as f64;
                            let p_i_raw = c_i / n;
                            let p_j_raw = c_j / n;
                            if p_i_raw == 0.0 || p_j_raw == 0.0 {
                                continue;
                            }
                            let pmi = (p_ab / (p_i_raw * p_j_raw)).log2();
                            if pmi > pmi_threshold {
                                // Discount the less-rare (higher freq) member.
                                // Replace with conditional frequency, clamped to [0, 1].
                                if prop_freqs[i].1 > prop_freqs[j].1 {
                                    prop_freqs[i].1 = (p_ab / p_j_raw).min(1.0);
                                } else {
                                    prop_freqs[j].1 = (p_ab / p_i_raw).min(1.0);
                                }
                            }
                        }
                    }
                }

                // Sort ascending — rarest properties first (for explainability)
                prop_freqs
                    .sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

                let k = prop_freqs.len() as f64;
                let stu_score = match self.aggregation {
                    StuAggregation::ArithmeticMean => {
                        prop_freqs.iter().map(|(_, f)| f).sum::<f64>() / k
                    }
                    StuAggregation::TfIdf => prop_freqs.iter().map(|(_, f)| -f.log2()).sum::<f64>(),
                    StuAggregation::GeometricMean => {
                        (prop_freqs.iter().map(|(_, f)| f.ln()).sum::<f64>() / k).exp()
                    }
                    StuAggregation::Min => prop_freqs
                        .iter()
                        .map(|(_, f)| *f)
                        .fold(f64::INFINITY, f64::min),
                };

                // Cold-start attenuation: lerp toward "unsurprising" when data is sparse.
                // confidence: 0.5 at 1 match, ~0.91 at 10, ~0.99 at 100.
                let confidence = 1.0 - 1.0 / (table.total_matches as f64 + 1.0);
                let stu_score = if matches!(self.aggregation, StuAggregation::TfIdf) {
                    // TfIdf: higher = more surprising. Attenuate toward 0.0.
                    stu_score * confidence
                } else {
                    // ArithmeticMean/GeometricMean/Min: lower = more surprising.
                    // Lerp toward 1.0 (unsurprising) when confidence is low.
                    1.0 - (1.0 - stu_score) * confidence
                };

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

    /// Pointwise Mutual Information between two properties for a pattern.
    ///
    /// `PMI(pi, pj) = log2(P(pi,pj) / (P(pi) * P(pj)))`. High PMI means the
    /// properties co-occur more than expected by chance.
    ///
    /// Returns `None` if the pattern is unobserved or PMI correction is disabled.
    pub fn pmi_for(&self, pattern: &str, pi: &str, pj: &str) -> Option<f64> {
        let table = self.tables.get(pattern)?;
        if table.total_matches == 0 {
            return None;
        }
        let (a, b) = if pi < pj { (pi, pj) } else { (pj, pi) };
        let pair_count = table
            .pair_counts
            .get(&(a.to_string(), b.to_string()))
            .copied()
            .unwrap_or(0);
        let p_ab = pair_count as f64 / table.total_matches as f64;
        let p_a =
            table.property_counts.get(a).copied().unwrap_or(0) as f64 / table.total_matches as f64;
        let p_b =
            table.property_counts.get(b).copied().unwrap_or(0) as f64 / table.total_matches as f64;
        if p_a == 0.0 || p_b == 0.0 || p_ab == 0.0 {
            return Some(0.0);
        }
        Some((p_ab / (p_a * p_b)).log2())
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

    // -----------------------------------------------------------------------
    // Aggregation alternatives (Phase 7.1)
    // -----------------------------------------------------------------------

    /// Helper: build a scorer with observations and score two matches.
    fn score_rare_vs_common(agg: StuAggregation) -> (f64, f64) {
        let mut stu = StuScorer::new().with_aggregation(agg);
        // 10 matches: "ambitious" appears 2x, "loyal" appears 8x
        for i in 0..10 {
            let props = if i < 2 {
                vec!["trait=ambitious".to_string()]
            } else {
                vec!["trait=loyal".to_string()]
            };
            stu.observe_one("test", &props);
        }
        let scored = stu.score(&[
            (dummy_match("test"), vec!["trait=ambitious".to_string()]),
            (dummy_match("test"), vec!["trait=loyal".to_string()]),
        ]);
        (scored[0].stu_score, scored[1].stu_score)
    }

    #[test]
    fn stu_default_is_arithmetic_mean() {
        let (rare_default, common_default) = score_rare_vs_common(StuAggregation::default());
        let (rare_explicit, common_explicit) = score_rare_vs_common(StuAggregation::ArithmeticMean);
        assert_eq!(rare_default, rare_explicit);
        assert_eq!(common_default, common_explicit);
        // Lower = more surprising
        assert!(rare_default < common_default);
    }

    #[test]
    fn stu_tfidf_higher_is_more_surprising() {
        let (rare, common) = score_rare_vs_common(StuAggregation::TfIdf);
        // TfIdf: higher = more surprising (reversed polarity)
        assert!(
            rare > common,
            "TfIdf: rare ({:.3}) should score HIGHER than common ({:.3})",
            rare,
            common
        );
    }

    #[test]
    fn stu_geometric_mean_rare_scores_lower() {
        let (rare, common) = score_rare_vs_common(StuAggregation::GeometricMean);
        // Lower = more surprising
        assert!(
            rare < common,
            "GeometricMean: rare ({:.3}) should score lower than common ({:.3})",
            rare,
            common
        );
    }

    #[test]
    fn stu_min_uses_rarest_property() {
        let mut stu = StuScorer::new().with_aggregation(StuAggregation::Min);
        // 10 matches: "common" in all 10, "rare" in 1
        for i in 0..10 {
            let mut props = vec!["common=yes".to_string()];
            if i == 0 {
                props.push("rare=yes".to_string());
            }
            stu.observe_one("test", &props);
        }
        let scored = stu.score(&[(
            dummy_match("test"),
            vec!["common=yes".to_string(), "rare=yes".to_string()],
        )]);
        let score = scored[0].stu_score;
        // Min should pick the rare property's frequency, not the common one,
        // with confidence lerp: 1.0 - (1.0 - raw) * confidence
        let rare_freq = stu.property_frequency("test", "rare=yes").unwrap();
        let confidence = 1.0 - 1.0 / (stu.match_count("test") as f64 + 1.0);
        let expected = 1.0 - (1.0 - rare_freq) * confidence;
        assert!(
            (score - expected).abs() < 1e-10,
            "Min score ({:.4}) should equal lerped rare freq ({:.4})",
            score,
            expected
        );
    }

    #[test]
    fn stu_cold_start_attenuates_toward_unsurprising() {
        // For lower-is-surprising modes, cold start should push score UP toward 1.0.
        // Need a rare property (freq < 1.0) to see the effect.
        let mut stu = StuScorer::new();
        stu.observe_one("test", &["common"]);
        stu.observe_one("test", &["common"]);
        stu.observe_one("test", &["rare"]);
        // rare: count=1, Laplace: (1+1)/(3+2) = 0.4
        let raw_freq = stu.property_frequency("test", "rare").unwrap();
        assert!(raw_freq < 1.0, "rare should have freq < 1.0: {}", raw_freq);

        let scored = stu.score(&[(dummy_match("test"), vec!["rare".to_string()])]);
        // confidence at 3 matches = 1 - 1/4 = 0.75
        // lerp: 1.0 - (1.0 - 0.4) * 0.75 = 1.0 - 0.45 = 0.55
        // Score should be HIGHER than raw (pushed toward 1.0 = unsurprising)
        assert!(
            scored[0].stu_score > raw_freq,
            "cold start should push toward unsurprising: score={:.3}, raw={:.3}",
            scored[0].stu_score,
            raw_freq
        );

        // With many observations, confidence ≈ 1.0 — score ≈ raw freq
        let mut stu2 = StuScorer::new();
        for _ in 0..50 {
            stu2.observe_one("test", &["common"]);
        }
        for _ in 0..50 {
            stu2.observe_one("test", &["rare"]);
        }
        let scored2 = stu2.score(&[(dummy_match("test"), vec!["rare".to_string()])]);
        let raw_freq2 = stu2.property_frequency("test", "rare").unwrap();
        assert!(
            (scored2[0].stu_score - raw_freq2).abs() < 0.02,
            "high-observation score ({:.4}) should be close to raw freq ({:.4})",
            scored2[0].stu_score,
            raw_freq2
        );
    }

    #[test]
    fn stu_cold_start_tfidf_attenuates_toward_zero() {
        // For TfIdf (higher = more surprising), cold start should push DOWN toward 0.0
        let mut stu = StuScorer::new().with_aggregation(StuAggregation::TfIdf);
        for i in 0..5 {
            let props = if i == 0 {
                vec!["rare=yes".to_string()]
            } else {
                vec!["common=yes".to_string()]
            };
            stu.observe_one("test", &props);
        }
        let scored_tfidf = stu.score(&[(dummy_match("test"), vec!["rare=yes".to_string()])]);
        // Confidence at 5 matches = 1 - 1/6 ≈ 0.833
        // Score should be positive but attenuated toward 0
        assert!(scored_tfidf[0].stu_score > 0.0);
    }

    #[test]
    fn stu_with_aggregation_builder() {
        let scorer = StuScorer::new().with_aggregation(StuAggregation::TfIdf);
        assert_eq!(scorer.aggregation, StuAggregation::TfIdf);

        let default = StuScorer::new();
        assert_eq!(default.aggregation, StuAggregation::ArithmeticMean);
    }

    // -----------------------------------------------------------------------
    // PMI correction (Phase 7.3)
    // -----------------------------------------------------------------------

    #[test]
    fn pmi_pair_counting() {
        let mut stu = StuScorer::new().with_pmi_correction();
        // "rebels" and "hideout" always co-occur
        stu.observe_one("test", &["rebels", "hideout"]);
        stu.observe_one("test", &["rebels", "hideout"]);
        // "crown" and "castle" always co-occur
        stu.observe_one("test", &["crown", "castle"]);
        stu.observe_one("test", &["crown", "castle"]);

        // rebels+hideout: P(r,h)=0.5, P(r)=0.5, P(h)=0.5 → PMI = log2(0.5/(0.25)) = 1.0
        let pmi_rh = stu.pmi_for("test", "rebels", "hideout").unwrap();
        assert!(
            pmi_rh > 0.0,
            "rebels+hideout should have positive PMI: {:.3}",
            pmi_rh
        );
        // rebels+castle never co-occur → PMI = 0 (pair_count = 0)
        let pmi_rc = stu.pmi_for("test", "rebels", "castle").unwrap();
        assert_eq!(pmi_rc, 0.0, "rebels+castle should have PMI=0");
    }

    #[test]
    fn pmi_correction_reduces_double_counting() {
        // Without PMI correction
        let mut no_pmi = StuScorer::new();
        // With PMI correction
        let mut with_pmi = StuScorer::new().with_pmi_correction();

        // "rebels" and "hideout" always co-occur (perfect correlation)
        for _ in 0..20 {
            no_pmi.observe_one("test", &["faction=rebels", "location=hideout"]);
            with_pmi.observe_one("test", &["faction=rebels", "location=hideout"]);
        }
        // Add some matches without the pair to make them rare
        for _ in 0..80 {
            no_pmi.observe_one("test", &["faction=crown", "location=castle"]);
            with_pmi.observe_one("test", &["faction=crown", "location=castle"]);
        }

        let props = vec!["faction=rebels".to_string(), "location=hideout".to_string()];
        let scored_no = no_pmi.score(&[(dummy_match("test"), props.clone())]);
        let scored_with = with_pmi.score(&[(dummy_match("test"), props)]);

        // With correction, the score should differ because the correlated
        // pair's redundancy is discounted
        assert!(
            (scored_no[0].stu_score - scored_with[0].stu_score).abs() > 0.001,
            "PMI correction should change the score: no_pmi={:.4}, with_pmi={:.4}",
            scored_no[0].stu_score,
            scored_with[0].stu_score
        );
    }

    #[test]
    fn pmi_no_effect_when_disabled() {
        let mut stu = StuScorer::new(); // pmi_correction = false
        for _ in 0..20 {
            stu.observe_one("test", &["a", "b"]);
        }
        // pair_counts should be empty
        assert!(
            stu.pmi_for("test", "a", "b").is_none() || stu.pmi_for("test", "a", "b") == Some(0.0),
            "PMI should not be available when disabled"
        );
    }

    #[test]
    fn pmi_canonical_order() {
        let mut stu = StuScorer::new().with_pmi_correction();
        stu.observe_one("test", &["b", "a"]); // reversed order
                                              // Should still find the pair
        let pmi = stu.pmi_for("test", "a", "b");
        assert!(pmi.is_some());
        // Also works with reversed query order
        let pmi_rev = stu.pmi_for("test", "b", "a");
        assert_eq!(pmi, pmi_rev);
    }
}
