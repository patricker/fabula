//! Allen's interval algebra over generic time types.
//!
//! Implements Allen (1983) "Maintaining Knowledge about Temporal Intervals"
//! (CACM 26(11)) — the 13 mutually exclusive temporal relations between
//! intervals, plus metric gap computation from Dechter, Meiri, Pearl (1991)
//! "Temporal Constraint Networks" and Meiri (1996) for combining qualitative
//! Allen relations with quantitative STN-style metric bounds.
//!
//! Provides a generic `Interval` type with open-ended support.

use std::fmt;
use std::ops::Sub;

/// Conversion trait for time types to f64 (for metric gap comparison).
///
/// Built-in for `i64`, `i32`, `f64`, `f32`. For custom time newtypes,
/// implement this trait directly or use a feature-gated impl in the
/// type's owning crate.
pub trait NumericTime {
    fn as_f64(&self) -> f64;
}

impl NumericTime for i64 { fn as_f64(&self) -> f64 { *self as f64 } }
impl NumericTime for i32 { fn as_f64(&self) -> f64 { *self as f64 } }
impl NumericTime for f64 { fn as_f64(&self) -> f64 { *self } }
impl NumericTime for f32 { fn as_f64(&self) -> f64 { *self as f64 } }

/// A time interval `[start, end)`. If `end` is `None`, the interval is open-ended
/// (still active / ongoing).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Interval<T> {
    /// Inclusive start of the interval.
    pub start: T,
    /// Exclusive end of the interval, or `None` if open-ended.
    pub end: Option<T>,
}

impl<T: Ord + Clone> Interval<T> {
    /// Create a bounded interval `[start, end)`.
    pub fn new(start: T, end: T) -> Self {
        Self {
            start,
            end: Some(end),
        }
    }

    /// Create an open-ended interval `[start, ∞)`.
    pub fn open(start: T) -> Self {
        Self { start, end: None }
    }

    /// Does this interval cover time `t`?
    pub fn covers(&self, t: &T) -> bool {
        match &self.end {
            Some(end) => &self.start <= t && t < end,
            None => &self.start <= t,
        }
    }

    /// Is this interval bounded (has an end)?
    pub fn is_bounded(&self) -> bool {
        self.end.is_some()
    }

    /// Classify the temporal relation between `self` and `other` using Allen's
    /// 13-relation algebra. Returns `None` if either interval is open-ended
    /// and the relation cannot be determined.
    pub fn relation(&self, other: &Interval<T>) -> Option<AllenRelation> {
        let (a_start, a_end) = (&self.start, self.end.as_ref()?);
        let (b_start, b_end) = (&other.start, other.end.as_ref()?);

        if a_end < b_start {
            Some(AllenRelation::Before)
        } else if b_end < a_start {
            Some(AllenRelation::After)
        } else if a_end == b_start {
            Some(AllenRelation::Meets)
        } else if b_end == a_start {
            Some(AllenRelation::MetBy)
        } else if a_start == b_start && a_end == b_end {
            Some(AllenRelation::Equals)
        } else if a_start == b_start && a_end < b_end {
            Some(AllenRelation::Starts)
        } else if a_start == b_start && a_end > b_end {
            Some(AllenRelation::StartedBy)
        } else if a_end == b_end && a_start > b_start {
            Some(AllenRelation::Finishes)
        } else if a_end == b_end && a_start < b_start {
            Some(AllenRelation::FinishedBy)
        } else if a_start > b_start && a_end < b_end {
            Some(AllenRelation::During)
        } else if a_start < b_start && a_end > b_end {
            Some(AllenRelation::Contains)
        } else if a_start < b_start && a_end > b_start && a_end < b_end {
            Some(AllenRelation::Overlaps)
        } else if b_start < a_start && b_end > a_start && b_end < a_end {
            Some(AllenRelation::OverlappedBy)
        } else {
            None
        }
    }

    /// Does `self` start strictly before `other`?
    pub fn before(&self, other: &Interval<T>) -> bool {
        match &self.end {
            Some(end) => end <= &other.start,
            None => false,
        }
    }

    /// Does `self` end exactly where `other` starts?
    pub fn meets(&self, other: &Interval<T>) -> bool {
        match &self.end {
            Some(end) => end == &other.start,
            None => false,
        }
    }

    /// Do the two intervals share any time?
    pub fn intersects(&self, other: &Interval<T>) -> bool {
        let start_ok = match &other.end {
            Some(b_end) => &self.start < b_end,
            None => true,
        };
        let end_ok = match &self.end {
            Some(a_end) => &other.start < a_end,
            None => true,
        };
        start_ok && end_ok
    }
}

impl<T: Ord + Clone + Sub<Output = T> + NumericTime> Interval<T> {
    /// Compute the gap distance for a given Allen relation.
    ///
    /// Each Allen relation has a natural "gap" derived from endpoint decomposition
    /// (Meiri 1996). Returns `None` if the computation requires an endpoint that
    /// is missing (open-ended interval).
    pub fn gap_for_relation(&self, other: &Interval<T>, relation: AllenRelation) -> Option<f64> {
        use AllenRelation::*;
        match relation {
            Before => {
                let a_end = self.end.as_ref()?;
                Some((other.start.clone() - a_end.clone()).as_f64())
            }
            After => {
                let b_end = other.end.as_ref()?;
                Some((self.start.clone() - b_end.clone()).as_f64())
            }
            Meets | MetBy | Equals => Some(0.0),
            Overlaps => {
                let a_end = self.end.as_ref()?;
                Some((a_end.clone() - other.start.clone()).as_f64())
            }
            OverlappedBy => {
                let b_end = other.end.as_ref()?;
                Some((b_end.clone() - self.start.clone()).as_f64())
            }
            During => {
                Some((self.start.clone() - other.start.clone()).as_f64())
            }
            Contains => {
                Some((other.start.clone() - self.start.clone()).as_f64())
            }
            Starts => {
                let a_end = self.end.as_ref()?;
                let b_end = other.end.as_ref()?;
                Some((b_end.clone() - a_end.clone()).as_f64())
            }
            StartedBy => {
                let a_end = self.end.as_ref()?;
                let b_end = other.end.as_ref()?;
                Some((a_end.clone() - b_end.clone()).as_f64())
            }
            Finishes => {
                Some((self.start.clone() - other.start.clone()).as_f64())
            }
            FinishedBy => {
                Some((other.start.clone() - self.start.clone()).as_f64())
            }
        }
    }
}

