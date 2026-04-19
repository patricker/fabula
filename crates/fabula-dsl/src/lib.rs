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

pub use compiler::{compile_pattern_body, compile_pattern_body_with, MemMapper, TypeMapper};

/// Result of parsing a document with patterns, graphs, and compose directives.
///
/// Generic over label and value types with defaults for backward compatibility.
/// Use `ParsedDocument` (no params) for the default `Pattern<String, MemValue>`.
pub struct ParsedDocument<L = String, V = MemValue> {
    /// All compiled patterns (both directly declared and composed).
    pub patterns: Vec<Pattern<L, V>>,
    /// Parsed graphs (always `MemGraph` -- graphs are test-only).
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
