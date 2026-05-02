//! Narrative scoring and thread management for fabula.
//!
//! Provides the GM's evaluation toolkit -- scoring signals that tell the MCTS
//! evaluation function whether a candidate action improves the narrative.
//!
//! # Research foundation
//!
//! - Nelson & Mateas (2005) "Search-Based Drama Management" (AIIDE 2005)
//!   -- GM as optimizer with quality function over narrative states. This crate
//!   IS that quality function. The [`scorer`] module implements the composite
//!   evaluation from signals to score.
//! - Schulz et al. (2024) "Narrative Information Theory" (arXiv:2411.12907)
//!   -- Five measures (Complexity, Pivot, Predictability, Suspense, Plot Twist).
//!   [`pivot`] implements the Pivot measure via Jensen-Shannon Divergence.
//! - Booth (2009) "The AI Director: Left 4 Dead's Approach to Procedural
//!   Intensity" -- Pacing via trajectory sampling. [`tension`] classifies
//!   numeric trajectories (Rising/Falling/Plateau/Peak/Valley).
//! - Ely, Frankel, Kamenica (2015) "Suspense and Surprise" -- Trajectory
//!   matters more than absolute value; informs tension scoring.
//! - Kowal, Mary Robinette. MICE Quotient (Writing Excuses) -- Narrative
//!   threads open and close; well-formed stories close in reverse order
//!   (FILO). [`thread`] validates this nesting property.
//!
//! # Modules
//!
//! - [`thread`] -- Thread lifecycle management (MICE-style open/close tracking, FILO validation)
//! - [`tension`] -- Numeric trajectory sampling (is tension rising/falling/plateaued?)
//! - [`pivot`] -- Event distribution shift detection (Schulz 2024 JSD pivot measure)
//! - [`scorer`] -- Composite narrative quality score for MCTS evaluation
//!
//! # Design principles
//!
//! 1. **Scoring, not patterns.** The GM needs scoring signals, not more patterns.
//!    The engine finds matches; this crate evaluates narrative quality.
//! 2. **DataSource-agnostic where possible.** `ThreadTracker` and `PivotDetector`
//!    work on engine output (TickDelta, SiftEvent). `TensionTracker` accepts
//!    caller-provided samples, not DataSource queries.
//! 3. **Research-backed.** Each module cites its academic foundation.

pub mod distance;
pub mod pivot;
pub mod scorer;
pub mod tension;
pub mod thread;

pub use distance::{DistanceMetric, Hellinger, JensenShannon, KullbackLeibler};
