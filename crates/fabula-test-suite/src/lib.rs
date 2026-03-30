//! Golden test suite for fabula DataSource adapters.
//!
//! This crate defines the `TestGraph` trait, implements it for all three
//! adapters (MemGraph, PetTemporalGraph, GrafeoGraph), and provides a library
//! of scenario functions generic over `TestGraph`. The companion
//! `tests/golden.rs` stamps out `#[test]` functions for every
//! (scenario x adapter) combination via the `golden_tests!` macro.
//!
//! # Adding a new golden scenario
//!
//! 1. Write a `pub fn my_scenario<G: TestGraph>()` in `src/scenarios/`.
//!    Build the graph, build the pattern, run the engine, assert results.
//! 2. Add the function name to the `golden_tests!` invocation in
//!    `tests/golden.rs`.
//! 3. Done — it now runs against all three adapters.

pub mod scenarios;

use fabula::datasource::DataSource;

// Re-export adapter types so tests/golden.rs can reference them.
pub use fabula_memory::{MemGraph, MemValue};
pub use fabula_petgraph::{PetTemporalGraph, PetValue};
pub use fabula_grafeo::{GrafeoGraph, GrafeoValue};

/// Concrete petgraph type used in golden tests.
pub type PetGraph = PetTemporalGraph<String, String, PetValue<String>, i64>;

/// Abstraction over the three adapter types, providing a uniform way to
/// build test graphs and patterns without knowing the concrete value type.
///
/// Each adapter implements this trait once (in this crate, satisfying orphan
/// rules since TestGraph is local). Scenario functions are generic over
/// `TestGraph` and call these methods.
pub trait TestGraph: DataSource<N = String, L = String, T = i64> + Sized {
    /// Create a new empty graph.
    fn new_graph() -> Self;

    /// Set the current time.
    fn set_current_time(&mut self, t: i64);

    /// Add a string-valued edge: `from --[label]--> "value"` starting at `start`.
    fn add_str_edge(&mut self, from: &str, label: &str, value: &str, start: i64);

    /// Add a node-reference edge: `from --[label]--> @to` starting at `start`.
    fn add_ref_edge(&mut self, from: &str, label: &str, to: &str, start: i64);

    /// Add a numeric-valued edge: `from --[label]--> num` starting at `start`.
    fn add_num_edge(&mut self, from: &str, label: &str, value: f64, start: i64);

    /// Add a string-valued edge with a bounded interval `[start, end)`.
    fn add_str_edge_bounded(&mut self, from: &str, label: &str, value: &str, start: i64, end: i64);

    /// Add a node-reference edge with a bounded interval `[start, end)`.
    fn add_ref_edge_bounded(&mut self, from: &str, label: &str, to: &str, start: i64, end: i64);

    // --- Value constructors (needed for building patterns) ---

    /// Create a string literal value.
    fn str_val(s: &str) -> Self::V;

    /// Create a node reference value.
    fn node_val(s: &str) -> Self::V;

    /// Create a numeric value.
    fn num_val(n: f64) -> Self::V;

    /// Check if a bound value is a node equal to the given string.
    fn is_node_eq(value: &fabula::prelude::BoundValue<String, Self::V>, expected: &str) -> bool {
        match value {
            fabula::prelude::BoundValue::Node(n) => n == expected,
            _ => false,
        }
    }
}

// ===========================================================================
// TestGraph impl: MemGraph
// ===========================================================================

impl TestGraph for MemGraph {
    fn new_graph() -> Self {
        MemGraph::new()
    }

    fn set_current_time(&mut self, t: i64) {
        self.set_time(t);
    }

    fn add_str_edge(&mut self, from: &str, label: &str, value: &str, start: i64) {
        self.add_str(from, label, value, start);
    }

    fn add_ref_edge(&mut self, from: &str, label: &str, to: &str, start: i64) {
        self.add_ref(from, label, to, start);
    }

    fn add_num_edge(&mut self, from: &str, label: &str, value: f64, start: i64) {
        self.add_num(from, label, value, start);
    }

    fn add_str_edge_bounded(&mut self, from: &str, label: &str, value: &str, start: i64, end: i64) {
        self.add_edge_bounded(from, label, MemValue::Str(value.to_string()), start, end);
    }

