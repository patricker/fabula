use fabula::engine::SiftEngine;
use fabula::prelude::*;
use fabula_dsl::*;

#[test]
fn parse_simple_pattern() {
    let dsl = r#"
        pattern test {
            stage e1 {
                e1.eventType = "enter"
                e1.actor -> ?guest
            }
        }
    "#;
    let pattern = parse_pattern(dsl).unwrap();
    assert_eq!(pattern.name, "test");
    assert_eq!(pattern.stages.len(), 1);
    assert_eq!(pattern.stages[0].clauses.len(), 2);
}

#[test]
fn parse_simple_graph() {
    let dsl = r#"
        graph {
            @1 ev1.eventType = "enter"
            @1 ev1.actor -> alice
            now = 10
        }
    "#;
    let graph = parse_graph(dsl).unwrap();
    assert_eq!(graph.edge_count(), 2);
}

#[test]
fn roundtrip_hospitality() {
    let pattern_dsl = r#"
        pattern violation_of_hospitality {
            stage e1 {
                e1.eventType = "enterTown"
                e1.actor -> ?guest
            }
            stage e2 {
                e2.eventType = "showHospitality"
                e2.actor -> ?host
                e2.target -> ?guest
            }
            stage e3 {
                e3.eventType = "harm"
                e3.actor -> ?host
                e3.target -> ?guest
            }
            unless between e1 e3 {
                eMid.eventType = "leaveTown"
                eMid.actor -> ?guest
            }
        }
    "#;

    let graph_dsl = r#"
        graph {
            @1 e1.eventType = "enterTown"
            @1 e1.actor -> alice
            @2 e2.eventType = "showHospitality"
            @2 e2.actor -> bob
            @2 e2.target -> alice
            @3 e3.eventType = "harm"
            @3 e3.actor -> bob
            @3 e3.target -> alice
            now = 10
        }
    "#;

    let pattern = parse_pattern(pattern_dsl).unwrap();
    let graph = parse_graph(graph_dsl).unwrap();

    let mut engine = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    let matches = engine.evaluate(&graph);
    assert_eq!(matches.len(), 1, "hospitality pattern should match once");
}

#[test]
fn roundtrip_hospitality_no_match() {
    let pattern_dsl = r#"
        pattern violation_of_hospitality {
            stage e1 {
                e1.eventType = "enterTown"
                e1.actor -> ?guest
            }
            stage e2 {
                e2.eventType = "showHospitality"
                e2.actor -> ?host
                e2.target -> ?guest
            }
            stage e3 {
                e3.eventType = "harm"
                e3.actor -> ?host
                e3.target -> ?guest
            }
            unless between e1 e3 {
                eMid.eventType = "leaveTown"
                eMid.actor -> ?guest
            }
        }
    "#;

    // Guest leaves before harm → negation fires → no match
    let graph_dsl = r#"
        graph {
            @1 e1.eventType = "enterTown"
            @1 e1.actor -> alice
            @2 e2.eventType = "showHospitality"
            @2 e2.actor -> bob
            @2 e2.target -> alice
            @3 eMid.eventType = "leaveTown"
            @3 eMid.actor -> alice
            @4 e3.eventType = "harm"
            @4 e3.actor -> bob
            @4 e3.target -> alice
            now = 10
        }
    "#;

    let pattern = parse_pattern(pattern_dsl).unwrap();
    let graph = parse_graph(graph_dsl).unwrap();

    let mut engine = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    let matches = engine.evaluate(&graph);
    assert_eq!(
        matches.len(),
        0,
        "guest left → negation should kill the match"
    );
}

#[test]
fn parse_error_location() {
    let dsl = "pattern test { stage e1 { e1.type = } }";
    let err = parse_pattern(dsl).unwrap_err();
    assert!(err.line > 0);
    assert!(err.column > 0);
}

#[test]
fn parse_negated_clause() {
    let dsl = r#"
        pattern test {
            stage e1 {
                e1.eventType = "check"
                ! e1.trait = "impulsive"
            }
        }
    "#;
    let pattern = parse_pattern(dsl).unwrap();
    assert_eq!(pattern.stages[0].clauses.len(), 2);
    assert!(pattern.stages[0].clauses[1].negated);
}

#[test]
fn parse_value_constraint() {
    let dsl = r#"
        pattern test {
            stage e {
                e.eventType = "loyalty_check"
                e.loyalty < 0.5
            }
        }
    "#;
    let pattern = parse_pattern(dsl).unwrap();
    assert_eq!(pattern.stages[0].clauses.len(), 2);
}

#[test]
fn parse_unless_global() {
    let dsl = r#"
        pattern test {
            stage e1 {
                e1.eventType = "start"
            }
            stage e2 {
                e2.eventType = "end"
            }
            unless {
                mid.eventType = "cancel"
            }
        }
    "#;
    let pattern = parse_pattern(dsl).unwrap();
    assert_eq!(pattern.negations.len(), 1);
}

#[test]
fn parse_unless_after() {
    let dsl = r#"
        pattern test {
            stage e1 {
                e1.eventType = "start"
            }
            unless after e1 {
                bad.eventType = "cancel"
            }
        }
    "#;
    let pattern = parse_pattern(dsl).unwrap();
    assert_eq!(pattern.negations.len(), 1);
}

#[test]
fn parse_temporal_constraint() {
    let dsl = r#"
        pattern test {
            stage e1 {
                e1.eventType = "start"
            }
            stage e2 {
                e2.eventType = "end"
            }
            temporal e1 before e2
        }
    "#;
    let pattern = parse_pattern(dsl).unwrap();
    assert_eq!(pattern.temporal.len(), 1);
}

