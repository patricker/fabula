//! Public type definitions for the sift engine.
//!
//! Match results, partial match state, events, gap analysis, stats, and
//! plant/payoff tracking types.

use crate::interval::Interval;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

/// Bindings + intervals pair used internally during evaluation.
pub(super) type MatchCandidate<N, V, T> = (HashMap<String, BoundValue<N, V>>, HashMap<String, Interval<T>>);

// ---------------------------------------------------------------------------
// Matches and events
// ---------------------------------------------------------------------------

/// A complete match — all stages satisfied, temporal constraints met,
/// negation windows clear.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Match<N: Debug + PartialEq, V: Debug + PartialEq, T: Debug + Clone + PartialEq> {
    /// Which pattern matched.
    pub pattern: String,
    /// Index of the pattern in the engine's registry.
    /// `Some(idx)` when produced by engine methods (`evaluate`, `drain_completed`),
    /// `None` when produced by the free `evaluate_pattern` function.
    pub pattern_idx: Option<usize>,
    /// Variable -> bound node or value.
    pub bindings: HashMap<String, BoundValue<N, V>>,
    /// Stage anchor variable -> matched time interval.
    pub intervals: HashMap<String, Interval<T>>,
    /// Metadata from the matched pattern, propagated for downstream consumers.
    pub metadata: HashMap<String, String>,
}

/// Order-independent hash of a HashMap. Uses XOR of per-entry hashes
/// so iteration order doesn't affect the result.
fn hash_map_order_independent<K: Hash, V: Hash, H: Hasher>(map: &HashMap<K, V>, state: &mut H) {
    let mut xor: u64 = 0;
    for (k, v) in map {
        let mut entry_hasher = std::collections::hash_map::DefaultHasher::new();
        k.hash(&mut entry_hasher);
        v.hash(&mut entry_hasher);
        xor ^= std::hash::Hasher::finish(&entry_hasher);
    }
    xor.hash(state);
    map.len().hash(state);
}

impl<N, V, T> Hash for Match<N, V, T>
where
    N: Debug + PartialEq + Hash,
    V: Debug + PartialEq + Hash,
    T: Debug + Clone + PartialEq + Hash,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pattern.hash(state);
        self.pattern_idx.hash(state);
        hash_map_order_independent(&self.bindings, state);
        hash_map_order_independent(&self.intervals, state);
        hash_map_order_independent(&self.metadata, state);
    }
}

/// A value bound to a variable — either a node reference or a data value.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BoundValue<N: Debug, V: Debug> {
    /// A graph node (can be followed as a source in subsequent clauses).
    Node(N),
    /// A data value (string, number, boolean — not traversable).
    Value(V),
}

impl<N: Debug + Hash, V: Debug + Hash> Hash for BoundValue<N, V> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            BoundValue::Node(n) => n.hash(state),
            BoundValue::Value(v) => v.hash(state),
        }
    }
}

impl<N: Debug + PartialEq, V: Debug + PartialEq> BoundValue<N, V> {
    /// Check if this bound value matches a data source value.
    ///
    /// Takes a `value_as_node` closure (typically `|v| ds.value_as_node(v)`)
    /// to determine if the value is a node reference. This decouples
    /// BoundValue from the DataSource trait.
    pub(crate) fn matches_value(
        &self,
        value_as_node: &impl Fn(&V) -> Option<N>,
        value: &V,
    ) -> bool {
        match self {
            BoundValue::Node(n) => {
                // The value must be a node reference to the same node
                value_as_node(value)
                    .is_some_and(|vn| &vn == n)
            }
            BoundValue::Value(v) => value == v,
        }
    }
}

/// A partial match — some stages satisfied, waiting for more events.
#[derive(Debug, Clone)]
pub struct PartialMatch<N: Debug + Clone, V: Debug + Clone, T: Clone> {
    /// Index of the pattern in the engine's pattern list.
    pub pattern_idx: usize,
    /// Variable -> bound value so far.
    pub bindings: HashMap<String, BoundValue<N, V>>,
    /// Intervals of matched stage anchors (for temporal constraint checking).
    pub intervals: HashMap<String, Interval<T>>,
    /// Index of the next stage to match (0-indexed).
    pub next_stage: usize,
    /// Current state.
    pub state: MatchState,
    /// Unique id for tracking.
    pub id: usize,
    /// Timestamp when this partial match was first initiated.
    pub created_at: T,
    /// Precomputed dedup hash of (pattern_idx, next_stage, bindings, intervals).
    pub fingerprint: u64,
    /// Engine tick at which this partial match was first created.
    /// Used for deadline-based expiration. Inherited on advancement.
    pub created_at_tick: u64,
}