    fn add_ref_edge_bounded(&mut self, from: &str, label: &str, to: &str, start: i64, end: i64) {
        self.add_edge_bounded(from, label, MemValue::Node(to.to_string()), start, end);
    }

    fn str_val(s: &str) -> MemValue {
        MemValue::Str(s.to_string())
    }

    fn node_val(s: &str) -> MemValue {
        MemValue::Node(s.to_string())
    }

    fn num_val(n: f64) -> MemValue {
        MemValue::Num(n)
    }
}

// ===========================================================================
// TestGraph impl: PetTemporalGraph (via PetGraph type alias)
// ===========================================================================

impl TestGraph for PetGraph {
    fn new_graph() -> Self {
        PetTemporalGraph::new(0)
    }

    fn set_current_time(&mut self, t: i64) {
        self.set_time(t);
    }

    fn add_str_edge(&mut self, from: &str, label: &str, value: &str, start: i64) {
        self.add_node(from.to_string());
        self.add_edge(
            from.to_string(),
            label.to_string(),
            PetValue::Str(value.to_string()),
            fabula::interval::Interval::open(start),
        );
    }

    fn add_ref_edge(&mut self, from: &str, label: &str, to: &str, start: i64) {
        self.add_node(from.to_string());
        self.add_node(to.to_string());
        self.add_edge(
            from.to_string(),
            label.to_string(),
            PetValue::Node(to.to_string()),
            fabula::interval::Interval::open(start),
        );
    }

    fn add_num_edge(&mut self, from: &str, label: &str, value: f64, start: i64) {
        self.add_node(from.to_string());
        self.add_edge(
            from.to_string(),
            label.to_string(),
            PetValue::Num(value),
            fabula::interval::Interval::open(start),
        );
    }

    fn add_str_edge_bounded(&mut self, from: &str, label: &str, value: &str, start: i64, end: i64) {
        self.add_node(from.to_string());
        self.add_edge_bounded(
            from.to_string(),
            label.to_string(),
            PetValue::Str(value.to_string()),
            start,
            end,
        );
    }

    fn add_ref_edge_bounded(&mut self, from: &str, label: &str, to: &str, start: i64, end: i64) {
        self.add_node(from.to_string());
        self.add_node(to.to_string());
        self.add_edge_bounded(
            from.to_string(),
            label.to_string(),
            PetValue::Node(to.to_string()),
            start,
            end,
        );
    }

    fn str_val(s: &str) -> PetValue<String> {
        PetValue::Str(s.to_string())
    }

    fn node_val(s: &str) -> PetValue<String> {
        PetValue::Node(s.to_string())
    }

    fn num_val(n: f64) -> PetValue<String> {
        PetValue::Num(n)
    }
}

// ===========================================================================
// TestGraph impl: GrafeoGraph
// ===========================================================================

impl TestGraph for GrafeoGraph {
    fn new_graph() -> Self {
        GrafeoGraph::new()
    }

    fn set_current_time(&mut self, t: i64) {
        self.set_time(t);
    }

    fn add_str_edge(&mut self, from: &str, label: &str, value: &str, start: i64) {
        self.add_str(from, label, value, start);
    }

    fn add_ref_edge(&mut self, from: &str, label: &str, to: &str, start: i64) {
        self.add_ref(from, label, to, start);
    }

    fn add_num_edge(&mut self, from: &str, label: &str, value: f64, start: i64) {
        self.add_num(from, label, value, start);
    }

    fn add_str_edge_bounded(&mut self, from: &str, label: &str, value: &str, start: i64, end: i64) {
        self.add_edge_bounded(from, label, GrafeoValue::Str(value.to_string()), start, end);
    }

    fn add_ref_edge_bounded(&mut self, from: &str, label: &str, to: &str, start: i64, end: i64) {
        self.add_edge_bounded(from, label, GrafeoValue::Node(to.to_string()), start, end);
    }

    fn str_val(s: &str) -> GrafeoValue {
        GrafeoValue::Str(s.to_string())
    }

    fn node_val(s: &str) -> GrafeoValue {
        GrafeoValue::Node(s.to_string())
    }

    fn num_val(n: f64) -> GrafeoValue {
        GrafeoValue::Num(n)
    }
}