#[test]
fn parse_bounded_interval_graph() {
    let dsl = r#"
        graph {
            @1..10 ev.eventType = "siege"
            @3..5 inner.eventType = "sortie"
            now = 4
        }
    "#;
    let graph = parse_graph(dsl).unwrap();
    assert_eq!(graph.edge_count(), 2);
}

#[test]
fn parse_document_mixed() {
    let dsl = r#"
        pattern test {
            stage e {
                e.type = "hello"
            }
        }
        graph {
            @1 e.type = "hello"
            now = 5
        }
    "#;
    let doc = parse_document(dsl).unwrap();
    assert_eq!(doc.patterns.len(), 1);
    assert_eq!(doc.graphs.len(), 1);
}

#[test]
fn parse_comments() {
    let dsl = r#"
        // This is a comment
        pattern test {
            // Stage comment
            stage e {
                e.type = "hello" // inline-ish (next line)
            }
        }
    "#;
    let pattern = parse_pattern(dsl).unwrap();
    assert_eq!(pattern.name, "test");
}

#[test]
fn parse_numeric_edge() {
    let dsl = r#"
        graph {
            @1 e.score = 42.5
            now = 5
        }
    "#;
    let graph = parse_graph(dsl).unwrap();
    assert_eq!(graph.edge_count(), 1);
}

#[test]
fn parse_boolean_edge() {
    let dsl = r#"
        graph {
            @1 e.active = true
            @2 e2.active = false
            now = 5
        }
    "#;
    let graph = parse_graph(dsl).unwrap();
    assert_eq!(graph.edge_count(), 2);
}

// ---- Variable/literal source distinction tests ----

#[test]
fn parse_var_source() {
    let dsl = r#"
        pattern test {
            stage e1 {
                e1.eventType = "betray"
                e1.actor -> ?char
                ?char.trait = "impulsive"
            }
        }
    "#;
    let pattern = parse_pattern(dsl).unwrap();
    assert_eq!(pattern.stages[0].clauses.len(), 3);
}

#[test]
fn error_unbound_var_source() {
    let dsl = r#"
        pattern test {
            stage e1 {
                e1.eventType = "betray"
                ?char.trait = "impulsive"
            }
        }
    "#;
    let err = parse_pattern(dsl).unwrap_err();
    assert!(
        err.message.contains("?char"),
        "error should mention ?char: {}",
        err.message
    );
    assert!(
        err.message.contains("not yet bound"),
        "error should say 'not yet bound': {}",
        err.message
    );
}

#[test]
fn bare_source_is_literal() {
    let dsl = r#"
        pattern test {
            stage e1 {
                e1.eventType = "check"
                alice.trait = "impulsive"
            }
        }
    "#;
    // "alice" is a literal node name -- no error
    let pattern = parse_pattern(dsl).unwrap();
    assert_eq!(pattern.stages[0].clauses.len(), 2);
}

#[test]
fn var_from_prior_stage() {
    let dsl = r#"
        pattern test {
            stage e1 {
                e1.actor -> ?char
            }
            stage e2 {
                e2.eventType = "betray"
                ?char.trait = "impulsive"
            }
        }
    "#;
    let pattern = parse_pattern(dsl).unwrap();
    assert_eq!(pattern.stages.len(), 2);
    assert_eq!(pattern.stages[1].clauses.len(), 2);
}

#[test]
fn var_from_earlier_clause_same_stage() {
    let dsl = r#"
        pattern test {
            stage e1 {
                e1.actor -> ?char
                ?char.trait = "impulsive"
                e1.eventType = "betray"
            }
        }
    "#;
    let pattern = parse_pattern(dsl).unwrap();
    assert_eq!(pattern.stages[0].clauses.len(), 3);
}

#[test]
fn negation_var_target_references_parent() {
    let dsl = r#"
        pattern test {
            stage e1 {
                e1.actor -> ?char
                e1.eventType = "betray"
            }
            unless after e1 {
                mid.eventType = "reconcile"
                mid.actor -> ?char
            }
        }
    "#;
    let pattern = parse_pattern(dsl).unwrap();
    assert_eq!(pattern.negations.len(), 1);
}

#[test]
fn negated_var_source() {
    let dsl = r#"
        pattern test {
            stage e1 {
                e1.actor -> ?char
                ! ?char.trait = "cowardly"
            }
        }
    "#;
    let pattern = parse_pattern(dsl).unwrap();
    assert_eq!(pattern.stages[0].clauses.len(), 2);
    assert!(pattern.stages[0].clauses[1].negated);
}

#[test]
fn error_question_without_ident() {
    let dsl = r#"
        pattern test {
            stage e1 {
                ?.trait = "impulsive"
            }
        }
    "#;
    let err = parse_pattern(dsl).unwrap_err();
    assert!(
        err.message.contains("variable name after '?'"),
        "error: {}",
        err.message
    );
}

#[test]
fn error_negated_constraint() {
    let dsl = r#"
        pattern test {
            stage e {
                e.eventType = "loyalty_check"
                ! e.loyalty < 0.5
            }
        }
    "#;
    let err = parse_pattern(dsl).unwrap_err();
    assert!(
        err.message.contains("negated constraints"),
        "error: {}",
        err.message
    );
    assert!(
        err.message.contains("inverse"),
        "should suggest inverse: {}",
        err.message
    );
}

