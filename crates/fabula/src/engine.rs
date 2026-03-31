//! The sift engine — pattern registration, batch evaluation, incremental
//! matching, and gap analysis.
//!
//! This is the core of fabula. The engine maintains registered patterns and
//! their partial match state. It can evaluate patterns in batch mode (against
//! a snapshot) or incrementally (as new edges arrive).
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

use crate::datasource::{DataSource, ValueConstraint};
use crate::interval::Interval;
use crate::pattern::*;
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Write};

/// Bindings + intervals pair used internally during evaluation.
type MatchCandidate<N, V, T> = (HashMap<String, BoundValue<N, V>>, HashMap<String, Interval<T>>);

// ---------------------------------------------------------------------------
// Matches and events
// ---------------------------------------------------------------------------

/// A complete match — all stages satisfied, temporal constraints met,
/// negation windows clear.
#[derive(Debug, Clone)]
pub struct Match<N: Debug, V: Debug> {
    /// Which pattern matched.
    pub pattern: String,
    /// Variable -> bound node or value.
    pub bindings: HashMap<String, BoundValue<N, V>>,
}

/// A value bound to a variable — either a node reference or a data value.
#[derive(Debug, Clone)]
pub enum BoundValue<N: Debug, V: Debug> {
    /// A graph node (can be followed as a source in subsequent clauses).
    Node(N),
    /// A data value (string, number, boolean — not traversable).
    Value(V),
}

