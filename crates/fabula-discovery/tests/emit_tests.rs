use fabula::builder::PatternBuilder;
use fabula::datasource::ValueConstraint;
use fabula::interval::AllenRelation;
use fabula::pattern::{Clause, MetricGap, Negation, Pattern, Stage, Target, Var};
use fabula_discovery::pattern_to_dsl;
use std::collections::HashMap;

#[test]
fn emit_simple_two_stage() {
    let pattern = PatternBuilder::<String, String>::new("hospitality")
        .stage("e1", |s| s.edge_bind("e1", "arrives".to_string(), "guest"))
        .stage("e2", |s| s.edge_bind("e2", "greets".to_string(), "guest"))
        .temporal("e1", AllenRelation::Before, "e2")
        .build();

    let dsl = pattern_to_dsl(&pattern);

    // Should parse back without error
    let parsed = fabula_dsl::parse_document(&dsl);
    assert!(
        parsed.is_ok(),
        "Round-trip failed: {}\nDSL:\n{}",
        parsed.unwrap_err(),
        dsl
    );
}

#[test]
fn emit_with_negation() {
    let pattern = PatternBuilder::<String, String>::new("trust_unbroken")
        .stage("e1", |s| s.edge_bind("e1", "trusts".to_string(), "target"))
        .stage("e2", |s| s.edge_bind("e2", "helps".to_string(), "target"))
        .temporal("e1", AllenRelation::Before, "e2")
        .unless_between("e1", "e2", |n| {
            n.edge_bind("e1", "betrays".to_string(), "target")
        })
        .build();

    let dsl = pattern_to_dsl(&pattern);
    let parsed = fabula_dsl::parse_document(&dsl);
    assert!(
        parsed.is_ok(),
        "Round-trip failed: {}\nDSL:\n{}",
        parsed.unwrap_err(),
        dsl
    );
}

#[test]
fn emit_with_literal_value() {
    let pattern = PatternBuilder::<String, String>::new("specific_value")
        .stage("e1", |s| {
            s.edge("e1", "status".to_string(), "active".to_string())
        })
        .build();

    let dsl = pattern_to_dsl(&pattern);
    assert!(
        dsl.contains("\"active\""),
        "DSL should contain quoted literal: {}",
        dsl
    );
    let parsed = fabula_dsl::parse_document(&dsl);
    assert!(
        parsed.is_ok(),
        "Round-trip failed: {}\nDSL:\n{}",
        parsed.unwrap_err(),
        dsl
    );
}

#[test]
fn emit_private_pattern() {
    let pattern = PatternBuilder::<String, String>::new("hidden")
        .stage("e1", |s| s.edge_bind("e1", "event".to_string(), "target"))
        .private()
        .build();

    let dsl = pattern_to_dsl(&pattern);
    assert!(
        dsl.contains("private"),
        "DSL should contain 'private': {}",
        dsl
    );
    let parsed = fabula_dsl::parse_document(&dsl);
    assert!(
        parsed.is_ok(),
        "Round-trip failed: {}\nDSL:\n{}",
        parsed.unwrap_err(),
        dsl
    );
}

#[test]
fn emit_with_deadline() {
    let pattern = PatternBuilder::<String, String>::new("time_limited")
        .stage("e1", |s| s.edge_bind("e1", "offer".to_string(), "who"))
        .stage("e2", |s| s.edge_bind("e2", "accept".to_string(), "who"))
        .deadline(30)
        .build();

    let dsl = pattern_to_dsl(&pattern);
    assert!(
        dsl.contains("deadline 30"),
        "DSL should contain deadline: {}",
        dsl
    );
    let parsed = fabula_dsl::parse_document(&dsl);
    assert!(
        parsed.is_ok(),
        "Round-trip failed: {}\nDSL:\n{}",
        parsed.unwrap_err(),
        dsl
    );
}