#[test]
fn error_negated_binding() {
    let dsl = r#"
        pattern test {
            stage e {
                e.eventType = "check"
                ! e.actor -> ?char
            }
        }
    "#;
    let err = parse_pattern(dsl).unwrap_err();
    assert!(
        err.message.contains("negated bindings"),
        "error: {}",
        err.message
    );
}

#[test]
fn error_binding_collides_with_anchor() {
    let dsl = r#"
        pattern test {
            stage e1 {
                e1.actor -> ?e1
            }
        }
    "#;
    let err = parse_pattern(dsl).unwrap_err();
    assert!(
        err.message.contains("collides with stage anchor"),
        "error: {}",
        err.message
    );
}

#[test]
fn binding_different_name_from_anchor_ok() {
    let dsl = r#"
        pattern test {
            stage e1 {
                e1.actor -> ?protagonist
            }
        }
    "#;
    let pattern = parse_pattern(dsl).unwrap();
    assert_eq!(pattern.stages[0].clauses.len(), 1);
}

#[test]
fn parse_temporal_with_gap_range() {
    let dsl = r#"
        pattern test {
            stage e1 { e1.eventType = "start" }
            stage e2 { e2.eventType = "end" }
            temporal e1 before e2 gap 3..10
        }
    "#;
    let pattern = parse_pattern(dsl).unwrap();
    assert_eq!(pattern.temporal.len(), 1);
    let tc = &pattern.temporal[0];
    let gap = tc.gap.as_ref().expect("should have gap");
    assert_eq!(gap.min, Some(3.0));
    assert_eq!(gap.max, Some(10.0));
}

#[test]
fn parse_temporal_gap_max_only() {
    let dsl = r#"
        pattern test {
            stage e1 { e1.eventType = "start" }
            stage e2 { e2.eventType = "end" }
            temporal e1 before e2 gap ..10
        }
    "#;
    let pattern = parse_pattern(dsl).unwrap();
    let gap = pattern.temporal[0].gap.as_ref().unwrap();
    assert_eq!(gap.min, None);
    assert_eq!(gap.max, Some(10.0));
}

#[test]
fn parse_temporal_gap_min_only() {
    let dsl = r#"
        pattern test {
            stage e1 { e1.eventType = "start" }
            stage e2 { e2.eventType = "end" }
            temporal e1 before e2 gap 3..
        }
    "#;
    let pattern = parse_pattern(dsl).unwrap();
    let gap = pattern.temporal[0].gap.as_ref().unwrap();
    assert_eq!(gap.min, Some(3.0));
    assert_eq!(gap.max, None);
}

#[test]
fn parse_temporal_no_gap_backwards_compat() {
    let dsl = r#"
        pattern test {
            stage e1 { e1.eventType = "start" }
            stage e2 { e2.eventType = "end" }
            temporal e1 before e2
        }
    "#;
    let pattern = parse_pattern(dsl).unwrap();
    assert!(pattern.temporal[0].gap.is_none());
}

#[test]
fn roundtrip_metric_gap_compiles() {
    // DSL round-trip: gap bounds parse and compile into Pattern
    let dsl = r#"
        pattern test {
            stage e1 { e1.eventType = "crisis" }
            stage e2 { e2.eventType = "betrayal" }
            temporal e1 before e2 gap 3..10
        }
    "#;
    let pattern = parse_pattern(dsl).unwrap();
    let gap = pattern.temporal[0].gap.as_ref().unwrap();
    assert_eq!(gap.min, Some(3.0));
    assert_eq!(gap.max, Some(10.0));
}

#[test]
fn roundtrip_metric_during_gap() {
    // During + gap: overlapping intervals where gap = start margin
    let dsl = r#"
        pattern test {
            stage e_outer { e_outer.eventType = "siege" }
            stage e_inner { e_inner.eventType = "sortie" }
            temporal e_inner during e_outer gap 2..50
        }
    "#;
    let graph_dsl = r#"
        graph {
            @1..100 e_outer.eventType = "siege"
            @3..5 e_inner.eventType = "sortie"
            now = 4
        }
    "#;
    // During: gap = start(inner) - start(outer) = 3 - 1 = 2, in [2, 50] ✓
    let pattern = parse_pattern(dsl).unwrap();
    let graph = parse_graph(graph_dsl).unwrap();
    let mut engine = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    assert_eq!(
        engine.evaluate(&graph).len(),
        1,
        "during gap=2 within [2,50]"
    );
}

#[test]
fn roundtrip_metric_during_gap_too_small() {
    let dsl = r#"
        pattern test {
            stage e_outer { e_outer.eventType = "siege" }
            stage e_inner { e_inner.eventType = "sortie" }
            temporal e_inner during e_outer gap 5..50
        }
    "#;
    let graph_dsl = r#"
        graph {
            @1..100 e_outer.eventType = "siege"
            @3..5 e_inner.eventType = "sortie"
            now = 4
        }
    "#;
    // During: gap = 3 - 1 = 2, but min is 5 → fails
    let pattern = parse_pattern(dsl).unwrap();
    let graph = parse_graph(graph_dsl).unwrap();
    let mut engine = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    assert_eq!(engine.evaluate(&graph).len(), 0, "during gap=2 below min=5");
}

#[test]
fn roundtrip_two_betrayals_with_var_source() {
    let pattern_dsl = r#"
        pattern two_impulsive_betrayals {
            stage e1 {
                e1.eventType = "betray"
                e1.actor -> ?char
                ?char.trait = "impulsive"
            }
            stage e2 {
                e2.eventType = "betray"
                e2.actor -> ?char
            }
        }
    "#;
    let graph_dsl = r#"
        graph {
            @0 alice.trait = "impulsive"
            @1 e1.eventType = "betray"
            @1 e1.actor -> alice
            @3 e2.eventType = "betray"
            @3 e2.actor -> alice
            now = 10
        }
    "#;
    let pattern = parse_pattern(pattern_dsl).unwrap();
    let graph = parse_graph(graph_dsl).unwrap();
    let mut engine = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);
    let matches = engine.evaluate(&graph);
    assert_eq!(
        matches.len(),
        1,
        "two betrayals by impulsive alice should match"
    );
}

