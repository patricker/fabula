//! Automated sifting pattern discovery.
//!
//! Provides a pluggable generate-evaluate framework for discovering
//! narratively interesting patterns from simulation trace data.
//!
//! # Architecture
//!
//! The system works as an iterative loop:
//! 1. A [`CandidateGenerator`] proposes candidate patterns from a [`TraceCorpus`]
//! 2. [`PatternEvaluator`]s score each candidate
//! 3. Scored results feed back to the generator for the next round
//! 4. A [`PatternFilter`] decides which patterns to keep
//!
//! The [`DiscoverySession`] orchestrates this loop with configurable budgets.

mod corpus;
mod emit;
mod score;
mod session;
mod traits;

pub mod evaluators;
pub mod generators;

// Re-exports uncommented as modules are implemented:
pub use corpus::{PairwiseHit, SharedNode, TraceCorpus, TraceEdge};
pub use emit::pattern_to_dsl;
pub use score::{PatternScore, ScoredPattern};
pub use session::{DiscoverySession, SessionConfig, SessionHistory};
pub use traits::{CandidateGenerator, PatternEvaluator, PatternFilter, ThresholdFilter};
