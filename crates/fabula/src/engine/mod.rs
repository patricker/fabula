//! The sift engine -- pattern registration, batch evaluation, incremental
//! matching, and gap analysis.
//!
//! This is the core of fabula. The engine maintains registered patterns and
//! their partial match state. It can evaluate patterns in batch mode (against
//! a snapshot) or incrementally (as new edges arrive).
//!
//! ## Research foundation
//!
//! - Kreminski et al. (2019) "Felt: A Simple Story Sifter" (ICIDS 2019)
//!   -- Core sifting model: patterns as Datalog-like queries with logic
//!   variables over EAV graphs. Plant/payoff tracking for narrative causality.
//! - Kreminski et al. (2021) "Winnow: A Domain-Specific Language for
//!   Incremental Story Sifting" (AIIDE 2021) -- Incremental matching with
//!   negation windows (`unless-event ... between`). The 4-phase algorithm
//!   (negation check → initiation → advancement → cleanup) is adapted from
//!   Winnow's streaming evaluation model.
//! - Rete networks (Forgy 1982) -- Pattern lifecycle conventions: disabled
//!   patterns kill active partial matches immediately; fingerprint-based
//!   deduplication prevents unbounded PM accumulation.
//!
//! ## Intentional omissions from Felt
//!
//! Felt is both a sifting engine and an action-selection framework. Fabula
//! implements only the sifting/pattern-matching side:
//! - No `registerAction` / `possibleActions` / `realizeEvent`
//! - No `registerEffectHandler` / `processEffect` / `addEvent`
//!
//! Fabula detects patterns; it doesn't generate events. Action selection and
//! effect processing belong to the simulation layer that feeds edges into fabula.

use crate::datasource::DataSource;
use crate::interval::Interval;
use crate::pattern::*;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

mod eval;
mod free;
mod let_evaluator;
mod types;

pub use free::{
    evaluate_pattern, evaluate_pattern_at, evaluate_pattern_first, evaluate_pattern_limit,
    gap_analysis, gap_analysis_at,
};
pub use let_evaluator::{DefaultLetEvaluator, LetEvaluator, NoLetEvaluator};
pub use types::*;

// ---------------------------------------------------------------------------
// The engine
// ---------------------------------------------------------------------------

/// The sift engine. Generic over node, label, value, and time types.
///
/// Decoupled from [`DataSource`] -- the engine stores patterns and partial
/// matches using the four type parameters directly. Methods that need graph
/// access take `&impl DataSource<N=N, L=L, V=V, T=T>` as a parameter,
/// allowing the engine to outlive any particular DataSource instance.
///
/// `Clone` creates an independent copy of all engine state -- patterns,
/// partial matches, metrics, enabled flags. Use this for speculative
/// evaluation (MCTS forking): clone the engine, evaluate on a forked
/// DataSource, score the result, discard or commit.
///
/// ```rust,ignore
/// // Fork: clone engine + fork data source
/// let mut fork_engine = engine.clone();
/// let fork_ds = fork_data_source(&ds);
///
/// // Speculate: add a hypothetical edge and see what matches
/// fork_engine.on_edge_added(&fork_ds, &source, &label, &value, &interval);
/// let delta = fork_engine.tick_delta(&events, 50);
///
/// // Score and decide whether to commit this branch
/// let score = evaluate_narrative_quality(&delta);
/// if score > best_score { best_engine = fork_engine; }
/// ```
pub struct SiftEngine<N: Debug + Clone, L, V: Debug + Clone, T: Clone, E> {
    pub(super) patterns: Vec<Pattern<L, V>>,
    pub(super) partial_matches: Vec<PartialMatch<N, V, T>>,
    pub(super) next_match_id: usize,
    pub(super) stats: EngineStats,
    // Per-pattern lifecycle state
    pub(super) enabled: Vec<bool>,
    pub(super) last_advanced_tick: Vec<u64>,
    pub(super) completion_count: Vec<u64>,
    pub(super) advancement_count: Vec<u64>,
    pub(super) negation_count: Vec<u64>,
    pub(super) tick_counter: u64,
    pub(super) plant_payoff_pairs: Vec<PlantPayoffPair>,
    // Per-tick event accumulators for end_tick().
    // Populated by on_edge_added(), cleared by end_tick().
    pub(super) tick_advanced: HashSet<String>,
    pub(super) tick_completed: HashSet<String>,
    pub(super) tick_negated: HashSet<String>,
    pub(super) tick_expired: HashSet<String>,
    /// Let-binding evaluator. Use `DefaultLetEvaluator` if your `V`
    /// implements [`ArithmeticValue`]; `NoLetEvaluator` for let-free
    /// patterns; or supply your own `LetEvaluator` impl for foreign V.
    pub(super) let_evaluator: E,
}

