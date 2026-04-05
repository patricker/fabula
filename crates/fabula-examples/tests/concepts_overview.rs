use fabula::prelude::*;
use fabula_memory::MemValue;

#[test]
fn broken_promise_pattern() {
    // #region broken_promise
    PatternBuilder::<String, MemValue>::new("broken_promise")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("promise".into()))
                .edge_bind("e1", "actor".into(), "person")
        })
        .stage("e2", |s| {
            s.edge(
                "e2",
                "eventType".into(),
                MemValue::Str("break_promise".into()),
            )
            .edge_bind("e2", "actor".into(), "person")
        })
        .build();
    // #endregion
}

#[test]
fn negation_window() {
    // #region negation_window
    PatternBuilder::<String, MemValue>::new("hospitality_violation")
        .stage("e1", |s| {
            s.edge("e1", "eventType".into(), MemValue::Str("enter".into()))
                .edge_bind("e1", "actor".into(), "guest")
        })
        .stage("e3", |s| {
            s.edge("e3", "eventType".into(), MemValue::Str("harm".into()))
                .edge_bind("e3", "target".into(), "guest")
        })
        .unless_between("e1", "e3", |neg| {
            neg.edge(
                "eMid",
                "eventType".into(),
                MemValue::Str("leaveTown".into()),
            )
            .edge_bind("eMid", "actor".into(), "guest")
        })
        .build();
    // #endregion
}
