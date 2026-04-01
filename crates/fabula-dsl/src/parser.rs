//! Recursive descent parser for the fabula DSL.

use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::{Token, TokenKind};

/// Parser state: a cursor over a token stream.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Parse a complete document (patterns, graphs, and compose directives).
    pub fn parse_document(&mut self) -> Result<Document, ParseError> {
        let mut items = Vec::new();

        while !self.at_eof() {
            match &self.peek().kind {
                TokenKind::Pattern => items.push(DocumentItem::Pattern(self.parse_pattern()?)),
                TokenKind::Graph => items.push(DocumentItem::Graph(self.parse_graph()?)),
                TokenKind::Compose => items.push(DocumentItem::Compose(self.parse_compose()?)),
                _ => return Err(self.error("expected 'pattern', 'graph', or 'compose'")),
            }
        }

        Ok(Document { items })
    }

    /// Parse a single pattern declaration.
    pub fn parse_pattern_only(&mut self) -> Result<PatternAst, ParseError> {
        let pat = self.parse_pattern()?;
        if !self.at_eof() {
            return Err(self.error("unexpected content after pattern"));
        }
        Ok(pat)
    }

    /// Parse a single graph declaration.
    pub fn parse_graph_only(&mut self) -> Result<GraphAst, ParseError> {
        let g = self.parse_graph()?;
        if !self.at_eof() {
            return Err(self.error("unexpected content after graph"));
        }
        Ok(g)
    }

    // ---- Pattern parsing ----

    fn parse_pattern(&mut self) -> Result<PatternAst, ParseError> {
        self.expect(TokenKind::Pattern)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;

        let mut stages = Vec::new();
        let mut negations = Vec::new();
        let mut temporals = Vec::new();

        while !self.check(TokenKind::RBrace) {
            match &self.peek().kind {
                TokenKind::Stage => stages.push(self.parse_stage()?),
                TokenKind::Unless => negations.push(self.parse_negation()?),
                TokenKind::Temporal => temporals.push(self.parse_temporal()?),
                _ => return Err(self.error("expected 'stage', 'unless', or 'temporal'")),
            }
        }

        self.expect(TokenKind::RBrace)?;
        Ok(PatternAst { name, stages, negations, temporals })
    }

    fn parse_stage(&mut self) -> Result<StageAst, ParseError> {
        self.expect(TokenKind::Stage)?;
        let anchor = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;

        let mut clauses = Vec::new();
        while !self.check(TokenKind::RBrace) {
            clauses.push(self.parse_clause()?);
        }

        self.expect(TokenKind::RBrace)?;
        Ok(StageAst { anchor, clauses })
    }

    fn parse_negation(&mut self) -> Result<NegationAst, ParseError> {
        self.expect(TokenKind::Unless)?;

        let kind = if self.check(TokenKind::Between) {
            self.advance();
            let start = self.expect_ident()?;
            let end = self.expect_ident()?;
            NegationKind::Between(start, end)
        } else if self.check(TokenKind::After) {
            self.advance();
            let start = self.expect_ident()?;
            NegationKind::After(start)
        } else {
            // Global negation: unless { ... }
            NegationKind::Global
        };

        self.expect(TokenKind::LBrace)?;
        let mut clauses = Vec::new();
        while !self.check(TokenKind::RBrace) {
            clauses.push(self.parse_clause()?);
        }
        self.expect(TokenKind::RBrace)?;

        Ok(NegationAst { kind, clauses })
    }

    fn parse_temporal(&mut self) -> Result<TemporalAst, ParseError> {
        self.expect(TokenKind::Temporal)?;
        let left = self.expect_ident()?;
        let relation = self.expect_ident()?;
        let right = self.expect_ident()?;

        // Optional: gap min..max
        let (gap_min, gap_max) = if matches!(self.peek().kind, TokenKind::Ident(ref s) if s == "gap") {
            self.advance();
            self.parse_gap_range()?
        } else {
            (None, None)
        };

        Ok(TemporalAst { left, relation, right, gap_min, gap_max })
    }

    fn parse_gap_range(&mut self) -> Result<(Option<f64>, Option<f64>), ParseError> {
        // Syntax: 3..10, ..10, 3..
        if self.check(TokenKind::DotDot) {
            // ..max (no min)
            self.advance();
            let max = self.expect_number()?;
            Ok((None, Some(max)))
        } else if matches!(self.peek().kind, TokenKind::Number(_)) {
            let first = self.expect_number()?;
            if self.check(TokenKind::DotDot) {
                self.advance();
                // min.. or min..max
                if matches!(self.peek().kind, TokenKind::Number(_)) {
                    let second = self.expect_number()?;
                    Ok((Some(first), Some(second)))
                } else {
                    // min.. (no max)
                    Ok((Some(first), None))
                }
            } else {
                // Single number = exact gap (min == max)
                Ok((Some(first), Some(first)))
            }
        } else {
            Err(self.error("expected gap range (e.g., '3..10', '..10', '3..')"))
        }
    }

    // ---- Compose parsing ----

    fn parse_compose(&mut self) -> Result<ComposeAst, ParseError> {
        self.expect(TokenKind::Compose)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::Eq)?;

        // First operand is always a pattern name
        let first = self.expect_ident()?;

        // Determine operator: >> (sequence), | (choice), * (repeat)
        let body = if self.check(TokenKind::GtGt) {
            // Sequence: first >> second sharing(...)
            self.advance();
            let second = self.expect_ident()?;
            let shared = self.parse_sharing_clause()?;
            ComposeBody::Sequence { left: first, right: second, shared }
        } else if self.check(TokenKind::Pipe) {
            // Choice: first | second | third ...
            let mut alternatives = vec![first];
            while self.check(TokenKind::Pipe) {
                self.advance();
                alternatives.push(self.expect_ident()?);
            }
            ComposeBody::Choice { alternatives }
        } else if self.check(TokenKind::Star) {
            // Repeat: first * count sharing(...)
            self.advance();
            let count = self.expect_number()? as usize;
            let shared = self.parse_sharing_clause()?;
            ComposeBody::Repeat { pattern: first, count, shared }
        } else {
            return Err(self.error("expected '>>' (sequence), '|' (choice), or '*' (repeat)"));
        };

        Ok(ComposeAst { name, body })
    }

    fn parse_sharing_clause(&mut self) -> Result<Vec<String>, ParseError> {
        if !self.check(TokenKind::Sharing) {
            return Ok(Vec::new());
        }
        self.advance(); // consume 'sharing'
        self.expect(TokenKind::LParen)?;

        let mut vars = vec![self.expect_ident()?];
        while self.check(TokenKind::Comma) {
            self.advance();
            vars.push(self.expect_ident()?);
        }

        self.expect(TokenKind::RParen)?;
        Ok(vars)
    }

    fn parse_clause(&mut self) -> Result<ClauseAst, ParseError> {
        // Optional negation prefix: !
        let negated = if self.check(TokenKind::Bang) {
            self.advance();
            true
        } else {
            false
        };

        // Check for ?var source (variable reference) vs bare literal
        let source_kind = if self.check(TokenKind::Question) {
            self.advance();
            // Must be followed by an identifier (the variable name)
            if !matches!(self.peek().kind, TokenKind::Ident(_)) {
                return Err(self.error("expected variable name after '?'"));
            }
            SourceKind::Var
        } else {
            SourceKind::Literal
        };

        let source = self.expect_ident()?;
        self.expect(TokenKind::Dot)?;
        let label = self.expect_ident_or_string()?;

        // Now: = value, -> ?var, -> node, < num, > num, <= num, >= num
        let target = if self.check(TokenKind::Eq) {
            self.advance();
            self.parse_literal_target()?
        } else if self.check(TokenKind::Arrow) {
            self.advance();
            if self.check(TokenKind::Question) {
                self.advance();
                let var = self.expect_ident()?;
                ClauseTarget::Bind(var)
            } else {
                let node = self.expect_ident()?;
                ClauseTarget::NodeRef(node)
            }
        } else if self.check(TokenKind::Lt) {
            self.advance();
            let val = self.parse_constraint_value()?;
            ClauseTarget::Constraint(ConstraintOp::Lt, val)
        } else if self.check(TokenKind::Gt) {
            self.advance();
            let val = self.parse_constraint_value()?;
            ClauseTarget::Constraint(ConstraintOp::Gt, val)
        } else if self.check(TokenKind::Lte) {
            self.advance();
            let val = self.parse_constraint_value()?;
            ClauseTarget::Constraint(ConstraintOp::Lte, val)
        } else if self.check(TokenKind::Gte) {
            self.advance();
            let val = self.parse_constraint_value()?;
            ClauseTarget::Constraint(ConstraintOp::Gte, val)
        } else {
            return Err(self.error("expected '=', '->', '<', '>', '<=', or '>='"));
        };

        Ok(ClauseAst { source, source_kind, label, target, negated })
    }

    fn parse_literal_target(&mut self) -> Result<ClauseTarget, ParseError> {
        match &self.peek().kind {
            TokenKind::String(_) => {
                if let TokenKind::String(s) = &self.advance().kind {
                    Ok(ClauseTarget::LiteralStr(s.clone()))
                } else {
                    unreachable!()
                }
            }
            TokenKind::Number(_) => {
                if let TokenKind::Number(n) = &self.advance().kind {
                    Ok(ClauseTarget::LiteralNum(*n))
                } else {
                    unreachable!()
                }
            }
            TokenKind::True => {
                self.advance();
                Ok(ClauseTarget::LiteralBool(true))
            }
            TokenKind::False => {
                self.advance();
                Ok(ClauseTarget::LiteralBool(false))
            }
            _ => Err(self.error("expected a string, number, or boolean value")),
        }
    }

    fn parse_constraint_value(&mut self) -> Result<ConstraintValue, ParseError> {
        match &self.peek().kind {
            TokenKind::Number(_) => {
                if let TokenKind::Number(n) = &self.advance().kind {
                    Ok(ConstraintValue::Num(*n))
                } else {
                    unreachable!()
                }
            }
            TokenKind::String(_) => {
                if let TokenKind::String(s) = &self.advance().kind {
                    Ok(ConstraintValue::Str(s.clone()))
                } else {
                    unreachable!()
                }
            }
            _ => Err(self.error("expected a number or string value")),
        }
    }

    // ---- Graph parsing ----

    fn parse_graph(&mut self) -> Result<GraphAst, ParseError> {
        self.expect(TokenKind::Graph)?;
        self.expect(TokenKind::LBrace)?;

        let mut edges = Vec::new();
        let mut now = None;

        while !self.check(TokenKind::RBrace) {
            if self.check(TokenKind::Now) {
                self.advance();
                self.expect(TokenKind::Eq)?;
                let n = self.expect_number()?;
                now = Some(n as i64);
            } else if self.check(TokenKind::At) {
                edges.push(self.parse_graph_edge()?);
            } else {
                return Err(self.error("expected '@' (edge) or 'now' in graph block"));
            }
        }

        self.expect(TokenKind::RBrace)?;
        Ok(GraphAst { edges, now })
    }

    fn parse_graph_edge(&mut self) -> Result<EdgeAst, ParseError> {
        self.expect(TokenKind::At)?;
        let time_start = self.expect_number()? as i64;

        // Check for bounded interval: @1..5
        let time_end = if self.check(TokenKind::DotDot) {
            self.advance();
            Some(self.expect_number()? as i64)
        } else {
            None
        };

        let source = self.expect_ident()?;
        self.expect(TokenKind::Dot)?;
        let label = self.expect_ident_or_string()?;

        let target = if self.check(TokenKind::Eq) {
            self.advance();
            self.parse_edge_target_literal()?
        } else if self.check(TokenKind::Arrow) {
            self.advance();
            let node = self.expect_ident()?;
            EdgeTarget::NodeRef(node)
        } else {
            return Err(self.error("expected '=' or '->' in graph edge"));
        };

        Ok(EdgeAst { time_start, time_end, source, label, target })
    }

    fn parse_edge_target_literal(&mut self) -> Result<EdgeTarget, ParseError> {
        match &self.peek().kind {
            TokenKind::String(_) => {
                if let TokenKind::String(s) = &self.advance().kind {
                    Ok(EdgeTarget::Str(s.clone()))
                } else {
                    unreachable!()
                }
            }
            TokenKind::Number(_) => {
                if let TokenKind::Number(n) = &self.advance().kind {
                    Ok(EdgeTarget::Num(*n))
                } else {
                    unreachable!()
                }
            }
            TokenKind::True => {
                self.advance();
                Ok(EdgeTarget::Bool(true))
            }
            TokenKind::False => {
                self.advance();
                Ok(EdgeTarget::Bool(false))
            }
            _ => Err(self.error("expected a string, number, or boolean value")),
        }
    }

    // ---- Utilities ----

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos];
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn at_eof(&self) -> bool {
        matches!(self.tokens[self.pos].kind, TokenKind::Eof)
    }

    fn check(&self, kind: TokenKind) -> bool {
        std::mem::discriminant(&self.tokens[self.pos].kind) == std::mem::discriminant(&kind)
    }

    fn expect(&mut self, expected: TokenKind) -> Result<&Token, ParseError> {
        if self.check(expected.clone()) {
            Ok(self.advance())
        } else {
            Err(self.error(&format!("expected {:?}", expected)))
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        match &self.peek().kind {
            TokenKind::Ident(_) => {
                if let TokenKind::Ident(s) = &self.advance().kind {
                    Ok(s.clone())
                } else {
                    unreachable!()
                }
            }
            // Allow keywords as identifiers in certain positions
            TokenKind::Between => { self.advance(); Ok("between".to_string()) }
            TokenKind::After => { self.advance(); Ok("after".to_string()) }
            TokenKind::Compose => { self.advance(); Ok("compose".to_string()) }
            TokenKind::Sharing => { self.advance(); Ok("sharing".to_string()) }
            _ => Err(self.error("expected identifier")),
        }
    }

    fn expect_ident_or_string(&mut self) -> Result<String, ParseError> {
        match &self.peek().kind {
            TokenKind::Ident(_) => {
                if let TokenKind::Ident(s) = &self.advance().kind {
                    Ok(s.clone())
                } else {
                    unreachable!()
                }
            }
            TokenKind::String(_) => {
                if let TokenKind::String(s) = &self.advance().kind {
                    Ok(s.clone())
                } else {
                    unreachable!()
                }
            }
            _ => Err(self.error("expected identifier or string")),
        }
    }

    fn expect_number(&mut self) -> Result<f64, ParseError> {
        match &self.peek().kind {
            TokenKind::Number(_) => {
                if let TokenKind::Number(n) = &self.advance().kind {
                    Ok(*n)
                } else {
                    unreachable!()
                }
            }
            _ => Err(self.error("expected number")),
        }
    }

    fn error(&self, msg: &str) -> ParseError {
        let tok = &self.tokens[self.pos];
        ParseError {
            line: tok.line,
            column: tok.column,
            span: tok.span(),
            message: msg.to_string(),
        }
    }
}
