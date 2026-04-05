use fabula::interval::AllenRelation;
use fabula::prelude::*;
use fabula_memory::MemValue;

#[test]
fn during_pattern() {
    // #region during_pattern
    let pattern = PatternBuilder::<String, MemValue>::new("during_pattern")
        .stage("outer", |s| {
            s.edge(
                "outer",
                "eventType".into(),
                MemValue::Str("siege".into()),
            )
        })
        .stage("inner", |s| {
            s.edge(
                "inner",
                "eventType".into(),
                MemValue::Str("sortie".into()),
            )
        })
        .temporal("inner", AllenRelation::During, "outer")
        .build();
    // #endregion

    assert_eq!(pattern.name, "during_pattern");
    assert_eq!(pattern.stages.len(), 2);
    assert_eq!(pattern.temporal.len(), 1);
}