// ---- Compose DSL tests ----

#[test]
fn parse_compose_sequence() {
    let dsl = r#"
        pattern setup {
            stage e1 { e1.eventType = "promise"  e1.actor -> ?char }
        }
        pattern payoff {
            stage e2 { e2.eventType = "fulfill"  e2.actor -> ?char }
        }
        compose promise_kept = setup >> payoff sharing(char)
    "#;
    let doc = parse_document(dsl).unwrap();
    // setup, payoff, and composed promise_kept
    assert_eq!(doc.patterns.len(), 3);
    let composed = &doc.patterns[2];
    assert_eq!(composed.name, "promise_kept");
    assert_eq!(composed.stages.len(), 2);
}

#[test]
fn parse_compose_choice() {
    let dsl = r#"
        pattern war { stage e { e.eventType = "war" } }
        pattern famine { stage e { e.eventType = "famine" } }
        pattern plague { stage e { e.eventType = "plague" } }
        compose crisis = war | famine | plague
    "#;
    let doc = parse_document(dsl).unwrap();
    // 3 originals + 3 choice alternatives = 6
    assert_eq!(doc.patterns.len(), 6);
    // Choice patterns have group set
    assert_eq!(doc.patterns[3].group, Some("crisis".to_string()));
    assert_eq!(doc.patterns[4].group, Some("crisis".to_string()));
    assert_eq!(doc.patterns[5].group, Some("crisis".to_string()));
}

#[test]
fn parse_compose_repeat() {
    let dsl = r#"
        pattern offense {
            stage e { e.eventType = "offense"  e.actor -> ?offender }
        }
        compose three_strikes = offense * 3 sharing(offender)
    "#;
    let doc = parse_document(dsl).unwrap();
    assert_eq!(doc.patterns.len(), 2); // offense + three_strikes
    let composed = &doc.patterns[1];
    assert_eq!(composed.name, "three_strikes");
    assert_eq!(composed.stages.len(), 3);
}

#[test]
fn compose_sequence_roundtrip_evaluation() {
    let dsl = r#"
        pattern setup {
            stage e1 { e1.eventType = "promise"  e1.actor -> ?char }
        }
        pattern payoff {
            stage e2 { e2.eventType = "fulfill"  e2.actor -> ?char }
        }
        compose promise_kept = setup >> payoff sharing(char)
    "#;
    let graph_dsl = r#"
        graph {
            @1 ev1.eventType = "promise"
            @1 ev1.actor -> alice
            @5 ev2.eventType = "fulfill"
            @5 ev2.actor -> alice
            now = 10
        }
    "#;
    let doc = parse_document(dsl).unwrap();
    let graph = parse_graph(graph_dsl).unwrap();
    let mut engine = SiftEngine::new(DefaultLetEvaluator);
    for p in doc.patterns {
        engine.register(p);
    }
    let matches = engine.evaluate(&graph);
    // setup matches, payoff matches, composed promise_kept matches
    let composed_matches: Vec<_> = matches
        .iter()
        .filter(|m| m.pattern == "promise_kept")
        .collect();
    assert_eq!(composed_matches.len(), 1, "composed sequence should match");
}

#[test]
fn compose_chain_of_composes() {
    let dsl = r#"
        pattern a { stage e1 { e1.eventType = "start" } }
        pattern b { stage e2 { e2.eventType = "middle" } }
        pattern c { stage e3 { e3.eventType = "end" } }
        compose ab = a >> b
        compose abc = ab >> c
    "#;
    let doc = parse_document(dsl).unwrap();
    // a, b, c, ab (2 stages), abc (3 stages)
    assert_eq!(doc.patterns.len(), 5);
    let abc = doc.patterns.iter().find(|p| p.name == "abc").unwrap();
    assert_eq!(abc.stages.len(), 3);
}

#[test]
fn compose_error_forward_reference() {
    let dsl = r#"
        compose arc = setup >> payoff
        pattern setup { stage e { e.eventType = "a" } }
        pattern payoff { stage e { e.eventType = "b" } }
    "#;
    let err = parse_document(dsl).unwrap_err();
    assert!(
        err.message.contains("not been defined yet"),
        "error: {}",
        err.message
    );
}

#[test]
fn compose_keyword_as_pattern_name() {
    // "compose" and "sharing" should work as identifiers
    let dsl = r#"
        pattern compose {
            stage e { e.eventType = "meta" }
        }
    "#;
    let pattern = parse_pattern(dsl).unwrap();
    assert_eq!(pattern.name, "compose");
}

#[test]
fn compose_choice_referenceable() {
    // A choice group name should be usable by subsequent composes
    let dsl = r#"
        pattern war { stage e { e.eventType = "war" } }
        pattern famine { stage e { e.eventType = "famine" } }
        compose crisis = war | famine
        pattern recovery { stage e2 { e2.eventType = "recovery" } }
        compose arc = crisis >> recovery
    "#;
    let doc = parse_document(dsl).unwrap();
    let arc = doc.patterns.iter().find(|p| p.name == "arc").unwrap();
    assert_eq!(arc.stages.len(), 2);
}