/// State of a partial match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchState {
    /// Waiting for the next stage to match.
    Active,
    /// All stages matched — this is a complete match.
    Complete,
    /// Killed by a negation window.
    Dead,
}

/// Events emitted by incremental matching.
#[derive(Debug)]
pub enum SiftEvent<N: Debug, V: Debug> {
    /// A partial match advanced (new stage satisfied).
    Advanced {
        pattern: String,
        match_id: usize,
        stage_index: usize,
        metadata: HashMap<String, String>,
    },
    /// A pattern fully matched.
    Completed {
        pattern: String,
        match_id: usize,
        bindings: HashMap<String, BoundValue<N, V>>,
        metadata: HashMap<String, String>,
    },
    /// A partial match was killed by a negation.
    Negated {
        pattern: String,
        match_id: usize,
        /// Which negation clause's label triggered the kill.
        clause_label: String,
        /// The source node of the edge that triggered the kill.
        trigger_source: N,
        metadata: HashMap<String, String>,
    },
    /// A partial match expired — its pattern's deadline was exceeded.
    Expired {
        pattern: String,
        match_id: usize,
        bindings: HashMap<String, BoundValue<N, V>>,
        /// How far the PM got (next_stage index at time of expiry).
        stage_reached: usize,
        /// How many ticks elapsed since the PM was created.
        ticks_elapsed: u64,
        metadata: HashMap<String, String>,
    },
}

// ---------------------------------------------------------------------------
// Gap analysis
// ---------------------------------------------------------------------------

/// Result of `why_not` — clause-by-clause analysis of why a pattern didn't match.
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GapAnalysis {
    pub pattern: String,
    pub stages: Vec<StageAnalysis>,
}

