//! Role-conditioned variant of [`StuScorer`](super::stu::StuScorer).
//!
//! Conditions per-property frequency on an entity-role attribute. Solves the
//! "villain doing villain things" problem: a villain's harmful actions have
//! high marginal surprise but low conditional surprise given their role.
//!
//! Observations and lookups are keyed on `(pattern, role)` instead of just
//! `pattern`. Math, aggregation modes, and Laplace smoothing are identical
//! to [`StuScorer`].

use crate::scoring::stu::PropertyTable;
use std::collections::HashMap;

/// Surprise scorer that conditions on entity role.
#[derive(Debug, Clone, Default)]
pub struct RoleConditionedStuScorer {
    /// `(pattern, role) -> PropertyTable`.
    tables: HashMap<(String, String), PropertyTable>,
}

impl RoleConditionedStuScorer {
    /// Create a new empty scorer with arithmetic mean aggregation (default).
    pub fn new() -> Self {
        Self::default()
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