#[test]
fn compose_no_sharing_clause() {
    let dsl = r#"
        pattern a { stage e1 { e1.eventType = "x" } }
        pattern b { stage e2 { e2.eventType = "y" } }
        compose ab = a >> b
    "#;
    let doc = parse_document(dsl).unwrap();
    let composed = doc.patterns.iter().find(|p| p.name == "ab").unwrap();
    // Without sharing, anchors should be prefixed
    assert_eq!(composed.stages[0].anchor.0, "a_e1");
    assert_eq!(composed.stages[1].anchor.0, "b_e2");
}

// ===========================================================================
// Cross-stage value comparison (ConstraintVar)
// ===========================================================================

#[test]
fn roundtrip_constraint_var_gt() {
    use fabula_memory::MemGraph;

    let dsl = r#"
        pattern escalation {
            stage e1 {
                e1.type = "order"
                e1.price -> ?base_price
            }
            stage e2 {
                e2.type = "order"
                e2.price > ?base_price
            }
        }
        graph {
            @1 ev1.type = "order"
            @1 ev1.price = 100
            @2 ev2.type = "order"
            @2 ev2.price = 150
            now = 10
        }
    "#;
    let doc = parse_document(dsl).unwrap();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    for p in &doc.patterns {
        engine.register(p.clone());
    }
    let matches = engine.evaluate(&doc.graphs[0]);
    assert_eq!(matches.len(), 1, "150 > 100 should match via DSL roundtrip");
}

#[test]
fn roundtrip_constraint_var_no_match() {
    use fabula_memory::MemGraph;

    let dsl = r#"
        pattern escalation {
            stage e1 {
                e1.type = "order"
                e1.price -> ?base_price
            }
            stage e2 {
                e2.type = "order"
                e2.price > ?base_price
            }
        }
        graph {
            @1 ev1.type = "order"
            @1 ev1.price = 100
            @2 ev2.type = "order"
            @2 ev2.price = 50
            now = 10
        }
    "#;
    let doc = parse_document(dsl).unwrap();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    for p in &doc.patterns {
        engine.register(p.clone());
    }
    let matches = engine.evaluate(&doc.graphs[0]);
    assert_eq!(matches.len(), 0, "50 > 100 should NOT match");
}

#[test]
fn roundtrip_constraint_var_eq() {
    use fabula_memory::MemGraph;

    let dsl = r#"
        pattern exact_match {
            stage e1 {
                e1.type = "invoice"
                e1.amount -> ?expected
            }
            stage e2 {
                e2.type = "payment"
                e2.amount = ?expected
            }
        }
        graph {
            @1 ev1.type = "invoice"
            @1 ev1.amount = 500
            @2 ev2.type = "payment"
            @2 ev2.amount = 500
            now = 10
        }
    "#;
    let doc = parse_document(dsl).unwrap();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    for p in &doc.patterns {
        engine.register(p.clone());
    }
    let matches = engine.evaluate(&doc.graphs[0]);
    assert_eq!(matches.len(), 1, "500 == 500 should match via EqVar");
}

// ===========================================================================
// Repeat with range (min..max)
// ===========================================================================

#[test]
fn parse_compose_repeat_range() {
    let dsl = r#"
        pattern offense { stage e1 { e1.type = "offense" e1.target -> ?target } }
        compose strikes = offense * 3..5 sharing(target)
    "#;
    let doc = parse_document(dsl).unwrap();
    let strikes = doc.patterns.iter().find(|p| p.name == "strikes").unwrap();
    assert!(strikes.repeat_range.is_some(), "should have repeat_range");
    let rr = strikes.repeat_range.as_ref().unwrap();
    assert_eq!(rr.min_reps, 3);
    assert_eq!(rr.max_reps, Some(5));
}

#[test]
fn parse_compose_repeat_unbounded() {
    let dsl = r#"
        pattern offense { stage e1 { e1.type = "offense" } }
        compose brute = offense * 5..
    "#;
    let doc = parse_document(dsl).unwrap();
    let brute = doc.patterns.iter().find(|p| p.name == "brute").unwrap();
    assert!(brute.repeat_range.is_some(), "should have repeat_range");
    let rr = brute.repeat_range.as_ref().unwrap();
    assert_eq!(rr.min_reps, 5);
    assert_eq!(rr.max_reps, None, "unbounded max should be None");
}

#[test]
fn parse_compose_repeat_exact_unchanged() {
    // * N (exact count) should still work and NOT produce repeat_range
    let dsl = r#"
        pattern offense { stage e1 { e1.type = "offense" e1.target -> ?target } }
        compose strikes = offense * 3 sharing(target)
    "#;
    let doc = parse_document(dsl).unwrap();
    let strikes = doc.patterns.iter().find(|p| p.name == "strikes").unwrap();
    assert!(
        strikes.repeat_range.is_none(),
        "exact repeat should use unrolled approach"
    );
    assert_eq!(strikes.stages.len(), 3, "exact repeat should have 3 stages");
}

