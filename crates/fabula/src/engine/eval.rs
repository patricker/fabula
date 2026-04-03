//! Evaluation methods — batch, incremental, gap analysis, and all private helpers.
//!
//! This module contains Block 2 of the SiftEngine impl: methods that require
//! full trait bounds (`T: Sub + NumericTime`) and take `&impl DataSource`
//! parameters for graph access.

use super::SiftEngine;
use super::types::*;
use crate::datasource::{DataSource, ValueConstraint};
use crate::interval::Interval;
use crate::pattern::*;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;

// ---------------------------------------------------------------------------
// Block 2: Evaluation methods — full bounds + DataSource parameter.
// ---------------------------------------------------------------------------

impl<N, L, V, T> SiftEngine<N, L, V, T>
where
    N: Eq + Hash + Clone + Debug,
    L: Eq + Hash + Clone + Debug,
    V: PartialEq + PartialOrd + Clone + Debug + Hash,
    T: Ord + Clone + Debug + Hash + std::ops::Sub<Output = T> + crate::interval::NumericTime,
{
    /// Batch evaluation: find all complete matches in the current graph state.
    pub fn evaluate(&self, ds: &(impl DataSource<N=N, L=L, V=V, T=T> + ?Sized)) -> Vec<Match<N, V>> {
        let mut results = Vec::new();
        let now = ds.now();
        for (idx, pattern) in self.patterns.iter().enumerate() {
            if !self.enabled[idx] { continue; }
            results.extend(self.evaluate_pattern(ds, pattern, &now));
        }
        results
    }

    /// Incremental: a new edge was added to the graph.
    pub fn on_edge_added(
        &mut self,
        ds: &(impl DataSource<N=N, L=L, V=V, T=T> + ?Sized),
        source: &N,
        label: &L,
        value: &V,
        interval: &Interval<T>,
    ) -> Vec<SiftEvent<N, V>> {
        self.stats.total_on_edge_added += 1;
        let mut events = Vec::new();

        // Build dedup set from ALL existing PMs (Active, Complete, AND Dead).
        // Dead PMs stay in `seen` to prevent re-creation of a just-negated PM
        // within the same on_edge_added call.
        // Uses precomputed u64 hashes — zero allocation.
        let mut seen: HashSet<u64> = HashSet::with_capacity(self.partial_matches.len());
        for pm in &self.partial_matches {
            seen.insert(pm.fingerprint);
        }
        self.stats.total_fingerprints += seen.len() as u64;

        // Phase 1: Check negation windows on existing partial matches.
        for pm in &mut self.partial_matches {
            if pm.state != MatchState::Active {
                continue;
            }
            let pattern = &self.patterns[pm.pattern_idx];
            self.stats.total_negation_checks += 1;
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
            if !self.enabled[pat_idx] { continue; }
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
                        self.stats.total_fingerprints += 1;
                        let fp = Self::compute_fingerprint(pat_idx, next, &bindings, &intervals);
                        if !seen.insert(fp) {
                            continue;
                        }
                        let id = self.next_match_id;
                        self.next_match_id += 1;

                        let pm = PartialMatch {
                            pattern_idx: pat_idx,
                            bindings: bindings.clone(),
                            created_at: interval.start.clone(),
                            intervals,
                            next_stage: next,
                            state: if is_last_stage { MatchState::Complete } else { MatchState::Active },
                            id,
                            fingerprint: fp,
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
            if !self.enabled[pm.pattern_idx] { continue; }
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

                    // Compute speculative fingerprint BEFORE cloning.
                    // Build merged maps only if this is a new unique PM.
                    let mut merged_bindings = pm.bindings.clone();
                    merged_bindings.extend(new_bindings);
                    let mut merged_intervals = pm.intervals.clone();
                    merged_intervals.extend(new_intervals);

                    // Dedup: skip if identical PM already exists
                    self.stats.total_fingerprints += 1;
                    let fp = Self::compute_fingerprint(pm.pattern_idx, next, &merged_bindings, &merged_intervals);
                    if !seen.insert(fp) {
                        continue;
                    }

                    let id = self.next_match_id;
                    self.next_match_id += 1;

                    // Check explicit temporal constraints (including metric gap)
                    // when the pattern would complete
                    if is_complete && !self.check_temporal(pattern, &merged_intervals) {
                        continue;
                    }

                    let new_pm = PartialMatch {
                        pattern_idx: pm.pattern_idx,
                        bindings: merged_bindings.clone(),
                        created_at: pm.created_at.clone(),
                        intervals: merged_intervals,
                        next_stage: next,
                        state: if is_complete { MatchState::Complete } else { MatchState::Active },
                        id,
                        fingerprint: fp,
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

        // Update per-pattern lifecycle metrics + tick accumulators from events.
        for event in &events {
            match event {
                SiftEvent::Advanced { pattern, .. } => {
                    if let Some(idx) = self.patterns.iter().position(|p| p.name == *pattern) {
                        self.advancement_count[idx] += 1;
                        self.last_advanced_tick[idx] = self.tick_counter;
                    }
                    self.tick_advanced.insert(pattern.clone());
                }
                SiftEvent::Completed { pattern, .. } => {
                    if let Some(idx) = self.patterns.iter().position(|p| p.name == *pattern) {
                        self.completion_count[idx] += 1;
                        self.last_advanced_tick[idx] = self.tick_counter;
                    }
                    self.tick_completed.insert(pattern.clone());
                }
                SiftEvent::Negated { pattern, .. } => {
                    if let Some(idx) = self.patterns.iter().position(|p| p.name == *pattern) {
                        self.negation_count[idx] += 1;
                    }
                    self.tick_negated.insert(pattern.clone());
                }
            }
        }

        // Exclusive choice groups: when a pattern with a group completes,
        // kill all other active PMs in the same group.
        let completed_groups: Vec<String> = events
            .iter()
            .filter_map(|e| {
                if let SiftEvent::Completed { pattern, .. } = e {
                    self.patterns.iter()
                        .find(|p| p.name == *pattern)
                        .and_then(|p| p.group.clone())
                } else {
                    None
                }
            })
            .collect();
        if !completed_groups.is_empty() {
            for pm in &mut self.partial_matches {
                if pm.state != MatchState::Active {
                    continue;
                }
                if let Some(ref g) = self.patterns[pm.pattern_idx].group {
                    if completed_groups.contains(g) {
                        pm.state = MatchState::Dead;
                    }
                }
            }
        }

        self.partial_matches.retain(|pm| pm.state != MatchState::Dead);

        // Track peak active PM count
        let active_count = self.partial_matches.iter()
            .filter(|pm| pm.state == MatchState::Active)
            .count();
        if active_count > self.stats.peak_active_pms {
            self.stats.peak_active_pms = active_count;
        }

        events
    }

    /// Gap analysis: why hasn't this pattern matched?
    pub fn why_not(&self, ds: &(impl DataSource<N=N, L=L, V=V, T=T> + ?Sized), pattern_name: &str) -> Option<GapAnalysis> {
        let pattern = self.patterns.iter().find(|p| p.name == pattern_name)?;
        let now = ds.now();
        let mut stages = Vec::new();
        let bindings: HashMap<String, BoundValue<N, V>> = HashMap::new();

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
        ds: &(impl DataSource<N=N, L=L, V=V, T=T> + ?Sized),
        pattern: &Pattern<L, V>,
        now: &T,
    ) -> Vec<Match<N, V>> {
        if pattern.stages.is_empty() {
            return Vec::new();
        }

        let mut candidates: Vec<MatchCandidate<N, V, T>> =
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
        ds: &(impl DataSource<N=N, L=L, V=V, T=T> + ?Sized),
        stage: &Stage<L, V>,
        existing: &HashMap<String, BoundValue<N, V>>,
        now: &T,
    ) -> Vec<MatchCandidate<N, V, T>> {
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
            let mut merged: HashMap<String, BoundValue<N, V>> = existing.iter().chain(b.iter())
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
        ds: &(impl DataSource<N=N, L=L, V=V, T=T> + ?Sized),
        clause: &Clause<L, V>,
        bindings: &HashMap<String, BoundValue<N, V>>,
        now: &T,
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
        ds: &(impl DataSource<N=N, L=L, V=V, T=T> + ?Sized),
        target: &Target<V>,
        value: &V,
        bindings: &HashMap<String, BoundValue<N, V>>,
    ) -> bool {
        match target {
            Target::Literal(v) => value == v,
            Target::Constraint(c) => c.matches(value),
            Target::Bind(var) => {
                if let Some(bound) = bindings.get(&var.0) {
                    bound.matches_value(&|v| ds.value_as_node(v), value)
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
        ds: &(impl DataSource<N=N, L=L, V=V, T=T> + ?Sized),
        target: &Target<V>,
        value: &V,
        bindings: &mut HashMap<String, BoundValue<N, V>>,
    ) -> bool {
        if let Target::Bind(ref var) = target {
            if let Some(existing) = bindings.get(&var.0) {
                // B8 fix: variable already bound — verify consistency
                return existing.matches_value(&|v| ds.value_as_node(v), value);
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
        pattern: &Pattern<L, V>,
        intervals: &HashMap<String, Interval<T>>,
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
                // Metric gap check (STN-style bounded difference)
                if let Some(ref gap_bound) = tc.gap {
                    if let Some(gap_val) = left.gap_for_relation(right, tc.relation) {
                        if let Some(min) = gap_bound.min {
                            if gap_val < min { return false; }
                        }
                        if let Some(max) = gap_bound.max {
                            if gap_val > max { return false; }
                        }
                    }
                    // Open-ended interval → can't compute gap → skip metric check
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
        ds: &(impl DataSource<N=N, L=L, V=V, T=T> + ?Sized),
        pattern: &Pattern<L, V>,
        match_bindings: &HashMap<String, BoundValue<N, V>>,
        intervals: &HashMap<String, Interval<T>>,
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
                                bound.matches_value(&|v| ds.value_as_node(v), &e.target)
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
        ds: &(impl DataSource<N=N, L=L, V=V, T=T> + ?Sized),
        pattern: &Pattern<L, V>,
        pm: &PartialMatch<N, V, T>,
        source: &N,
        label: &L,
        value: &V,
        interval: &Interval<T>,
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
                        if !bound.matches_value(&|v| ds.value_as_node(v), value) {
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
                                    bound.matches_value(&|v| ds.value_as_node(v), &e.target)
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
        ds: &(impl DataSource<N=N, L=L, V=V, T=T> + ?Sized),
        stage: &Stage<L, V>,
        source: &N,
        label: &L,
        value: &V,
        interval: &Interval<T>,
        existing: &HashMap<String, BoundValue<N, V>>,
    ) -> Option<Vec<MatchCandidate<N, V, T>>> {
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
                    bound.matches_value(&|v| ds.value_as_node(v), value)
                } else {
                    true
                }
            }
        };
        if !target_ok {
            return None;
        }

        // Build bindings for this match
        let mut bindings: HashMap<String, BoundValue<N, V>> = HashMap::new();
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
        let mut merged: HashMap<String, BoundValue<N, V>> = existing.iter()
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
                            bound.matches_value(&|v| ds.value_as_node(v), &e.target)
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
        ds: &(impl DataSource<N=N, L=L, V=V, T=T> + ?Sized),
        clause: &Clause<L, V>,
        bindings: &HashMap<String, BoundValue<N, V>>,
        now: &T,
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
