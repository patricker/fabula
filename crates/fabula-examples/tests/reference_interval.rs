use fabula::interval::{AllenRelation, Interval};

#[test]
fn interval_creation() {
    // #region interval_creation
    let bounded = Interval::new(1, 5); // [1, 5)
    let open = Interval::open(3); // [3, inf)
                                  // #endregion

    assert!(bounded.is_bounded());
    assert!(!open.is_bounded());
}

#[test]
fn allen_relation_usage() {
    // #region allen_relation
    let a = Interval::new(1, 3);
    let b = Interval::new(5, 7);
    assert_eq!(a.relation(&b), Some(AllenRelation::Before));
    // #endregion
}
