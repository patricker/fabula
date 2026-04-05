use fabula::prelude::*;
use fabula_memory::MemValue;

#[test]
fn pattern_builder_usage() {
    // #region pattern_builder_usage
    let pattern = PatternBuilder::<String, MemValue>::new("my_pattern")
        .stage("event1", |s| {
            s.edge(
                "event1",
                "type".into(),
                MemValue::Str("failure".into()),
            )
            .edge_bind("event1", "actor".into(), "character")
        })
        .stage("event2", |s| {
            s.edge(
                "event2",
                "type".into(),
                MemValue::Str("betrayal".into()),
            )
            .edge_bind("event2", "target".into(), "character")
        })
        .unless_between("event1", "event2", |neg| {
            neg.edge(
                "recovery",
                "type".into(),
                MemValue::Str("trust_restored".into()),
            )
        })
        .build();
    // #endregion

    assert_eq!(pattern.name, "my_pattern");
    assert_eq!(pattern.stages.len(), 2);
    assert_eq!(pattern.negations.len(), 1);
}

#[test]
fn unordered_group_usage() {
    // #region unordered_group
    let pattern = PatternBuilder::<String, MemValue>::new("multi_signal")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), MemValue::Str("trigger".into()))
        })
        .unordered_group(|g| {
            g.stage("e2", |s| {
                s.edge("e2", "type".into(), MemValue::Str("signal_a".into()))
            })
            .stage("e3", |s| {
                s.edge("e3", "type".into(), MemValue::Str("signal_b".into()))
            })
        })
        .stage("e4", |s| {
            s.edge("e4", "type".into(), MemValue::Str("confirm".into()))
        })
        .build();
    // #endregion

    assert_eq!(pattern.stages.len(), 4);
    assert_eq!(pattern.unordered_groups.len(), 1);
}