#[test]
fn emit_with_metadata() {
    let pattern = PatternBuilder::<String, String>::new("annotated")
        .stage("e1", |s| {
            s.edge("e1", "type".to_string(), "event".to_string())
        })
        .metadata("severity", "high")
        .metadata("category", "conflict")
        .build();

    let dsl = pattern_to_dsl(&pattern);
    assert!(
        dsl.contains("meta(\"category\", \"conflict\")"),
        "DSL should contain meta: {}",
        dsl
    );
    assert!(
        dsl.contains("meta(\"severity\", \"high\")"),
        "DSL should contain meta: {}",
        dsl
    );
    let parsed = fabula_dsl::parse_document(&dsl);
    assert!(
        parsed.is_ok(),
        "Round-trip failed: {}\nDSL:\n{}",
        parsed.unwrap_err(),
        dsl
    );
}

#[test]
fn emit_with_temporal_gap() {
    let pattern = PatternBuilder::<String, String>::new("quick_response")
        .stage("e1", |s| {
            s.edge("e1", "type".to_string(), "request".to_string())
        })
        .stage("e2", |s| {
            s.edge("e2", "type".to_string(), "response".to_string())
        })
        .temporal_with_gap(
            "e1",
            AllenRelation::Before,
            "e2",
            MetricGap {
                min: Some(0.0),
                max: Some(100.0),
            },
        )
        .build();

    let dsl = pattern_to_dsl(&pattern);
    assert!(
        dsl.contains("gap 0..100"),
        "DSL should contain gap: {}",
        dsl
    );
    let parsed = fabula_dsl::parse_document(&dsl);
    assert!(
        parsed.is_ok(),
        "Round-trip failed: {}\nDSL:\n{}",
        parsed.unwrap_err(),
        dsl
    );
}

#[test]
fn emit_unless_after() {
    let pattern = PatternBuilder::<String, String>::new("unresolved")
        .stage("e1", |s| {
            s.edge("e1", "type".to_string(), "conflict".to_string())
        })
        .stage("e2", |s| {
            s.edge("e2", "type".to_string(), "escalate".to_string())
        })
        .unless_after("e2", |n| {
            n.edge("end", "type".to_string(), "resolve".to_string())
        })
        .build();

    let dsl = pattern_to_dsl(&pattern);
    assert!(
        dsl.contains("unless after e2"),
        "DSL should contain 'unless after': {}",
        dsl
    );
    let parsed = fabula_dsl::parse_document(&dsl);
    assert!(
        parsed.is_ok(),
        "Round-trip failed: {}\nDSL:\n{}",
        parsed.unwrap_err(),
        dsl
    );
}

#[test]
fn emit_negated_clause() {
    let pattern = PatternBuilder::<String, String>::new("unwelcome")
        .stage("e1", |s| {
            s.edge("e1", "type".to_string(), "enter".to_string())
                .not_edge("e1", "status".to_string(), "invited".to_string())
        })
        .build();

    let dsl = pattern_to_dsl(&pattern);
    assert!(
        dsl.contains("! "),
        "DSL should contain negated clause marker: {}",
        dsl
    );
    let parsed = fabula_dsl::parse_document(&dsl);
    assert!(
        parsed.is_ok(),
        "Round-trip failed: {}\nDSL:\n{}",
        parsed.unwrap_err(),
        dsl
    );
}

#[test]
fn emit_constraint_var() {
    let pattern = PatternBuilder::<String, String>::new("escalating")
        .stage("e1", |s| s.edge_bind("e1", "severity".to_string(), "level"))
        .stage("e2", |s| {
            s.edge_gt_var("e2", "severity".to_string(), "level")
        })
        .build();

    let dsl = pattern_to_dsl(&pattern);
    assert!(
        dsl.contains("> ?level"),
        "DSL should contain '> ?level': {}",
        dsl
    );
    let parsed = fabula_dsl::parse_document(&dsl);
    assert!(
        parsed.is_ok(),
        "Round-trip failed: {}\nDSL:\n{}",
        parsed.unwrap_err(),
        dsl
    );
}