#[test]
fn roundtrip_repeat_range() {
    use fabula_memory::MemGraph;

    let dsl = r#"
        pattern offense {
            stage e1 {
                e1.type = "offense"
                e1.target -> ?target
            }
        }
        compose strikes = offense * 2..4 sharing(target)
        graph {
            @1 ev1.type = "offense"
            @1 ev1.target -> alice
            @2 ev2.type = "offense"
            @2 ev2.target -> alice
            @3 ev3.type = "offense"
            @3 ev3.target -> alice
            now = 10
        }
    "#;
    let doc = parse_document(dsl).unwrap();
    let strikes = doc.patterns.iter().find(|p| p.name == "strikes").unwrap();
    assert!(strikes.repeat_range.is_some(), "should use repeat_range");

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    for p in &doc.patterns {
        engine.register(p.clone());
    }

    // Feed edges incrementally
    let g = &doc.graphs[0];
    for i in 1..=3i64 {
        let src = format!("ev{}", i);
        engine.on_edge_added(
            g,
            &src,
            &"type".into(),
            &fabula_memory::MemValue::Str("offense".into()),
            &Interval::open(i),
        );
    }

    let completed = engine.drain_completed();
    assert!(
        !completed.is_empty(),
        "repeat_range should produce completions via DSL roundtrip"
    );
    // At least one completion should have the shared target = alice
    assert!(
        completed.iter().any(|m| {
            matches!(m.bindings.get("target"), Some(BoundValue::Node(n)) if n == "alice")
        }),
        "shared target should be alice"
    );
}

#[test]
fn parse_compose_choice_nonexclusive() {
    let src = r#"
        pattern war { stage e1 { e1.type = "war" } }
        pattern famine { stage e1 { e1.type = "famine" } }
        compose crisis = war | famine nonexclusive
    "#;
    let doc = fabula_dsl::parse_document(src).unwrap();
    for p in &doc.patterns {
        if p.name.starts_with("crisis_") {
            assert_eq!(
                p.group, None,
                "non-exclusive choice should have no group: {}",
                p.name
            );
        }
    }
}

#[test]
fn parse_private_pattern() {
    let src = r#"
        private pattern helper {
            stage e1 { e1.type = "setup" }
        }
        pattern visible {
            stage e1 { e1.type = "setup" }
        }
    "#;
    let doc = fabula_dsl::parse_document(src).unwrap();
    assert_eq!(doc.patterns.len(), 2);

    let helper = doc.patterns.iter().find(|p| p.name == "helper").unwrap();
    assert!(helper.private, "helper should be private");

    let visible = doc.patterns.iter().find(|p| p.name == "visible").unwrap();
    assert!(!visible.private, "visible should be public");
}

#[test]
fn parse_advance_in_place_pattern() {
    let src = r#"
        advance_in_place pattern fast_path {
            stage e1 { e1.type = "enter" }
            stage e2 { e2.type = "leave" }
        }
        pattern default_path {
            stage e1 { e1.type = "enter" }
            stage e2 { e2.type = "leave" }
        }
    "#;
    let doc = fabula_dsl::parse_document(src).unwrap();
    assert_eq!(doc.patterns.len(), 2);

    let fast = doc.patterns.iter().find(|p| p.name == "fast_path").unwrap();
    assert!(
        fast.advance_in_place,
        "fast_path should opt into advance_in_place"
    );

    let default_p = doc
        .patterns
        .iter()
        .find(|p| p.name == "default_path")
        .unwrap();
    assert!(
        !default_p.advance_in_place,
        "default_path should keep the classic fork behavior"
    );
}

#[test]
fn parse_advance_in_place_composes_with_private() {
    // `private` first
    let src = r#"
        private advance_in_place pattern internal_fast {
            stage e1 { e1.type = "enter" }
            stage e2 { e2.type = "leave" }
        }
    "#;
    let doc = fabula_dsl::parse_document(src).unwrap();
    let p = &doc.patterns[0];
    assert!(p.private);
    assert!(p.advance_in_place);

    // Reverse order
    let src = r#"
        advance_in_place private pattern internal_fast_rev {
            stage e1 { e1.type = "enter" }
            stage e2 { e2.type = "leave" }
        }
    "#;
    let doc = fabula_dsl::parse_document(src).unwrap();
    let p = &doc.patterns[0];
    assert!(p.private);
    assert!(p.advance_in_place);
}

#[test]
fn parse_duplicate_modifier_is_error() {
    let dup_private = r#"
        private private pattern foo {
            stage e1 { e1.type = "enter" }
        }
    "#;
    let err = fabula_dsl::parse_document(dup_private).unwrap_err();
    assert!(
        err.to_string().contains("duplicate 'private'"),
        "expected duplicate-private error, got: {err}"
    );

    let dup_advance = r#"
        advance_in_place advance_in_place pattern foo {
            stage e1 { e1.type = "enter" }
        }
    "#;
    let err = fabula_dsl::parse_document(dup_advance).unwrap_err();
    assert!(
        err.to_string().contains("duplicate 'advance_in_place'"),
        "expected duplicate-advance_in_place error, got: {err}"
    );
}

#[test]
fn parse_compose_choice_exclusive_default() {
    let src = r#"
        pattern war { stage e1 { e1.type = "war" } }
        pattern famine { stage e1 { e1.type = "famine" } }
        compose crisis = war | famine
    "#;
    let doc = fabula_dsl::parse_document(src).unwrap();
    for p in &doc.patterns {
        if p.name.starts_with("crisis_") {
            assert_eq!(
                p.group,
                Some("crisis".to_string()),
                "default choice should be exclusive: {}",
                p.name
            );
        }
    }
}

#[test]
fn private_pattern_with_nonexclusive_choice() {
    let src = r#"
        private pattern setup { stage e1 { e1.type = "setup" } }
        pattern action_a { stage e1 { e1.type = "action_a" } }
        pattern action_b { stage e1 { e1.type = "action_b" } }
        compose options = action_a | action_b nonexclusive
    "#;
    let doc = fabula_dsl::parse_document(src).unwrap();

    // setup is private
    let setup = doc.patterns.iter().find(|p| p.name == "setup").unwrap();
    assert!(setup.private);

    // choice alternatives are non-exclusive (no group)
    let choices: Vec<_> = doc
        .patterns
        .iter()
        .filter(|p| p.name.starts_with("options_"))
        .collect();
    assert_eq!(choices.len(), 2);
    for c in &choices {
        assert_eq!(c.group, None, "non-exclusive should have no group");
        assert!(!c.private, "choice alternatives should not inherit private");
    }
}

