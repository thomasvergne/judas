pub mod expr;
pub mod item;
pub mod stmt;
pub mod types;

use crate::error::ParseError;
use crate::token::{Token, TokenKind};
use judas_ast::span::Span;
use judas_ast::types::Ident;

/// The Judas recursive-descent parser.
///
/// Converts a stream of [`Token`]s into untyped AST items,
/// statements, expressions, and types.
pub struct Parser {
    pub(crate) tokens: Vec<Token>,
    pub(crate) cursor: usize,
    pub(crate) allow_gt: bool,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            cursor: 0,
            allow_gt: true,
        }
    }

    /// Returns the current token.
    pub(crate) fn current(&self) -> &Token {
        if self.cursor < self.tokens.len() {
            &self.tokens[self.cursor]
        } else {
            self.tokens.last().unwrap()
        }
    }

    /// Returns the current token kind.
    pub(crate) fn peek(&self) -> &TokenKind {
        &self.current().kind
    }

    /// Returns the token kind at the given offset ahead of
    /// the current token.
    pub(crate) fn peek_ahead(&self, offset: usize) -> &TokenKind {
        let idx = self.cursor + offset;
        if idx < self.tokens.len() {
            &self.tokens[idx].kind
        } else {
            &TokenKind::Eof
        }
    }

    pub(crate) fn current_span(&self) -> Span {
        self.current().span
    }

    pub(crate) fn is_at_end(&self) -> bool {
        matches!(self.peek(), TokenKind::Eof)
    }

    pub(crate) fn advance(&mut self) -> Token {
        let tok = self.current().clone();
        if !self.is_at_end() {
            self.cursor += 1;
        }
        tok
    }

    pub(crate) fn check(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(self.peek()) == std::mem::discriminant(kind)
    }

    pub(crate) fn match_token(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub(crate) fn expect(&mut self, kind: &TokenKind) -> Result<Token, ParseError> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            Err(ParseError::new(
                format!("Expected {}, found {}", kind, self.peek()),
                self.current_span(),
            ))
        }
    }

    pub(crate) fn expect_ident(&mut self) -> Result<Ident, ParseError> {
        let span = self.current_span();
        match self.advance().kind {
            TokenKind::Ident(name) => Ok(Ident::new(name, span)),
            other => Err(ParseError::new(
                format!("Expected identifier, found {}", other),
                span,
            )),
        }
    }

    pub(crate) fn skip_semicolons(&mut self) {
        while self.match_token(&TokenKind::Semicolon) {}
    }
}
