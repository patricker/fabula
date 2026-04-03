//! Evaluation methods — batch, incremental, gap analysis, and all private helpers.
//!
//! This module contains Block 2 of the SiftEngine impl: methods that require
//! full trait bounds (`T: Sub + NumericTime`) and take `&impl DataSource`
//! parameters for graph access.
//!
//! Stateless evaluation logic lives in `super::free`. Methods here delegate to
//! those free functions, adding engine-specific concerns (pattern registry,
//! partial match state, lifecycle metrics, choice groups).

use super::SiftEngine;
use super::free;
use super::types::*;
use crate::datasource::DataSource;
use crate::interval::Interval;
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
    pub fn evaluate(&self, ds: &(impl DataSource<N=N, L=L, V=V, T=T> + ?Sized)) -> Vec<Match<N, V, T>> {
        let mut results = Vec::new();
        let now = ds.now();
        for (idx, pattern) in self.patterns.iter().enumerate() {
            if !self.enabled[idx] { continue; }
            let mut matches = free::evaluate_pattern_at(ds, pattern, &now);
            for m in &mut matches {
                m.pattern_idx = Some(idx);
            }
            results.extend(matches);
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
                });
            }
        }

        // Phase 2: Try to initiate new partial matches (match first stage).
        let mut new_matches = Vec::new();
        for (pat_idx, pattern) in self.patterns.iter().enumerate() {
            if !self.enabled[pat_idx] { continue; }
            if let Some(first_stage) = pattern.stages.first() {
                if let Some(match_results) =
                    free::try_match_stage(ds, first_stage, source, label, value, interval, &HashMap::new())
                {
                    for (bindings, intervals) in match_results {
                        let is_last_stage = pattern.stages.len() == 1;
                        let negation_blocks = is_last_stage
                            && !free::check_negations_batch(ds, pattern, &bindings, &intervals);
                        if negation_blocks {
                            continue;
                        }
                        let next = if is_last_stage { pattern.stages.len() } else { 1 };
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
                free::try_match_stage(ds, stage, source, label, value, interval, &pm.bindings)
            {
                for (new_bindings, new_intervals) in match_results {
                    let temporal_ok = pm.intervals.values().all(|prev_iv| prev_iv.start < interval.start);
                    if !temporal_ok {
                        continue;
                    }

                    let next = stage_idx + 1;
                    let is_complete = next >= pattern.stages.len();

                    let mut merged_bindings = pm.bindings.clone();
                    merged_bindings.extend(new_bindings);
                    let mut merged_intervals = pm.intervals.clone();
                    merged_intervals.extend(new_intervals);

                    self.stats.total_fingerprints += 1;
                    let fp = Self::compute_fingerprint(pm.pattern_idx, next, &merged_bindings, &merged_intervals);
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
        Some(free::gap_analysis_at(ds, pattern, &ds.now()))
    }
}
