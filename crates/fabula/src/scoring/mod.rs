//! Statistical surprise scoring for pattern matches.
//!
//! Ranks matches by how unexpected they are relative to a baseline frequency.
//! Operates as a post-processing step — the engine finds matches, the scorer
//! ranks them. No engine modification required.
//!
//! ## Research foundation
//!
//! - **Pattern-level**: Shannon surprise (`-log₂(observed / baseline)`) with
//!   Laplace smoothing. Standard information-theoretic self-information applied
//!   to pattern match frequencies.
//! - **Property-level**: Kreminski, Dickinson, Wardrip-Fruin, Mateas (2022)
//!   "Select the Unexpected: A Statistical Heuristic for Story Sifting"
//!   (ICIDS 2022). Scores individual matches by the mean empirical frequency
//!   of their *properties* — two matches of the same pattern score differently
//!   if one involves rarer attributes.

mod surprise;
mod stu;

pub use surprise::{ScoredMatch, SurpriseScorer};
pub use stu::{StuScoredMatch, StuScorer};
