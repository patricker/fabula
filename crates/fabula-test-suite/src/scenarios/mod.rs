//! Golden test scenarios — each function is generic over `TestGraph` and runs
//! against every adapter.
//!
//! # Naming convention
//!
//! `batch_*`       — tests using `engine.evaluate()` (full snapshot scan)
//! `incremental_*` — tests using `engine.on_edge_added()` (streaming)
//! `gap_*`         — tests using `engine.why_not()` (gap analysis)

mod allen_temporal;
mod composition;
mod consistency;
mod cross_stage_constraints;
mod gap_analysis;
mod hospitality;
mod incremental;
mod multi_pattern;
mod negation;
mod romantic_arc;
mod temporal;
mod two_betrayals;
mod unordered_groups;
mod value_constraints;
mod value_disjunction;
mod winnow_replay;

pub use allen_temporal::*;
pub use composition::*;
pub use consistency::*;
pub use cross_stage_constraints::*;
pub use gap_analysis::*;
pub use hospitality::*;
pub use incremental::*;
pub use multi_pattern::*;
pub use negation::*;
pub use romantic_arc::*;
pub use temporal::*;
pub use two_betrayals::*;
pub use unordered_groups::*;
pub use value_constraints::*;
pub use value_disjunction::*;
pub use winnow_replay::*;
