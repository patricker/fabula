//! Benchmark workload generators for fabula.
//!
//! Provides two workload builders:
//! - [`build_isolated_workload`] — deterministic, parameterized workloads for divan benchmarks
//! - [`build_gm_workload`] — realistic GM-profile workload for profiling (samply, dhat)
//!
//! Both are generic over [`TestGraph`] so they run on any fabula adapter.

pub mod workload;

pub use workload::{
    build_gm_workload, build_isolated_workload, GmWorkload, IsolatedWorkload, PendingEdge, Tick,
    WorkloadConfig,
};