#[test]
fn emit_constraint_between() {
    let pattern = PatternBuilder::<String, String>::new("ranged")
        .stage("e1", |s| {
            s.edge_constrained(
                "e1",
                "score".to_string(),
                ValueConstraint::Between("10".to_string(), "20".to_string()),
            )
        })
        .build();

    let dsl = pattern_to_dsl(&pattern);
    // Between is lossy — emitted as >= lo
    assert!(
        dsl.contains(">= \"10\""),
        "DSL should contain '>= \"10\"' for Between: {}",
        dsl
    );
    let parsed = fabula_dsl::parse_document(&dsl);
    assert!(
        parsed.is_ok(),
        "Round-trip failed: {}\nDSL:\n{}",
        parsed.unwrap_err(),
        dsl
    );
}

#[test]
fn emit_unless_global() {
    // Build a pattern with is_global = true directly on the Negation struct,
    // since PatternBuilder::build() resolves global negations to anchors.
    let pattern = Pattern {
        name: "global_neg".to_string(),
        stages: vec![
            Stage {
                anchor: Var::new("e1"),
                clauses: vec![Clause {
                    source: Var::new("e1"),
                    label: "trusts".to_string(),
                    target: Target::Bind(Var::new("target")),
                    negated: false,
                }],
            },
            Stage {
                anchor: Var::new("e2"),
                clauses: vec![Clause {
                    source: Var::new("e2"),
                    label: "helps".to_string(),
                    target: Target::Bind(Var::new("target")),
                    negated: false,
                }],
            },
        ],
        temporal: vec![],
        negations: vec![Negation {
            between_start: Var::new("e1"),
            between_end: Some(Var::new("e2")),
            clauses: vec![Clause {
                source: Var::new("e1"),
                label: "betrays".to_string(),
                target: Target::Bind(Var::new("target")),
                negated: false,
            }],
            is_global: true,
        }],
        group: None,
        metadata: HashMap::new(),
        deadline_ticks: None,
        repeat_range: None,
        unordered_groups: Vec::new(),
        private: false,
        importance: 1.0,
    };

    let dsl = pattern_to_dsl(&pattern);
    assert!(
        dsl.contains("unless {"),
        "DSL should contain 'unless {{' (no anchor names) for global negation: {}",
        dsl
    );
    // Should NOT contain "unless between" or "unless after"
    assert!(
        !dsl.contains("unless between"),
        "DSL should not contain 'unless between' for global negation: {}",
        dsl
    );
    assert!(
        !dsl.contains("unless after"),
        "DSL should not contain 'unless after' for global negation: {}",
        dsl
    );
    let parsed = fabula_dsl::parse_document(&dsl);
    assert!(
        parsed.is_ok(),
        "Round-trip failed: {}\nDSL:\n{}",
        parsed.unwrap_err(),
        dsl
    );
}

#[test]
fn emit_constraint_any() {
    let pattern = PatternBuilder::<String, String>::new("wildcard")
        .stage("e1", |s| {
            s.edge_constrained("e1", "status".to_string(), ValueConstraint::Any)
        })
        .build();

    let dsl = pattern_to_dsl(&pattern);
    assert!(
        dsl.contains("-> ?_any"),
        "DSL should contain '-> ?_any' for ValueConstraint::Any: {}",
        dsl
    );
    let parsed = fabula_dsl::parse_document(&dsl);
    assert!(
        parsed.is_ok(),
        "Round-trip failed: {}\nDSL:\n{}",
        parsed.unwrap_err(),
        dsl
    );
}

