//! Free functions for pattern evaluation and gap analysis without a SiftEngine.
//!
//! These functions enable standalone evaluation -- useful when a consumer
//! wants to evaluate individual patterns or run gap analysis without owning
//! an engine instance.

use super::types::*;
use crate::datasource::{DataSource, ValueConstraint};
use crate::interval::Interval;
use crate::pattern::*;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

// ---------------------------------------------------------------------------
// BoundVar resolution helpers
// ---------------------------------------------------------------------------

/// Resolve a `*Var` constraint against a bindings map, producing a concrete constraint.
///
/// Returns `None` if the variable is not bound or is bound to a `Node`
/// (type mismatch -- comparisons require `Value`, not `Node`).
fn resolve_constraint<N: Debug, V: Clone + PartialEq + PartialOrd + Debug>(
    constraint: &ValueConstraint<V>,
    bindings: &HashMap<String, BoundValue<N, V>>,
) -> Option<ValueConstraint<V>> {
    match constraint {
        ValueConstraint::EqVar(var) => {
            extract_value(bindings, var).map(|v| ValueConstraint::Eq(v.clone()))
        }
        ValueConstraint::LtVar(var) => {
            extract_value(bindings, var).map(|v| ValueConstraint::Lt(v.clone()))
        }
        ValueConstraint::GtVar(var) => {
            extract_value(bindings, var).map(|v| ValueConstraint::Gt(v.clone()))
        }
        ValueConstraint::LteVar(var) => {
            extract_value(bindings, var).map(|v| ValueConstraint::Lte(v.clone()))
        }
        ValueConstraint::GteVar(var) => {
            extract_value(bindings, var).map(|v| ValueConstraint::Gte(v.clone()))
        }
        other => Some(other.clone()),
    }
}

fn extract_value<'a, N: Debug, V: Debug>(
    bindings: &'a HashMap<String, BoundValue<N, V>>,
    var: &str,
) -> Option<&'a V> {
    match bindings.get(var) {
        Some(BoundValue::Value(v)) => Some(v),
        _ => None, // Not bound, or bound to Node (type mismatch)
    }
}

/// Evaluate a stage's `let_bindings` against the merged binding map and merge
/// successful results back in. Returns `false` if any let fails to evaluate or
/// shadows an existing binding (defense in depth -- the DSL compiler should
/// already reject shadowing at compile time).
pub(super) fn eval_stage_lets<N, L, V>(
    stage: &Stage<L, V>,
    bindings: &mut HashMap<String, BoundValue<N, V>>,
) -> bool
where
    N: Eq + Hash + Clone + Debug,
    V: crate::expr::ArithmeticValue + Clone + Debug,
{
    for cb in &stage.let_bindings {
        if bindings.contains_key(&cb.name) {
            return false;
        }
        match cb.expr.eval(bindings) {
            Some(v) => {
                bindings.insert(cb.name.clone(), BoundValue::Value(v));
            }
            None => return false,
        }
    }
    true
}

