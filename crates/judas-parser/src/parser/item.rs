use crate::error::ParseError;
use crate::parser::Parser;
use crate::token::TokenKind;
use judas_ast::item::{
    AssertDecl, ConnectionDecl, EnumDecl, EnumVariant, GenericParam, GenericParamKind,
    ImportDecl, ImportItem, ImportKind, ImportSource, InterfaceDecl, InterfacePortDecl,
    InterfaceRole, InterfaceSignal, InterfaceSignalDir, Item, ItemKind, LetDecl, ModuleDecl,
    ModuleItem, ModuleItemKind, ModuleParam, NamespaceDecl, OnBlock, PortDecl, PortDirection,
    RegisterDecl, SourceFile, StructDecl, StructField,
};
use judas_ast::path::{Path, Visibility};
use judas_ast::stmt::{EdgeKind, Trigger};

impl Parser {
    /// Parses a qualified path (e.g. `foo`, `math::Adder`, `a::b::c`).
    pub fn parse_path(&mut self) -> Result<Path, ParseError> {
        let first = self.expect_ident()?;
        let mut segments = vec![first.clone()];
        let mut end_span = first.span;
        while self.check(&TokenKind::ColonColon) && matches!(self.peek_ahead(1), TokenKind::Ident(_)) {
            self.advance(); // consume ::
            let seg = self.expect_ident()?;
            end_span = seg.span;
            segments.push(seg);
        }
        let span = first.span.merge(end_span);
        Ok(Path::new(segments, span))
    }

    /// Parses an entire source file consisting of module,
    /// enum, struct, and interface items.
    pub fn parse_source_file(&mut self) -> Result<SourceFile, ParseError> {
        let start_span = self.current_span();
        let mut items = Vec::new();

        self.skip_semicolons();
        while !self.is_at_end() {
            items.push(self.parse_item()?);
            self.skip_semicolons();
        }

        let end_span = self.current_span();
        Ok(SourceFile {
            items,
            span: start_span.merge(end_span),
        })
    }

    /// Parses a top-level item (`module`, `enum`, `struct`, `interface`, `namespace`, `import`, or `export`).
    pub fn parse_item(&mut self) -> Result<Item, ParseError> {
        let is_pub = self.match_token(&TokenKind::Pub);
        let visibility = if is_pub {
            Visibility::Public
        } else {
            Visibility::Private
        };

        match self.peek() {
            TokenKind::Namespace => self.parse_namespace_decl(visibility).map(|ns| {
                let span = ns.span;
                Item::new(ItemKind::Namespace(ns), span)
            }),
            TokenKind::Import => self.parse_import_decl(is_pub).map(|imp| {
                let span = imp.span;
                Item::new(ItemKind::Import(imp), span)
            }),
            TokenKind::Extern | TokenKind::Module => self.parse_module_decl(visibility).map(|m| {
                let span = m.span;
                Item::new(ItemKind::Module(m), span)
            }),
            TokenKind::Enum => self.parse_enum_decl(visibility).map(|e| {
                let span = e.span;
                Item::new(ItemKind::Enum(e), span)
            }),
            TokenKind::Struct => self.parse_struct_decl(visibility).map(|s| {
                let span = s.span;
                Item::new(ItemKind::Struct(s), span)
            }),
            TokenKind::Interface => self.parse_interface_decl(visibility).map(|i| {
                let span = i.span;
                Item::new(ItemKind::Interface(i), span)
            }),
            other => Err(ParseError::new(
                format!(
                    "Expected `module`, `extern module`, `enum`, `struct`, `interface`, `namespace`, `import`, or `export`, found {}",
                    other
                ),
                self.current_span(),
            )),
        }
    }

    /// Parses a packed struct declaration.
    ///
    /// # Returns
    ///
    /// * `Ok(struct)` - The parsed struct declaration.
    /// * `Err(err)` - The parse error.
    ///
    /// # Example
    ///
    /// ```text
    /// struct Point {
    ///     x: UInt<8>,
    ///     y: UInt<8>,
    /// }
    /// ```
    pub fn parse_struct_decl(&mut self, visibility: Visibility) -> Result<StructDecl, ParseError> {
        let start_tok = self.expect(&TokenKind::Struct)?;
        let name = self.expect_ident()?;
        self.expect(&TokenKind::LBrace)?;

        let mut fields = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let f_start = self.current_span();
            let f_name = self.expect_ident()?;
            self.expect(&TokenKind::Colon)?;
            let f_ty = self.parse_type()?;
            let f_span = f_start.merge(f_ty.span);
            fields.push(StructField {
                name: f_name,
                ty: f_ty,
                span: f_span,
            });

            if !self.match_token(&TokenKind::Comma) && !self.check(&TokenKind::RBrace) {
                self.skip_semicolons();
            }
        }

