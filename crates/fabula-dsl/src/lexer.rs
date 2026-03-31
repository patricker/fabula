//! Tokenizer for the fabula DSL.

use crate::error::ParseError;

/// Token types produced by the lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords
    Pattern,
    Stage,
    Unless,
    Between,
    After,
    Graph,
    Now,
    Temporal,
    True,
    False,

    // Symbols
    LBrace,    // {
    RBrace,    // }
    Dot,       // .
    Arrow,     // ->
    Eq,        // =
    Lt,        // <
    Gt,        // >
    Lte,       // <=
    Gte,       // >=
    Bang,      // !
    At,        // @
    DotDot,    // ..
    Question,  // ?

    // Literals
    Ident(String),
    String(String),
    Number(f64),

    Eof,
}

/// A token with its source location.
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub column: usize,
    pub offset: usize,
    pub len: usize,
}

impl Token {
    pub fn span(&self) -> (usize, usize) {
        (self.offset, self.offset + self.len)
    }
}

/// Tokenizes DSL source text into a stream of tokens.
pub struct Lexer<'a> {
    source: &'a str,
    bytes: &'a [u8],
    pos: usize,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, ParseError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            if self.pos >= self.bytes.len() {
                tokens.push(Token {
                    kind: TokenKind::Eof,
                    line: self.line,
                    column: self.col,
                    offset: self.pos,
                    len: 0,
                });
                break;
            }
            tokens.push(self.next_token()?);
        }
        Ok(tokens)
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            // Skip whitespace
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
                if self.bytes[self.pos] == b'\n' {
                    self.line += 1;
                    self.col = 1;
                } else {
                    self.col += 1;
                }
                self.pos += 1;
            }
            // Skip line comments
            if self.pos + 1 < self.bytes.len()
                && self.bytes[self.pos] == b'/'
                && self.bytes[self.pos + 1] == b'/'
            {
                while self.pos < self.bytes.len() && self.bytes[self.pos] != b'\n' {
                    self.pos += 1;
                }
                continue;
            }
            break;
        }
    }

    fn next_token(&mut self) -> Result<Token, ParseError> {
        let start = self.pos;
        let line = self.line;
        let col = self.col;
        let ch = self.bytes[self.pos];

        match ch {
            b'{' => {
                self.advance();
                Ok(Token { kind: TokenKind::LBrace, line, column: col, offset: start, len: 1 })
            }
            b'}' => {
                self.advance();
                Ok(Token { kind: TokenKind::RBrace, line, column: col, offset: start, len: 1 })
            }
            b'@' => {
                self.advance();
                Ok(Token { kind: TokenKind::At, line, column: col, offset: start, len: 1 })
            }
            b'?' => {
                self.advance();
                Ok(Token { kind: TokenKind::Question, line, column: col, offset: start, len: 1 })
            }
            b'!' => {
                self.advance();
                Ok(Token { kind: TokenKind::Bang, line, column: col, offset: start, len: 1 })
            }
            b'.' => {
                self.advance();
                if self.pos < self.bytes.len() && self.bytes[self.pos] == b'.' {
                    self.advance();
                    Ok(Token { kind: TokenKind::DotDot, line, column: col, offset: start, len: 2 })
                } else {
                    Ok(Token { kind: TokenKind::Dot, line, column: col, offset: start, len: 1 })
                }
            }
            b'-' => {
                self.advance();
                if self.pos < self.bytes.len() && self.bytes[self.pos] == b'>' {
                    self.advance();
                    Ok(Token { kind: TokenKind::Arrow, line, column: col, offset: start, len: 2 })
                } else {
                    // Negative number
                    if self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
                        self.read_number(start, line, col, true)
                    } else {
                        Err(self.error_at(line, col, start, "unexpected character '-'"))
                    }
                }
            }
            b'=' => {
                self.advance();
                // == is just = (equality), since we don't have assignment
                if self.pos < self.bytes.len() && self.bytes[self.pos] == b'=' {
                    self.advance();
                }
                Ok(Token { kind: TokenKind::Eq, line, column: col, offset: start, len: self.pos - start })
            }
            b'<' => {
                self.advance();
                if self.pos < self.bytes.len() && self.bytes[self.pos] == b'=' {
                    self.advance();
                    Ok(Token { kind: TokenKind::Lte, line, column: col, offset: start, len: 2 })
                } else {
                    Ok(Token { kind: TokenKind::Lt, line, column: col, offset: start, len: 1 })
                }
            }
            b'>' => {
                self.advance();
                if self.pos < self.bytes.len() && self.bytes[self.pos] == b'=' {
                    self.advance();
                    Ok(Token { kind: TokenKind::Gte, line, column: col, offset: start, len: 2 })
                } else {
                    Ok(Token { kind: TokenKind::Gt, line, column: col, offset: start, len: 1 })
                }
            }
            b'"' => self.read_string(line, col),
            b'0'..=b'9' => self.read_number(start, line, col, false),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.read_ident(start, line, col),
            _ => Err(self.error_at(line, col, start, &format!("unexpected character '{}'", ch as char))),
        }
    }

    fn advance(&mut self) {
        self.pos += 1;
        self.col += 1;
    }

    fn read_string(&mut self, line: usize, col: usize) -> Result<Token, ParseError> {
        let start = self.pos;
        self.advance(); // skip opening "
        let content_start = self.pos;
        while self.pos < self.bytes.len() && self.bytes[self.pos] != b'"' {
            if self.bytes[self.pos] == b'\n' {
                return Err(self.error_at(line, col, start, "unterminated string literal"));
            }
            self.pos += 1;
            self.col += 1;
        }
        if self.pos >= self.bytes.len() {
            return Err(self.error_at(line, col, start, "unterminated string literal"));
        }
        let s = self.source[content_start..self.pos].to_string();
        self.advance(); // skip closing "
        Ok(Token {
            kind: TokenKind::String(s),
            line,
            column: col,
            offset: start,
            len: self.pos - start,
        })
    }

    fn read_number(&mut self, start: usize, line: usize, col: usize, already_negated: bool) -> Result<Token, ParseError> {
        // pos is already past the optional '-' if already_negated
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
            self.col += 1;
        }
        // Check for decimal point (but not ..)
        if self.pos + 1 < self.bytes.len()
            && self.bytes[self.pos] == b'.'
            && self.bytes[self.pos + 1] != b'.'
            && self.bytes[self.pos + 1].is_ascii_digit()
        {
            self.pos += 1;
            self.col += 1;
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
                self.pos += 1;
                self.col += 1;
            }
        }
        let num_str = &self.source[start..self.pos];
        let val: f64 = num_str.parse().map_err(|_| {
            self.error_at(line, col, start, &format!("invalid number '{}'", num_str))
        })?;
        let _ = already_negated; // sign is included in the slice
        Ok(Token {
            kind: TokenKind::Number(val),
            line,
            column: col,
            offset: start,
            len: self.pos - start,
        })
    }

    fn read_ident(&mut self, start: usize, line: usize, col: usize) -> Result<Token, ParseError> {
        while self.pos < self.bytes.len()
            && (self.bytes[self.pos].is_ascii_alphanumeric() || self.bytes[self.pos] == b'_')
        {
            self.pos += 1;
            self.col += 1;
        }
        let word = &self.source[start..self.pos];
        let kind = match word {
            "pattern" => TokenKind::Pattern,
            "stage" => TokenKind::Stage,
            "unless" => TokenKind::Unless,
            "between" => TokenKind::Between,
            "after" => TokenKind::After,
            "graph" => TokenKind::Graph,
            "now" => TokenKind::Now,
            "temporal" => TokenKind::Temporal,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            _ => TokenKind::Ident(word.to_string()),
        };
        Ok(Token {
            kind,
            line,
            column: col,
            offset: start,
            len: self.pos - start,
        })
    }

    fn error_at(&self, line: usize, col: usize, offset: usize, msg: &str) -> ParseError {
        ParseError {
            line,
            column: col,
            span: (offset, self.pos.max(offset + 1)),
            message: msg.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_simple_pattern() {
        let src = r#"pattern test { stage e1 { e1.eventType = "enter" } }"#;
        let tokens = Lexer::new(src).tokenize().unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::Pattern));
        assert!(matches!(tokens[1].kind, TokenKind::Ident(ref s) if s == "test"));
        assert!(matches!(tokens[2].kind, TokenKind::LBrace));
        assert!(matches!(tokens[3].kind, TokenKind::Stage));
    }

    #[test]
    fn tokenize_graph() {
        let src = r#"graph { @1 ev.type = "enter" @2..5 ev2.type = "siege" }"#;
        let tokens = Lexer::new(src).tokenize().unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::Graph));
        assert!(matches!(tokens[2].kind, TokenKind::At));
        assert!(matches!(tokens[3].kind, TokenKind::Number(n) if n == 1.0));
    }

    #[test]
    fn tokenize_comments() {
        let src = "// this is a comment\npattern test {}";
        let tokens = Lexer::new(src).tokenize().unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::Pattern));
    }

    #[test]
    fn tokenize_arrow_and_question() {
        let src = "e1.actor -> ?guest";
        let tokens = Lexer::new(src).tokenize().unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::Ident(ref s) if s == "e1"));
        assert!(matches!(tokens[1].kind, TokenKind::Dot));
        assert!(matches!(tokens[2].kind, TokenKind::Ident(ref s) if s == "actor"));
        assert!(matches!(tokens[3].kind, TokenKind::Arrow));
        assert!(matches!(tokens[4].kind, TokenKind::Question));
        assert!(matches!(tokens[5].kind, TokenKind::Ident(ref s) if s == "guest"));
    }
}
