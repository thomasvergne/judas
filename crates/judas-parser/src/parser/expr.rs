use crate::error::ParseError;
use crate::parser::Parser;
use crate::token::TokenKind;
use judas_ast::expr::{BinaryOp, CallArg, Expr, ExprKind, FieldInit, Literal, UnaryOp};
use judas_ast::path::Path;

impl Parser {
    /// Parses an expression with operator precedence climbing.
    pub fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary_expr(0)
    }

    /// Parses a binary expression with operator precedence climbing.
    ///
    /// # Arguments
    ///
    /// * `min_prec` - The minimum precedence level to parse.
    pub(crate) fn parse_binary_expr(&mut self, min_prec: u8) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary_expr()?;

        while let Some((op, prec)) = self.get_binary_op_prec() {
            if prec < min_prec {
                break;
            }

            self.advance(); // consume operator
            let right = self.parse_binary_expr(prec + 1)?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }

        Ok(left)
    }

    /// Returns the precedence of the binary operator at the current token.
    ///
    /// # Returns
    ///
    /// * `Some((op, prec))` - The binary operator and its precedence level.
    /// * `None` - No binary operator at the current token.
    pub(crate) fn get_binary_op_prec(&self) -> Option<(BinaryOp, u8)> {
        match self.peek() {
            TokenKind::PipePipe => Some((BinaryOp::Or, 1)),
            TokenKind::AmpAmp => Some((BinaryOp::And, 2)),
            TokenKind::Pipe => Some((BinaryOp::BitOr, 3)),
            TokenKind::Caret => Some((BinaryOp::BitXor, 4)),
            TokenKind::Amp => Some((BinaryOp::BitAnd, 5)),
            TokenKind::EqEq => Some((BinaryOp::Eq, 6)),
            TokenKind::NotEq => Some((BinaryOp::Ne, 6)),
            TokenKind::Lt => Some((BinaryOp::Lt, 7)),
            TokenKind::LtEq => Some((BinaryOp::Le, 7)),
            TokenKind::Gt if self.allow_gt => Some((BinaryOp::Gt, 7)),
            TokenKind::GtEq if self.allow_gt => Some((BinaryOp::Ge, 7)),
            TokenKind::Shl => Some((BinaryOp::Shl, 8)),
            TokenKind::Shr => Some((BinaryOp::Shr, 8)),
            TokenKind::Plus => Some((BinaryOp::Add, 9)),
            TokenKind::Minus => Some((BinaryOp::Sub, 9)),
            TokenKind::Star => Some((BinaryOp::Mul, 10)),
            TokenKind::Slash => Some((BinaryOp::Div, 10)),
            TokenKind::Percent => Some((BinaryOp::Mod, 10)),
            _ => None,
        }
    }

    /// Parses a unary expression.
    ///
    /// # Returns
    ///
    /// * `Ok(expr)` - The parsed unary expression.
    /// * `Err(err)` - The parse error.
    pub(crate) fn parse_unary_expr(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.current_span();
        match self.peek() {
            TokenKind::Bang => {
                self.advance();
                let expr = self.parse_unary_expr()?;
                let span = start_span.merge(expr.span);
                Ok(Expr::new(
                    ExprKind::Unary {
                        op: UnaryOp::Not,
                        expr: Box::new(expr),
                    },
                    span,
                ))
            }
            TokenKind::Minus => {
                self.advance();
                let expr = self.parse_unary_expr()?;
                let span = start_span.merge(expr.span);
                Ok(Expr::new(
                    ExprKind::Unary {
                        op: UnaryOp::Neg,
                        expr: Box::new(expr),
                    },
                    span,
                ))
            }
            TokenKind::Tilde => {
                self.advance();
                let expr = self.parse_unary_expr()?;
                let span = start_span.merge(expr.span);
                Ok(Expr::new(
                    ExprKind::Unary {
                        op: UnaryOp::BitNot,
                        expr: Box::new(expr),
                    },
                    span,
                ))
            }
            _ => self.parse_postfix_expr(),
        }
    }

    /// Checks if current `<` is the start of generic call
    /// arguments: `<...> (`
    ///
    /// Returns `true` if the current token is `<` and the
    /// next token is `(`, indicating generic call arguments.
    pub(crate) fn is_generic_call_args(&self) -> bool {
        if !self.check(&TokenKind::Lt) {
            return false;
        }

        let mut depth = 0;
        let mut offset = 0;
        loop {
            match self.peek_ahead(offset) {
                TokenKind::Lt => depth += 1,
                TokenKind::Gt => {
                    depth -= 1;
                    if depth == 0 {
                        // Check if token immediately following '>' is '('
                        return matches!(self.peek_ahead(offset + 1), TokenKind::LParen);
                    }
                }
                TokenKind::Eof | TokenKind::Semicolon | TokenKind::LBrace | TokenKind::RBrace => {
                    return false;
                }
                _ => {}
            }
            offset += 1;
        }
    }

    /// Parses a postfix expression.
    ///
    /// # Returns
    ///
    /// * `Ok(expr)` - The parsed postfix expression.
    /// * `Err(err)` - The parse error.
    ///
    /// Can be followed by a dot `.` for field access,
    /// or an index `[]` for array access,
    /// or a call `()` for function invocation.
    /// Calls can be prefixed with a `<...>` for type parameters.
    pub(crate) fn parse_postfix_expr(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary_expr()?;

        loop {
            match self.peek() {
                TokenKind::Dot => {
                    self.advance(); // consume '.'
                    let field = self.expect_ident()?;
                    let span = expr.span.merge(field.span);
                    expr = Expr::new(
                        ExprKind::FieldAccess {
                            base: Box::new(expr),
                            field,
                        },
                        span,
                    );
                }
                TokenKind::LBracket => {
                    self.advance(); // consume '['
                    let index = self.parse_expr()?;
                    let end_tok = self.expect(&TokenKind::RBracket)?;
                    let span = expr.span.merge(end_tok.span);
                    expr = Expr::new(
                        ExprKind::Index {
                            base: Box::new(expr),
                            index: Box::new(index),
                        },
                        span,
                    );
                }
                TokenKind::Lt if self.is_generic_call_args() => {
                    // Generic call: foo<T>(...)
                    self.advance(); // consume '<'
                    let mut generic_args = Vec::new();
                    while !self.check(&TokenKind::Gt) && !self.is_at_end() {
                        generic_args.push(self.parse_type_arg()?);
                        if !self.match_token(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(&TokenKind::Gt)?;

                    // Now parse '(' args ')'
                    self.expect(&TokenKind::LParen)?;
                    let args = self.parse_call_args()?;
                    let end_tok = self.expect(&TokenKind::RParen)?;
                    let span = expr.span.merge(end_tok.span);

                    expr = Expr::new(
                        ExprKind::Call {
                            callee: Box::new(expr),
                            generic_args,
                            args,
                        },
                        span,
                    );
                }
                TokenKind::LParen => {
                    // Regular call: foo(...)
                    self.advance(); // consume '('
                    let args = self.parse_call_args()?;
                    let end_tok = self.expect(&TokenKind::RParen)?;
                    let span = expr.span.merge(end_tok.span);

                    expr = Expr::new(
                        ExprKind::Call {
                            callee: Box::new(expr),
                            generic_args: Vec::new(),
                            args,
                        },
                        span,
                    );
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    /// Parses a list of call arguments.
    ///
    /// # Returns
    ///
    /// * `Ok(args)` - The parsed list of call arguments.
    /// * `Err(err)` - The parse error.
    ///
    /// As specified in the AST module, call arguments may be
    /// named or unnamed, and must be separated by commas.
    pub(crate) fn parse_call_args(&mut self) -> Result<Vec<CallArg>, ParseError> {
        let mut args = Vec::new();

        while !self.check(&TokenKind::RParen) && !self.is_at_end() {
            let start_span = self.current_span();

            // Check if this is a named argument: `name: value`
            let (name, value) = if matches!(self.peek(), TokenKind::Ident(_))
                && matches!(self.peek_ahead(1), TokenKind::Colon)
            {
                let ident = self.expect_ident()?;
                self.expect(&TokenKind::Colon)?;
                let val = self.parse_expr()?;
                (Some(ident), val)
            } else {
                let val = self.parse_expr()?;
                (None, val)
            };

            let span = start_span.merge(value.span);
            args.push(CallArg { name, value, span });

            if !self.match_token(&TokenKind::Comma) {
                break;
            }
        }

        Ok(args)
    }

    /// Parses a primary expression.
    ///
    /// # Returns
    ///
    /// * `Ok(expr)` - The parsed primary expression.
    /// * `Err(err)` - The parse error.
    ///
    /// Primary expressions are literals, identifiers,
    /// or expressions in parentheses.
    pub(crate) fn parse_primary_expr(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        match self.peek().clone() {
            TokenKind::IntLiteral(val) => {
                self.advance();
                Ok(Expr::new(ExprKind::Literal(Literal::Integer(val)), span))
            }
            TokenKind::BoolLiteral(val) => {
                self.advance();
                Ok(Expr::new(ExprKind::Literal(Literal::Bool(val)), span))
            }
            TokenKind::StringLiteral(val) => {
                self.advance();
                Ok(Expr::new(ExprKind::Literal(Literal::String(val)), span))
            }
            TokenKind::Ident(_) => {
                let first_ident = self.expect_ident()?;
                let (path, is_qualified) = if self.check(&TokenKind::ColonColon) {
                    let mut segments = vec![first_ident.clone()];
                    let mut end_span = first_ident.span;
                    while self.match_token(&TokenKind::ColonColon) {
                        let seg = self.expect_ident()?;
                        end_span = seg.span;
                        segments.push(seg);
                    }
                    (Path::new(segments, first_ident.span.merge(end_span)), true)
                } else {
                    (Path::from_ident(first_ident.clone()), false)
                };

                // Check if this is a struct initialization: `Path { field: val, ... }`
                if self.check(&TokenKind::LBrace)
                    && (self.peek_ahead(1) == &TokenKind::RBrace
                        || (matches!(self.peek_ahead(1), TokenKind::Ident(_))
                            && self.peek_ahead(2) == &TokenKind::Colon))
                {
                    self.expect(&TokenKind::LBrace)?;
                    let mut fields = Vec::new();
                    while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
                        let f_start = self.current_span();
                        let f_name = self.expect_ident()?;
                        self.expect(&TokenKind::Colon)?;
                        let f_val = self.parse_expr()?;
                        let f_span = f_start.merge(f_val.span);
                        fields.push(FieldInit {
                            name: f_name,
                            value: f_val,
                            span: f_span,
                        });
                        if !self.match_token(&TokenKind::Comma) && !self.check(&TokenKind::RBrace) {
                            self.skip_semicolons();
                        }
                    }
                    let end_tok = self.expect(&TokenKind::RBrace)?;
                    let full_span = path.span.merge(end_tok.span);
                    Ok(Expr::new(
                        ExprKind::StructInit {
                            path,
                            fields,
                        },
                        full_span,
                    ))
                } else if is_qualified {
                    let span = path.span;
                    Ok(Expr::new(ExprKind::Path(path), span))
                } else {
                    let span = first_ident.span;
                    Ok(Expr::new(ExprKind::Ident(first_ident), span))
                }
            }
            TokenKind::LParen => {
                self.advance(); // consume '('
                let inner = self.parse_expr()?;
                let end_tok = self.expect(&TokenKind::RParen)?;
                let full_span = span.merge(end_tok.span);
                Ok(Expr::new(inner.kind, full_span))
            }
            other => Err(ParseError::new(
                format!("Expected expression, found {}", other),
                span,
            )),
        }
    }
}
