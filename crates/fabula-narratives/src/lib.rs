//! Narrative scoring and thread management for fabula.
//!
//! Provides the GM's evaluation toolkit — scoring signals that tell the MCTS
//! evaluation function whether a candidate action improves the narrative.
//!
//! # Modules
//!
//! - [`thread`] — Thread lifecycle management (MICE-style open/close tracking, FILO validation)
//! - [`tension`] — Numeric trajectory sampling (is tension rising/falling/plateaued?)
//! - [`pivot`] — Event distribution shift detection (Schulz 2024 JSD pivot measure)
//! - [`scorer`] — Composite narrative quality score for MCTS evaluation
//!
//! # Design principles
//!
//! 1. **Scoring, not patterns.** The GM needs scoring signals, not more patterns.
//!    The engine finds matches; this crate evaluates narrative quality.
//! 2. **DataSource-agnostic where possible.** `ThreadTracker` and `PivotDetector`
//!    work on engine output (TickDelta, SiftEvent). `TensionTracker` queries
//!    the DataSource for numeric values.
//! 3. **Research-backed.** Each module cites its academic foundation.

pub mod thread;
pub mod tension;
pub mod pivot;
pub mod scorer;