/// Resolve `*Var` constraints in a Target, returning the resolved target.
/// Returns `None` if a `*Var` constraint cannot be resolved.
fn resolve_target<N: Debug, V: Clone + PartialEq + PartialOrd + Debug>(
    target: &Target<V>,
    bindings: &HashMap<String, BoundValue<N, V>>,
) -> Option<Target<V>> {
    match target {
        Target::Constraint(c) if c.is_var() => {
            resolve_constraint(c, bindings).map(Target::Constraint)
        }
        other => Some(other.clone()),
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Evaluate a single pattern against a data source, returning all complete matches.
///
/// This is the standalone equivalent of registering a pattern with a
/// [`super::SiftEngine`] and calling `evaluate()`. It performs batch evaluation
/// without any engine state.
///
/// Returned matches have `pattern_idx: None` since there is no engine registry.
pub fn evaluate_pattern<N, L, V, T>(
    ds: &(impl DataSource<N = N, L = L, V = V, T = T> + ?Sized),
    pattern: &Pattern<L, V>,
) -> Vec<Match<N, V, T>>
where
    N: Eq + Hash + Clone + Debug,
    L: Eq + Hash + Clone + Debug,
    V: PartialEq + PartialOrd + Clone + Debug + Hash + crate::expr::ArithmeticValue,
    T: Ord + Clone + Debug + Hash + std::ops::Sub<Output = T> + crate::interval::NumericTime,
{
    let now = ds.now();
    evaluate_pattern_at(ds, pattern, &now)
}

/// Evaluate a single pattern, returning only the first complete match.
///
/// Stops as soon as one match is found -- O(1) matches instead of O(all).
/// For the common case where you only need "does at least one match exist?"
pub fn evaluate_pattern_first<N, L, V, T>(
    ds: &(impl DataSource<N = N, L = L, V = V, T = T> + ?Sized),
    pattern: &Pattern<L, V>,
) -> Option<Match<N, V, T>>
where
    N: Eq + Hash + Clone + Debug,
    L: Eq + Hash + Clone + Debug,
    V: PartialEq + PartialOrd + Clone + Debug + Hash + crate::expr::ArithmeticValue,
    T: Ord + Clone + Debug + Hash + std::ops::Sub<Output = T> + crate::interval::NumericTime,
{
    let now = ds.now();
    evaluate_pattern_limit(ds, pattern, &now, 1)
        .into_iter()
        .next()
}

/// Evaluate a single pattern, returning at most `max` complete matches.
///
/// Stops candidate expansion early when possible. For storylet pools where
/// 50 matches exist but only the top 5 matter.
pub fn evaluate_pattern_limit<N, L, V, T>(
    ds: &(impl DataSource<N = N, L = L, V = V, T = T> + ?Sized),
    pattern: &Pattern<L, V>,
    at: &T,
    max: usize,
) -> Vec<Match<N, V, T>>
where
    N: Eq + Hash + Clone + Debug,
    L: Eq + Hash + Clone + Debug,
    V: PartialEq + PartialOrd + Clone + Debug + Hash + crate::expr::ArithmeticValue,
    T: Ord + Clone + Debug + Hash + std::ops::Sub<Output = T> + crate::interval::NumericTime,
{
    if max == 0 || pattern.stages.is_empty() {
        return Vec::new();
    }

    let steps = build_stage_steps(pattern);
    let mut candidates: Vec<MatchCandidate<N, V, T>> = match &steps[0] {
        StageStep::Single(idx) => {
            find_stage_matches(ds, &pattern.stages[*idx], &HashMap::new(), at)
        }
        StageStep::Unordered(indices) => {
            let init = vec![(HashMap::new(), HashMap::new())];
            expand_unordered_group(ds, pattern, &init, indices, at)
        }
    };

    for step in &steps[1..] {
        match step {
            StageStep::Single(idx) => {
                let mut next = Vec::new();
                for (bindings, intervals) in &candidates {
                    for (new_b, new_i) in
                        find_stage_matches(ds, &pattern.stages[*idx], bindings, at)
                    {
                        let mut merged_b = bindings.clone();
                        merged_b.extend(new_b);
                        let mut merged_i = intervals.clone();
                        merged_i.extend(new_i);
                        next.push((merged_b, merged_i));
                    }
                }
                candidates = next;
            }
            StageStep::Unordered(indices) => {
                candidates = expand_unordered_group(ds, pattern, &candidates, indices, at);
            }
        }
    }

    candidates
        .into_iter()
        .filter(|(bindings, intervals)| {
            check_temporal(pattern, intervals)
                && check_negations_batch(ds, pattern, bindings, intervals)
        })
        .take(max)
        .map(|(bindings, intervals)| Match {
            pattern: pattern.name.clone(),
            pattern_idx: None,
            bindings,
            intervals,
            metadata: pattern.metadata.clone(),
        })
        .collect()
}

/// Standalone gap analysis -- why hasn't this pattern matched?
///
/// This is the standalone equivalent of [`super::SiftEngine::why_not`]. It
/// analyzes a pattern against a data source without requiring engine registration.
///
/// Unlike `SiftEngine::why_not` (which returns `Option` because the pattern
/// might not be registered), this always returns a `GapAnalysis` since the
/// pattern is provided directly.
pub fn gap_analysis<N, L, V, T>(
    ds: &(impl DataSource<N = N, L = L, V = V, T = T> + ?Sized),
    pattern: &Pattern<L, V>,
) -> GapAnalysis<L, V>
where
    N: Eq + Hash + Clone + Debug,
    L: Eq + Hash + Clone + Debug,
    V: PartialEq + PartialOrd + Clone + Debug + Hash,
    T: Ord + Clone + Debug + Hash + std::ops::Sub<Output = T> + crate::interval::NumericTime,
{
    let now = ds.now();
    gap_analysis_at(ds, pattern, &now)
}

// ---------------------------------------------------------------------------
// Public variants with explicit time parameter
// ---------------------------------------------------------------------------

/// Evaluate a single pattern at a specific time point.
///
/// Like [`evaluate_pattern`] but takes an explicit `at` time instead of
/// calling `ds.now()`. Useful for speculative evaluation -- checking what
/// would match at a future or past timestamp without mutating the graph's clock.
pub fn evaluate_pattern_at<N, L, V, T>(
    ds: &(impl DataSource<N = N, L = L, V = V, T = T> + ?Sized),
    pattern: &Pattern<L, V>,
    now: &T,
) -> Vec<Match<N, V, T>>
where
    N: Eq + Hash + Clone + Debug,
    L: Eq + Hash + Clone + Debug,
    V: PartialEq + PartialOrd + Clone + Debug + Hash + crate::expr::ArithmeticValue,
    T: Ord + Clone + Debug + Hash + std::ops::Sub<Output = T> + crate::interval::NumericTime,
{
    if pattern.stages.is_empty() {
        return Vec::new();
    }

    // Build step list: each step is either a single stage or an unordered group.
    // Steps are processed sequentially; within an unordered group, all stages
    // must match but in any order.
    let steps = build_stage_steps(pattern);

    // Process first step
    let mut candidates: Vec<MatchCandidate<N, V, T>> = match &steps[0] {
        StageStep::Single(idx) => {
            find_stage_matches(ds, &pattern.stages[*idx], &HashMap::new(), now)
        }
        StageStep::Unordered(indices) => {
            let init = vec![(HashMap::new(), HashMap::new())];
            expand_unordered_group(ds, pattern, &init, indices, now)
        }
    };

    // Process remaining steps
    for step in &steps[1..] {
        match step {
            StageStep::Single(idx) => {
                let mut next = Vec::new();
                for (bindings, intervals) in &candidates {
                    for (new_b, new_i) in
                        find_stage_matches(ds, &pattern.stages[*idx], bindings, now)
                    {
                        let mut merged_b = bindings.clone();
                        merged_b.extend(new_b);
                        let mut merged_i = intervals.clone();
                        merged_i.extend(new_i);
                        next.push((merged_b, merged_i));
                    }
                }
                candidates = next;
            }
            StageStep::Unordered(indices) => {
                candidates = expand_unordered_group(ds, pattern, &candidates, indices, now);
            }
        }
    }

    candidates
        .into_iter()
        .filter(|(bindings, intervals)| {
            check_temporal(pattern, intervals)
                && check_negations_batch(ds, pattern, bindings, intervals)
        })
        .map(|(bindings, intervals)| Match {
            pattern: pattern.name.clone(),
            pattern_idx: None,
            bindings,
            intervals,
            metadata: pattern.metadata.clone(),
        })
        .collect()
}

/// Standalone gap analysis at a specific time point.
///
/// Like [`gap_analysis`] but takes an explicit `at` time instead of
/// calling `ds.now()`.
pub fn gap_analysis_at<N, L, V, T>(
    ds: &(impl DataSource<N = N, L = L, V = V, T = T> + ?Sized),
    pattern: &Pattern<L, V>,
    now: &T,
) -> GapAnalysis<L, V>
where
    N: Eq + Hash + Clone + Debug,
    L: Eq + Hash + Clone + Debug,
    V: PartialEq + PartialOrd + Clone + Debug + Hash,
    T: Ord + Clone + Debug + Hash + std::ops::Sub<Output = T> + crate::interval::NumericTime,
{
    let mut stages = Vec::new();
    let bindings: HashMap<String, BoundValue<N, V>> = HashMap::new();

    for stage in &pattern.stages {
        let mut clause_analyses = Vec::new();
        let mut stage_matched = true;

        for clause in &stage.clauses {
            let (matched, reason) = analyze_clause(ds, clause, &bindings, now);
            if !matched {
                stage_matched = false;
            }
            clause_analyses.push(ClauseAnalysis {
                description: format!(
                    "?{} --[{:?}]--> {:?}{}",
                    clause.source.0,
                    clause.label,
                    clause.target,
                    if clause.negated { " (NOT)" } else { "" }
                ),
                matched,
                reason,
                source_var: clause.source.0.clone(),
                label: clause.label.clone(),
                target: clause.target.clone(),
                negated: clause.negated,
            });
        }

        let matched_count = clause_analyses.iter().filter(|c| c.matched).count();
        let total = clause_analyses.len();
        let status = if stage_matched {
            StageStatus::Matched
        } else if matched_count > 0 {
            StageStatus::PartiallyMatched {
                matched: matched_count,
                total,
            }
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

    GapAnalysis {
        pattern: pattern.name.clone(),
        stages,
    }
}

// ---------------------------------------------------------------------------
// Unordered group helpers for batch evaluation
// ---------------------------------------------------------------------------

/// A step in the batch evaluation pipeline: either a single ordered stage
/// or a group of unordered stages that can match in any order.
enum StageStep {
    Single(usize),
    Unordered(Vec<usize>),
}

/// Build a list of evaluation steps from a pattern's stages and unordered groups.
/// Consecutive stages in an unordered group are merged into a single step.
fn build_stage_steps<L, V>(pattern: &Pattern<L, V>) -> Vec<StageStep> {
    let mut steps = Vec::new();
    let mut consumed = vec![false; pattern.stages.len()];

    for i in 0..pattern.stages.len() {
        if consumed[i] {
            continue;
        }
        if let Some(group) = pattern.unordered_group_for(i) {
            for &gi in group {
                consumed[gi] = true;
            }
            steps.push(StageStep::Unordered(group.clone()));
        } else {
            consumed[i] = true;
            steps.push(StageStep::Single(i));
        }
    }
    steps
}

/// Expand candidates through an unordered group: for each candidate, match
/// ALL stages in the group. Stages are tried in index order; join constraints
/// ensure consistent results regardless of matching order since bindings
/// accumulate across stages and `find_stage_matches` handles both bound and
/// unbound variables.
fn expand_unordered_group<N, L, V, T>(
    ds: &(impl DataSource<N = N, L = L, V = V, T = T> + ?Sized),
    pattern: &Pattern<L, V>,
    candidates: &[MatchCandidate<N, V, T>],
    group_indices: &[usize],
    now: &T,
) -> Vec<MatchCandidate<N, V, T>>
where
    N: Eq + Hash + Clone + Debug,
    L: Eq + Hash + Clone + Debug,
    V: PartialEq + PartialOrd + Clone + Debug + Hash + crate::expr::ArithmeticValue,
    T: Ord + Clone + Debug + Hash,
{
    let mut result = Vec::new();
    for (bindings, intervals) in candidates {
        let mut partial = vec![(bindings.clone(), intervals.clone())];
        // Try each remaining unmatched stage -- since stages are unordered,
        // we process them in index order but each stage match is independent.
        // This works because find_stage_matches uses bindings for join
        // constraints, so each subsequent stage is constrained by prior matches.
        // All permutations that satisfy join constraints will produce the same
        // final bindings (set of matched values is the same regardless of order).
        for &si in group_indices {
            let mut next_partial = Vec::new();
            for (b, iv) in &partial {
                for (new_b, new_iv) in find_stage_matches(ds, &pattern.stages[si], b, now) {
                    let mut merged_b = b.clone();
                    merged_b.extend(new_b);
                    let mut merged_iv = iv.clone();
                    merged_iv.extend(new_iv);
                    next_partial.push((merged_b, merged_iv));
                }
            }
            partial = next_partial;
        }
        result.extend(partial);
    }
    result
}

pub(super) fn find_stage_matches<N, L, V, T>(
    ds: &(impl DataSource<N = N, L = L, V = V, T = T> + ?Sized),
    stage: &Stage<L, V>,
    existing: &HashMap<String, BoundValue<N, V>>,
    now: &T,
) -> Vec<MatchCandidate<N, V, T>>
where
    N: Eq + Hash + Clone + Debug,
    L: Eq + Hash + Clone + Debug,
    V: PartialEq + PartialOrd + Clone + Debug + Hash + crate::expr::ArithmeticValue,
    T: Ord + Clone + Debug + Hash,
{
    if stage.clauses.is_empty() {
        return Vec::new();
    }

    let first = &stage.clauses[0];
    let mut candidates = Vec::new();

    // Resolve *Var constraints in the first clause before matching
    let resolved_first_target = match resolve_target(&first.target, existing) {
        Some(t) => t,
        None => return Vec::new(),
    };

    if let Some(bound) = existing.get(&first.source.0) {
        if let BoundValue::Node(ref node) = bound {
            for e in ds.edges_from(node, &first.label, now) {
                if target_matches_ds(ds, &resolved_first_target, &e.target, existing) {
                    let mut b = HashMap::new();
                    if !bind_target(ds, &first.target, &e.target, &mut b) {
                        continue;
                    }
                    let mut iv = HashMap::new();
                    iv.insert(stage.anchor.0.clone(), e.interval.clone());
                    candidates.push((b, iv));
                }
            }
        }
    } else {
        let constraint = match &resolved_first_target {
            Target::Literal(v) => ValueConstraint::Eq(v.clone()),
            Target::Constraint(c) => c.clone(),
            Target::Bind(_) => ValueConstraint::Any,
        };
        for e in ds.scan(&first.label, &constraint, now) {
            let mut b = HashMap::new();
            b.insert(first.source.0.clone(), BoundValue::Node(e.source.clone()));
            b.insert(stage.anchor.0.clone(), BoundValue::Node(e.source.clone()));
            if !bind_target(ds, &first.target, &e.target, &mut b) {
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
        let mut merged: HashMap<String, BoundValue<N, V>> = existing
            .iter()
            .chain(b.iter())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let mut all_ok = true;
        for c in &stage.clauses[1..] {
            if !clause_satisfied(ds, c, &merged, now) {
                all_ok = false;
                break;
            }
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
            // Re-merge any bindings added during the clause loop into `merged`,
            // since clause bindings written into `b` may not have been mirrored.
            for (k, v) in b.iter() {
                merged.entry(k.clone()).or_insert_with(|| v.clone());
            }
            if !eval_stage_lets(stage, &mut merged) {
                continue;
            }
            // Surface let-derived bindings into the candidate's `b` so callers
            // (which extend their own bindings with `b`) see them.
            for cb in &stage.let_bindings {
                if let Some(bv) = merged.get(&cb.name) {
                    b.insert(cb.name.clone(), bv.clone());
                }
            }
            result.push((b, iv));
        }
    }

    result
}

pub(super) fn target_matches_ds<N, L, V, T>(
    ds: &(impl DataSource<N = N, L = L, V = V, T = T> + ?Sized),
    target: &Target<V>,
    value: &V,
    bindings: &HashMap<String, BoundValue<N, V>>,
) -> bool
where
    N: Eq + Hash + Clone + Debug,
    V: PartialEq + PartialOrd + Clone + Debug,
{
    match target {
        Target::Literal(v) => value == v,
        Target::Constraint(c) => {
            if c.is_var() {
                // Resolve *Var constraints before matching
                match resolve_constraint(c, bindings) {
                    Some(resolved) => resolved.matches(value),
                    None => false,
                }
            } else {
                c.matches(value)
            }
        }
        Target::Bind(var) => {
            if let Some(bound) = bindings.get(&var.0) {
                bound.matches_value(&|v| ds.value_as_node(v), value)
            } else {
                true
            }
        }
    }
}

pub(super) fn bind_target<N, L, V, T>(
    ds: &(impl DataSource<N = N, L = L, V = V, T = T> + ?Sized),
    target: &Target<V>,
    value: &V,
    bindings: &mut HashMap<String, BoundValue<N, V>>,
) -> bool
where
    N: Eq + Hash + Clone + Debug,
    V: PartialEq + PartialOrd + Clone + Debug,
{
    if let Target::Bind(ref var) = target {
        if let Some(existing) = bindings.get(&var.0) {
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

pub(super) fn clause_satisfied<N, L, V, T>(
    ds: &(impl DataSource<N = N, L = L, V = V, T = T> + ?Sized),
    clause: &Clause<L, V>,
    bindings: &HashMap<String, BoundValue<N, V>>,
    now: &T,
) -> bool
where
    N: Eq + Hash + Clone + Debug,
    L: Eq + Hash + Clone + Debug,
    V: PartialEq + PartialOrd + Clone + Debug,
    T: Ord + Clone + Debug,
{
    let source = match bindings.get(&clause.source.0) {
        Some(BoundValue::Node(n)) => n,
        _ => return false,
    };
    let edges = ds.edges_from(source, &clause.label, now);
    let found = edges
        .iter()
        .any(|e| target_matches_ds(ds, &clause.target, &e.target, bindings));
    if clause.negated {
        !found
    } else {
        found
    }
}

pub(super) fn check_temporal<L, V, T>(
    pattern: &Pattern<L, V>,
    intervals: &HashMap<String, Interval<T>>,
) -> bool
where
    T: Ord + Clone + Debug + std::ops::Sub<Output = T> + crate::interval::NumericTime,
{
    // Invariant: unordered groups contain consecutive stage indices.
    debug_assert!(
        pattern.unordered_groups.iter().all(|g| {
            if g.is_empty() {
                return true;
            }
            let min = *g.iter().min().unwrap();
            let max = *g.iter().max().unwrap();
            max - min + 1 == g.len()
        }),
        "unordered groups must contain consecutive stage indices"
    );

    // Implicit temporal ordering: segments are ordered left-to-right.
    // A "segment" is either a single ordered stage or an unordered group.
    // Within an unordered group, no ordering is enforced.
    // Between segments, ALL stages of the earlier segment must precede
    // ALL stages of the later segment.
    let steps = build_stage_steps(pattern);
    for pair in steps.windows(2) {
        let left_indices = match &pair[0] {
            StageStep::Single(i) => vec![*i],
            StageStep::Unordered(g) => g.clone(),
        };
        let right_indices = match &pair[1] {
            StageStep::Single(i) => vec![*i],
            StageStep::Unordered(g) => g.clone(),
        };
        for &li in &left_indices {
            for &ri in &right_indices {
                if let (Some(left_iv), Some(right_iv)) = (
                    intervals.get(&pattern.stages[li].anchor.0),
                    intervals.get(&pattern.stages[ri].anchor.0),
                ) {
                    if left_iv.start >= right_iv.start {
                        return false;
                    }
                }
            }
        }
    }
    // Explicit constraints
    for tc in &pattern.temporal {
        if let (Some(left), Some(right)) = (intervals.get(&tc.left.0), intervals.get(&tc.right.0)) {
            match left.relation(right) {
                Some(rel) if rel == tc.relation => {}
                None if tc.relation.is_before_or_meets() && left.start < right.start => {}
                _ => return false,
            }
            if let Some(ref gap_bound) = tc.gap {
                if let Some(gap_val) = left.gap_for_relation(right, tc.relation) {
                    if let Some(min) = gap_bound.min {
                        if gap_val < min {
                            return false;
                        }
                    }
                    if let Some(max) = gap_bound.max {
                        if gap_val > max {
                            return false;
                        }
                    }
                }
            }
        }
    }
    true
}

pub(super) fn check_negations_batch<N, L, V, T>(
    ds: &(impl DataSource<N = N, L = L, V = V, T = T> + ?Sized),
    pattern: &Pattern<L, V>,
    match_bindings: &HashMap<String, BoundValue<N, V>>,
    intervals: &HashMap<String, Interval<T>>,
) -> bool
where
    N: Eq + Hash + Clone + Debug,
    L: Eq + Hash + Clone + Debug,
    V: PartialEq + PartialOrd + Clone + Debug + Hash,
    T: Ord + Clone + Debug + Hash,
{
    let now = ds.now();

    for negation in &pattern.negations {
        let start = match intervals.get(&negation.between_start.0) {
            Some(iv) => &iv.start,
            None => continue,
        };
        let end = negation
            .between_end
            .as_ref()
            .and_then(|v| intervals.get(&v.0))
            .map(|iv| &iv.start);

        if negation.clauses.is_empty() {
            continue;
        }

        let first = &negation.clauses[0];
        let constraint = match &first.target {
            Target::Literal(v) => ValueConstraint::Eq(v.clone()),
            Target::Constraint(c) => {
                match resolve_constraint(c, match_bindings) {
                    Some(resolved) => resolved,
                    None => continue, // Variable unbound → skip negation
                }
            }
            _ => ValueConstraint::Any,
        };
        let candidates = ds.scan_any_time(&first.label, &constraint);

        for cand in &candidates {
            let in_window =
                &cand.interval.start > start && end.is_none_or(|e| &cand.interval.start < e);
            if !in_window {
                continue;
            }

            let neg_entity = &cand.source;

            let all_ok = negation.clauses[1..].iter().all(|clause| {
                let src = if clause.source.0 == first.source.0 {
                    neg_entity.clone()
                } else {
                    return false;
                };
                let edges = ds.edges_from(&src, &clause.label, &now);
                let resolved_target = match resolve_target(&clause.target, match_bindings) {
                    Some(t) => t,
                    None => return false,
                };
                edges.iter().any(|e| match &resolved_target {
                    Target::Literal(v) => &e.target == v,
                    Target::Constraint(c) => c.matches(&e.target),
                    Target::Bind(var) => {
                        if let Some(bound) = match_bindings.get(&var.0) {
                            bound.matches_value(&|v| ds.value_as_node(v), &e.target)
                        } else {
                            true
                        }
                    }
                })
            });

            if all_ok {
                return false;
            }
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
pub(super) fn check_negation_kill<N, L, V, T>(
    ds: &(impl DataSource<N = N, L = L, V = V, T = T> + ?Sized),
    pattern: &Pattern<L, V>,
    pm: &PartialMatch<N, V, T>,
    source: &N,
    label: &L,
    value: &V,
    interval: &Interval<T>,
) -> Option<String>
where
    N: Eq + Hash + Clone + Debug,
    L: Eq + Hash + Clone + Debug,
    V: PartialEq + PartialOrd + Clone + Debug + Hash,
    T: Ord + Clone + Debug + Hash,
{
    for negation in &pattern.negations {
        let start = match pm.intervals.get(&negation.between_start.0) {
            Some(iv) => iv,
            None => continue,
        };
        if let Some(ref end_var) = negation.between_end {
            if pm.intervals.contains_key(&end_var.0) {
                continue;
            }
        }
        if interval.start <= start.start {
            continue;
        }

        for (i, clause) in negation.clauses.iter().enumerate() {
            if &clause.label != label {
                continue;
            }
            let target_ok = match &clause.target {
                Target::Literal(v) => value == v,
                Target::Constraint(c) => {
                    if c.is_var() {
                        match resolve_constraint(c, &pm.bindings) {
                            Some(resolved) => resolved.matches(value),
                            None => false,
                        }
                    } else {
                        c.matches(value)
                    }
                }
                Target::Bind(_) => true,
            };
            if !target_ok {
                continue;
            }
            if let Some(BoundValue::Node(ref n)) = pm.bindings.get(&clause.source.0) {
                if source != n {
                    continue;
                }
            }
            if let Target::Bind(ref var) = clause.target {
                if let Some(bound) = pm.bindings.get(&var.0) {
                    if !bound.matches_value(&|v| ds.value_as_node(v), value) {
                        continue;
                    }
                }
            }

            let now = ds.now();
            let mut all_others_ok = true;
            for (j, other) in negation.clauses.iter().enumerate() {
                if j == i {
                    continue;
                }
                let src = if other.source.0 == clause.source.0 {
                    source.clone()
                } else if let Some(BoundValue::Node(ref n)) = pm.bindings.get(&other.source.0) {
                    n.clone()
                } else {
                    all_others_ok = false;
                    break;
                };

                let edges = ds.edges_from(&src, &other.label, &now);
                let resolved_other_target = match resolve_target(&other.target, &pm.bindings) {
                    Some(t) => t,
                    None => {
                        all_others_ok = false;
                        break;
                    }
                };
                let found = edges.iter().any(|e| match &resolved_other_target {
                    Target::Literal(v) => &e.target == v,
                    Target::Constraint(c) => c.matches(&e.target),
                    Target::Bind(var) => {
                        if let Some(bound) = pm.bindings.get(&var.0) {
                            bound.matches_value(&|v| ds.value_as_node(v), &e.target)
                        } else {
                            true
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

#[allow(clippy::type_complexity)]
pub(super) fn try_match_stage<N, L, V, T>(
    ds: &(impl DataSource<N = N, L = L, V = V, T = T> + ?Sized),
    stage: &Stage<L, V>,
    source: &N,
    label: &L,
    value: &V,
    interval: &Interval<T>,
    existing: &HashMap<String, BoundValue<N, V>>,
) -> Option<Vec<MatchCandidate<N, V, T>>>
where
    N: Eq + Hash + Clone + Debug,
    L: Eq + Hash + Clone + Debug,
    V: PartialEq + PartialOrd + Clone + Debug + Hash + crate::expr::ArithmeticValue,
    T: Ord + Clone + Debug + Hash,
{
    let first = stage.clauses.first()?;

    if &first.label != label {
        return None;
    }

    if let Some(BoundValue::Node(ref n)) = existing.get(&first.source.0) {
        if source != n {
            return None;
        }
    }

    // Resolve *Var constraints in the first clause
    let resolved_first_target = resolve_target(&first.target, existing)?;
    let target_ok = match &resolved_first_target {
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

    let event_time = &interval.start;

    let mut merged: HashMap<String, BoundValue<N, V>> = existing
        .iter()
        .chain(bindings.iter())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    for clause in &stage.clauses[1..] {
        let src_node = match merged.get(&clause.source.0) {
            Some(BoundValue::Node(n)) => n.clone(),
            _ => return None,
        };
        let edges = ds.edges_from(&src_node, &clause.label, event_time);
        // Resolve *Var constraints against current bindings
        let resolved_clause_target = resolve_target(&clause.target, &merged)?;
        let matching_edge = edges.iter().find(|e| match &resolved_clause_target {
            Target::Literal(v) => &e.target == v,
            Target::Constraint(c) => c.matches(&e.target),
            Target::Bind(var) => {
                if let Some(bound) = merged.get(&var.0) {
                    bound.matches_value(&|v| ds.value_as_node(v), &e.target)
                } else {
                    true
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

    // Evaluate stage lets against the merged map and surface results back.
    if !eval_stage_lets(stage, &mut merged) {
        return None;
    }
    for cb in &stage.let_bindings {
        if let Some(bv) = merged.get(&cb.name) {
            bindings.insert(cb.name.clone(), bv.clone());
        }
    }

    Some(vec![(bindings, intervals)])
}

pub(super) fn analyze_clause<N, L, V, T>(
    ds: &(impl DataSource<N = N, L = L, V = V, T = T> + ?Sized),
    clause: &Clause<L, V>,
    bindings: &HashMap<String, BoundValue<N, V>>,
    now: &T,
) -> (bool, Option<String>)
where
    N: Eq + Hash + Clone + Debug,
    L: Eq + Hash + Clone + Debug,
    V: PartialEq + PartialOrd + Clone + Debug,
    T: Ord + Clone + Debug,
{
    let source = match bindings.get(&clause.source.0) {
        Some(BoundValue::Node(n)) => n,
        Some(_) => {
            return (
                false,
                Some(format!(
                    "?{} is bound to a value, not a node",
                    clause.source.0
                )),
            )
        }
        None => return (false, Some(format!("?{} is not bound", clause.source.0))),
    };

    let edges = ds.edges_from(source, &clause.label, now);
    let found = edges
        .iter()
        .any(|e| target_matches_ds(ds, &clause.target, &e.target, bindings));
    let ok = if clause.negated { !found } else { found };

    if ok {
        (true, None)
    } else if clause.negated {
        (
            false,
            Some(format!("edge {:?} exists but should not", clause.label)),
        )
    } else if edges.is_empty() {
        (
            false,
            Some(format!(
                "no edges with label {:?} from ?{}",
                clause.label, clause.source.0
            )),
        )
    } else {
        (
            false,
            Some("edges exist but none match target constraint".to_string()),
        )
    }
}
