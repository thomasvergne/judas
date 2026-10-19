use crate::error::ParseError;
use crate::parser::Parser;
use crate::token::TokenKind;
use judas_ast::types::{Type, TypeArg, TypeKind};

impl Parser {
    /// Parses a hardware type (e.g. `Clock`, `Bool`, `UInt<8>`,
    /// `Array<Bit, 16>`, `Named<T>`).
    pub fn parse_type(&mut self) -> Result<Type, ParseError> {
        let start_span = self.current_span();
        let ident = self.expect_ident()?;

        match ident.name.as_str() {
            "Clock" => Ok(Type::new(TypeKind::Clock, ident.span)),
            "Bool" => Ok(Type::new(TypeKind::Bool, ident.span)),
            "Bit" => Ok(Type::new(TypeKind::Bit, ident.span)),
            "UInt" => {
                self.expect(&TokenKind::Lt)?;
                let arg = self.parse_type_arg()?;
                let end_tok = self.expect(&TokenKind::Gt)?;
                let span = start_span.merge(end_tok.span);
                Ok(Type::new(TypeKind::UInt(Box::new(arg)), span))
            }
            "Int" => {
                self.expect(&TokenKind::Lt)?;
                let arg = self.parse_type_arg()?;
                let end_tok = self.expect(&TokenKind::Gt)?;
                let span = start_span.merge(end_tok.span);
                Ok(Type::new(TypeKind::Int(Box::new(arg)), span))
            }
            "Bits" => {
                self.expect(&TokenKind::Lt)?;
                let arg = self.parse_type_arg()?;
                let end_tok = self.expect(&TokenKind::Gt)?;
                let span = start_span.merge(end_tok.span);
                Ok(Type::new(TypeKind::Bits(Box::new(arg)), span))
            }
            "Array" => {
                self.expect(&TokenKind::Lt)?;
                let elem = self.parse_type()?;
                self.expect(&TokenKind::Comma)?;
                let size = self.parse_type_arg()?;
                let end_tok = self.expect(&TokenKind::Gt)?;
                let span = start_span.merge(end_tok.span);
                Ok(Type::new(
                    TypeKind::Array {
                        element: Box::new(elem),
                        size: Box::new(size),
                    },
                    span,
                ))
            }
            _ => {
                // User-defined / Generic type: Name or Name<Arg1, Arg2>
                if self.check(&TokenKind::Lt) {
                    self.advance();
                    let mut args = Vec::new();
                    while !self.check(&TokenKind::Gt) && !self.is_at_end() {
                        args.push(self.parse_type_arg()?);
                        if !self.match_token(&TokenKind::Comma) {
                            break;
                        }
                    }
                    let end_tok = self.expect(&TokenKind::Gt)?;
                    let span = start_span.merge(end_tok.span);
                    Ok(Type::new(TypeKind::Named { name: ident, args }, span))
                } else {
                    let span = ident.span;
                    Ok(Type::new(
                        TypeKind::Named {
                            name: ident,
                            args: Vec::new(),
                        },
                        span,
                    ))
                }
            }
        }
    }

    /// Parses a type argument (e.g. `T`, `UInt<8>`, `Array<Bit, 16>`).
    ///
    /// Returns an error if the type argument is not valid.
    ///
    /// # Examples
    ///
    /// ```text
    /// T
    /// ```
    ///
    /// ```text
    /// UInt<8>
    /// ```
    pub(crate) fn parse_type_arg(&mut self) -> Result<TypeArg, ParseError> {
        let prev_allow_gt = self.allow_gt;
        self.allow_gt = false;
        let res = self.parse_type_arg_inner();
        self.allow_gt = prev_allow_gt;
        res
    }

    /// Parses a type argument (e.g. `T`, `UInt<8>`, `Array<Bit, 16>`).
    ///
    /// Returns an error if the type argument is not valid.
    pub(crate) fn parse_type_arg_inner(&mut self) -> Result<TypeArg, ParseError> {
        match self.peek() {
            TokenKind::Ident(name) => {
                if matches!(
                    name.as_str(),
                    "Clock" | "Bool" | "Bit" | "UInt" | "Int" | "Bits" | "Array"
                ) {
                    let ty = self.parse_type()?;
                    Ok(TypeArg::Type(ty))
                } else if self.peek_ahead(1) == &TokenKind::Lt && !self.is_generic_call_args() {
                    let ident = self.expect_ident()?;
                    self.advance(); // consume '<'
                    let mut args = Vec::new();
                    while !self.check(&TokenKind::Gt) && !self.is_at_end() {
                        args.push(self.parse_type_arg()?);
                        if !self.match_token(&TokenKind::Comma) {
                            break;
                        }
                    }
                    let end_tok = self.expect(&TokenKind::Gt)?;
                    let span = ident.span.merge(end_tok.span);
                    Ok(TypeArg::Type(Type::new(
                        TypeKind::Named { name: ident, args },
                        span,
                    )))
                } else {
                    let expr = self.parse_expr()?;
                    Ok(TypeArg::Expr(expr))
                }
            }
            _ => {
                let expr = self.parse_expr()?;
                Ok(TypeArg::Expr(expr))
            }
        }
    }
}