/// Convenience alias: extract type params from a [`DataSource`] impl.
///
/// Defaults the let-evaluator type to [`DefaultLetEvaluator`]. Use the
/// second type parameter to override (e.g., `SiftEngineFor<MemGraph, NoLetEvaluator>`).
///
/// ```rust,ignore
/// // Instead of SiftEngine<String, String, MemValue, i64, DefaultLetEvaluator>:
/// let engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
/// ```
pub type SiftEngineFor<DS, E = DefaultLetEvaluator> = SiftEngine<
    <DS as DataSource>::N,
    <DS as DataSource>::L,
    <DS as DataSource>::V,
    <DS as DataSource>::T,
    E,
>;

// NOTE: tick accumulators are NOT included in Clone -- a forked engine
// starts with empty accumulators (no events in its new timeline).

// ---------------------------------------------------------------------------
// Block 1: Lifecycle methods -- lighter bounds, no DataSource needed.
// wk-sift can construct and register patterns without T: Sub + NumericTime.
// ---------------------------------------------------------------------------

impl<N, L, V, T, E> SiftEngine<N, L, V, T, E>
where
    N: Eq + Hash + Clone + Debug,
    L: Eq + Hash + Clone + Debug,
    V: PartialEq + PartialOrd + Clone + Debug + Hash,
    T: Ord + Clone + Debug + Hash,
{
    /// Create a new empty engine with the given let evaluator.
    ///
    /// Pick `DefaultLetEvaluator` if `V: ArithmeticValue`, `NoLetEvaluator`
    /// for let-free use, or your own `LetEvaluator` impl for foreign V.
    pub fn new(let_evaluator: E) -> Self {
        Self {
            patterns: Vec::new(),
            partial_matches: Vec::new(),
            next_match_id: 0,
            stats: EngineStats::default(),
            enabled: Vec::new(),
            last_advanced_tick: Vec::new(),
            completion_count: Vec::new(),
            advancement_count: Vec::new(),
            negation_count: Vec::new(),
            tick_counter: 0,
            plant_payoff_pairs: Vec::new(),
            tick_advanced: HashSet::new(),
            tick_completed: HashSet::new(),
            tick_negated: HashSet::new(),
            tick_expired: HashSet::new(),
            let_evaluator,
        }
    }

    /// Register a pattern. Returns its index.
    pub fn register(&mut self, pattern: Pattern<L, V>) -> usize {
        let idx = self.patterns.len();
        self.patterns.push(pattern);
        self.enabled.push(true);
        self.last_advanced_tick.push(0);
        self.completion_count.push(0);
        self.advancement_count.push(0);
        self.negation_count.push(0);
        idx
    }

    /// All registered patterns.
    pub fn patterns(&self) -> &[Pattern<L, V>] {
        &self.patterns
    }

    /// All partial matches (including completed ones).
    pub fn partial_matches(&self) -> &[PartialMatch<N, V, T>] {
        &self.partial_matches
    }

    /// Active partial matches for a specific pattern (by name).
    pub fn active_matches_for(&self, name: &str) -> Vec<&PartialMatch<N, V, T>> {
        self.partial_matches
            .iter()
            .filter(|pm| {
                pm.state == MatchState::Active
                    && self
                        .patterns
                        .get(pm.pattern_idx)
                        .is_some_and(|p| p.name == name)
            })
            .collect()
    }

    /// Drain completed matches, removing them from internal storage.
    /// Cumulative operation counters.
    pub fn stats(&self) -> &EngineStats {
        &self.stats
    }

    /// Reset all counters to zero.
    pub fn reset_stats(&mut self) {
        self.stats = EngineStats::default();
    }

    // -----------------------------------------------------------------------
    // Pattern lifecycle
    // -----------------------------------------------------------------------

    /// Enable or disable a pattern. Disabled patterns are skipped during
    /// `evaluate()` and `on_edge_added()`. When disabling, all active PMs
    /// for the pattern are killed (Rete convention: stale PMs become invalid).
    pub fn set_pattern_enabled(&mut self, idx: usize, enabled: bool) {
        if idx < self.enabled.len() {
            self.enabled[idx] = enabled;
            if !enabled {
                // Kill all active PMs for this pattern
                for pm in &mut self.partial_matches {
                    if pm.pattern_idx == idx && pm.state == MatchState::Active {
                        pm.state = MatchState::Dead;
                    }
                }
                self.partial_matches
                    .retain(|pm| pm.state != MatchState::Dead);
            }
        }
    }

    /// Check if a pattern is enabled.
    pub fn is_pattern_enabled(&self, idx: usize) -> bool {
        self.enabled.get(idx).copied().unwrap_or(false)
    }

    /// Soft-delete a pattern. Disables it and kills all its PMs.
    /// The pattern stays in the Vec (index stability) but will never match again.
    pub fn deregister(&mut self, idx: usize) {
        self.set_pattern_enabled(idx, false);
    }

    /// Kill all active partial matches whose bindings contain the given node.
    ///
    /// Use this when an entity is removed from the simulation (death, departure,
    /// despawn). Any in-progress patterns involving that entity become invalid
    /// and should be cleaned up in one call rather than waiting for them to
    /// expire or stall.
    ///
    /// Returns the number of PMs killed.
    pub fn kill_pms_involving(&mut self, node: &N) -> usize
    where
        N: PartialEq,
    {
        let mut killed = 0;
        for pm in &mut self.partial_matches {
            if pm.state != MatchState::Active {
                continue;
            }
            let involves = pm.bindings.values().any(|bv| match bv {
                BoundValue::Node(n) => n == node,
                BoundValue::Value(_) => false,
            });
            if involves {
                pm.state = MatchState::Dead;
                killed += 1;
            }
        }
        self.partial_matches
            .retain(|pm| pm.state != MatchState::Dead);
        killed
    }

    /// Advance the tick counter. Call once per simulation step.
    /// Used for staleness detection. Does NOT produce a delta summary --
    /// use [`end_tick`] for the happy path, or [`tick_delta`] with
    /// manually collected events for filtered deltas.
    pub fn tick(&mut self) {
        self.tick_counter += 1;
    }

    /// End the current tick: increment the tick counter, build a
    /// [`TickDelta`] from accumulated events, and clear the accumulators.
    ///
    /// This is the happy-path API for GM consumers. Call `on_edge_added()`
    /// for each edge in the tick (events accumulate internally), then call
    /// `end_tick()` to get the summary.
    ///
    /// ```rust,ignore
    /// for edge in new_edges {
    ///     engine.on_edge_added(&ds, &src, &label, &val, &interval);
    /// }
    /// let delta = engine.end_tick(50); // stale threshold = 50 ticks
    /// if !delta.stalled.is_empty() { /* alert GM */ }
    /// ```
    pub fn end_tick(&mut self, stale_threshold: u64) -> (TickDelta, Vec<SiftEvent<N, V>>) {
        self.tick_counter += 1;

        // Scan for expired partial matches (deadline exceeded).
        let mut expired_events = Vec::new();
        for pm in &mut self.partial_matches {
            if pm.state != MatchState::Active {
                continue;
            }
            let pattern = &self.patterns[pm.pattern_idx];
            if let Some(deadline) = pattern.deadline_ticks {
                let elapsed = self.tick_counter.saturating_sub(pm.created_at_tick);
                if elapsed > deadline {
                    expired_events.push(SiftEvent::Expired {
                        pattern: pattern.name.clone(),
                        match_id: pm.id,
                        bindings: pm.bindings.clone(),
                        stage_reached: pm.next_stage,
                        ticks_elapsed: elapsed,
                        metadata: pattern.metadata.clone(),
                    });
                    pm.state = MatchState::Dead;
                    self.tick_expired.insert(pattern.name.clone());
                }
            }
        }
        // Inactivity-based pruning
        for pm in &mut self.partial_matches {
            if pm.state != MatchState::Active {
                continue;
            }
            if let Some(threshold) = self.patterns[pm.pattern_idx].inactivity_threshold {
                if self.tick_counter - pm.last_advanced_tick >= threshold {
                    pm.state = MatchState::Dead;
                }
            }
        }
        self.partial_matches
            .retain(|pm| pm.state != MatchState::Dead);

        // Filter expired events for private patterns.
        expired_events.retain(|e| {
            if let SiftEvent::Expired { pattern, .. } = e {
                !self
                    .patterns
                    .iter()
                    .any(|p| p.name == *pattern && p.private)
            } else {
                true
            }
        });

        let stalled: Vec<String> = self
            .stale_patterns(stale_threshold)
            .iter()
            .filter_map(|&idx| self.patterns.get(idx))
            .filter(|p| !p.private)
            .map(|p| p.name.clone())
            .collect();

        let active_pm_count = self
            .partial_matches
            .iter()
            .filter(|pm| pm.state == MatchState::Active)
            .count();

        let is_private = |name: &String| self.patterns.iter().any(|p| p.name == *name && p.private);
        let mut advanced: Vec<String> = self
            .tick_advanced
            .drain()
            .filter(|n| !is_private(n))
            .collect();
        let mut completed: Vec<String> = self
            .tick_completed
            .drain()
            .filter(|n| !is_private(n))
            .collect();
        let mut negated: Vec<String> = self
            .tick_negated
            .drain()
            .filter(|n| !is_private(n))
            .collect();
        let mut expired: Vec<String> = self
            .tick_expired
            .drain()
            .filter(|n| !is_private(n))
            .collect();
        advanced.sort();
        completed.sort();
        negated.sort();
        expired.sort();

        let delta = TickDelta {
            advanced,
            completed,
            negated,
            expired,
            stalled,
            active_pm_count,
        };
        (delta, expired_events)
    }

    /// Current tick counter.
    pub fn current_tick(&self) -> u64 {
        self.tick_counter
    }

    /// Per-pattern lifecycle metrics.
    pub fn pattern_metrics(&self, idx: usize) -> Option<PatternMetrics> {
        if idx >= self.patterns.len() {
            return None;
        }
        let active_pm_count = self
            .partial_matches
            .iter()
            .filter(|pm| pm.pattern_idx == idx && pm.state == MatchState::Active)
            .count();
        Some(PatternMetrics {
            enabled: self.enabled[idx],
            last_advanced_tick: self.last_advanced_tick[idx],
            completion_count: self.completion_count[idx],
            advancement_count: self.advancement_count[idx],
            negation_count: self.negation_count[idx],
            active_pm_count,
        })
    }

    /// Find patterns that have not advanced for at least `threshold` ticks
    /// but still have active partial matches (stale plants).
    pub fn stale_patterns(&self, threshold: u64) -> Vec<usize> {
        (0..self.patterns.len())
            .filter(|&idx| {
                self.enabled[idx]
                    && self
                        .tick_counter
                        .saturating_sub(self.last_advanced_tick[idx])
                        >= threshold
                    && self
                        .partial_matches
                        .iter()
                        .any(|pm| pm.pattern_idx == idx && pm.state == MatchState::Active)
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // Plant/payoff tracking
    // -----------------------------------------------------------------------

    /// Register a plant/payoff pair for Chekhov's gun tracking.
    ///
    /// The plant pattern is narrative setup ("the gun on the mantelpiece");
    /// the payoff pattern is the resolution ("the gun fires"). When the plant
    /// has active PMs and the payoff hasn't completed, the setup is "in flight."
    /// When the payoff completes, the setup is resolved.
    ///
    /// Inspired by Chatman (1978) "Story and Discourse" -- kernel (plot-critical)
    /// vs satellite (texture) events. Plants are satellites that become kernels
    /// when they resolve.
    ///
    /// `shared_binding` optionally constrains the pair: the payoff only
    /// counts as resolving the plant if both share a binding with this
    /// variable name pointing to the same entity.
    pub fn register_plant_payoff(
        &mut self,
        plant_idx: usize,
        payoff_idx: usize,
        shared_binding: Option<String>,
    ) {
        self.plant_payoff_pairs.push(PlantPayoffPair {
            plant_idx,
            payoff_idx,
            shared_binding,
        });
    }

    /// All registered plant/payoff pairs.
    pub fn plant_payoff_pairs(&self) -> &[PlantPayoffPair] {
        &self.plant_payoff_pairs
    }

    /// Status of all plant/payoff pairs. Shows which setups are unresolved,
    /// which are stale (Chekhov's guns gathering dust), and which have been
    /// paid off.
    pub fn plant_status(&self, stale_threshold: u64) -> Vec<PlantStatus> {
        self.plant_payoff_pairs
            .iter()
            .filter_map(|pair| {
                let plant = self.patterns.get(pair.plant_idx)?;
                let payoff = self.patterns.get(pair.payoff_idx)?;

                let active_plants = self
                    .partial_matches
                    .iter()
                    .filter(|pm| pm.pattern_idx == pair.plant_idx && pm.state == MatchState::Active)
                    .count();

                let ticks_since = self
                    .tick_counter
                    .saturating_sub(self.last_advanced_tick[pair.plant_idx]);

                let stale = active_plants > 0 && ticks_since >= stale_threshold;

                Some(PlantStatus {
                    plant_pattern: plant.name.clone(),
                    payoff_pattern: payoff.name.clone(),
                    active_plants,
                    payoff_completions: self.completion_count[pair.payoff_idx],
                    ticks_since_plant_advanced: ticks_since,
                    stale,
                })
            })
            .collect()
    }

    /// Compute a delta summary from the events produced by `on_edge_added()`.
    ///
    /// Call after each tick's `on_edge_added()` calls. Pass the events returned
    /// by the engine and a staleness threshold (patterns with active PMs that
    /// haven't advanced for this many ticks are reported as stalled).
    ///
    /// ```rust,ignore
    /// engine.tick();
    /// let events = engine.on_edge_added(&graph, ...);
    /// let delta = engine.tick_delta(&events, 50);
    /// if !delta.stalled.is_empty() { /* alert GM about stale plants */ }
    /// ```
    pub fn tick_delta(&self, events: &[SiftEvent<N, V>], stale_threshold: u64) -> TickDelta {
        let mut advanced = Vec::new();
        let mut completed = Vec::new();
        let mut negated = Vec::new();
        let mut expired = Vec::new();
        let mut seen_advanced = HashSet::new();
        let mut seen_completed = HashSet::new();
        let mut seen_negated = HashSet::new();
        let mut seen_expired = HashSet::new();

        for event in events {
            match event {
                SiftEvent::Advanced { pattern, .. } => {
                    if seen_advanced.insert(pattern.clone()) {
                        advanced.push(pattern.clone());
                    }
                }
                SiftEvent::Completed { pattern, .. } => {
                    if seen_completed.insert(pattern.clone()) {
                        completed.push(pattern.clone());
                    }
                }
                SiftEvent::Negated { pattern, .. } => {
                    if seen_negated.insert(pattern.clone()) {
                        negated.push(pattern.clone());
                    }
                }
                SiftEvent::Expired { pattern, .. } => {
                    if seen_expired.insert(pattern.clone()) {
                        expired.push(pattern.clone());
                    }
                }
            }
        }

        let stalled: Vec<String> = self
            .stale_patterns(stale_threshold)
            .iter()
            .filter_map(|&idx| self.patterns.get(idx))
            .filter(|p| !p.private)
            .map(|p| p.name.clone())
            .collect();

        let active_pm_count = self
            .partial_matches
            .iter()
            .filter(|pm| pm.state == MatchState::Active)
            .count();

        TickDelta {
            advanced,
            completed,
            negated,
            expired,
            stalled,
            active_pm_count,
        }
    }

    pub fn drain_completed(&mut self) -> Vec<Match<N, V, T>> {
        let mut completed = Vec::new();
        self.partial_matches.retain(|pm| {
            if pm.state == MatchState::Complete {
                completed.push(Match {
                    pattern: self.patterns[pm.pattern_idx].name.clone(),
                    pattern_idx: Some(pm.pattern_idx),
                    bindings: pm.bindings.clone(),
                    intervals: pm.intervals.clone(),
                    metadata: self.patterns[pm.pattern_idx].metadata.clone(),
                });
                false
            } else {
                true
            }
        });
        // Filter out matches from private patterns.
        completed.retain(|m| {
            !self
                .patterns
                .iter()
                .any(|p| p.name == m.pattern && p.private)
        });
        completed
    }

    /// Compute a deterministic dedup hash for a partial match.
    ///
    /// Prevents duplicate PMs from accumulating -- a key concern from Rete
    /// network literature where unbounded token accumulation degrades
    /// performance. Uses order-independent XOR of per-entry hashes so
    /// HashMap iteration order doesn't matter. Zero allocation.
    fn compute_fingerprint_with_rep(
        pattern_idx: usize,
        next_stage: usize,
        bindings: &HashMap<String, BoundValue<N, V>>,
        intervals: &HashMap<String, Interval<T>>,
        repetition_count: u32,
    ) -> u64 {
        Self::compute_fingerprint_full(
            pattern_idx,
            next_stage,
            bindings,
            intervals,
            repetition_count,
            0,
        )
    }

    fn compute_fingerprint_full(
        pattern_idx: usize,
        next_stage: usize,
        bindings: &HashMap<String, BoundValue<N, V>>,
        intervals: &HashMap<String, Interval<T>>,
        repetition_count: u32,
        matched_stages: u64,
    ) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut h = DefaultHasher::new();
        pattern_idx.hash(&mut h);
        next_stage.hash(&mut h);
        repetition_count.hash(&mut h);
        matched_stages.hash(&mut h);

        // XOR of per-entry hashes -- order-independent, no sorting needed.
        // Mix in map length to distinguish empty maps from self-cancelling entries.
        let mut bindings_xor: u64 = 0;
        for (k, v) in bindings {
            let mut entry_h = DefaultHasher::new();
            k.hash(&mut entry_h);
            v.hash(&mut entry_h);
            bindings_xor ^= entry_h.finish();
        }
        bindings.len().hash(&mut h);
        bindings_xor.hash(&mut h);

        let mut intervals_xor: u64 = 0;
        for (k, v) in intervals {
            let mut entry_h = DefaultHasher::new();
            k.hash(&mut entry_h);
            v.hash(&mut entry_h);
            intervals_xor ^= entry_h.finish();
        }
        intervals.len().hash(&mut h);
        intervals_xor.hash(&mut h);

        h.finish()
    }
}

/// Constructs an engine using `E::default()` as the let evaluator.
///
/// **Choose your evaluator deliberately.** `SiftEngine::default()` over
/// `NoLetEvaluator` produces an engine that silently drops every
/// let-bearing pattern (lets always evaluate to `None`, killing the
/// partial match). For let-free use this is the intended behavior, but
/// for arithmetic V types you almost certainly want
/// `SiftEngine::<_, _, _, _, DefaultLetEvaluator>::default()` — or
/// better, the explicit `SiftEngine::new(DefaultLetEvaluator)`.
impl<N, L, V, T, E> Default for SiftEngine<N, L, V, T, E>
where
    N: Eq + Hash + Clone + Debug,
    L: Eq + Hash + Clone + Debug,
    V: PartialEq + PartialOrd + Clone + Debug + Hash,
    T: Ord + Clone + Debug + Hash,
    E: Default,
{
    fn default() -> Self {
        Self::new(E::default())
    }
}

// Manual Clone: tick accumulators are intentionally empty in cloned engines.
// Do NOT replace with #[derive(Clone)] -- forked engines start fresh.
impl<N: Debug + Clone, L: Clone, V: Debug + Clone, T: Clone, E: Clone> Clone for SiftEngine<N, L, V, T, E> {
    /// Clone the entire engine state for speculative evaluation.
    ///
    /// Both the original and clone are independent -- advancing one
    /// does not affect the other. Use this for MCTS-style forking:
    /// clone the engine, evaluate on a forked graph, score, discard or commit.
    fn clone(&self) -> Self {
        Self {
            patterns: self.patterns.clone(),
            partial_matches: self.partial_matches.clone(),
            next_match_id: self.next_match_id,
            stats: self.stats.clone(),
            enabled: self.enabled.clone(),
            last_advanced_tick: self.last_advanced_tick.clone(),
            completion_count: self.completion_count.clone(),
            advancement_count: self.advancement_count.clone(),
            negation_count: self.negation_count.clone(),
            tick_counter: self.tick_counter,
            plant_payoff_pairs: self.plant_payoff_pairs.clone(),
            // Forked engine starts with empty tick accumulators
            tick_advanced: HashSet::new(),
            tick_completed: HashSet::new(),
            tick_negated: HashSet::new(),
            tick_expired: HashSet::new(),
            let_evaluator: self.let_evaluator.clone(),
        }
    }
}
