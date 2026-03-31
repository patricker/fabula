//! Text DSL parser for fabula patterns and graphs.
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

use error::ParseError;
use fabula::pattern::Pattern;
use fabula_memory::{MemGraph, MemValue};

/// Result of parsing a document with both patterns and graphs.
pub struct ParsedDocument {
    pub patterns: Vec<Pattern<String, MemValue>>,
    pub graphs: Vec<MemGraph>,
}

/// Parse a pattern DSL string into a compiled `Pattern`.
pub fn parse_pattern(input: &str) -> Result<Pattern<String, MemValue>, ParseError> {
    let tokens = lexer::Lexer::new(input).tokenize()?;
    let ast = parser::Parser::new(tokens).parse_pattern_only()?;
    compiler::compile_pattern(&ast)
}

/// Parse a graph DSL string into a `MemGraph`.
pub fn parse_graph(input: &str) -> Result<MemGraph, ParseError> {
    let tokens = lexer::Lexer::new(input).tokenize()?;
    let ast = parser::Parser::new(tokens).parse_graph_only()?;
    Ok(compiler::compile_graph(&ast))
}

/// Parse a document containing any combination of pattern and graph declarations.
pub fn parse_document(input: &str) -> Result<ParsedDocument, ParseError> {
    let tokens = lexer::Lexer::new(input).tokenize()?;
    let doc = parser::Parser::new(tokens).parse_document()?;

    let mut patterns = Vec::new();
    for pat_ast in &doc.patterns {
        patterns.push(compiler::compile_pattern(pat_ast)?);
    }

    let graphs: Vec<MemGraph> = doc.graphs.iter().map(compiler::compile_graph).collect();

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
}