#[test]
fn emit_constraint_lt_lte_gte() {
    // Lt
    let pattern_lt = PatternBuilder::<String, String>::new("lt_test")
        .stage("e1", |s| {
            s.edge_constrained(
                "e1",
                "score".to_string(),
                ValueConstraint::Lt("50".to_string()),
            )
        })
        .build();
    let dsl_lt = pattern_to_dsl(&pattern_lt);
    assert!(
        dsl_lt.contains("< \"50\""),
        "DSL should contain '< \"50\"' for Lt: {}",
        dsl_lt
    );
    let parsed_lt = fabula_dsl::parse_document(&dsl_lt);
    assert!(
        parsed_lt.is_ok(),
        "Round-trip Lt failed: {}\nDSL:\n{}",
        parsed_lt.unwrap_err(),
        dsl_lt
    );

    // Lte
    let pattern_lte = PatternBuilder::<String, String>::new("lte_test")
        .stage("e1", |s| {
            s.edge_constrained(
                "e1",
                "score".to_string(),
                ValueConstraint::Lte("100".to_string()),
            )
        })
        .build();
    let dsl_lte = pattern_to_dsl(&pattern_lte);
    assert!(
        dsl_lte.contains("<= \"100\""),
        "DSL should contain '<= \"100\"' for Lte: {}",
        dsl_lte
    );
    let parsed_lte = fabula_dsl::parse_document(&dsl_lte);
    assert!(
        parsed_lte.is_ok(),
        "Round-trip Lte failed: {}\nDSL:\n{}",
        parsed_lte.unwrap_err(),
        dsl_lte
    );

    // Gte
    let pattern_gte = PatternBuilder::<String, String>::new("gte_test")
        .stage("e1", |s| {
            s.edge_constrained(
                "e1",
                "score".to_string(),
                ValueConstraint::Gte("0".to_string()),
            )
        })
        .build();
    let dsl_gte = pattern_to_dsl(&pattern_gte);
    assert!(
        dsl_gte.contains(">= \"0\""),
        "DSL should contain '>= \"0\"' for Gte: {}",
        dsl_gte
    );
    let parsed_gte = fabula_dsl::parse_document(&dsl_gte);
    assert!(
        parsed_gte.is_ok(),
        "Round-trip Gte failed: {}\nDSL:\n{}",
        parsed_gte.unwrap_err(),
        dsl_gte
    );
}

#[test]
fn emit_concurrent_block() {
    let pattern = PatternBuilder::<String, String>::new("concurrent_test")
        .unordered_group(|g| {
            g.stage("a", |s| {
                s.edge("a", "type".to_string(), "alpha".to_string())
            })
            .stage("b", |s| s.edge("b", "type".to_string(), "beta".to_string()))
        })
        .build();

    let dsl = pattern_to_dsl(&pattern);
    assert!(
        dsl.contains("concurrent {"),
        "DSL should contain 'concurrent {{': {}",
        dsl
    );
    let parsed = fabula_dsl::parse_document(&dsl);
    assert!(
        parsed.is_ok(),
        "Round-trip failed: {}\nDSL:\n{}",
        parsed.unwrap_err(),
        dsl
    );
}

#[test]
fn emit_quoted_value_with_embedded_double_quote() {
    let pattern = PatternBuilder::<String, String>::new("quoted_test")
        .stage("e1", |s| {
            s.edge(
                "e1",
                "dialogue".to_string(),
                "He said \"hello\" loudly".to_string(),
            )
        })
        .build();

    let dsl = pattern_to_dsl(&pattern);
    // The value contains a double-quote, so triple-quote syntax must be used
    assert!(
        dsl.contains("\"\"\""),
        "DSL should use triple-quote syntax for value containing double-quote: {}",
        dsl
    );
    let parsed = fabula_dsl::parse_document(&dsl);
    assert!(
        parsed.is_ok(),
        "Round-trip failed: {}\nDSL:\n{}",
        parsed.unwrap_err(),
        dsl
    );
}

#[test]
fn emit_multi_clause_stage() {
    let pattern = PatternBuilder::<String, String>::new("multi_clause")
        .stage("e1", |s| {
            s.edge("e1", "type".to_string(), "attack".to_string())
                .edge_bind("e1", "actor".to_string(), "attacker")
                .edge_bind("e1", "target".to_string(), "victim")
        })
        .build();

    let dsl = pattern_to_dsl(&pattern);
    // Verify the DSL round-trips correctly with multiple clauses
    let parsed = fabula_dsl::parse_document(&dsl);
    assert!(
        parsed.is_ok(),
        "Round-trip failed for multi-clause stage: {}\nDSL:\n{}",
        parsed.unwrap_err(),
        dsl
    );
    // The parsed pattern should have the same number of clauses
    let doc = parsed.unwrap();
    assert_eq!(doc.patterns.len(), 1, "Should parse exactly one pattern");
    assert_eq!(doc.patterns[0].stages.len(), 1, "Should have one stage");
    assert_eq!(
        doc.patterns[0].stages[0].clauses.len(),
        3,
        "Should have 3 clauses in the stage"
    );
}
