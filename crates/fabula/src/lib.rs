//! # Fabula
//!
//! Incremental pattern matching over temporal graphs.
//!
//! Fabula finds patterns in graphs where edges have validity intervals. You define
//! patterns ("find a character whose loyalty dropped after an institutional failure,
//! with no trust recovery in between"), register them with the engine, and it tracks
//! partial matches incrementally as new edges arrive.
//!
//! ## Crate Structure
//!
//! - **`fabula`** (this crate) -- core library with zero dependencies. Pattern types,
//!   the `DataSource` trait, Allen interval algebra, the `SiftEngine`.
//! - **`fabula-memory`** -- `MemGraph`, a simple in-memory `DataSource` for testing.
//! - **`fabula-petgraph`** -- `DataSource` adapter wrapping `petgraph::StableGraph`.
//! - **`fabula-grafeo`** -- `DataSource` adapter for the Grafeo graph database.
//!
//! ## Quick Start
//!
//! ```rust
//! use fabula::prelude::*;
//!
//! // Define a pattern: two betrayals by the same character
//! let pattern = PatternBuilder::<String, String>::new("double_betrayal")
//!     .stage("e1", |s| s
//!         .edge("e1", "eventType".to_string(), "betray".to_string())
//!         .edge_bind("e1", "actor".to_string(), "char"))
//!     .stage("e2", |s| s
//!         .edge("e2", "eventType".to_string(), "betray".to_string())
//!         .edge_bind("e2", "actor".to_string(), "char"))
//!     .build();
//!
//! assert_eq!(pattern.stages.len(), 2);
//! assert_eq!(pattern.name, "double_betrayal");
//! ```
//!
//! For full evaluation examples, see `fabula-memory` which provides `MemGraph`.

pub mod builder;
pub mod causality;
pub mod compose;
pub mod datasource;
pub mod engine;
pub mod interval;
pub mod pattern;
pub mod scoring;

/// Convenience re-exports for common usage.
pub mod prelude {
    pub use crate::builder::PatternBuilder;
    pub use crate::causality::{
        causal_paths, cleanliness_score, event_causal_surprise,
        event_causal_surprise_batch, CausalEdge, CausalPath,
    };
    pub use crate::datasource::{DataSource, Label, NodeId, Val, ValueConstraint};
    pub use crate::engine::{
        evaluate_pattern, evaluate_pattern_at, evaluate_pattern_first, evaluate_pattern_limit,
        gap_analysis, gap_analysis_at, BoundValue, ClauseAnalysis, EngineStats, GapAnalysis, Match,
        MatchState, PartialMatch, PatternMetrics, PlantPayoffPair, PlantStatus, SiftEngine,
        SiftEngineFor, SiftEvent, StageAnalysis, StageStatus, TickDelta,
    };
    pub use crate::interval::{AllenRelation, Interval, NumericTime};
    pub use crate::pattern::{
        Clause, MetricGap, Negation, Pattern, Stage, Target, TemporalConstraint, Var,
    };
    pub use crate::scoring::{
        ScoredMatch, SequentialScorer, StuAggregation, StuScoredMatch, StuScorer, SurpriseScorer,
    };
}