        let end_tok = self.expect(&TokenKind::RBrace)?;
        Ok(StructDecl {
            name,
            fields,
            visibility,
            span: start_tok.span.merge(end_tok.span),
        })
    }

    /// Parses an interface declaration.
    ///
    /// # Returns
    ///
    /// * `Ok(interface)` - The parsed interface declaration.
    /// * `Err(err)` - The parse error.
    ///
    /// # Example
    ///
    /// ```text
    /// interface Test {
    ///     in field1: Input,
    ///     out field2: Output,
    /// }
    /// ```
    pub fn parse_interface_decl(&mut self, visibility: Visibility) -> Result<InterfaceDecl, ParseError> {
        let start_tok = self.expect(&TokenKind::Interface)?;
        let name = self.expect_ident()?;
        self.expect(&TokenKind::LBrace)?;

        let mut signals = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let sig_start = self.current_span();
            let dir = match self.peek() {
                TokenKind::Input => {
                    self.advance();
                    InterfaceSignalDir::In
                }
                TokenKind::Output => {
                    self.advance();
                    InterfaceSignalDir::Out
                }
                TokenKind::Ident(s) if s == "in" => {
                    self.advance();
                    InterfaceSignalDir::In
                }
                TokenKind::Ident(s) if s == "out" => {
                    self.advance();
                    InterfaceSignalDir::Out
                }
                other => {
                    return Err(ParseError::new(
                        format!(
                            "Expected `in`, `input`, `out`, or `output`, found {}",
                            other
                        ),
                        sig_start,
                    ));
                }
            };
            let sig_name = self.expect_ident()?;
            self.expect(&TokenKind::Colon)?;
            let sig_ty = self.parse_type()?;
            let sig_span = sig_start.merge(sig_ty.span);
            signals.push(InterfaceSignal {
                dir,
                name: sig_name,
                ty: sig_ty,
                span: sig_span,
            });

            if !self.match_token(&TokenKind::Comma) && !self.check(&TokenKind::RBrace) {
                self.skip_semicolons();
            }
        }

        let end_tok = self.expect(&TokenKind::RBrace)?;
        Ok(InterfaceDecl {
            name,
            signals,
            visibility,
            span: start_tok.span.merge(end_tok.span),
        })
    }

    /// Parses an enum declaration.
    ///
    /// # Returns
    ///
    /// * `Ok(enum)` - The parsed enum declaration.
    /// * `Err(err)` - The parse error.
    ///
    /// # Example
    ///
    /// ```text
    /// enum Test {
    ///     Variant1,
    ///     Variant2 = 0b10,
    ///     Variant3
    /// }
    /// ```
    pub fn parse_enum_decl(&mut self, visibility: Visibility) -> Result<EnumDecl, ParseError> {
        let start_tok = self.expect(&TokenKind::Enum)?;
        let name = self.expect_ident()?;
        self.expect(&TokenKind::LBrace)?;

        let mut variants = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let v_name = self.expect_ident()?;
            let mut v_span = v_name.span;
            let value = if self.match_token(&TokenKind::Eq) {
                let val_expr = self.parse_expr()?;
                v_span = v_span.merge(val_expr.span);
                Some(val_expr)
            } else {
                None
            };
            variants.push(EnumVariant {
                name: v_name,
                value,
                span: v_span,
            });
            if !self.match_token(&TokenKind::Comma) && !self.check(&TokenKind::RBrace) {
                self.skip_semicolons();
            }
        }

        let end_tok = self.expect(&TokenKind::RBrace)?;
        Ok(EnumDecl {
            name,
            variants,
            visibility,
            span: start_tok.span.merge(end_tok.span),
        })
    }

    /// Parses a module declaration.
    ///
    /// # Returns
    ///
    /// * `Ok(module)` - The parsed module declaration.
    /// * `Err(err)` - The parse error.
    ///
    /// # Example
    ///
    /// ```text
    /// module Test {
    ///     input clk: Clock
    ///     input field1: Input
    ///     output field2: Output
    ///
    ///     on rising(clk) { ... }
    /// }
    /// ```
    pub fn parse_module_decl(&mut self, visibility: Visibility) -> Result<ModuleDecl, ParseError> {
        let (is_extern, foreign_path, start_span) = if self.check(&TokenKind::Extern) {
            let extern_tok = self.advance();
            let mut path = None;
            if let TokenKind::StringLiteral(s) = self.peek() {
                path = Some(s.clone());
                self.advance();
            }
            self.expect(&TokenKind::Module)?;
            if path.is_none()
                && let TokenKind::StringLiteral(s) = self.peek()
            {
                path = Some(s.clone());
                self.advance();
            }
            (true, path, extern_tok.span)
        } else {
            let start = self.expect(&TokenKind::Module)?.span;
            (false, None, start)
        };

        let name = self.expect_ident()?;

        // Optional generics: <T, width: usize>
        let generics = if self.check(&TokenKind::Lt) {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };

        // Optional module params: (width: usize = 8)
        let params = if self.check(&TokenKind::LParen) {
            self.parse_module_params()?
        } else {
            Vec::new()
        };

        self.expect(&TokenKind::LBrace)?;

        let mut items = Vec::new();
        self.skip_semicolons();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            items.push(self.parse_module_item()?);
            self.match_token(&TokenKind::Comma);
            self.skip_semicolons();
        }

        let end_tok = self.expect(&TokenKind::RBrace)?;

        Ok(ModuleDecl {
            name,
            generics,
            params,
            items,
            is_extern,
            foreign_path,
            visibility,
            span: start_span.merge(end_tok.span),
        })
    }

    /// Parses generic parameters.
    ///
    /// # Returns
    ///
    /// * `Ok(params)` - The parsed generic parameters.
    /// * `Err(err)` - The parse error.
    ///
    /// # Example
    ///
    /// ```text
    /// struct Test<T, N: usize> { ... }
    /// ```
    pub(crate) fn parse_generic_params(&mut self) -> Result<Vec<GenericParam>, ParseError> {
        self.expect(&TokenKind::Lt)?;
        let prev_allow_gt = self.allow_gt;
        self.allow_gt = false;
        let mut params = Vec::new();

        while !self.check(&TokenKind::Gt) && !self.is_at_end() {
            let start_span = self.current_span();
            let name = self.expect_ident()?;

            if self.match_token(&TokenKind::Colon) {
                // Const generic param: width: usize [= default]
                let ty = self.parse_type()?;
                let default = if self.match_token(&TokenKind::Eq) {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                let end_span = default.as_ref().map(|e| e.span).unwrap_or_else(|| ty.span);
                params.push(GenericParam {
                    kind: GenericParamKind::Const {
                        name,
                        ty,
                        default: default.map(Box::new),
                    },
                    span: start_span.merge(end_span),
                });
            } else {
                // Type param: T
                let span = name.span;
                params.push(GenericParam {
                    kind: GenericParamKind::Type { name },
                    span,
                });
            }

            if !self.match_token(&TokenKind::Comma) {
                break;
            }
        }

        let res = self.expect(&TokenKind::Gt);
        self.allow_gt = prev_allow_gt;
        res?;
        Ok(params)
    }

    /// Parses module parameters.
    ///
    /// # Returns
    ///
    /// * `Ok(params)` - The parsed module parameters.
    /// * `Err(err)` - The parse error.
    ///
    /// # Example
    ///
    /// ```text
    /// module Test(N: usize = 8, T) { ... }
    /// ```
    pub(crate) fn parse_module_params(&mut self) -> Result<Vec<ModuleParam>, ParseError> {
        self.expect(&TokenKind::LParen)?;
        let mut params = Vec::new();

        while !self.check(&TokenKind::RParen) && !self.is_at_end() {
            let start_span = self.current_span();
            let name = self.expect_ident()?;
            self.expect(&TokenKind::Colon)?;
            let ty = self.parse_type()?;

            let default = if self.match_token(&TokenKind::Eq) {
                Some(self.parse_expr()?)
            } else {
                None
            };

            let end_span = default.as_ref().map(|e| e.span).unwrap_or_else(|| ty.span);

            params.push(ModuleParam {
                name,
                ty,
                default,
                span: start_span.merge(end_span),
            });

            if !self.match_token(&TokenKind::Comma) {
                break;
            }
        }

        self.expect(&TokenKind::RParen)?;
        Ok(params)
    }

    /// Parses a module item.
    ///
    /// # Returns
    ///
    /// * `Ok(item)` - The parsed module item.
    /// * `Err(err)` - The parse error.
    pub fn parse_module_item(&mut self) -> Result<ModuleItem, ParseError> {
        match self.peek() {
            TokenKind::Input => self.parse_port_decl(PortDirection::Input),
            TokenKind::Output => self.parse_port_decl(PortDirection::Output),
            TokenKind::Register => self.parse_register_decl(),
            TokenKind::Let => self.parse_let_decl(),
            TokenKind::Connect => self.parse_connect_decl(),
            TokenKind::On => self.parse_on_block(),
            TokenKind::Assert => self.parse_assert_module_item(),
            TokenKind::Master => self.parse_interface_port_decl(InterfaceRole::Master),
            TokenKind::Slave => self.parse_interface_port_decl(InterfaceRole::Slave),
            _ => {
                // Combinational connection: target = value
                self.parse_implicit_connect_decl()
            }
        }
    }

    /// Parses a module item.
    ///
    /// # Returns
    ///
    /// * `Ok(item)` - The parsed module item.
    /// * `Err(err)` - The parse error.
    pub(crate) fn parse_interface_port_decl(
        &mut self,
        role: InterfaceRole,
    ) -> Result<ModuleItem, ParseError> {
        let start_tok = self.advance(); // master or slave
        let name = self.expect_ident()?;
        self.expect(&TokenKind::Colon)?;
        let interface_name = self.parse_path()?;
        let span = start_tok.span.merge(interface_name.span);
        Ok(ModuleItem::new(
            ModuleItemKind::InterfacePort(InterfacePortDecl {
                role,
                name,
                interface_name,
                span,
            }),
            span,
        ))
    }

    /// Parses an assert declaration.
    ///
    /// # Returns
    ///
    /// * `Ok(assert)` - The parsed assert declaration.
    /// * `Err(err)` - The parse error.
    ///
    /// # Example
    ///
    /// ```text
    /// assert !enabled, "Circuit mustn't be enabled"
    /// ```
    pub(crate) fn parse_assert_decl(&mut self) -> Result<AssertDecl, ParseError> {
        let start_tok = self.expect(&TokenKind::Assert)?;
        let condition = self.parse_expr()?;
        let (message, end_span) = if self.match_token(&TokenKind::Comma) {
            let msg_span = self.current_span();
            match self.advance().kind {
                TokenKind::StringLiteral(s) => (Some(s), msg_span),
                other => {
                    return Err(ParseError::new(
                        format!(
                            "Expected string literal after `,` in assert, found {}",
                            other
                        ),
                        msg_span,
                    ));
                }
            }
        } else {
            (None, condition.span)
        };
        let span = start_tok.span.merge(end_span);
        Ok(AssertDecl {
            condition,
            message,
            span,
        })
    }

    /// Parses an assert module item.
    ///
    /// # Returns
    ///
    /// * `Ok(item)` - The parsed assert module item.
    /// * `Err(err)` - The parse error.
    pub(crate) fn parse_assert_module_item(&mut self) -> Result<ModuleItem, ParseError> {
        let decl = self.parse_assert_decl()?;
        let span = decl.span;
        Ok(ModuleItem::new(ModuleItemKind::Assert(decl), span))
    }

    /// Parses a port declaration.
    ///
    /// # Returns
    ///
    /// * `Ok(item)` - The parsed port declaration.
    /// * `Err(err)` - The parse error.
    ///
    /// # Example
    ///
    /// ```text
    /// input test: Bits<8>
    /// output test: Bits<8>
    /// ```
    pub(crate) fn parse_port_decl(
        &mut self,
        direction: PortDirection,
    ) -> Result<ModuleItem, ParseError> {
        let start_tok = self.advance(); // input or output
        let name = self.expect_ident()?;
        self.expect(&TokenKind::Colon)?;
        let ty = self.parse_type()?;

        let initializer = if self.match_token(&TokenKind::Eq) {
            Some(self.parse_expr()?)
        } else {
            None
        };

        let end_span = initializer
            .as_ref()
            .map(|e| e.span)
            .unwrap_or_else(|| ty.span);
        let span = start_tok.span.merge(end_span);

        Ok(ModuleItem::new(
            ModuleItemKind::Port(PortDecl {
                direction,
                name,
                ty,
                initializer,
                span,
            }),
            span,
        ))
    }

    /// Parses a register declaration.
    ///
    /// # Returns
    ///
    /// * `Ok(item)` - The parsed register declaration.
    /// * `Err(err)` - The parse error.
    ///
    /// # Example
    ///
    /// ```text
    /// register test: Bits<8> = 0
    /// ```
    pub(crate) fn parse_register_decl(&mut self) -> Result<ModuleItem, ParseError> {
        let start_tok = self.expect(&TokenKind::Register)?;
        let name = self.expect_ident()?;
        self.expect(&TokenKind::Colon)?;
        let ty = self.parse_type()?;
        self.expect(&TokenKind::Eq)?;
        let reset_value = self.parse_expr()?;

        let span = start_tok.span.merge(reset_value.span);
        Ok(ModuleItem::new(
            ModuleItemKind::Register(RegisterDecl {
                name,
                ty,
                reset_value,
                span,
            }),
            span,
        ))
    }

    /// Parses a let declaration.
    ///
    /// # Returns
    ///
    /// * `Ok(item)` - The parsed let declaration.
    /// * `Err(err)` - The parse error.
    ///
    /// # Example
    ///
    /// ```text
    /// let test: Bits<8> = 0
    /// ```
    pub(crate) fn parse_let_decl(&mut self) -> Result<ModuleItem, ParseError> {
        let start_tok = self.expect(&TokenKind::Let)?;
        let name = self.expect_ident()?;

        let ty = if self.match_token(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(&TokenKind::Eq)?;
        let value = self.parse_expr()?;

        let span = start_tok.span.merge(value.span);
        Ok(ModuleItem::new(
            ModuleItemKind::Let(LetDecl {
                name,
                ty,
                value,
                span,
            }),
            span,
        ))
    }

    /// Parses a connect declaration.
    ///
    /// # Returns
    ///
    /// * `Ok(item)` - The parsed connect declaration.
    /// * `Err(err)` - The parse error.
    ///
    /// # Example
    ///
    /// ```text
    /// connect i2c.sda = sda
    /// ```
    pub(crate) fn parse_connect_decl(&mut self) -> Result<ModuleItem, ParseError> {
        let start_tok = self.expect(&TokenKind::Connect)?;
        let target = self.parse_expr()?;
        self.expect(&TokenKind::Eq)?;
        let value = self.parse_expr()?;

        let span = start_tok.span.merge(value.span);
        Ok(ModuleItem::new(
            ModuleItemKind::Connection(ConnectionDecl {
                target,
                value,
                is_explicit_connect: true,
                span,
            }),
            span,
        ))
    }

    /// Parses an implicit connect declaration.
    ///
    /// # Returns
    ///
    /// * `Ok(item)` - The parsed implicit connect declaration.
    /// * `Err(err)` - The parse error.
    ///
    /// # Example
    ///
    /// ```text
    /// i2c.sda = sda
    /// ```
    pub(crate) fn parse_implicit_connect_decl(&mut self) -> Result<ModuleItem, ParseError> {
        let target = self.parse_expr()?;
        self.expect(&TokenKind::Eq)?;
        let value = self.parse_expr()?;

        let span = target.span.merge(value.span);
        Ok(ModuleItem::new(
            ModuleItemKind::Connection(ConnectionDecl {
                target,
                value,
                is_explicit_connect: false,
                span,
            }),
            span,
        ))
    }

    /// Parses an on block.
    ///
    /// # Returns
    ///
    /// * `Ok(item)` - The parsed on block.
    /// * `Err(err)` - The parse error.
    ///
    /// # Example
    ///
    /// ```text
    /// on rising(clk) { ... }
    /// ```
    pub(crate) fn parse_on_block(&mut self) -> Result<ModuleItem, ParseError> {
        let start_tok = self.expect(&TokenKind::On)?;
        let trigger = self.parse_trigger()?;
        self.expect(&TokenKind::LBrace)?;

        let mut body = Vec::new();
        self.skip_semicolons();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            body.push(self.parse_stmt()?);
            self.skip_semicolons();
        }

        let end_tok = self.expect(&TokenKind::RBrace)?;
        let span = start_tok.span.merge(end_tok.span);

        Ok(ModuleItem::new(
            ModuleItemKind::On(OnBlock {
                trigger,
                body,
                span,
            }),
            span,
        ))
    }

    /// Parses a trigger.
    ///
    /// # Returns
    ///
    /// * `Ok(trigger)` - The parsed trigger.
    /// * `Err(err)` - The parse error.
    ///
    /// # Example
    ///
    /// ```text
    /// rising(clk)
    /// ```
    pub(crate) fn parse_trigger(&mut self) -> Result<Trigger, ParseError> {
        let start_span = self.current_span();
        let edge = if self.match_token(&TokenKind::Rising) {
            EdgeKind::Rising
        } else if self.match_token(&TokenKind::Falling) {
            EdgeKind::Falling
        } else {
            return Err(ParseError::new(
                format!("Expected `rising` or `falling`, found {}", self.peek()),
                start_span,
            ));
        };

        self.expect(&TokenKind::LParen)?;
        let signal = self.parse_expr()?;
        let end_tok = self.expect(&TokenKind::RParen)?;

        let span = start_span.merge(end_tok.span);
        Ok(Trigger { edge, signal, span })
    }

    /// Parses a namespace declaration.
    pub fn parse_namespace_decl(
        &mut self,
        visibility: Visibility,
    ) -> Result<NamespaceDecl, ParseError> {
        let start_tok = self.expect(&TokenKind::Namespace)?;
        let name = self.expect_ident()?;
        self.expect(&TokenKind::LBrace)?;

        let mut items = Vec::new();
        self.skip_semicolons();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            items.push(self.parse_item()?);
            self.skip_semicolons();
        }

        let end_tok = self.expect(&TokenKind::RBrace)?;
        Ok(NamespaceDecl {
            name,
            items,
            visibility,
            span: start_tok.span.merge(end_tok.span),
        })
    }

    /// Parses an import declaration.
    pub fn parse_import_decl(&mut self, is_pub: bool) -> Result<ImportDecl, ParseError> {
        let start_tok = self.expect(&TokenKind::Import)?;

        let (source, _base_span) = if let TokenKind::StringLiteral(path_str) = self.peek() {
            let s = path_str.clone();
            let tok = self.advance();
            (ImportSource::File(s), tok.span)
        } else {
            let path = self.parse_path()?;
            let span = path.span;
            (ImportSource::Namespace(path), span)
        };

        let kind = if self.match_token(&TokenKind::ColonColon) {
            if self.match_token(&TokenKind::Star) {
                ImportKind::Wildcard
            } else if self.match_token(&TokenKind::LBrace) {
                let mut items = Vec::new();
                while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
                    let name = self.expect_ident()?;
                    let alias = if self.match_token(&TokenKind::As) {
                        Some(self.expect_ident()?)
                    } else {
                        None
                    };
                    let span = if let Some(ref a) = alias {
                        name.span.merge(a.span)
                    } else {
                        name.span
                    };
                    items.push(ImportItem { name, alias, span });
                    if !self.match_token(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RBrace)?;
                ImportKind::Multiple(items)
            } else {
                let name = self.expect_ident()?;
                let alias = if self.match_token(&TokenKind::As) {
                    Some(self.expect_ident()?)
                } else {
                    None
                };
                ImportKind::Item { name, alias }
            }
        } else if self.match_token(&TokenKind::As) {
            let alias = self.expect_ident()?;
            ImportKind::ModuleAlias(alias)
        } else {
            match &source {
                ImportSource::Namespace(p) => {
                    let name = p.last_ident().clone();
                    ImportKind::Item { name, alias: None }
                }
                ImportSource::File(_) => ImportKind::Wildcard,
            }
        };

        self.match_token(&TokenKind::Semicolon);
        let end_span = self.current_span();
        Ok(ImportDecl {
            source,
            kind,
            is_pub,
            span: start_tok.span.merge(end_span),
        })
    }
}
