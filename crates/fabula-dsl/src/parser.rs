//! Recursive descent parser for the fabula DSL.
//!
//! The parser is designed for composability: downstream DSLs can reuse it to
//! parse fabula pattern syntax embedded in their own blocks. Key entry points:
//!
//! - [`Parser::parse_pattern_body()`] — parse stages, negations, and temporals
//!   without the `pattern name { }` wrapper
//! - [`Parser::pos()`] / [`Parser::into_inner()`] — read or recover the cursor
//!   position for resumable parsing
//! - [`Parser::from_tokens_at()`] — construct a parser at a specific position
//!   in an existing token stream

use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::{Token, TokenKind};

/// Parser state: a cursor over a token stream.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    /// Create a new parser from a token stream, starting at position 0.
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Create a parser starting at a specific position in the token stream.
    ///
    /// Use this to resume parsing after handing the token stream to another
    /// parser (e.g., a downstream DSL parser that calls fabula's parser for
    /// pattern sections).
    pub fn from_tokens_at(tokens: Vec<Token>, pos: usize) -> Self {
        Self { tokens, pos }
    }

    /// Current cursor position in the token stream.
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Consume the parser, returning the token stream and cursor position.
    ///
    /// Use this to recover the tokens after parsing a section, so a
    /// downstream DSL can continue parsing from where fabula left off.
    pub fn into_inner(self) -> (Vec<Token>, usize) {
        (self.tokens, self.pos)
    }

    // ---- Document-level parsing ----

    /// Parse a complete document (patterns, graphs, and compose directives).
    pub fn parse_document(&mut self) -> Result<Document, ParseError> {
        let mut items = Vec::new();

        while !self.at_eof() {
            match &self.peek().kind {
                TokenKind::Ident(ref s) if s == "private" => {
                    self.advance(); // consume "private"
                                    // Next token must be "pattern"
                    if !self.check(TokenKind::Pattern) {
                        return Err(self.error("expected 'pattern' after 'private'"));
                    }
                    let mut pat = self.parse_pattern()?;
                    pat.private = true;
                    items.push(DocumentItem::Pattern(pat));
                }
                TokenKind::Pattern => items.push(DocumentItem::Pattern(self.parse_pattern()?)),
                TokenKind::Graph => items.push(DocumentItem::Graph(self.parse_graph()?)),
                TokenKind::Compose => items.push(DocumentItem::Compose(self.parse_compose()?)),
                _ => return Err(self.error("expected 'pattern', 'graph', or 'compose'")),
            }
        }

        Ok(Document { items })
    }

    /// Parse a single pattern declaration, then assert EOF.
    pub fn parse_pattern_only(&mut self) -> Result<PatternAst, ParseError> {
        let pat = self.parse_pattern()?;
        if !self.at_eof() {
            return Err(self.error("unexpected content after pattern"));
        }
        Ok(pat)
    }

    /// Parse a single graph declaration, then assert EOF.
    pub fn parse_graph_only(&mut self) -> Result<GraphAst, ParseError> {
        let g = self.parse_graph()?;
        if !self.at_eof() {
            return Err(self.error("unexpected content after graph"));
        }
        Ok(g)
    }

    // ---- Pattern parsing ----

    /// Parse a full `pattern name { ... }` declaration.
    pub fn parse_pattern(&mut self) -> Result<PatternAst, ParseError> {
        self.expect(TokenKind::Pattern)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;
        let body = self.parse_pattern_body()?;
        self.expect(TokenKind::RBrace)?;
        Ok(PatternAst {
            name,
            stages: body.stages,
            negations: body.negations,
            temporals: body.temporals,
            metadata: body.metadata,
            deadline: body.deadline,
            unordered_groups: body.unordered_groups,
            private: false,
        })
    }

    /// Parse the body of a pattern — stages, negations, and temporal
    /// constraints — without the `pattern name { }` wrapper.
    ///
    /// Stops when it sees `}` or EOF but does **not** consume the closing
    /// brace. The caller owns the block structure and is responsible for
    /// consuming the delimiter.
    ///
    /// This is the primary composability entry point for downstream DSLs
    /// that embed fabula pattern syntax in their own blocks:
    ///
    /// ```rust,ignore
    /// // salience-dsl example
    /// parser.expect_ident()?;             // "precondition"
    /// parser.expect(TokenKind::LBrace)?;  // {
    /// let body = parser.parse_pattern_body()?;
    /// parser.expect(TokenKind::RBrace)?;  // }
    /// let pattern = compile_pattern_body_with("name", &body, &mapper)?;
    /// ```
    pub fn parse_pattern_body(&mut self) -> Result<PatternBody, ParseError> {
        let mut stages = Vec::new();
        let mut negations = Vec::new();
        let mut temporals = Vec::new();
        let mut metadata = Vec::new();
        let mut deadline = None;
        let mut unordered_groups = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.at_eof() {
            match &self.peek().kind {
                TokenKind::Stage => stages.push(self.parse_stage()?),
                TokenKind::Unless => negations.push(self.parse_negation()?),
                TokenKind::Temporal => temporals.push(self.parse_temporal()?),
                TokenKind::Concurrent => {
                    self.advance();
                    self.expect(TokenKind::LBrace)?;
                    let group_start = stages.len();
                    while !self.check(TokenKind::RBrace) && !self.at_eof() {
                        if !self.check(TokenKind::Stage) {
                            return Err(
                                self.error("only 'stage' blocks are allowed inside 'concurrent'")
                            );
                        }
                        stages.push(self.parse_stage()?);
                    }
                    self.expect(TokenKind::RBrace)?;
                    let group_end = stages.len();
                    if group_end > group_start {
                        let indices: Vec<usize> = (group_start..group_end).collect();
                        unordered_groups.push(indices);
                    }
                }
                TokenKind::Ident(s) if s == "meta" => {
                    metadata.push(self.parse_meta()?);
                }
                TokenKind::Ident(s) if s == "deadline" => {
                    self.advance();
                    deadline = Some(self.expect_number()?);
                }
                _ => return Err(self.error(
                    "expected 'stage', 'unless', 'temporal', 'concurrent', 'meta', or 'deadline'",
                )),
            }
        }

        Ok(PatternBody {
            stages,
            negations,
            temporals,
            metadata,
            deadline,
            unordered_groups,
            private: false,
        })
    }

    /// Parse a `meta("key", "value")` clause.
    fn parse_meta(&mut self) -> Result<(String, String), ParseError> {
        // "meta" identifier already matched by caller; consume it
        self.advance();
        self.expect(TokenKind::LParen)?;
        let key = match &self.peek().kind {
            TokenKind::String(_) => {
                if let TokenKind::String(s) = &self.advance().kind {
                    s.clone()
                } else {
                    unreachable!()
                }
            }
            _ => return Err(self.error("expected string literal for meta key")),
        };
        self.expect(TokenKind::Comma)?;
        let value = match &self.peek().kind {
            TokenKind::String(_) => {
                if let TokenKind::String(s) = &self.advance().kind {
                    s.clone()
                } else {
                    unreachable!()
                }
            }
            _ => return Err(self.error("expected string literal for meta value")),
        };
        self.expect(TokenKind::RParen)?;
        Ok((key, value))
    }

    /// Parse a `stage anchor { clauses... }` block.
    pub fn parse_stage(&mut self) -> Result<StageAst, ParseError> {
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

    /// Parse an `unless [between|after] ... { clauses }` negation block.
    pub fn parse_negation(&mut self) -> Result<NegationAst, ParseError> {
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

    /// Parse a `temporal left relation right [gap range]` constraint.
    pub fn parse_temporal(&mut self) -> Result<TemporalAst, ParseError> {
        self.expect(TokenKind::Temporal)?;
        let left = self.expect_ident()?;
        let relation = self.expect_ident()?;
        let right = self.expect_ident()?;

        // Optional: gap min..max
        let (gap_min, gap_max) = if matches!(self.peek().kind, TokenKind::Ident(ref s) if s == "gap")
        {
            self.advance();
            self.parse_gap_range()?
        } else {
            (None, None)
        };

        Ok(TemporalAst {
            left,
            relation,
            right,
            gap_min,
            gap_max,
        })
    }

    fn parse_gap_range(&mut self) -> Result<(Option<f64>, Option<f64>), ParseError> {
        // Syntax: 3..10, ..10, 3..
        if self.check(TokenKind::DotDot) {
            // ..max (no min)
            self.advance();
            let max = self.expect_number()?;
            Ok((None, Some(max)))
        } else if matches!(self.peek().kind, TokenKind::Number(_) | TokenKind::Minus) {
            let first = self.expect_number()?;
            if self.check(TokenKind::DotDot) {
                self.advance();
                // min.. or min..max
                if matches!(self.peek().kind, TokenKind::Number(_) | TokenKind::Minus) {
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

    /// Parse a `compose name = ...` directive.
    pub fn parse_compose(&mut self) -> Result<ComposeAst, ParseError> {
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
            ComposeBody::Sequence {
                left: first,
                right: second,
                shared,
            }
        } else if self.check(TokenKind::Pipe) {
            // Choice: first | second | third ...
            let mut alternatives = vec![first];
            while self.check(TokenKind::Pipe) {
                self.advance();
                alternatives.push(self.expect_ident()?);
            }
            let exclusive = match &self.peek().kind {
                TokenKind::Ident(s) if s == "nonexclusive" => {
                    self.advance();
                    false
                }
                _ => true,
            };
            ComposeBody::Choice {
                alternatives,
                exclusive,
            }
        } else if self.check(TokenKind::Star) {
            // Repeat: first * N sharing(...) or first * N..M sharing(...) or first * N.. sharing(...)
            self.advance();
            let min = self.expect_number()? as usize;
            let max = if self.check(TokenKind::DotDot) {
                self.advance();
                // Check for explicit max or unbounded
                if matches!(self.peek().kind, TokenKind::Number(_) | TokenKind::Minus) {
                    Some(Some(self.expect_number()? as usize))
                } else {
                    Some(None) // unbounded: N..
                }
            } else {
                None // exact: N
            };
            let shared = self.parse_sharing_clause()?;
            match max {
                None => {
                    // Exact: * N → min=N, max=Some(N)
                    ComposeBody::Repeat {
                        pattern: first,
                        min,
                        max: Some(min),
                        shared,
                    }
                }
                Some(max_val) => {
                    // Range: * N..M or * N..
                    ComposeBody::Repeat {
                        pattern: first,
                        min,
                        max: max_val,
                        shared,
                    }
                }
            }
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

    /// Parse a single clause: `[!] [?]source.label = | -> | < | > | <= | >= target`.
    pub fn parse_clause(&mut self) -> Result<ClauseAst, ParseError> {
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

        // Now: = value, = ?var, -> ?var, -> node, < num, < ?var, > num, > ?var, <= num, <= ?var, >= num, >= ?var
        let target = if self.check(TokenKind::Eq) {
            self.advance();
            if self.check(TokenKind::Question) {
                self.advance();
                let var = self.expect_ident()?;
                ClauseTarget::ConstraintVar(ConstraintOp::Eq, var)
            } else {
                self.parse_literal_target()?
            }
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
            self.parse_constraint_target(ConstraintOp::Lt)?
        } else if self.check(TokenKind::Gt) {
            self.advance();
            self.parse_constraint_target(ConstraintOp::Gt)?
        } else if self.check(TokenKind::Lte) {
            self.advance();
            self.parse_constraint_target(ConstraintOp::Lte)?
        } else if self.check(TokenKind::Gte) {
            self.advance();
            self.parse_constraint_target(ConstraintOp::Gte)?
        } else {
            return Err(self.error("expected '=', '->', '<', '>', '<=', or '>='"));
        };

        Ok(ClauseAst {
            source,
            source_kind,
            label,
            target,
            negated,
        })
    }

    fn parse_literal_target(&mut self) -> Result<ClauseTarget, ParseError> {
        // Handle optional leading minus for negative number literals
        if self.check(TokenKind::Minus) {
            self.advance();
            if let TokenKind::Number(n) = &self.advance().kind {
                return Ok(ClauseTarget::LiteralNum(-n));
            }
            return Err(self.error("expected number after '-'"));
        }
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

    fn parse_constraint_target(&mut self, op: ConstraintOp) -> Result<ClauseTarget, ParseError> {
        if self.check(TokenKind::Question) {
            self.advance();
            let var = self.expect_ident()?;
            Ok(ClauseTarget::ConstraintVar(op, var))
        } else {
            let val = self.parse_constraint_value()?;
            Ok(ClauseTarget::Constraint(op, val))
        }
    }

    fn parse_constraint_value(&mut self) -> Result<ConstraintValue, ParseError> {
        let negative = if self.check(TokenKind::Minus) {
            self.advance();
            true
        } else {
            false
        };
        match &self.peek().kind {
            TokenKind::Number(_) => {
                if let TokenKind::Number(n) = &self.advance().kind {
                    Ok(ConstraintValue::Num(if negative { -*n } else { *n }))
                } else {
                    unreachable!()
                }
            }
            TokenKind::String(_) if !negative => {
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

    /// Parse a `graph { ... }` declaration.
    pub fn parse_graph(&mut self) -> Result<GraphAst, ParseError> {
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

        Ok(EdgeAst {
            time_start,
            time_end,
            source,
            label,
            target,
        })
    }

    fn parse_edge_target_literal(&mut self) -> Result<EdgeTarget, ParseError> {
        if self.check(TokenKind::Minus) {
            self.advance();
            if let TokenKind::Number(n) = &self.advance().kind {
                return Ok(EdgeTarget::Num(-n));
            }
            return Err(self.error("expected number after '-'"));
        }
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

    // ---- Token cursor utilities ----

    /// Peek at the current token without advancing.
    pub fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    /// Advance the cursor and return the consumed token.
    pub fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos];
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    /// Check if the cursor is at the end of the token stream.
    pub fn at_eof(&self) -> bool {
        matches!(self.tokens[self.pos].kind, TokenKind::Eof)
    }

    /// Check if the current token matches the given kind (by discriminant).
    pub fn check(&self, kind: TokenKind) -> bool {
        std::mem::discriminant(&self.tokens[self.pos].kind) == std::mem::discriminant(&kind)
    }

    /// Expect the current token to be of the given kind, advance, and return it.
    pub fn expect(&mut self, expected: TokenKind) -> Result<&Token, ParseError> {
        if self.check(expected.clone()) {
            Ok(self.advance())
        } else {
            Err(self.error(&format!("expected {:?}", expected)))
        }
    }

    /// Expect and consume an identifier token. Some keywords are allowed as
    /// identifiers in certain positions (between, after, compose, sharing).
    pub fn expect_ident(&mut self) -> Result<String, ParseError> {
        match &self.peek().kind {
            TokenKind::Ident(_) => {
                if let TokenKind::Ident(s) = &self.advance().kind {
                    Ok(s.clone())
                } else {
                    unreachable!()
                }
            }
            // Allow keywords as identifiers in certain positions
            TokenKind::Between => {
                self.advance();
                Ok("between".to_string())
            }
            TokenKind::After => {
                self.advance();
                Ok("after".to_string())
            }
            TokenKind::Compose => {
                self.advance();
                Ok("compose".to_string())
            }
            TokenKind::Sharing => {
                self.advance();
                Ok("sharing".to_string())
            }
            TokenKind::Concurrent => {
                self.advance();
                Ok("concurrent".to_string())
            }
            _ => Err(self.error("expected identifier")),
        }
    }

    /// Expect and consume an identifier or string literal token.
    pub fn expect_ident_or_string(&mut self) -> Result<String, ParseError> {
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

    /// Expect and consume a number literal token, with optional leading `-`.
    pub fn expect_number(&mut self) -> Result<f64, ParseError> {
        let negative = if self.check(TokenKind::Minus) {
            self.advance();
            true
        } else {
            false
        };
        match &self.peek().kind {
            TokenKind::Number(_) => {
                if let TokenKind::Number(n) = &self.advance().kind {
                    Ok(if negative { -*n } else { *n })
                } else {
                    unreachable!()
                }
            }
            _ => Err(self.error("expected number")),
        }
    }

    /// Create a parse error at the current token position.
    pub fn error(&self, msg: &str) -> ParseError {
        let tok = &self.tokens[self.pos];
        ParseError {
            line: tok.line,
            column: tok.column,
            span: tok.span(),
            message: msg.to_string(),
        }
    }
}