// ===========================================================================
// Value disjunction -- in [...] syntax
// ===========================================================================

#[test]
fn roundtrip_one_of_matches() {
    let dsl = r#"
        pattern hostile_action {
            stage e1 {
                e1.eventType in ["attack", "betray"]
                e1.actor -> ?char
            }
        }
        graph {
            @1 ev1.eventType = "attack"
            @1 ev1.actor -> alice
            now = 10
        }
    "#;
    let doc = parse_document(dsl).unwrap();
    let mut engine: SiftEngineFor<fabula_memory::MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    for p in &doc.patterns {
        engine.register(p.clone());
    }
    let matches = engine.evaluate(&doc.graphs[0]);
    assert_eq!(matches.len(), 1, "attack is in [attack, betray]");
}

#[test]
fn roundtrip_one_of_no_match() {
    let dsl = r#"
        pattern hostile_action {
            stage e1 {
                e1.eventType in ["attack", "betray"]
                e1.actor -> ?char
            }
        }
        graph {
            @1 ev1.eventType = "trade"
            @1 ev1.actor -> alice
            now = 10
        }
    "#;
    let doc = parse_document(dsl).unwrap();
    let mut engine: SiftEngineFor<fabula_memory::MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    for p in &doc.patterns {
        engine.register(p.clone());
    }
    let matches = engine.evaluate(&doc.graphs[0]);
    assert_eq!(matches.len(), 0, "trade is not in [attack, betray]");
}

#[test]
fn roundtrip_one_of_numeric() {
    let dsl = r#"
        pattern level_check {
            stage e1 {
                e1.level in [1, 2, 3]
            }
        }
    "#;
    // Just verify it compiles without error
    let pattern = parse_pattern(dsl).unwrap();
    assert_eq!(pattern.stages.len(), 1);
}

#[test]
fn lex_in_bracket_syntax() {
    use fabula_dsl::lexer::{Lexer, TokenKind};

    let src = r#"e1.eventType in ["attack", "betray"]"#;
    let tokens = Lexer::new(src).tokenize().unwrap();

    // e1 . eventType in [ "attack" , "betray" ] EOF
    assert!(matches!(tokens[0].kind, TokenKind::Ident(ref s) if s == "e1"));
    assert!(matches!(tokens[1].kind, TokenKind::Dot));
    assert!(matches!(tokens[2].kind, TokenKind::Ident(ref s) if s == "eventType"));
    assert!(matches!(tokens[3].kind, TokenKind::In));
    assert!(matches!(tokens[4].kind, TokenKind::LBracket));
    assert!(matches!(tokens[5].kind, TokenKind::String(ref s) if s == "attack"));
    assert!(matches!(tokens[6].kind, TokenKind::Comma));
    assert!(matches!(tokens[7].kind, TokenKind::String(ref s) if s == "betray"));
    assert!(matches!(tokens[8].kind, TokenKind::RBracket));
}

// ===========================================================================
// Importance directive
// ===========================================================================

#[test]
fn roundtrip_importance() {
    let dsl = r#"
        pattern climax importance 10.0 {
            stage e1 {
                e1.eventType = "confrontation"
            }
        }
    "#;
    let pattern = parse_pattern(dsl).unwrap();
    assert_eq!(pattern.name, "climax");
    assert_eq!(pattern.importance, 10.0);
}

#[test]
fn roundtrip_importance_default() {
    let dsl = r#"
        pattern normal {
            stage e1 {
                e1.eventType = "greeting"
            }
        }
    "#;
    let pattern = parse_pattern(dsl).unwrap();
    assert_eq!(pattern.importance, 1.0);
}

// ---------------------------------------------------------------------------
// Template tests (Tasks 3, 4, 5)
// ---------------------------------------------------------------------------

/// Helper: parse DSL source to a raw `Document` AST (not yet compiled).
fn parse_doc_ast(src: &str) -> Result<fabula_dsl::ast::Document, fabula_dsl::error::ParseError> {
    let tokens = fabula_dsl::lexer::Lexer::new(src).tokenize()?;
    fabula_dsl::parser::Parser::new(tokens).parse_document()
}

// --- Task 3: parse template definitions ---

#[test]
fn parse_template_definition() {
    let src = r#"
        template harm_arc(aggressor, victim) {
            stage e1 {
                e1.eventType = "harm"
                e1.actor -> ?aggressor
            }
        }
    "#;
    let doc = parse_doc_ast(src).expect("template should parse");
    let templates = doc.templates();
    assert_eq!(templates.len(), 1);
    let t = templates[0];
    assert_eq!(t.name, "harm_arc");
    assert_eq!(t.params, vec!["aggressor".to_string(), "victim".to_string()]);
    assert_eq!(t.stages.len(), 1);
    assert_eq!(t.stages[0].clauses.len(), 2);
}

#[test]
fn parse_template_with_zero_params() {
    let src = r#"
        template empty() {
            stage e1 {
                e1.eventType = "anything"
            }
        }
    "#;
    let doc = parse_doc_ast(src).expect("zero-param template should parse");
    let templates = doc.templates();
    assert_eq!(templates[0].params.len(), 0);
}

