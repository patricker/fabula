//! Retrospective causal pathfinding over temporal graphs.
//!
//! Given an effect node in the graph and a set of causal edge labels, finds
//! all paths of length up to `max_hops` leading to the effect, ordered by a
//! cleanliness score that combines edge weights, time gaps, and path
//! divergence.
//!
//! # Design
//!
//! Causality is represented as explicit edges in the graph (e.g., an edge
//! labeled `"causes"` from event A to event B means A caused B). Callers
//! mark which labels are causal and with what weight. Edges not in the
//! causal labels map are ignored.
//!
//! Based on ROADMAP Phase 6.1.

/// A traced causal chain leading to an effect.
#[derive(Debug, Clone)]
pub struct CausalPath<N, V, T> {
    /// Nodes along the path, ordered from root cause to effect (inclusive at both ends).
    pub nodes: Vec<N>,
    /// Edges traversed, one fewer than `nodes.len()`.
    pub edges: Vec<CausalEdge<V, T>>,
    /// Cleanliness score in `[0.0, 1.0]`, higher is better.
    pub cleanliness: f64,
    /// Confidence estimate derived from edge weights.
    pub confidence: f64,
}

/// An edge in a traced causal path.
#[derive(Debug, Clone)]
pub struct CausalEdge<V, T> {
    /// The edge's target value (as it appeared in the graph).
    pub value: V,
    /// The edge's start time.
    pub time: T,
    /// The weight this edge contributed (from the causal labels map).
    pub weight: f64,
}
