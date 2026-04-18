#![allow(non_snake_case)]
//! Golden test runner — stamps out every scenario for every adapter.
//!
//! # How it works
//!
//! 1. `TestGraph` impls for all adapters live in `fabula_test_suite::lib`
//!    (satisfying orphan rules).
//! 2. Scenario functions in `fabula_test_suite::scenarios` are generic over
//!    `TestGraph` — they build graphs, patterns, and assertions once.
//! 3. The `golden_tests!` macro below generates 3 `#[test]` functions per
//!    scenario (one per adapter).
//!
//! # Adding a new golden test
//!
//! 1. Write `pub fn my_scenario<G: TestGraph>()` in `src/scenarios/*.rs`.
//! 2. Re-export it from `src/scenarios/mod.rs`.
//! 3. Add `my_scenario,` to the `golden_tests!` invocation below.
//! 4. `cargo test -p fabula-test-suite` now runs it against all three adapters.

use fabula_test_suite::scenarios;
use fabula_test_suite::{GrafeoGraph, MemGraph, PetGraph};

// ===========================================================================
// The golden_tests! macro
// ===========================================================================

/// Generates `#[test]` functions for every (adapter, scenario) pair.
///
/// Expands each `scenario_name` into:
///   - `mem__scenario_name`
///   - `pet__scenario_name`
///   - `grafeo__scenario_name`
macro_rules! golden_tests {
    ( $( $scenario:ident ),* $(,)? ) => {
        $(
            paste::paste! {
                #[test]
                fn [< mem__ $scenario >]() {
                    scenarios::$scenario::<MemGraph>();
                }

                #[test]
                fn [< pet__ $scenario >]() {
                    scenarios::$scenario::<PetGraph>();
                }

                #[test]
                fn [< grafeo__ $scenario >]() {
                    scenarios::$scenario::<GrafeoGraph>();
                }
            }
        )*
    };
}

// ===========================================================================
// Scenario registry — add one line here, get tests for all three adapters.
// ===========================================================================

golden_tests! {
    // --- Hospitality (batch) ---
    batch_hospitality_matches,
    batch_hospitality_negated_when_guest_leaves,
    batch_hospitality_unrelated_leave,
    batch_hospitality_two_guests,
    batch_hospitality_missing_middle,
    batch_hospitality_host_property_check,

    // --- Romantic arc (batch) ---
    batch_romantic_arc_matches,
    batch_romantic_arc_different_characters,
    batch_romantic_arc_inline_negation,
    batch_romantic_arc_combinatorial,

    // --- Value constraints (batch) ---
    batch_value_lt_matches,
    batch_value_lt_no_match,
    batch_value_between_matches,
    batch_value_between_no_match,
    batch_value_gt_matches,
    batch_value_eq_string,
    batch_value_constraint_in_negation,

    // --- Temporal ordering (batch) ---
    batch_rejects_wrong_temporal_order,
    batch_correct_temporal_order,
    temporal_same_timestamp_no_sequence,
    temporal_large_gap_still_matches,

    // --- Negation windows (batch) ---
    batch_unless_after_blocks,
    batch_unless_global,
    batch_double_negation_two_windows,
    batch_negation_multi_clause_body,
    batch_negation_at_boundary_exclusive,

    // --- Two betrayals (batch) ---
    batch_two_betrayals_match,
    batch_two_betrayals_intervening_blocks,
    batch_two_betrayals_other_actor_doesnt_block,
    batch_two_betrayals_non_impulsive_no_match,

    // --- Allen temporal (batch) ---
    batch_explicit_before_constraint,
    batch_explicit_during_constraint,
    batch_explicit_overlaps_constraint,

    // --- Causality ---
    causality_single_hop,
    causality_multi_hop_chain,
    causality_no_causal_edges,
    causality_max_hops_limit,
    causality_sorted_by_cleanliness,

    // --- Winnow replay ---
    incremental_winnow_7step_sequence,
    batch_winnow_multi_pattern,

    // --- Incremental matching ---
    incremental_hospitality_three_stages,
    incremental_negation_kills,
    incremental_unrelated_leave_no_kill,
    incremental_single_stage_completes,
    incremental_drain_completed,
    incremental_irrelevant_edges_silent,
    incremental_negation_event_details,
    incremental_second_host_forks_pm,
    incremental_original_pm_survives_advance,
    incremental_dead_and_complete_inert,
    incremental_negation_checked_before_advance,
    incremental_negation_only_when_window_open,
    incremental_negation_after_completion_no_retroactive,

    // --- Gap analysis ---
    gap_empty_graph,
    gap_unknown_pattern,
    gap_with_partial_data,
    gap_partially_matched_stage,
    gap_second_stage_fails_binding,
    gap_negated_clause_in_stage,

    // --- Multi-pattern ---
    incremental_multiple_patterns_fire,
    multi_pattern_all_four_winnow,
    multi_pattern_shared_events,

    // --- Consistency ---
    batch_incremental_negation_consistency,
    batch_incremental_multi_match_consistency,
    drain_completed_idempotent,
    drain_completed_interleaved,

    // --- Composition ---
    batch_sequence_shared_binding,
    batch_sequence_different_actors_no_match,
    batch_sequence_with_negation,
    incremental_choice_exclusive,
    incremental_choice_exclusive_multistage,
    incremental_choice_nonexclusive,
    batch_repeat_shared_binding,
    batch_repeat_different_actors_no_match,
    private_pattern_suppresses_events,

    // --- Cross-stage value comparison ---
    batch_cross_stage_gt_matches,
    batch_cross_stage_gt_no_match,
    batch_cross_stage_lt_matches,
    batch_cross_stage_eq_matches,
    incremental_cross_stage_gt,

    // --- Value disjunction (OneOf) ---
    batch_one_of_matches_any,
    batch_one_of_rejects_unlisted,
    batch_one_of_with_variable_join,
    incremental_one_of_advances,

    // --- Unordered (concurrent) stage groups ---
    batch_unordered_group_any_order,
    batch_unordered_group_after_ordered,
    batch_unordered_group_before_ordered,
    batch_unordered_group_ordering_with_ordered,
    incremental_unordered_group,
    batch_unordered_group_shared_binding,
}