#[test]
fn parse_template_with_no_stages_errors() {
    let src = r#"template harm() { }"#;
    let result = parse_doc_ast(src);
    assert!(result.is_err(), "empty template body should error");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("at least one stage") || err_msg.contains("expected stage"),
        "error should mention missing stage; got: {}",
        err_msg
    );
}

// --- Task 4: parse instantiate in pattern bodies ---

#[test]
fn parse_instantiate_in_pattern_body() {
    let src = r#"
        template harm_arc(a, v) {
            stage e1 { e1.eventType = "harm" }
        }
        pattern alice_revenge {
            instantiate harm_arc("alice", "bob")
        }
    "#;
    let doc = parse_doc_ast(src).expect("should parse");
    let patterns = doc.patterns();
    assert_eq!(patterns.len(), 1);
    let pattern = patterns[0];
    // The pattern has one instantiation and no direct stages.
    assert_eq!(pattern.instantiations.len(), 1);
    let (pos, inst) = &pattern.instantiations[0];
    assert_eq!(*pos, 0); // inserted before any stage
    assert_eq!(inst.template_name, "harm_arc");
    assert_eq!(inst.args, vec!["alice".to_string(), "bob".to_string()]);
}

#[test]
fn parse_pattern_can_mix_stages_and_instantiations() {
    let src = r#"
        template harm(a, v) { stage e1 { e1.actor -> ?a } }
        pattern complex {
            stage prep { prep.eventType = "setup" }
            instantiate harm("alice", "bob")
            stage final_stage { final_stage.eventType = "cleanup" }
        }
    "#;
    let doc = parse_doc_ast(src).expect("should parse");
    let pattern = doc.patterns()[0];
    // 2 direct stages, 1 instantiation at position 1 (after `prep`)
    assert_eq!(pattern.stages.len(), 2);
    assert_eq!(pattern.instantiations.len(), 1);
    let (pos, _) = &pattern.instantiations[0];
    assert_eq!(*pos, 1, "instantiate follows the first stage");
}

// --- Task 5: compile-time template expansion ---

#[test]
fn instantiate_substitutes_parameters_into_clauses() {
    use fabula::pattern::Target;

    let src = r#"
        template harm_arc(aggressor, victim) {
            stage e1 {
                e1.eventType = "harm"
                e1.actor -> ?aggressor
                e1.target -> ?victim
            }
            stage e2 {
                e2.eventType = "retaliation"
                e2.actor -> ?victim
                e2.target -> ?aggressor
            }
        }
        pattern alice_revenge {
            instantiate harm_arc("alice", "bob")
        }
        pattern carol_revenge {
            instantiate harm_arc("carol", "dave")
        }
    "#;

    let result = parse_document(src).expect("should parse and compile");
    let patterns = &result.patterns;
    assert_eq!(patterns.len(), 2);
    let alice = patterns.iter().find(|p| p.name == "alice_revenge").unwrap();
    let carol = patterns.iter().find(|p| p.name == "carol_revenge").unwrap();

    assert_eq!(alice.stages.len(), 2);
    assert_eq!(carol.stages.len(), 2);

    // alice's e1 stage: actor -> alice
    let alice_e1 = &alice.stages[0];
    let alice_actor_clause = alice_e1
        .clauses
        .iter()
        .find(|c| c.label == "actor")
        .expect("alice e1 should have an actor clause");
    match &alice_actor_clause.target {
        Target::Bind(var) => assert_eq!(var.0, "alice"),
        other => panic!("expected Bind(\"alice\"), got {:?}", other),
    }

    // carol's e2 stage: actor -> dave
    let carol_e2 = &carol.stages[1];
    let carol_actor_clause = carol_e2
        .clauses
        .iter()
        .find(|c| c.label == "actor")
        .expect("carol e2 should have an actor clause");
    match &carol_actor_clause.target {
        Target::Bind(var) => assert_eq!(var.0, "dave"),
        other => panic!("expected Bind(\"dave\"), got {:?}", other),
    }
}

#[test]
fn instantiate_unknown_template_errors() {
    let src = r#"
        pattern p {
            instantiate undefined_template("a")
        }
    "#;
    let result = parse_document(src);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("undefined_template") || err.contains("unknown template"),
        "error should mention the missing template: {}",
        err
    );
}

#[test]
fn instantiate_arity_mismatch_errors() {
    let src = r#"
        template t(a, b) { stage e1 { e1.actor -> ?a } }
        pattern p {
            instantiate t("just_one_arg")
        }
    "#;
    let result = parse_document(src);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("arity") || err.contains("argument") || err.contains("expected 2"),
        "error should mention arity mismatch: {}",
        err
    );
}

#[test]
fn instantiate_can_be_invoked_multiple_times() {
    use fabula::pattern::Target;

    let src = r#"
        template t(name) { stage e1 { e1.actor -> ?name } }
        pattern p {
            instantiate t("alice")
            instantiate t("bob")
        }
    "#;
    let result = parse_document(src).expect("should compile");
    let p = &result.patterns[0];
    assert_eq!(p.stages.len(), 2, "two instantiations produce two stages");

    let stage1_clause = p.stages[0].clauses.iter().find(|c| c.label == "actor").unwrap();
    let stage2_clause = p.stages[1].clauses.iter().find(|c| c.label == "actor").unwrap();

    match (&stage1_clause.target, &stage2_clause.target) {
        (Target::Bind(v1), Target::Bind(v2)) => {
            assert_eq!(v1.0, "alice");
            assert_eq!(v2.0, "bob");
        }
        other => panic!("unexpected targets: {:?}", other),
    }
}