impl GapAnalysis {
    /// Fraction of total clauses that matched (0.0 = nothing, 1.0 = full match).
    pub fn closeness(&self) -> f64 {
        let mut total = 0usize;
        let mut matched = 0usize;
        for stage in &self.stages {
            for clause in &stage.clauses {
                total += 1;
                if clause.matched {
                    matched += 1;
                }
            }
        }
        if total == 0 {
            0.0
        } else {
            matched as f64 / total as f64
        }
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StageAnalysis {
    pub anchor: String,
    pub status: StageStatus,
    pub clauses: Vec<ClauseAnalysis>,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StageStatus {
    Matched,
    PartiallyMatched { matched: usize, total: usize },
    Unmatched,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClauseAnalysis {
    pub description: String,
    pub matched: bool,
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Engine stats
// ---------------------------------------------------------------------------

/// Cumulative operation counters for performance analysis.
/// Incremented during `evaluate()` and `on_edge_added()`.
/// Reset with `engine.reset_stats()`.
#[derive(Debug, Clone, Default)]
pub struct EngineStats {
    /// Number of `on_edge_added()` (incremental) calls.
    pub total_on_edge_added: u64,
    /// Fingerprint work: initial dedup set builds + per-candidate checks.
    pub total_fingerprints: u64,
    /// Negation checks attempted (once per active PM per `on_edge_added`).
    pub total_negation_checks: u64,
    /// High-water mark of active partial matches.
    pub peak_active_pms: usize,
}

/// Per-pattern lifecycle metrics. Returned by [`SiftEngine::pattern_metrics`].
#[derive(Debug, Clone, Default)]
pub struct PatternMetrics {
    /// Whether the pattern is enabled for matching.
    pub enabled: bool,
    /// Last tick at which any PM for this pattern advanced or completed.
    pub last_advanced_tick: u64,
    /// Total completions (cumulative).
    pub completion_count: u64,
    /// Total stage advancements (cumulative).
    pub advancement_count: u64,
    /// Total negation kills (cumulative).
    pub negation_count: u64,
    /// Number of currently active partial matches.
    pub active_pm_count: usize,
}

/// Summary of what changed in one tick. Returned by [`SiftEngine::tick_delta`].
///
/// The GM uses this to assess narrative progress: which patterns are advancing
/// (setup), completing (payoff), dying (dead ends), or stalling (forgotten plants).
#[derive(Debug, Clone, Default)]
pub struct TickDelta {
    /// Patterns that had at least one PM advance this tick.
    pub advanced: Vec<String>,
    /// Patterns that completed this tick.
    pub completed: Vec<String>,
    /// Patterns that had PMs negated this tick.
    pub negated: Vec<String>,
    /// Patterns that had PMs expire (deadline exceeded) this tick.
    pub expired: Vec<String>,
    /// Patterns with active PMs that have not advanced for `stale_threshold` ticks.
    pub stalled: Vec<String>,
    /// Total active PM count across all patterns.
    pub active_pm_count: usize,
}

/// A registered plant/payoff pair. The GM declares that when the plant pattern
/// has an active PM, it is narrative setup ("Chekhov's gun on the mantelpiece").
/// When the payoff pattern completes, the setup is resolved ("the gun fires").
#[derive(Debug, Clone)]
pub struct PlantPayoffPair {
    /// Pattern index of the plant (setup).
    pub plant_idx: usize,
    /// Pattern index of the payoff (resolution).
    pub payoff_idx: usize,
    /// Optional shared variable that must match across the pair
    /// (e.g., same character in both plant and payoff).
    pub shared_binding: Option<String>,
}

/// Status of a single plant from [`SiftEngine::plant_status`].
#[derive(Debug, Clone)]
pub struct PlantStatus {
    /// Plant pattern name.
    pub plant_pattern: String,
    /// Payoff pattern name.
    pub payoff_pattern: String,
    /// Number of active plant PMs (unresolved setups).
    pub active_plants: usize,
    /// Number of payoff completions (resolved setups).
    pub payoff_completions: u64,
    /// Ticks since the plant pattern last advanced. High = Chekhov's gun gathering dust.
    pub ticks_since_plant_advanced: u64,
    /// Whether the plant is stale (no advancement for a long time with active PMs).
    pub stale: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closeness_empty_analysis() {
        let gap = GapAnalysis {
            pattern: "test".to_string(),
            stages: vec![],
        };
        assert_eq!(gap.closeness(), 0.0);
    }

    #[test]
    fn closeness_all_matched() {
        let gap = GapAnalysis {
            pattern: "test".to_string(),
            stages: vec![StageAnalysis {
                anchor: "e1".to_string(),
                status: StageStatus::Matched,
                clauses: vec![
                    ClauseAnalysis { description: "a".into(), matched: true, reason: None },
                    ClauseAnalysis { description: "b".into(), matched: true, reason: None },
                ],
            }],
        };
        assert_eq!(gap.closeness(), 1.0);
    }

    #[test]
    fn closeness_partial() {
        let gap = GapAnalysis {
            pattern: "test".to_string(),
            stages: vec![
                StageAnalysis {
                    anchor: "e1".to_string(),
                    status: StageStatus::Matched,
                    clauses: vec![
                        ClauseAnalysis { description: "a".into(), matched: true, reason: None },
                        ClauseAnalysis { description: "b".into(), matched: true, reason: None },
                    ],
                },
                StageAnalysis {
                    anchor: "e2".to_string(),
                    status: StageStatus::PartiallyMatched { matched: 1, total: 2 },
                    clauses: vec![
                        ClauseAnalysis { description: "c".into(), matched: true, reason: None },
                        ClauseAnalysis { description: "d".into(), matched: false, reason: Some("no match".into()) },
                    ],
                },
            ],
        };
        assert_eq!(gap.closeness(), 0.75); // 3 of 4
    }

    #[test]
    fn closeness_none_matched() {
        let gap = GapAnalysis {
            pattern: "test".to_string(),
            stages: vec![StageAnalysis {
                anchor: "e1".to_string(),
                status: StageStatus::Unmatched,
                clauses: vec![
                    ClauseAnalysis { description: "a".into(), matched: false, reason: None },
                ],
            }],
        };
        assert_eq!(gap.closeness(), 0.0);
    }
}
