//! Golden test scenarios — each function is generic over `TestGraph` and runs
//! against every adapter.
//!
//! # Naming convention
//!
//! `batch_*`       — tests using `engine.evaluate()` (full snapshot scan)
//! `incremental_*` — tests using `engine.on_edge_added()` (streaming)
//! `gap_*`         — tests using `engine.why_not()` (gap analysis)

mod hospitality;
mod romantic_arc;
mod value_constraints;
mod temporal;
mod negation;
mod incremental;
mod gap_analysis;
mod multi_pattern;
mod two_betrayals;
mod winnow_replay;
mod allen_temporal;
mod consistency;
mod composition;

pub use hospitality::*;
pub use romantic_arc::*;
pub use value_constraints::*;
pub use temporal::*;
pub use negation::*;
pub use incremental::*;
pub use gap_analysis::*;
pub use multi_pattern::*;
pub use two_betrayals::*;
pub use winnow_replay::*;
pub use allen_temporal::*;
pub use consistency::*;
pub use composition::*;
