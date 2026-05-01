//! Evaluation methods -- batch, incremental, gap analysis, and all private helpers.
//!
//! This module contains Block 2 of the SiftEngine impl: methods that require
//! full trait bounds (`T: Sub + NumericTime`) and take `&impl DataSource`
//! parameters for graph access.
//!
//! Stateless evaluation logic lives in `super::free`. Methods here delegate to
//! those free functions, adding engine-specific concerns (pattern registry,
//! partial match state, lifecycle metrics, choice groups).

use super::free;
use super::types::*;
use super::SiftEngine;
use crate::datasource::DataSource;
use crate::interval::Interval;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;

// ---------------------------------------------------------------------------
// Block 2: Evaluation methods -- full bounds + DataSource parameter.
// ---------------------------------------------------------------------------

impl<N, L, V, T, E> SiftEngine<N, L, V, T, E>
where
    N: Eq + Hash + Clone + Debug,
    L: Eq + Hash + Clone + Debug,
    V: PartialEq + PartialOrd + Clone + Debug + Hash,
    T: Ord + Clone + Debug + Hash + std::ops::Sub<Output = T> + crate::interval::NumericTime,
    E: super::LetEvaluator<N, V>,
{
    /// Batch evaluation: find all complete matches in the current graph state.
    pub fn evaluate(
        &self,
        ds: &(impl DataSource<N = N, L = L, V = V, T = T> + ?Sized),
    ) -> Vec<Match<N, V, T>> {
        let mut results = Vec::new();
        let now = ds.now();
        for (idx, pattern) in self.patterns.iter().enumerate() {
            if !self.enabled[idx] {
                continue;
            }
            let mut matches = free::evaluate_pattern_at(ds, pattern, &now, &self.let_evaluator);
            for m in &mut matches {
                m.pattern_idx = Some(idx);
            }
            results.extend(matches);
        }
        // Filter out matches from private patterns.
        results.retain(|m| {
            !self
                .patterns
                .iter()
                .any(|p| p.name == m.pattern && p.private)
        });

        results
    }

    /// Incremental: a new edge was added to the graph.
    pub fn on_edge_added(
        &mut self,
        ds: &(impl DataSource<N = N, L = L, V = V, T = T> + ?Sized),
        source: &N,
        label: &L,
        value: &V,
        interval: &Interval<T>,
    ) -> Vec<SiftEvent<N, V>> {
        self.stats.total_on_edge_added += 1;
        let mut events = Vec::new();

        // Build dedup set from ALL existing PMs (Active, Complete, AND Dead).
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
                free::check_negation_kill(ds, pattern, pm, source, label, value, interval)
            {
                pm.state = MatchState::Dead;
                events.push(SiftEvent::Negated {
                    pattern: pattern.name.clone(),
                    match_id: pm.id,
                    clause_label: neg_label,
                    trigger_source: source.clone(),
                    metadata: pattern.metadata.clone(),
                });
            }
        }

        // Phase 2: Try to initiate new partial matches (match first stage).
        // If stage 0 is in an unordered group, try ALL group stages as initiators.
        let mut new_matches = Vec::new();
        for (pat_idx, pattern) in self.patterns.iter().enumerate() {
            if !self.enabled[pat_idx] {
                continue;
            }
            if pattern.stages.is_empty() {
                continue;
            }

            // Determine which stages to try for initiation
            let init_stages: Vec<usize> = if let Some(group) = pattern.unordered_group_for(0) {
                group.clone()
            } else {
                vec![0]
            };

            for &init_idx in &init_stages {
                let stage = &pattern.stages[init_idx];
                if let Some(match_results) = free::try_match_stage(
                    ds,
                    stage,
                    source,
                    label,
                    value,
                    interval,
                    &HashMap::new(),
                    &self.let_evaluator,
                ) {
                    for (bindings, intervals) in match_results {
                        // Determine next_stage and matched_stages based on group membership
                        let (next, init_mask) = if let Some(group) = pattern.unordered_group_for(0)
                        {
                            let mask = 1u64 << init_idx;
                            let all_matched = group.len() == 1;
                            if all_matched {
                                // Single-stage group: advance past it
                                let group_end = *group.iter().max().unwrap() + 1;
                                (group_end, mask)
                            } else {
                                // Stay at group start, track matched bit
                                (group[0], mask)
                            }
                        } else {
                            (1, 0)
                        };

                        let is_last_stage = next >= pattern.stages.len();
                        let negation_blocks = is_last_stage
                            && !free::check_negations_batch(ds, pattern, &bindings, &intervals);
                        if negation_blocks {
                            continue;
                        }
                        let final_next = if is_last_stage {
                            pattern.stages.len()
                        } else {
                            next
                        };
                        self.stats.total_fingerprints += 1;
                        let fp = Self::compute_fingerprint_full(
                            pat_idx, final_next, &bindings, &intervals, 0, init_mask,
                        );
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
                            next_stage: final_next,
                            state: if is_last_stage {
                                MatchState::Complete
                            } else {
                                MatchState::Active
                            },
                            id,
                            fingerprint: fp,
                            created_at_tick: self.tick_counter,
                            last_advanced_tick: self.tick_counter,
                            repetition_count: 0,
                            matched_stages: init_mask,
                        };

                        if is_last_stage {
                            events.push(SiftEvent::Completed {
                                pattern: pattern.name.clone(),
                                match_id: id,
                                bindings,
                                metadata: pattern.metadata.clone(),
                            });
                        } else {
                            events.push(SiftEvent::Advanced {
                                pattern: pattern.name.clone(),
                                match_id: id,
                                stage_index: init_idx,
                                metadata: pattern.metadata.clone(),
                            });
                        }
                        new_matches.push(pm);
                    }
                }
            }
        }

        // Phase 3: Try to advance existing active partial matches.
        // For unordered groups: try all unmatched stages in the group.
        let mut advanced = Vec::new();
        // IDs of PMs to mark Dead after the loop (advance_in_place + strict-forward push).
        let mut consume_ids: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for pm in &self.partial_matches {
            if pm.state != MatchState::Active {
                continue;
            }
            if !self.enabled[pm.pattern_idx] {
                continue;
            }
            let pattern = &self.patterns[pm.pattern_idx];
            let stage_idx = pm.next_stage;
            if stage_idx >= pattern.stages.len() {
                continue;
            }

            // Determine which stages to try based on unordered group membership
            let try_stages: Vec<usize> = if let Some(group) = pattern.unordered_group_for(stage_idx)
            {
                // Try all unmatched stages in the group
                group
                    .iter()
                    .filter(|&&si| pm.matched_stages & (1u64 << si) == 0)
                    .copied()
                    .collect()
            } else {
                vec![stage_idx]
            };

            for &try_idx in &try_stages {
                let stage = &pattern.stages[try_idx];
                if let Some(match_results) =
                    free::try_match_stage(ds, stage, source, label, value, interval, &pm.bindings, &self.let_evaluator)
                {
                    for (new_bindings, new_intervals) in match_results {
                        // Temporal check: new edge must come after all previously matched
                        // intervals, EXCEPT for intervals within the same unordered group
                        // (those have no ordering requirement).
                        let temporal_ok = pm.intervals.iter().all(|(var, prev_iv)| {
                            // Find the stage index for this interval variable
                            if let Some(prev_stage_idx) =
                                pattern.stages.iter().position(|s| s.anchor.0 == *var)
                            {
                                if pattern.same_unordered_group(prev_stage_idx, try_idx) {
                                    return true; // no ordering within group
                                }
                            }
                            prev_iv.start < interval.start
                        });
                        if !temporal_ok {
                            continue;
                        }

                        let mut merged_bindings = pm.bindings.clone();
                        merged_bindings.extend(new_bindings);
                        let mut merged_intervals = pm.intervals.clone();
                        merged_intervals.extend(new_intervals);

                        // Compute the new matched_stages mask and determine next_stage
                        let (next, new_mask) =
                            if let Some(group) = pattern.unordered_group_for(stage_idx) {
                                let mask = pm.matched_stages | (1u64 << try_idx);
                                let all_matched = group.iter().all(|&si| mask & (1u64 << si) != 0);
                                if all_matched {
                                    // Group complete -- advance past it
                                    let group_end = *group.iter().max().unwrap() + 1;
                                    (group_end, mask)
                                } else {
                                    // Stay at group start
                                    (group[0], mask)
                                }
                            } else {
                                (stage_idx + 1, pm.matched_stages)
                            };

                        let is_past_end = next >= pattern.stages.len();

                        // Check for repeat range looping
                        if is_past_end {
                            if let Some(ref rr) = pattern.repeat_range {
                                let increment = if pm.repetition_count == 0 { 2 } else { 1 };
                                let new_rep = pm.repetition_count + increment;
                                let min_met = new_rep >= rr.min_reps as u32;
                                let max_reached = rr.max_reps.is_some_and(|m| new_rep >= m as u32);

                                if min_met && free::check_temporal(pattern, &merged_intervals) {
                                    self.stats.total_fingerprints += 1;
                                    let cfp = Self::compute_fingerprint_full(
                                        pm.pattern_idx,
                                        next,
                                        &merged_bindings,
                                        &merged_intervals,
                                        new_rep,
                                        new_mask,
                                    );
                                    if seen.insert(cfp) {
                                        let cid = self.next_match_id;
                                        self.next_match_id += 1;
                                        if pattern.advance_in_place {
                                            consume_ids.insert(pm.id);
                                        }
                                        advanced.push(PartialMatch {
                                            pattern_idx: pm.pattern_idx,
                                            bindings: merged_bindings.clone(),
                                            created_at: pm.created_at.clone(),
                                            intervals: merged_intervals.clone(),
                                            next_stage: next,
                                            state: MatchState::Complete,
                                            id: cid,
                                            fingerprint: cfp,
                                            created_at_tick: pm.created_at_tick,
                                            last_advanced_tick: self.tick_counter,
                                            repetition_count: new_rep,
                                            matched_stages: new_mask,
                                        });
                                        events.push(SiftEvent::Completed {
                                            pattern: pattern.name.clone(),
                                            match_id: cid,
                                            bindings: merged_bindings.clone(),
                                            metadata: pattern.metadata.clone(),
                                        });
                                    }
                                }

                                if !max_reached {
                                    let mut loop_bindings = merged_bindings.clone();
                                    for si in rr.stage_start..rr.stage_end {
                                        let anchor = &pattern.stages[si].anchor.0;
                                        if !rr.shared_vars.contains(anchor) {
                                            loop_bindings.remove(anchor);
                                        }
                                        for clause in &pattern.stages[si].clauses {
                                            if !rr.shared_vars.contains(&clause.source.0) {
                                                loop_bindings.remove(&clause.source.0);
                                            }
                                            if let crate::pattern::Target::Bind(ref var) =
                                                clause.target
                                            {
                                                if !rr.shared_vars.contains(&var.0) {
                                                    loop_bindings.remove(&var.0);
                                                }
                                            }
                                        }
                                        for cb in &pattern.stages[si].let_bindings {
                                            if !rr.shared_vars.contains(&cb.name) {
                                                loop_bindings.remove(&cb.name);
                                            }
                                        }
                                    }

                                    self.stats.total_fingerprints += 1;
                                    let lfp = Self::compute_fingerprint_with_rep(
                                        pm.pattern_idx,
                                        rr.stage_start,
                                        &loop_bindings,
                                        &merged_intervals,
                                        new_rep,
                                    );
                                    if seen.insert(lfp) {
                                        let lid = self.next_match_id;
                                        self.next_match_id += 1;
                                        advanced.push(PartialMatch {
                                            pattern_idx: pm.pattern_idx,
                                            bindings: loop_bindings,
                                            created_at: pm.created_at.clone(),
                                            intervals: merged_intervals,
                                            next_stage: rr.stage_start,
                                            state: MatchState::Active,
                                            id: lid,
                                            fingerprint: lfp,
                                            created_at_tick: pm.created_at_tick,
                                            last_advanced_tick: self.tick_counter,
                                            repetition_count: new_rep,
                                            matched_stages: 0,
                                        });
                                        events.push(SiftEvent::Advanced {
                                            pattern: pattern.name.clone(),
                                            match_id: lid,
                                            stage_index: try_idx,
                                            metadata: pattern.metadata.clone(),
                                        });
                                    }
                                }
                                continue; // Skip normal PM creation
                            }
                        }

                        // Normal (non-repeat) advancement or completion
                        let is_complete = is_past_end;

                        self.stats.total_fingerprints += 1;
                        let fp = Self::compute_fingerprint_full(
                            pm.pattern_idx,
                            next,
                            &merged_bindings,
                            &merged_intervals,
                            pm.repetition_count,
                            new_mask,
                        );
                        if !seen.insert(fp) {
                            continue;
                        }

                        let id = self.next_match_id;
                        self.next_match_id += 1;

                        if is_complete && !free::check_temporal(pattern, &merged_intervals) {
                            continue;
                        }

                        let new_pm = PartialMatch {
                            pattern_idx: pm.pattern_idx,
                            bindings: merged_bindings.clone(),
                            created_at: pm.created_at.clone(),
                            intervals: merged_intervals,
                            next_stage: next,
                            state: if is_complete {
                                MatchState::Complete
                            } else {
                                MatchState::Active
                            },
                            id,
                            fingerprint: fp,
                            created_at_tick: pm.created_at_tick,
                            last_advanced_tick: self.tick_counter,
                            repetition_count: pm.repetition_count,
                            matched_stages: new_mask,
                        };

                        if is_complete {
                            events.push(SiftEvent::Completed {
                                pattern: pattern.name.clone(),
                                match_id: id,
                                bindings: merged_bindings,
                                metadata: pattern.metadata.clone(),
                            });
                        } else {
                            events.push(SiftEvent::Advanced {
                                pattern: pattern.name.clone(),
                                match_id: id,
                                stage_index: try_idx,
                                metadata: pattern.metadata.clone(),
                            });
                        }
                        // advance_in_place: consume the original PM when this push
                        // represents strict-forward progress (past the current stage,
                        // not a within-group rematch).
                        if pattern.advance_in_place && next > pm.next_stage {
                            consume_ids.insert(pm.id);
                        }
                        advanced.push(new_pm);
                    }
                }
            }
        }

        // Apply advance_in_place: mark the scheduled originals Dead so Phase 4's
        // retain() removes them. The freshly-pushed `advanced` PMs have new ids
        // and are not affected.
        if !consume_ids.is_empty() {
            for pm in self.partial_matches.iter_mut() {
                if consume_ids.contains(&pm.id) {
                    pm.state = MatchState::Dead;
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
                SiftEvent::Negated { pattern, .. } | SiftEvent::Expired { pattern, .. } => {
                    if let Some(idx) = self.patterns.iter().position(|p| p.name == *pattern) {
                        self.negation_count[idx] += 1;
                    }
                    self.tick_negated.insert(pattern.clone());
                }
            }
        }

        // Exclusive choice groups: when a pattern with a group completes,
        // kill all other active PMs in the same group.
        // Exception: repeat-range patterns exempt their own looping PMs --
        // completion at min should kill other alternatives, not the continuation.
        let completed_in_groups: Vec<(usize, String)> = events
            .iter()
            .filter_map(|e| {
                if let SiftEvent::Completed { pattern, .. } = e {
                    self.patterns
                        .iter()
                        .enumerate()
                        .find(|(_, p)| p.name == *pattern)
                        .and_then(|(idx, p)| p.group.clone().map(|g| (idx, g)))
                } else {
                    None
                }
            })
            .collect();
        if !completed_in_groups.is_empty() {
            for pm in &mut self.partial_matches {
                if pm.state != MatchState::Active {
                    continue;
                }
                if let Some(ref g) = self.patterns[pm.pattern_idx].group {
                    let dominated = completed_in_groups.iter().any(|(completed_idx, cg)| {
                        if g != cg {
                            return false;
                        }
                        // Exempt looping PMs from the same repeat-range pattern
                        if pm.pattern_idx == *completed_idx
                            && self.patterns[pm.pattern_idx].repeat_range.is_some()
                        {
                            return false;
                        }
                        true
                    });
                    if dominated {
                        pm.state = MatchState::Dead;
                    }
                }
            }
        }

        self.partial_matches
            .retain(|pm| pm.state != MatchState::Dead);

        // Track peak active PM count
        let active_count = self
            .partial_matches
            .iter()
            .filter(|pm| pm.state == MatchState::Active)
            .count();
        if active_count > self.stats.peak_active_pms {
            self.stats.peak_active_pms = active_count;
        }

        // Filter out events from private patterns.
        // This happens AFTER exclusive group handling -- private patterns still
        // participate in group kills, we only hide them from the returned events.
        events.retain(|e| {
            let pattern_name = match e {
                SiftEvent::Advanced { pattern, .. } => pattern,
                SiftEvent::Completed { pattern, .. } => pattern,
                SiftEvent::Negated { pattern, .. } => pattern,
                SiftEvent::Expired { pattern, .. } => pattern,
            };
            !self
                .patterns
                .iter()
                .any(|p| p.name == *pattern_name && p.private)
        });

        events
    }

    /// Gap analysis: why hasn't this pattern matched?
    pub fn why_not(
        &self,
        ds: &(impl DataSource<N = N, L = L, V = V, T = T> + ?Sized),
        pattern_name: &str,
    ) -> Option<GapAnalysis<L, V>> {
        let pattern = self.patterns.iter().find(|p| p.name == pattern_name)?;
        Some(free::gap_analysis_at(ds, pattern, &ds.now()))
    }
}
