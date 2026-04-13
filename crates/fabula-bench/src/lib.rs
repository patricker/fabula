//! Benchmark workload generators for fabula.
//!
//! Provides three workload builders:
//! - [`build_isolated_workload`] — deterministic, parameterized workloads for divan benchmarks
//! - [`build_gm_workload`] — realistic GM-profile workload for profiling (samply, dhat)
//! - [`generate_trace`] — synthetic narrative trace for scoring pipeline benchmarks
//!
//! Engine workloads are generic over [`TestGraph`]. Narrative workloads are
//! self-contained (no `SiftEngine` or `DataSource` needed).

pub mod narrative_workload;
pub mod workload;

pub use narrative_workload::{
    generate_trace, NarrativeShape, NarrativeTick, NarrativeTrace, NarrativeTraceConfig,
};
pub use workload::{
    build_gm_workload, build_isolated_workload, GmWorkload, IsolatedWorkload, PendingEdge, Tick,
    WorkloadConfig,
};
