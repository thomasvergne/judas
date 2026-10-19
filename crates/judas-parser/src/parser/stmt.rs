use crate::error::ParseError;
use crate::parser::Parser;
use crate::token::TokenKind;
use judas_ast::expr::Literal;
use judas_ast::stmt::{MatchArm, Pattern, PatternKind, Stmt, StmtKind};

impl Parser {
    /// Parses a statement inside a procedural block or module.
    pub fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        match self.peek() {
            TokenKind::If => self.parse_if_stmt(),
            TokenKind::Match => self.parse_match_stmt(),
            TokenKind::Let => self.parse_let_stmt(),
            TokenKind::Connect => self.parse_connect_stmt(),
            TokenKind::Assert => self.parse_assert_stmt(),
            _ => {
                let expr = self.parse_expr()?;
                if self.match_token(&TokenKind::Eq) {
                    let value = self.parse_expr()?;
                    let span = expr.span.merge(value.span);
                    Ok(Stmt::new(
                        StmtKind::Assign {
                            target: expr,
                            value,
                        },
                        span,
                    ))
                } else {
                    let span = expr.span;
                    Ok(Stmt::new(StmtKind::Expr(expr), span))
                }
            }
        }
    }

    /// Parses an if statement.
    ///
    /// Returns an error if the if statement is not valid.
    ///
    /// # Examples
    ///
    /// ```text
    /// if true {
    ///
    /// }
    /// ```
    ///
    /// ```text
    /// if cond {
    ///     ...
    /// } else {
    ///     ...
    /// }
    /// ```
    pub(crate) fn parse_if_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_tok = self.expect(&TokenKind::If)?;
        let condition = self.parse_expr()?;
        self.expect(&TokenKind::LBrace)?;

        let mut then_branch = Vec::new();
        self.skip_semicolons();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            then_branch.push(self.parse_stmt()?);
            self.skip_semicolons();
        }
        let end_then = self.expect(&TokenKind::RBrace)?;

        let (else_branch, end_span) = if self.match_token(&TokenKind::Else) {
            if self.check(&TokenKind::If) {
                let nested_if = self.parse_if_stmt()?;
                let end = nested_if.span;
                (Some(vec![nested_if]), end)
            } else {
                self.expect(&TokenKind::LBrace)?;
                let mut else_stmts = Vec::new();
                self.skip_semicolons();
                while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
                    else_stmts.push(self.parse_stmt()?);
                    self.skip_semicolons();
                }
                let end_else = self.expect(&TokenKind::RBrace)?;
                (Some(else_stmts), end_else.span)
            }
        } else {
            (None, end_then.span)
        };

        let span = start_tok.span.merge(end_span);
        Ok(Stmt::new(
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            },
            span,
        ))
    }

    /// Parses a match statement.
    ///
    /// Returns an error if the match statement is not valid.
    ///
    /// # Examples
    ///
    /// ```text
    /// match expr {
    ///     ...
    /// }
    /// ```
    pub(crate) fn parse_match_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_tok = self.expect(&TokenKind::Match)?;
        let scrutinee = self.parse_expr()?;
        self.expect(&TokenKind::LBrace)?;

        let mut arms = Vec::new();
        self.skip_semicolons();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            arms.push(self.parse_match_arm()?);
            self.skip_semicolons();
        }

        let end_tok = self.expect(&TokenKind::RBrace)?;
        let span = start_tok.span.merge(end_tok.span);

        Ok(Stmt::new(StmtKind::Match { scrutinee, arms }, span))
    }

    /// Parses a match arm.
    ///
    /// Returns an error if the match arm is not valid.
    ///
    /// # Examples
    ///
    /// ```text
    /// pattern => body
    /// ```
    pub(crate) fn parse_match_arm(&mut self) -> Result<MatchArm, ParseError> {
        let pattern = self.parse_pattern()?;
        self.expect(&TokenKind::FatArrow)?;

        let (body, end_span) = if self.check(&TokenKind::LBrace) {
            self.advance();
            let mut stmts = Vec::new();
            self.skip_semicolons();
            while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
                stmts.push(self.parse_stmt()?);
                self.skip_semicolons();
            }
            let end_brace = self.expect(&TokenKind::RBrace)?;
            (stmts, end_brace.span)
        } else {
            let stmt = self.parse_stmt()?;
            let span = stmt.span;
            (vec![stmt], span)
        };

        // Comma between match arms is optional
        self.match_token(&TokenKind::Comma);

        let span = pattern.span.merge(end_span);
        Ok(MatchArm {
            pattern,
            body,
            span,
        })
    }

    /// Parses a pattern.
    ///
    /// Returns an error if the pattern is not valid.
    ///
    /// # Examples
    ///
    /// `_`, `ident`, `123`, `true`, `false`, `()`
    pub(crate) fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        let span = self.current_span();
        match self.peek().clone() {
            TokenKind::Ident(ref s) if s == "_" => {
                self.advance();
                Ok(Pattern::new(PatternKind::Wildcard, span))
            }
            TokenKind::Ident(_) => {
                let ident = self.expect_ident()?;
                let span = ident.span;
                Ok(Pattern::new(PatternKind::Ident(ident), span))
            }
            TokenKind::IntLiteral(val) => {
                self.advance();
                Ok(Pattern::new(
                    PatternKind::Literal(Literal::Integer(val)),
                    span,
                ))
            }
            TokenKind::BoolLiteral(val) => {
                self.advance();
                Ok(Pattern::new(PatternKind::Literal(Literal::Bool(val)), span))
            }
            other => Err(ParseError::new(
                format!("Expected pattern, found {}", other),
                span,
            )),
        }
    }

    /// Parses a let statement.
    ///
    /// Returns an error if the let statement is not valid.
    ///
    /// # Examples
    ///
    /// ```text
    /// let name = value;
    /// ```
    ///
    /// ```text
    /// let name: type = value;
    /// ```
    pub(crate) fn parse_let_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_tok = self.expect(&TokenKind::Let)?;
        let name = self.expect_ident()?;

        let ty = if self.match_token(&TokenKind::Colon) {
            Ok(Some(self.parse_type()?))
        } else {
            Ok(None)
        }?;

        self.expect(&TokenKind::Eq)?;
        let value = self.parse_expr()?;

        let span = start_tok.span.merge(value.span);
        Ok(Stmt::new(StmtKind::Let { name, ty, value }, span))
    }

    /// Parses a connect statement.
    ///
    /// Returns an error if the connect statement is not valid.
    ///
    /// # Examples
    ///
    /// ```text
    /// connect target = value;
    /// ```
    pub(crate) fn parse_connect_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_tok = self.expect(&TokenKind::Connect)?;
        let target = self.parse_expr()?;
        self.expect(&TokenKind::Eq)?;
        let value = self.parse_expr()?;

        let span = start_tok.span.merge(value.span);
        Ok(Stmt::new(StmtKind::Connect { target, value }, span))
    }

    /// Parses an assert statement.
    ///
    /// Returns an error if the assert statement is not valid.
    ///
    /// # Examples
    ///
    /// ```text
    /// assert condition;
    /// ```
    ///
    /// ```text
    /// assert condition, message;
    /// ```
    pub(crate) fn parse_assert_stmt(&mut self) -> Result<Stmt, ParseError> {
        let decl = self.parse_assert_decl()?;
        let span = decl.span;
        Ok(Stmt::new(
            StmtKind::Assert {
                condition: decl.condition,
                message: decl.message,
            },
            span,
        ))
    }
}
