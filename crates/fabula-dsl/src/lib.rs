//! Text DSL parser for fabula patterns and graphs.
//!
//! Inspired by Kreminski et al. (2021) "Winnow: A Domain-Specific Language
//! for Incremental Story Sifting" (AIIDE 2021). Extends Winnow with metric
//! temporal constraints (Dechter/Meiri/Pearl 1991), strict variable scoping
//! validation, and composition operators (Kreminski et al. 2025 FDG).
//!
//! Provides a human-readable syntax for defining temporal graph patterns
//! and in-memory graphs, compiling them to fabula's core types.
//!
//! # Pattern syntax
//!
//! ```text
//! pattern violation_of_hospitality {
//!   stage e1 {
//!     e1.eventType = "enterTown"
//!     e1.actor -> ?guest
//!   }
//!   stage e2 {
//!     e2.eventType = "showHospitality"
//!     e2.actor -> ?host
//!     e2.target -> ?guest
//!   }
//!   unless between e1 e2 {
//!     eMid.eventType = "leaveTown"
//!     eMid.actor -> ?guest
//!   }
//! }
//! ```
//!
//! # Graph syntax
//!
//! ```text
//! graph {
//!   @1 ev1.eventType = "enterTown"
//!   @1 ev1.actor -> alice
//!   @2..5 ev2.eventType = "siege"
//!   now = 10
//! }
//! ```

pub mod ast;
pub mod compiler;
pub mod error;
pub mod lexer;
pub mod parser;

use ast::DocumentItem;
use error::ParseError;
use fabula::pattern::Pattern;
use fabula_memory::{MemGraph, MemValue};
use std::collections::HashMap;
use std::fmt::Debug;

pub use compiler::{TypeMapper, MemMapper};

/// Result of parsing a document with patterns, graphs, and compose directives.
///
/// Generic over label and value types with defaults for backward compatibility.
/// Use `ParsedDocument` (no params) for the default `Pattern<String, MemValue>`.
pub struct ParsedDocument<L = String, V = MemValue> {
    /// All compiled patterns (both directly declared and composed).
    pub patterns: Vec<Pattern<L, V>>,
    /// Parsed graphs (always `MemGraph` — graphs are test-only).
    pub graphs: Vec<MemGraph>,
}

impl<L: Debug, V: Debug> std::fmt::Debug for ParsedDocument<L, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParsedDocument")
            .field("patterns", &self.patterns.len())
            .field("graphs", &self.graphs.len())
            .finish()
    }
}

/// Parse a pattern DSL string into a compiled `Pattern<String, MemValue>`.
pub fn parse_pattern(input: &str) -> Result<Pattern<String, MemValue>, ParseError> {
    parse_pattern_with(input, &MemMapper)
}

/// Parse a pattern DSL string using a custom [`TypeMapper`].
pub fn parse_pattern_with<M: TypeMapper>(
    input: &str,
    mapper: &M,
) -> Result<Pattern<M::L, M::V>, ParseError> {
    let tokens = lexer::Lexer::new(input).tokenize()?;
    let ast = parser::Parser::new(tokens).parse_pattern_only()?;
    compiler::compile_pattern_with(&ast, mapper)
}

/// Parse a graph DSL string into a `MemGraph`.
pub fn parse_graph(input: &str) -> Result<MemGraph, ParseError> {
    let tokens = lexer::Lexer::new(input).tokenize()?;
    let ast = parser::Parser::new(tokens).parse_graph_only()?;
    Ok(compiler::compile_graph(&ast))
}

/// Parse a document using the default `MemMapper`.
///
/// Items are processed in declaration order. Compose directives can reference
/// any pattern or compose result defined before them (no forward references).
pub fn parse_document(input: &str) -> Result<ParsedDocument, ParseError> {
    parse_document_with(input, &MemMapper)
}

/// Parse a document using a custom [`TypeMapper`].
///
/// Patterns are compiled with the mapper; graphs are always `MemGraph`.
pub fn parse_document_with<M: TypeMapper>(
    input: &str,
    mapper: &M,
) -> Result<ParsedDocument<M::L, M::V>, ParseError> {
    let tokens = lexer::Lexer::new(input).tokenize()?;
    let doc = parser::Parser::new(tokens).parse_document()?;

    let mut patterns = Vec::new();
    let mut graphs = Vec::new();
    let mut named: HashMap<String, Pattern<M::L, M::V>> = HashMap::new();

    for item in &doc.items {
        match item {
            DocumentItem::Pattern(ast) => {
                let pat = compiler::compile_pattern_with(ast, mapper)?;
                named.insert(ast.name.clone(), pat.clone());
                patterns.push(pat);
            }
            DocumentItem::Graph(ast) => {
                graphs.push(compiler::compile_graph(ast));
            }
            DocumentItem::Compose(ast) => {
                let composed = compiler::compile_compose_with(ast, &named, mapper)?;
                for p in &composed {
                    named.insert(p.name.clone(), p.clone());
                }
                // For sequence/repeat (single result), the compose name
                // is the pattern name. For choice (multiple results), also
                // insert the group name pointing to the first alternative
                // so chained composes can reference it.
                if composed.len() > 1 && !named.contains_key(&ast.name) {
                    named.insert(ast.name.clone(), composed[0].clone());
                }
                patterns.extend(composed);
            }
        }
    }

    Ok(ParsedDocument { patterns, graphs })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fabula::engine::SiftEngine;

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

        let mut engine = SiftEngine::new();
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

        let mut engine = SiftEngine::new();
        engine.register(pattern);
        let matches = engine.evaluate(&graph);
        assert_eq!(matches.len(), 0, "guest left → negation should kill the match");
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
        assert!(err.message.contains("?char"), "error should mention ?char: {}", err.message);
        assert!(err.message.contains("not yet bound"), "error should say 'not yet bound': {}", err.message);
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
        // "alice" is a literal node name — no error
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
        assert!(err.message.contains("variable name after '?'"), "error: {}", err.message);
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
        assert!(err.message.contains("negated constraints"), "error: {}", err.message);
        assert!(err.message.contains("inverse"), "should suggest inverse: {}", err.message);
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
        assert!(err.message.contains("negated bindings"), "error: {}", err.message);
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
        assert!(err.message.contains("collides with stage anchor"), "error: {}", err.message);
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
        let mut engine = SiftEngine::new();
        engine.register(pattern);
        assert_eq!(engine.evaluate(&graph).len(), 1, "during gap=2 within [2,50]");
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
        let mut engine = SiftEngine::new();
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
        let mut engine = SiftEngine::new();
        engine.register(pattern);
        let matches = engine.evaluate(&graph);
        assert_eq!(matches.len(), 1, "two betrayals by impulsive alice should match");
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
        let mut engine = SiftEngine::new();
        for p in doc.patterns {
            engine.register(p);
        }
        let matches = engine.evaluate(&graph);
        // setup matches, payoff matches, composed promise_kept matches
        let composed_matches: Vec<_> = matches.iter().filter(|m| m.pattern == "promise_kept").collect();
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
        assert!(err.message.contains("not been defined yet"), "error: {}", err.message);
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
}