impl<T: fmt::Display> fmt::Display for Interval<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.end {
            Some(end) => write!(f, "[{}, {})", self.start, end),
            None => write!(f, "[{}, ∞)", self.start),
        }
    }
}

/// Allen's 13 mutually exclusive temporal relations between two bounded intervals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AllenRelation {
    /// A ends before B starts.
    Before,
    /// A starts after B ends.
    After,
    /// A ends exactly where B starts.
    Meets,
    /// A starts exactly where B ends.
    MetBy,
    /// A starts before B, A ends during B.
    Overlaps,
    /// B starts before A, B ends during A.
    OverlappedBy,
    /// A and B start together, A ends before B.
    Starts,
    /// A and B start together, A ends after B.
    StartedBy,
    /// A is entirely within B.
    During,
    /// B is entirely within A.
    Contains,
    /// A and B end together, A starts after B.
    Finishes,
    /// A and B end together, A starts before B.
    FinishedBy,
    /// A and B have identical start and end.
    Equals,
}

impl AllenRelation {
    /// The inverse relation (swap A and B).
    pub fn inverse(self) -> Self {
        match self {
            Self::Before => Self::After,
            Self::After => Self::Before,
            Self::Meets => Self::MetBy,
            Self::MetBy => Self::Meets,
            Self::Overlaps => Self::OverlappedBy,
            Self::OverlappedBy => Self::Overlaps,
            Self::Starts => Self::StartedBy,
            Self::StartedBy => Self::Starts,
            Self::During => Self::Contains,
            Self::Contains => Self::During,
            Self::Finishes => Self::FinishedBy,
            Self::FinishedBy => Self::Finishes,
            Self::Equals => Self::Equals,
        }
    }

    /// Does this relation imply A ends before or when B starts?
    pub fn is_before_or_meets(self) -> bool {
        matches!(self, Self::Before | Self::Meets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_before() {
        let a = Interval::new(1, 3);
        let b = Interval::new(5, 7);
        assert_eq!(a.relation(&b), Some(AllenRelation::Before));
        assert_eq!(b.relation(&a), Some(AllenRelation::After));
        assert!(a.before(&b));
    }

    #[test]
    fn test_meets() {
        let a = Interval::new(1, 3);
        let b = Interval::new(3, 5);
        assert_eq!(a.relation(&b), Some(AllenRelation::Meets));
        assert!(a.meets(&b));
    }

    #[test]
    fn test_overlaps() {
        let a = Interval::new(1, 4);
        let b = Interval::new(3, 6);
        assert_eq!(a.relation(&b), Some(AllenRelation::Overlaps));
    }

    #[test]
    fn test_during() {
        let a = Interval::new(3, 5);
        let b = Interval::new(1, 7);
        assert_eq!(a.relation(&b), Some(AllenRelation::During));
    }

    #[test]
    fn test_contains() {
        let a = Interval::new(1, 7);
        let b = Interval::new(3, 5);
        assert_eq!(a.relation(&b), Some(AllenRelation::Contains));
    }

    #[test]
    fn test_equals() {
        let a = Interval::new(1, 5);
        let b = Interval::new(1, 5);
        assert_eq!(a.relation(&b), Some(AllenRelation::Equals));
    }

    #[test]
    fn test_starts() {
        let a = Interval::new(1, 3);
        let b = Interval::new(1, 5);
        assert_eq!(a.relation(&b), Some(AllenRelation::Starts));
    }

    #[test]
    fn test_finishes() {
        let a = Interval::new(3, 5);
        let b = Interval::new(1, 5);
        assert_eq!(a.relation(&b), Some(AllenRelation::Finishes));
    }

    #[test]
    fn test_open_ended_covers() {
        let a = Interval::open(5);
        assert!(a.covers(&5));
        assert!(a.covers(&100));
        assert!(!a.covers(&4));
    }

    #[test]
    fn test_open_ended_relation_returns_none() {
        let a = Interval::open(1);
        let b = Interval::new(3, 5);
        assert_eq!(a.relation(&b), None);
    }

    #[test]
    fn test_intersects() {
        let a = Interval::new(1, 5);
        let b = Interval::new(3, 7);
        assert!(a.intersects(&b));

        let c = Interval::new(5, 7);
        assert!(!a.intersects(&c)); // [1,5) and [5,7) don't overlap

        let d = Interval::open(3);
        assert!(a.intersects(&d));
    }

    #[test]
    fn test_inverse() {
        assert_eq!(AllenRelation::Before.inverse(), AllenRelation::After);
        assert_eq!(AllenRelation::Meets.inverse(), AllenRelation::MetBy);
        assert_eq!(AllenRelation::Equals.inverse(), AllenRelation::Equals);
    }
}