impl<N: Debug + PartialEq, V: Debug + PartialEq> BoundValue<N, V> {
    /// Check if this bound value matches a data source value, using the
    /// data source's `value_as_node` to determine if the value is a node ref.
    fn matches_value<DS: DataSource<N = N, V = V>>(
        &self,
        ds: &DS,
        value: &V,
    ) -> bool {
        match self {
            BoundValue::Node(n) => {
                // The value must be a node reference to the same node
                ds.value_as_node(value)
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
    },
    /// A pattern fully matched.
    Completed {
        pattern: String,
        match_id: usize,
        bindings: HashMap<String, BoundValue<N, V>>,
    },
    /// A partial match was killed by a negation.
    Negated {
        pattern: String,
        match_id: usize,
        /// Which negation clause's label triggered the kill.
        clause_label: String,
        /// The source node of the edge that triggered the kill.
        trigger_source: N,
    },
}

// ---------------------------------------------------------------------------
// Gap analysis
// ---------------------------------------------------------------------------

/// Result of `why_not` — clause-by-clause analysis of why a pattern didn't match.
#[derive(Debug)]
pub struct GapAnalysis {
    pub pattern: String,
    pub stages: Vec<StageAnalysis>,
}

#[derive(Debug)]
pub struct StageAnalysis {
    pub anchor: String,
    pub status: StageStatus,
    pub clauses: Vec<ClauseAnalysis>,
}

#[derive(Debug)]
pub enum StageStatus {
    Matched,
    PartiallyMatched { matched: usize, total: usize },
    Unmatched,
}

#[derive(Debug)]
pub struct ClauseAnalysis {
    pub description: String,
    pub matched: bool,
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// The engine
// ---------------------------------------------------------------------------

/// The sift engine. Generic over a [`DataSource`] implementation.
pub struct SiftEngine<DS: DataSource> {
    patterns: Vec<Pattern<DS::L, DS::V>>,
    partial_matches: Vec<PartialMatch<DS::N, DS::V, DS::T>>,
    next_match_id: usize,
}

impl<DS: DataSource> SiftEngine<DS>
where
    DS::N: PartialEq,
    DS::V: PartialEq,
{
    /// Create a new empty engine.
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
            partial_matches: Vec::new(),
            next_match_id: 0,
        }
    }

    /// Register a pattern. Returns its index.
    pub fn register(&mut self, pattern: Pattern<DS::L, DS::V>) -> usize {
        let idx = self.patterns.len();
        self.patterns.push(pattern);
        idx
    }

    /// All registered patterns.
    pub fn patterns(&self) -> &[Pattern<DS::L, DS::V>] {
        &self.patterns
    }

    /// All partial matches (including completed ones).
    pub fn partial_matches(&self) -> &[PartialMatch<DS::N, DS::V, DS::T>] {
        &self.partial_matches
    }

    /// Active partial matches for a specific pattern (by name).
    pub fn active_matches_for(&self, name: &str) -> Vec<&PartialMatch<DS::N, DS::V, DS::T>> {
        self.partial_matches
            .iter()
            .filter(|pm| {
                pm.state == MatchState::Active
                    && self.patterns.get(pm.pattern_idx).is_some_and(|p| p.name == name)
            })
            .collect()
    }

    /// Drain completed matches, removing them from internal storage.
    pub fn drain_completed(&mut self) -> Vec<Match<DS::N, DS::V>> {
        let mut completed = Vec::new();
        self.partial_matches.retain(|pm| {
            if pm.state == MatchState::Complete {
                completed.push(Match {
                    pattern: self.patterns[pm.pattern_idx].name.clone(),
                    bindings: pm.bindings.clone(),
                });
                false
            } else {
                true
            }
        });
        completed
    }

    /// Compute a deterministic fingerprint for partial match deduplication.
    /// Two PMs with the same fingerprint have identical
    /// (pattern_idx, next_stage, bindings, intervals).
    fn pm_fingerprint(
        pattern_idx: usize,
        next_stage: usize,
        bindings: &HashMap<String, BoundValue<DS::N, DS::V>>,
        intervals: &HashMap<String, Interval<DS::T>>,
    ) -> String {
        let mut binding_keys: Vec<&String> = bindings.keys().collect();
        binding_keys.sort();
        let mut interval_keys: Vec<&String> = intervals.keys().collect();
        interval_keys.sort();

        let mut fp = String::with_capacity(128);
        write!(fp, "{}:{}:", pattern_idx, next_stage).unwrap();
        for k in &binding_keys {
            write!(fp, "{}={:?},", k, bindings[*k]).unwrap();
        }
        fp.push('|');
        for k in &interval_keys {
            write!(fp, "{}={:?},", k, intervals[*k]).unwrap();
        }
        fp
    }

    /// Batch evaluation: find all complete matches in the current graph state.
    pub fn evaluate(&self, ds: &DS) -> Vec<Match<DS::N, DS::V>> {
        let mut results = Vec::new();
        let now = ds.now();
        for pattern in &self.patterns {
            results.extend(self.evaluate_pattern(ds, pattern, &now));
        }
        results
    }

    /// Incremental: a new edge was added to the graph.
    pub fn on_edge_added(
        &mut self,
        ds: &DS,
        source: &DS::N,
        label: &DS::L,
        value: &DS::V,
        interval: &Interval<DS::T>,
    ) -> Vec<SiftEvent<DS::N, DS::V>> {
        let mut events = Vec::new();

        // Build dedup set from ALL existing PMs (Active, Complete, AND Dead).
        // Dead PMs stay in `seen` to prevent re-creation of a just-negated PM
        // within the same on_edge_added call.
        let mut seen: HashSet<String> = HashSet::new();
        for pm in &self.partial_matches {
            seen.insert(Self::pm_fingerprint(
                pm.pattern_idx, pm.next_stage, &pm.bindings, &pm.intervals,
            ));
        }

        // Phase 1: Check negation windows on existing partial matches.
        for pm in &mut self.partial_matches {
            if pm.state != MatchState::Active {
                continue;
            }
            let pattern = &self.patterns[pm.pattern_idx];
            if let Some(neg_label) =
                Self::check_negation_kill(ds, pattern, pm, source, label, value, interval)
            {
                pm.state = MatchState::Dead;
                events.push(SiftEvent::Negated {
                    pattern: pattern.name.clone(),
                    match_id: pm.id,
                    clause_label: neg_label,
                    trigger_source: source.clone(),
                });
            }
        }

        // Phase 2: Try to initiate new partial matches (match first stage).
        let mut new_matches = Vec::new();
        for (pat_idx, pattern) in self.patterns.iter().enumerate() {
            if let Some(first_stage) = pattern.stages.first() {
                if let Some(match_results) =
                    Self::try_match_stage(ds, first_stage, source, label, value, interval, &HashMap::new())
                {
                    for (bindings, intervals) in match_results {
                        let is_last_stage = pattern.stages.len() == 1;
                        // B3 fix: for single-stage patterns, check negations before marking complete.
                        let negation_blocks = is_last_stage
                            && !self.check_negations_batch(ds, pattern, &bindings, &intervals);
                        if negation_blocks {
                            continue; // Negation prevents this match
                        }
                        let next = if is_last_stage { pattern.stages.len() } else { 1 };
                        // Dedup: skip if identical PM already exists
                        let fp = Self::pm_fingerprint(pat_idx, next, &bindings, &intervals);
                        if !seen.insert(fp) {
                            continue;
                        }
                        let id = self.next_match_id;
                        self.next_match_id += 1;

                        let pm = PartialMatch {
                            pattern_idx: pat_idx,
                            bindings: bindings.clone(),
                            intervals,
                            next_stage: next,
                            state: if is_last_stage { MatchState::Complete } else { MatchState::Active },
                            id,
                        };

                        if is_last_stage {
                            events.push(SiftEvent::Completed {
                                pattern: pattern.name.clone(),
                                match_id: id,
                                bindings,
                            });
                        } else {
                            events.push(SiftEvent::Advanced {
                                pattern: pattern.name.clone(),
                                match_id: id,
                                stage_index: 0,
                            });
                        }
                        new_matches.push(pm);
                    }
                }
            }
        }

        // Phase 3: Try to advance existing active partial matches.
        let mut advanced = Vec::new();
        for pm in &self.partial_matches {
            if pm.state != MatchState::Active {
                continue;
            }
            let pattern = &self.patterns[pm.pattern_idx];
            let stage_idx = pm.next_stage;
            if stage_idx >= pattern.stages.len() {
                continue;
            }
            let stage = &pattern.stages[stage_idx];
            if let Some(match_results) =
                Self::try_match_stage(ds, stage, source, label, value, interval, &pm.bindings)
            {
                for (new_bindings, new_intervals) in match_results {
                    // B2 fix: check temporal ordering — new stage must come after previous stages
                    let temporal_ok = pm.intervals.values().all(|prev_iv| prev_iv.start < interval.start);
                    if !temporal_ok {
                        continue; // Skip this match — temporal order violated
                    }

                    let next = stage_idx + 1;
                    let is_complete = next >= pattern.stages.len();

                    let mut merged_bindings = pm.bindings.clone();
                    merged_bindings.extend(new_bindings);
                    let mut merged_intervals = pm.intervals.clone();
                    merged_intervals.extend(new_intervals);

                    // Dedup: skip if identical PM already exists
                    let fp = Self::pm_fingerprint(pm.pattern_idx, next, &merged_bindings, &merged_intervals);
                    if !seen.insert(fp) {
                        continue;
                    }

                    let id = self.next_match_id;
                    self.next_match_id += 1;

                    let new_pm = PartialMatch {
                        pattern_idx: pm.pattern_idx,
                        bindings: merged_bindings.clone(),
                        intervals: merged_intervals,
                        next_stage: next,
                        state: if is_complete { MatchState::Complete } else { MatchState::Active },
                        id,
                    };

                    if is_complete {
                        events.push(SiftEvent::Completed {
                            pattern: pattern.name.clone(),
                            match_id: id,
                            bindings: merged_bindings,
                        });
                    } else {
                        events.push(SiftEvent::Advanced {
                            pattern: pattern.name.clone(),
                            match_id: id,
                            stage_index: stage_idx,
                        });
                    }
                    advanced.push(new_pm);
                }
            }
        }

        self.partial_matches.extend(new_matches);
        self.partial_matches.extend(advanced);
        self.partial_matches.retain(|pm| pm.state != MatchState::Dead);

        events
    }

    /// Gap analysis: why hasn't this pattern matched?
    pub fn why_not(&self, ds: &DS, pattern_name: &str) -> Option<GapAnalysis> {
        let pattern = self.patterns.iter().find(|p| p.name == pattern_name)?;
        let now = ds.now();
        let mut stages = Vec::new();
        let bindings: HashMap<String, BoundValue<DS::N, DS::V>> = HashMap::new();

        for stage in &pattern.stages {
            let mut clause_analyses = Vec::new();
            let mut stage_matched = true;

            for clause in &stage.clauses {
                let (matched, reason) = self.analyze_clause(ds, clause, &bindings, &now);
                if !matched {
                    stage_matched = false;
                }
                clause_analyses.push(ClauseAnalysis {
                    description: format!("?{} --[{:?}]--> {:?}{}", clause.source.0, clause.label, clause.target, if clause.negated { " (NOT)" } else { "" }),
                    matched,
                    reason,
                });
            }

            let matched_count = clause_analyses.iter().filter(|c| c.matched).count();
            let total = clause_analyses.len();
            let status = if stage_matched {
                StageStatus::Matched
            } else if matched_count > 0 {
                StageStatus::PartiallyMatched { matched: matched_count, total }
            } else {
                StageStatus::Unmatched
            };

            stages.push(StageAnalysis {
                anchor: stage.anchor.0.clone(),
                status,
                clauses: clause_analyses,
            });

            if !stage_matched {
                break;
            }
        }

        Some(GapAnalysis {
            pattern: pattern_name.to_string(),
            stages,
        })
    }

    // -----------------------------------------------------------------------
    // Internal: batch evaluation
    // -----------------------------------------------------------------------

    fn evaluate_pattern(
        &self,
        ds: &DS,
        pattern: &Pattern<DS::L, DS::V>,
        now: &DS::T,
    ) -> Vec<Match<DS::N, DS::V>> {
        if pattern.stages.is_empty() {
            return Vec::new();
        }

        let mut candidates: Vec<MatchCandidate<DS::N, DS::V, DS::T>> =
            self.find_stage_matches(ds, &pattern.stages[0], &HashMap::new(), now);

        for stage in &pattern.stages[1..] {
            let mut next = Vec::new();
            for (bindings, intervals) in &candidates {
                for (new_b, new_i) in self.find_stage_matches(ds, stage, bindings, now) {
                    let mut merged_b = bindings.clone();
                    merged_b.extend(new_b);
                    let mut merged_i = intervals.clone();
                    merged_i.extend(new_i);
                    next.push((merged_b, merged_i));
                }
            }
            candidates = next;
        }

        candidates
            .into_iter()
            .filter(|(bindings, intervals)| {
                self.check_temporal(pattern, intervals)
                    && self.check_negations_batch(ds, pattern, bindings, intervals)
            })
            .map(|(bindings, _)| Match {
                pattern: pattern.name.clone(),
                bindings,
            })
            .collect()
    }

    fn find_stage_matches(
        &self,
        ds: &DS,
        stage: &Stage<DS::L, DS::V>,
        existing: &HashMap<String, BoundValue<DS::N, DS::V>>,
        now: &DS::T,
    ) -> Vec<MatchCandidate<DS::N, DS::V, DS::T>> {
        if stage.clauses.is_empty() {
            return Vec::new();
        }

        let first = &stage.clauses[0];
        let mut candidates = Vec::new();

        if let Some(bound) = existing.get(&first.source.0) {
            if let BoundValue::Node(ref node) = bound {
                for e in ds.edges_from(node, &first.label, now) {
                    if self.target_matches_ds(ds, &first.target, &e.target, existing) {
                        let mut b = HashMap::new();
                        if !self.bind_target(ds, &first.target, &e.target, &mut b) {
                            continue;
                        }
                        let mut iv = HashMap::new();
                        iv.insert(stage.anchor.0.clone(), e.interval.clone());
                        candidates.push((b, iv));
                    }
                }
            }
        } else {
            let constraint = match &first.target {
                Target::Literal(v) => ValueConstraint::Eq(v.clone()),
                Target::Constraint(c) => c.clone(),
                Target::Bind(_) => ValueConstraint::Any,
            };
            for e in ds.scan(&first.label, &constraint, now) {
                let mut b = HashMap::new();
                b.insert(first.source.0.clone(), BoundValue::Node(e.source.clone()));
                b.insert(stage.anchor.0.clone(), BoundValue::Node(e.source.clone()));
                // B8 fix: bind_target now returns false if var is already bound
                // and value doesn't match (e.g., self-loop check)
                if !self.bind_target(ds, &first.target, &e.target, &mut b) {
                    continue;
                }
                let mut iv = HashMap::new();
                iv.insert(stage.anchor.0.clone(), e.interval.clone());
                candidates.push((b, iv));
            }
        }

        // Check remaining clauses and bind their target variables
        let mut result = Vec::new();
        for (mut b, iv) in candidates {
            let mut merged: HashMap<String, BoundValue<DS::N, DS::V>> = existing.iter().chain(b.iter())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            let mut all_ok = true;
            for c in &stage.clauses[1..] {
                if !self.clause_satisfied(ds, c, &merged, now) {
                    all_ok = false;
                    break;
                }
                // Bind target variable from this clause if it's a Bind target
                if let Target::Bind(ref var) = c.target {
                    if !merged.contains_key(&var.0) {
                        if let Some(BoundValue::Node(ref src)) = merged.get(&c.source.0) {
                            let edges = ds.edges_from(src, &c.label, now);
                            if let Some(edge) = edges.first() {
                                if let Some(n) = ds.value_as_node(&edge.target) {
                                    let bv = BoundValue::Node(n);
                                    b.insert(var.0.clone(), bv.clone());
                                    merged.insert(var.0.clone(), bv);
                                } else {
                                    let bv = BoundValue::Value(edge.target.clone());
                                    b.insert(var.0.clone(), bv.clone());
                                    merged.insert(var.0.clone(), bv);
                                }
                            }
                        }
                    }
                }
            }
            if all_ok {
                result.push((b, iv));
            }
        }

        result
    }

    fn clause_satisfied(
        &self,
        ds: &DS,
        clause: &Clause<DS::L, DS::V>,
        bindings: &HashMap<String, BoundValue<DS::N, DS::V>>,
        now: &DS::T,
    ) -> bool {
        let source = match bindings.get(&clause.source.0) {
            Some(BoundValue::Node(n)) => n,
            _ => return false,
        };
        let edges = ds.edges_from(source, &clause.label, now);
        let found = edges.iter().any(|e| self.target_matches_ds(ds, &clause.target, &e.target, bindings));
        if clause.negated { !found } else { found }
    }

    fn target_matches_ds(
        &self,
        ds: &DS,
        target: &Target<DS::V>,
        value: &DS::V,
        bindings: &HashMap<String, BoundValue<DS::N, DS::V>>,
    ) -> bool {
        match target {
            Target::Literal(v) => value == v,
            Target::Constraint(c) => c.matches(value),
            Target::Bind(var) => {
                if let Some(bound) = bindings.get(&var.0) {
                    bound.matches_value::<DS>(ds, value)
                } else {
                    true // Unbound — any value matches
                }
            }
        }
    }

    /// Bind a target variable, or verify consistency if already bound.
    /// Returns false if the variable is bound but the value doesn't match (B8 fix).
    fn bind_target(
        &self,
        ds: &DS,
        target: &Target<DS::V>,
        value: &DS::V,
        bindings: &mut HashMap<String, BoundValue<DS::N, DS::V>>,
    ) -> bool {
        if let Target::Bind(ref var) = target {
            if let Some(existing) = bindings.get(&var.0) {
                // B8 fix: variable already bound — verify consistency
                return existing.matches_value::<DS>(ds, value);
            }
            if let Some(n) = ds.value_as_node(value) {
                bindings.insert(var.0.clone(), BoundValue::Node(n));
            } else {
                bindings.insert(var.0.clone(), BoundValue::Value(value.clone()));
            }
        }
        true
    }

    // -----------------------------------------------------------------------
    // Internal: temporal checks
    // -----------------------------------------------------------------------

    fn check_temporal(
        &self,
        pattern: &Pattern<DS::L, DS::V>,
        intervals: &HashMap<String, Interval<DS::T>>,
    ) -> bool {
        // Implicit: stages are ordered left-to-right by start time
        for pair in pattern.stages.windows(2) {
            if let (Some(left), Some(right)) =
                (intervals.get(&pair[0].anchor.0), intervals.get(&pair[1].anchor.0))
            {
                if left.start >= right.start {
                    return false;
                }
            }
        }
        // Explicit constraints
        for tc in &pattern.temporal {
            if let (Some(left), Some(right)) =
                (intervals.get(&tc.left.0), intervals.get(&tc.right.0))
            {
                match left.relation(right) {
                    Some(rel) if rel == tc.relation => {}
                    None if tc.relation.is_before_or_meets() && left.start < right.start => {}
                    _ => return false,
                }
            }
        }
        true
    }

    // -----------------------------------------------------------------------
    // Internal: negation checks
    // -----------------------------------------------------------------------

    /// Batch negation check: verify no entity satisfies ALL negation clauses
    /// simultaneously within the temporal window.
    fn check_negations_batch(
        &self,
        ds: &DS,
        pattern: &Pattern<DS::L, DS::V>,
        match_bindings: &HashMap<String, BoundValue<DS::N, DS::V>>,
        intervals: &HashMap<String, Interval<DS::T>>,
    ) -> bool {
        let now = ds.now();

        for negation in &pattern.negations {
            let start = match intervals.get(&negation.between_start.0) {
                Some(iv) => &iv.start,
                None => continue,
            };
            let end = negation.between_end.as_ref()
                .and_then(|v| intervals.get(&v.0))
                .map(|iv| &iv.start);

            if negation.clauses.is_empty() {
                continue;
            }

            // Find candidate entities via the first clause
            let first = &negation.clauses[0];
            let constraint = match &first.target {
                Target::Literal(v) => ValueConstraint::Eq(v.clone()),
                Target::Constraint(c) => c.clone(),
                _ => ValueConstraint::Any,
            };
            let candidates = ds.scan_any_time(&first.label, &constraint);

            for cand in &candidates {
                // B4 fix: window is exclusive on start (strict >), matching Winnow's `<` semantics
                let in_window = &cand.interval.start > start
                    && end.is_none_or(|e| &cand.interval.start < e);
                if !in_window {
                    continue;
                }

                let neg_entity = &cand.source;

                // Check ALL remaining clauses against this same entity,
                // using match_bindings for variable consistency
                let all_ok = negation.clauses[1..].iter().all(|clause| {
                    let src = if clause.source.0 == first.source.0 {
                        neg_entity.clone()
                    } else {
                        // B5 fix: different source — can't verify. Be truly conservative:
                        // assume clause fails, so negation doesn't fire. This prevents
                        // false negation kills on multi-source negation patterns.
                        return false;
                    };
                    let edges = ds.edges_from(&src, &clause.label, &now);
                    edges.iter().any(|e| match &clause.target {
                        Target::Literal(v) => &e.target == v,
                        Target::Constraint(c) => c.matches(&e.target),
                        Target::Bind(var) => {
                            // Check against the parent match's bindings
                            if let Some(bound) = match_bindings.get(&var.0) {
                                bound.matches_value::<DS>(ds, &e.target)
                            } else {
                                true
                            }
                        }
                    })
                });

                if all_ok {
                    return false; // Found an entity satisfying all negation clauses
                }
            }
        }
        true
    }

    /// Incremental negation check: does the new edge kill this partial match?
    ///
    /// For a negation to kill, ALL clauses in the negation block must be
    /// satisfiable for some entity within the temporal window. When the
    /// incoming edge matches one clause, we verify the remaining clauses
    /// by querying the data source for the same entity.
    ///
    /// Returns the label of the matched negation clause, if any.
    fn check_negation_kill(
        ds: &DS,
        pattern: &Pattern<DS::L, DS::V>,
        pm: &PartialMatch<DS::N, DS::V, DS::T>,
        source: &DS::N,
        label: &DS::L,
        value: &DS::V,
        interval: &Interval<DS::T>,
    ) -> Option<String> {
        for negation in &pattern.negations {
            let start = match pm.intervals.get(&negation.between_start.0) {
                Some(iv) => iv,
                None => continue,
            };
            // Only check if between_end is NOT yet bound (open window)
            if let Some(ref end_var) = negation.between_end {
                if pm.intervals.contains_key(&end_var.0) {
                    continue;
                }
            }
            // B4 fix: window is exclusive on start (strict >), matching Winnow semantics
            if interval.start <= start.start {
                continue;
            }

            // Check if the incoming edge matches any clause in this negation
            for (i, clause) in negation.clauses.iter().enumerate() {
                if &clause.label != label {
                    continue;
                }
                let target_ok = match &clause.target {
                    Target::Literal(v) => value == v,
                    Target::Constraint(c) => c.matches(value),
                    Target::Bind(_) => true,
                };
                if !target_ok {
                    continue;
                }
                // Check source binding consistency
                if let Some(BoundValue::Node(ref n)) = pm.bindings.get(&clause.source.0) {
                    if source != n {
                        continue;
                    }
                }
                // Check target binding consistency
                if let Target::Bind(ref var) = clause.target {
                    if let Some(bound) = pm.bindings.get(&var.0) {
                        if !bound.matches_value::<DS>(ds, value) {
                            continue;
                        }
                    }
                }

                // This clause matches. Now verify ALL OTHER clauses in the
                // negation block are also satisfiable for the same entity.
                let now = ds.now();
                let mut all_others_ok = true;
                for (j, other) in negation.clauses.iter().enumerate() {
                    if j == i {
                        continue;
                    }
                    // Determine the source node for this clause
                    let src = if other.source.0 == clause.source.0 {
                        // Same source variable as the matched clause
                        source.clone()
                    } else if let Some(BoundValue::Node(ref n)) = pm.bindings.get(&other.source.0) {
                        n.clone()
                    } else {
                        // Can't evaluate — source unknown. Be conservative: don't kill.
                        all_others_ok = false;
                        break;
                    };

                    let edges = ds.edges_from(&src, &other.label, &now);
                    let found = edges.iter().any(|e| {
                        match &other.target {
                            Target::Literal(v) => &e.target == v,
                            Target::Constraint(c) => c.matches(&e.target),
                            Target::Bind(var) => {
                                if let Some(bound) = pm.bindings.get(&var.0) {
                                    bound.matches_value::<DS>(ds, &e.target)
                                } else {
                                    true
                                }
                            }
                        }
                    });
                    if !found {
                        all_others_ok = false;
                        break;
                    }
                }

                if all_others_ok {
                    return Some(format!("{:?}", clause.label));
                }
            }
        }
        None
    }

    // -----------------------------------------------------------------------
    // Internal: incremental stage matching
    // -----------------------------------------------------------------------

    /// Try to match a stage against a newly added edge.
    /// Returns None if no match, Some(vec of (bindings, intervals)) if matched.
    #[allow(clippy::type_complexity)]
    fn try_match_stage(
        ds: &DS,
        stage: &Stage<DS::L, DS::V>,
        source: &DS::N,
        label: &DS::L,
        value: &DS::V,
        interval: &Interval<DS::T>,
        existing: &HashMap<String, BoundValue<DS::N, DS::V>>,
    ) -> Option<Vec<MatchCandidate<DS::N, DS::V, DS::T>>> {
        let first = stage.clauses.first()?;

        // Does the label match?
        if &first.label != label {
            return None;
        }

        // Does the source match? (if bound, must be the same node)
        if let Some(BoundValue::Node(ref n)) = existing.get(&first.source.0) {
            if source != n {
                return None;
            }
        }

        // Does the target match?
        let target_ok = match &first.target {
            Target::Literal(v) => value == v,
            Target::Constraint(c) => c.matches(value),
            Target::Bind(var) => {
                if let Some(bound) = existing.get(&var.0) {
                    bound.matches_value::<DS>(ds, value)
                } else {
                    true
                }
            }
        };
        if !target_ok {
            return None;
        }

        // Build bindings for this match
        let mut bindings: HashMap<String, BoundValue<DS::N, DS::V>> = HashMap::new();
        bindings.insert(stage.anchor.0.clone(), BoundValue::Node(source.clone()));
        if !existing.contains_key(&first.source.0) {
            bindings.insert(first.source.0.clone(), BoundValue::Node(source.clone()));
        }
        if let Target::Bind(ref var) = first.target {
            if !existing.contains_key(&var.0) && !bindings.contains_key(&var.0) {
                if let Some(n) = ds.value_as_node(value) {
                    bindings.insert(var.0.clone(), BoundValue::Node(n));
                } else {
                    bindings.insert(var.0.clone(), BoundValue::Value(value.clone()));
                }
            }
        }

        // B6/B7 fix: use event time, not ds.now(), for secondary clause validation
        let event_time = &interval.start;

        // Check remaining clauses and bind their target variables.
        // B1 fix: collect ALL matching binding sets, not just the first.
        let mut merged: HashMap<String, BoundValue<DS::N, DS::V>> = existing.iter()
            .chain(bindings.iter())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        for clause in &stage.clauses[1..] {
            let src_node = match merged.get(&clause.source.0) {
                Some(BoundValue::Node(n)) => n.clone(),
                _ => return None,
            };
            let edges = ds.edges_from(&src_node, &clause.label, event_time);
            let matching_edge = edges.iter().find(|e| {
                match &clause.target {
                    Target::Literal(v) => &e.target == v,
                    Target::Constraint(c) => c.matches(&e.target),
                    Target::Bind(var) => {
                        if let Some(bound) = merged.get(&var.0) {
                            bound.matches_value::<DS>(ds, &e.target)
                        } else {
                            true
                        }
                    }
                }
            });
            let ok = if clause.negated {
                matching_edge.is_none()
            } else {
                matching_edge.is_some()
            };
            if !ok {
                return None;
            }
            // Bind target variable from this clause
            if !clause.negated {
                if let Target::Bind(ref var) = clause.target {
                    if !merged.contains_key(&var.0) {
                        if let Some(edge) = matching_edge {
                            let bv = if let Some(n) = ds.value_as_node(&edge.target) {
                                BoundValue::Node(n)
                            } else {
                                BoundValue::Value(edge.target.clone())
                            };
                            bindings.insert(var.0.clone(), bv.clone());
                            merged.insert(var.0.clone(), bv);
                        }
                    }
                }
            }
        }

        let mut intervals = HashMap::new();
        intervals.insert(stage.anchor.0.clone(), interval.clone());

        Some(vec![(bindings, intervals)])
    }

    // -----------------------------------------------------------------------
    // Internal: gap analysis
    // -----------------------------------------------------------------------

    fn analyze_clause(
        &self,
        ds: &DS,
        clause: &Clause<DS::L, DS::V>,
        bindings: &HashMap<String, BoundValue<DS::N, DS::V>>,
        now: &DS::T,
    ) -> (bool, Option<String>) {
        let source = match bindings.get(&clause.source.0) {
            Some(BoundValue::Node(n)) => n,
            Some(_) => return (false, Some(format!("?{} is bound to a value, not a node", clause.source.0))),
            None => return (false, Some(format!("?{} is not bound", clause.source.0))),
        };

        let edges = ds.edges_from(source, &clause.label, now);
        let found = edges.iter().any(|e| self.target_matches_ds(ds, &clause.target, &e.target, bindings));
        let ok = if clause.negated { !found } else { found };

        if ok {
            (true, None)
        } else if clause.negated {
            (false, Some(format!("edge {:?} exists but should not", clause.label)))
        } else if edges.is_empty() {
            (false, Some(format!("no edges with label {:?} from ?{}", clause.label, clause.source.0)))
        } else {
            (false, Some("edges exist but none match target constraint".to_string()))
        }
    }
}

impl<DS: DataSource> Default for SiftEngine<DS>
where
    DS::N: PartialEq,
    DS::V: PartialEq,
{
    fn default() -> Self {
        Self::new()
    }
}
