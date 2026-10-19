use std::collections::HashMap;

use judas_ast::expr::Literal;
use judas_ast::item::{
    GenericParamKind, InterfaceRole, InterfaceSignalDir, ItemKind, ModuleDecl, ModuleItem,
    ModuleItemKind, PortDirection, SourceFile,
};

use crate::context::{InferContext, InterfaceSignature, ModuleSignature, StructSignature};
use crate::error::TypeError;
use crate::expr::infer_expr;
use crate::stmt::infer_stmt;
use crate::tast::{
    TypedAssertDecl, TypedConnectionDecl, TypedEnumDecl, TypedEnumVariant, TypedExpr,
    TypedExprKind, TypedInterfaceDecl, TypedInterfacePortDecl, TypedInterfaceSignal, TypedItem,
    TypedItemKind, TypedLetDecl, TypedModuleDecl, TypedModuleItem, TypedModuleItemKind,
    TypedOnBlock, TypedPortDecl, TypedRegisterDecl, TypedSourceFile, TypedStmt, TypedStmtKind,
    TypedStructDecl, TypedStructField, TypedTrigger,
};
use crate::types::{ResolvedType, WidthExpr};

/// Top-level entry point to typecheck an entire Judas source file.
///
/// Returns a [`TypedSourceFile`] with the typechecked items and
/// the original source file.
///
/// Typechecks each item in the source file and infers its type.
pub fn infer_source_file(source: &SourceFile) -> Result<TypedSourceFile, TypeError> {
    let mut ctx = InferContext::new();

    // Pass 1: Collect enum, struct, and interface declarations
    for item in &source.items {
        match &item.kind {
            ItemKind::Enum(enum_decl) => {
                let mut variants = Vec::new();
                for v in &enum_decl.variants {
                    variants.push(v.name.name.clone());
                    ctx.variant_to_enum
                        .insert(v.name.name.clone(), enum_decl.name.name.clone());
                }
                ctx.enums.insert(enum_decl.name.name.clone(), variants);
            }
            ItemKind::Struct(struct_decl) => {
                let mut fields = Vec::new();
                let mut total_width = WidthExpr::Const(0);
                for f in &struct_decl.fields {
                    let f_ty = ctx.resolve_type(&f.ty)?;
                    let w = f_ty.width_expr().ok_or_else(|| {
                        TypeError::new(
                            format!(
                                "Field `{}` in packed struct `{}` has type `{}` which does not have a statically known bit width",
                                f.name.name, struct_decl.name.name, f_ty
                            ),
                            f.span,
                        )
                    })?;
                    total_width = match (&total_width, &w) {
                        (WidthExpr::Const(c1), WidthExpr::Const(c2)) => WidthExpr::Const(c1 + c2),
                        _ => WidthExpr::Add(Box::new(total_width), Box::new(w)),
                    };
                    fields.push((f.name.name.clone(), f_ty));
                }
                ctx.struct_definitions.insert(
                    struct_decl.name.name.clone(),
                    StructSignature {
                        fields,
                        total_width,
                    },
                );
            }
            ItemKind::Interface(iface_decl) => {
                let mut signals = Vec::new();
                for s in &iface_decl.signals {
                    let s_ty = ctx.resolve_type(&s.ty)?;
                    signals.push((s.dir, s.name.name.clone(), s_ty));
                }
                ctx.interface_definitions
                    .insert(iface_decl.name.name.clone(), InterfaceSignature { signals });
            }
            _ => {}
        }
    }

    // Pass 2: Collect module signatures
    for item in &source.items {
        if let ItemKind::Module(module_decl) = &item.kind {
            let mut params = HashMap::new();
            for p in &module_decl.params {
                let ty = ctx.resolve_type(&p.ty)?;
                let default = if let Some(def_expr) = &p.default {
                    Some(ctx.eval_const_expr(def_expr)? as i64)
                } else {
                    None
                };
                params.insert(p.name.name.clone(), (ty, default));
            }

            let mut ports = HashMap::new();
            let mut interface_ports = HashMap::new();
            for m_item in &module_decl.items {
                match &m_item.kind {
                    ModuleItemKind::Port(port) => {
                        ports.insert(port.name.name.clone(), (port.direction, port.ty.clone()));
                    }
                    ModuleItemKind::InterfacePort(iface_port) => {
                        interface_ports.insert(
                            iface_port.name.name.clone(),
                            (iface_port.role, iface_port.interface_name.name.clone()),
                        );
                    }
                    _ => {}
                }
            }

            ctx.module_definitions.insert(
                module_decl.name.name.clone(),
                ModuleSignature {
                    params,
                    ports,
                    interface_ports,
                },
            );
        }
    }

    // Pass 3: Typecheck all items and construct TypedSourceFile
    let mut typed_items = Vec::new();
    for item in &source.items {
        match &item.kind {
            ItemKind::Enum(enum_decl) => {
                let mut typed_variants = Vec::new();
                let mut current_val = 0i128;
                for v in &enum_decl.variants {
                    let (typed_val, disc) = if let Some(val_expr) = &v.value {
                        let t_expr = infer_expr(&mut ctx, val_expr)?;
                        let disc_val = match &t_expr.kind {
                            TypedExprKind::Literal(Literal::Integer(i)) => *i,
                            _ => current_val,
                        };
                        current_val = disc_val + 1;
                        (Some(t_expr), disc_val)
                    } else {
                        let disc_val = current_val;
                        current_val += 1;
                        (None, disc_val)
                    };

                    typed_variants.push(TypedEnumVariant {
                        name: v.name.clone(),
                        value: typed_val,
                        discriminant: disc,
                        span: v.span,
                    });
                }

                typed_items.push(TypedItem::new(
                    TypedItemKind::Enum(TypedEnumDecl {
                        name: enum_decl.name.clone(),
                        variants: typed_variants,
                        span: enum_decl.span,
                    }),
                    item.span,
                ));
            }
            ItemKind::Struct(struct_decl) => {
                let sig = ctx.struct_definitions.get(&struct_decl.name.name).unwrap();
                let mut typed_fields = Vec::new();
                for (f_ast, (_, f_ty)) in struct_decl.fields.iter().zip(&sig.fields) {
                    typed_fields.push(TypedStructField {
                        name: f_ast.name.clone(),
                        ty: f_ty.clone(),
                        span: f_ast.span,
                    });
                }
                typed_items.push(TypedItem::new(
                    TypedItemKind::Struct(TypedStructDecl {
                        name: struct_decl.name.clone(),
                        fields: typed_fields,
                        total_width: sig.total_width.clone(),
                        span: struct_decl.span,
                    }),
                    item.span,
                ));
            }
            ItemKind::Interface(iface_decl) => {
                let sig = ctx
                    .interface_definitions
                    .get(&iface_decl.name.name)
                    .unwrap();
                let mut typed_signals = Vec::new();
                for (s_ast, (dir, _, s_ty)) in iface_decl.signals.iter().zip(&sig.signals) {
                    typed_signals.push(TypedInterfaceSignal {
                        dir: *dir,
                        name: s_ast.name.clone(),
                        ty: s_ty.clone(),
                        span: s_ast.span,
                    });
                }
                typed_items.push(TypedItem::new(
                    TypedItemKind::Interface(TypedInterfaceDecl {
                        name: iface_decl.name.clone(),
                        signals: typed_signals,
                        span: iface_decl.span,
                    }),
                    item.span,
                ));
            }
            ItemKind::Module(module_decl) => {
                let typed_module = infer_module_decl(&mut ctx, module_decl)?;
                typed_items.push(TypedItem::new(
                    TypedItemKind::Module(typed_module),
                    item.span,
                ));
            }
        }
    }

    // Pass 4: Final substitution pass to substitute all type variables
    let mut final_typed_source = TypedSourceFile {
        items: typed_items,
        span: source.span,
    };
    finalize_source_file(&ctx, &mut final_typed_source);

    Ok(final_typed_source)
}

/// Infers the type of a module declaration.
///
/// Returns a [`TypedModuleDecl`] with the inferred type and the original span.
///
/// Module type inference infers the type of each item in the module and
/// returns a [`TypedModuleDecl`] with the inferred type and the original span.
fn infer_module_decl(
    ctx: &mut InferContext,
    module: &ModuleDecl,
) -> Result<TypedModuleDecl, TypeError> {
    ctx.current_constants.clear();
    ctx.current_signals.clear();
    ctx.current_instances.clear();

    // Populate constants from module parameters / generics
    for param in &module.params {
        if let Some(def_expr) = &param.default {
            let val = ctx.eval_const_expr(def_expr)?;
            ctx.current_constants
                .insert(param.name.name.clone(), val as i64);
        } else {
            ctx.current_signals
                .insert(param.name.name.clone(), ResolvedType::uint(32));
        }
    }
    for generic in &module.generics {
        if let GenericParamKind::Const { name, default, .. } = &generic.kind {
            if let Some(def_expr) = default {
                let val = ctx.eval_const_expr(def_expr)?;
                ctx.current_constants.insert(name.name.clone(), val as i64);
            } else {
                ctx.current_signals
                    .insert(name.name.clone(), ResolvedType::uint(32));
            }
        }
    }

    // Pre-declare ports, registers, and interface ports in current scope
    for item in &module.items {
        match &item.kind {
            ModuleItemKind::Port(port) => {
                let ty = ctx.resolve_type(&port.ty)?;
                ctx.current_signals.insert(port.name.name.clone(), ty);
            }
            ModuleItemKind::Register(reg) => {
                let ty = ctx.resolve_type(&reg.ty)?;
                ctx.current_signals.insert(reg.name.name.clone(), ty);
            }
            ModuleItemKind::InterfacePort(iface_port) => {
                let iface_name = &iface_port.interface_name.name;
                if let Some(iface_sig) = ctx.interface_definitions.get(iface_name).cloned() {
                    let mut instance_signals = HashMap::new();
                    for (sig_dir, sig_name, sig_ty) in &iface_sig.signals {
                        let dir = match (iface_port.role, sig_dir) {
                            (InterfaceRole::Master, InterfaceSignalDir::Out) => {
                                PortDirection::Output
                            }
                            (InterfaceRole::Master, InterfaceSignalDir::In) => PortDirection::Input,
                            (InterfaceRole::Slave, InterfaceSignalDir::Out) => PortDirection::Input,
                            (InterfaceRole::Slave, InterfaceSignalDir::In) => PortDirection::Output,
                        };
                        instance_signals.insert(sig_name.clone(), (dir, sig_ty.clone()));
                    }
                    ctx.current_signals.insert(
                        iface_port.name.name.clone(),
                        ResolvedType::InterfaceInstance {
                            name: iface_name.clone(),
                            role: iface_port.role,
                            signals: instance_signals,
                        },
                    );
                } else {
                    let available: Vec<String> =
                        ctx.interface_definitions.keys().cloned().collect();
                    return Err(TypeError::unknown_interface(
                        iface_name,
                        iface_port.interface_name.span,
                        &available,
                    ));
                }
            }
            _ => {}
        }
    }

    // Typecheck each module item
    let mut typed_items = Vec::new();
    for item in &module.items {
        let typed_item = infer_module_item(ctx, item)?;
        typed_items.push(typed_item);
    }

    Ok(TypedModuleDecl {
        name: module.name.clone(),
        items: typed_items,
        span: module.span,
    })
}

/// Infers the type of a module item.
///
/// Returns a [`TypedModuleItem`] with the inferred type and the original span.
///
/// Module item type inference infers the type of each item in the module and
/// returns a [`TypedModuleItem`] with the inferred type and the original span.
fn infer_module_item(
    ctx: &mut InferContext,
    item: &ModuleItem,
) -> Result<TypedModuleItem, TypeError> {
    match &item.kind {
        ModuleItemKind::Port(port) => {
            let expected_ty = ctx.current_signals.get(&port.name.name).cloned().unwrap();
            let typed_init = if let Some(init_expr) = &port.initializer {
                let mut expr_typed = infer_expr(ctx, init_expr)?;
                let unified = ctx.unify(&expected_ty, &expr_typed.ty, init_expr.span)?;
                expr_typed.ty = unified;
                Some(expr_typed)
            } else {
                None
            };

            Ok(TypedModuleItem::new(
                TypedModuleItemKind::Port(TypedPortDecl {
                    direction: port.direction,
                    name: port.name.clone(),
                    ty: expected_ty,
                    initializer: typed_init,
                    span: port.span,
                }),
                item.span,
            ))
        }

        ModuleItemKind::Register(reg) => {
            let expected_ty = ctx.current_signals.get(&reg.name.name).cloned().unwrap();
            let mut typed_reset = infer_expr(ctx, &reg.reset_value)?;
            let unified = ctx.unify(&expected_ty, &typed_reset.ty, reg.reset_value.span)?;
            typed_reset.ty = unified.clone();

            Ok(TypedModuleItem::new(
                TypedModuleItemKind::Register(TypedRegisterDecl {
                    name: reg.name.clone(),
                    ty: unified,
                    reset_value: typed_reset,
                    span: reg.span,
                }),
                item.span,
            ))
        }

        ModuleItemKind::Let(let_decl) => {
            let typed_val = infer_expr(ctx, &let_decl.value)?;
            let final_ty = if let Some(ty_ast) = &let_decl.ty {
                let expected = ctx.resolve_type(ty_ast)?;
                ctx.unify(&expected, &typed_val.ty, let_decl.value.span)?
            } else {
                typed_val.ty.clone()
            };

            // Register in current signals
            ctx.current_signals
                .insert(let_decl.name.name.clone(), final_ty.clone());

            // If value is a ModuleInstance, register as instance
            if let ResolvedType::ModuleInstance { ports, .. } = &final_ty {
                ctx.current_instances
                    .insert(let_decl.name.name.clone(), ports.clone());
            }

            Ok(TypedModuleItem::new(
                TypedModuleItemKind::Let(TypedLetDecl {
                    name: let_decl.name.clone(),
                    ty: final_ty,
                    value: typed_val,
                    span: let_decl.span,
                }),
                item.span,
            ))
        }

        ModuleItemKind::Connection(conn) => {
            let typed_target = infer_expr(ctx, &conn.target)?;
            let typed_val = infer_expr(ctx, &conn.value)?;
            let norm_target = ctx.normalize(&typed_target.ty);
            let norm_val = ctx.normalize(&typed_val.ty);

            if let (
                ResolvedType::InterfaceInstance { name: n1, .. },
                ResolvedType::InterfaceInstance { name: n2, .. },
            ) = (&norm_target, &norm_val)
            {
                if n1 != n2 {
                    return Err(TypeError::new(
                        format!("Cannot connect interface `{n1}` to interface `{n2}`"),
                        conn.span,
                    ));
                }
            } else {
                ctx.unify(&typed_target.ty, &typed_val.ty, conn.span)?;
            }

            Ok(TypedModuleItem::new(
                TypedModuleItemKind::Connection(TypedConnectionDecl {
                    target: typed_target,
                    value: typed_val,
                    is_explicit_connect: conn.is_explicit_connect,
                    span: conn.span,
                }),
                item.span,
            ))
        }

        ModuleItemKind::Assert(decl) => {
            let typed_cond = infer_expr(ctx, &decl.condition)?;
            ctx.unify(&ResolvedType::Bool, &typed_cond.ty, decl.condition.span)?;
            Ok(TypedModuleItem::new(
                TypedModuleItemKind::Assert(TypedAssertDecl {
                    condition: typed_cond,
                    message: decl.message.clone(),
                    span: decl.span,
                }),
                item.span,
            ))
        }

        ModuleItemKind::InterfacePort(iface_port) => Ok(TypedModuleItem::new(
            TypedModuleItemKind::InterfacePort(TypedInterfacePortDecl {
                role: iface_port.role,
                name: iface_port.name.clone(),
                interface_name: iface_port.interface_name.clone(),
                span: iface_port.span,
            }),
            item.span,
        )),

        ModuleItemKind::On(on_block) => {
            let typed_signal = infer_expr(ctx, &on_block.trigger.signal)?;
            if ctx
                .unify(
                    &ResolvedType::Clock,
                    &typed_signal.ty,
                    on_block.trigger.signal.span,
                )
                .is_err()
            {
                return Err(TypeError::invalid_clock_trigger(
                    &typed_signal.ty,
                    on_block.trigger.signal.span,
                ));
            }

            let typed_trigger = TypedTrigger {
                edge: on_block.trigger.edge.clone(),
                signal: typed_signal,
                span: on_block.trigger.span,
            };

            let mut typed_body = Vec::new();
            for stmt in &on_block.body {
                typed_body.push(infer_stmt(ctx, stmt)?);
            }

            Ok(TypedModuleItem::new(
                TypedModuleItemKind::On(TypedOnBlock {
                    trigger: typed_trigger,
                    body: typed_body,
                    span: on_block.span,
                }),
                item.span,
            ))
        }
    }
}

/// Finalizes the source file by replacing any TyVar or InferInt
/// in the TypedSourceFile with its final resolved type.
fn finalize_source_file(ctx: &InferContext, source: &mut TypedSourceFile) {
    for item in &mut source.items {
        match &mut item.kind {
            TypedItemKind::Module(m) => {
                for m_item in &mut m.items {
                    match &mut m_item.kind {
                        TypedModuleItemKind::Port(port) => {
                            port.ty = ctx.finalize_type(&port.ty);
                            if let Some(init) = &mut port.initializer {
                                finalize_expr(ctx, init);
                            }
                        }
                        TypedModuleItemKind::Register(reg) => {
                            reg.ty = ctx.finalize_type(&reg.ty);
                            finalize_expr(ctx, &mut reg.reset_value);
                        }
                        TypedModuleItemKind::Let(let_decl) => {
                            let_decl.ty = ctx.finalize_type(&let_decl.ty);
                            finalize_expr(ctx, &mut let_decl.value);
                        }
                        TypedModuleItemKind::Connection(conn) => {
                            finalize_expr(ctx, &mut conn.target);
                            finalize_expr(ctx, &mut conn.value);
                        }
                        TypedModuleItemKind::On(on_block) => {
                            finalize_expr(ctx, &mut on_block.trigger.signal);
                            for stmt in &mut on_block.body {
                                finalize_stmt(ctx, stmt);
                            }
                        }
                        TypedModuleItemKind::Assert(decl) => {
                            finalize_expr(ctx, &mut decl.condition);
                        }
                        TypedModuleItemKind::InterfacePort(_) => {}
                    }
                }
            }
            TypedItemKind::Struct(s) => {
                for f in &mut s.fields {
                    f.ty = ctx.finalize_type(&f.ty);
                }
            }
            TypedItemKind::Interface(iface) => {
                for sig in &mut iface.signals {
                    sig.ty = ctx.finalize_type(&sig.ty);
                }
            }
            TypedItemKind::Enum(_) => {}
        }
    }
}

fn finalize_stmt(ctx: &InferContext, stmt: &mut TypedStmt) {
    match &mut stmt.kind {
        TypedStmtKind::Assign { target, value } => {
            finalize_expr(ctx, target);
            finalize_expr(ctx, value);
        }
        TypedStmtKind::Connect { target, value } => {
            finalize_expr(ctx, target);
            finalize_expr(ctx, value);
        }
        TypedStmtKind::Assert { condition, .. } => {
            finalize_expr(ctx, condition);
        }
        TypedStmtKind::Let { ty, value, .. } => {
            *ty = ctx.finalize_type(ty);
            finalize_expr(ctx, value);
        }
        TypedStmtKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            finalize_expr(ctx, condition);
            for s in then_branch {
                finalize_stmt(ctx, s);
            }
            if let Some(elses) = else_branch {
                for s in elses {
                    finalize_stmt(ctx, s);
                }
            }
        }
        TypedStmtKind::Match { scrutinee, arms } => {
            finalize_expr(ctx, scrutinee);
            for arm in arms {
                for s in &mut arm.body {
                    finalize_stmt(ctx, s);
                }
            }
        }
        TypedStmtKind::Expr(e) => {
            finalize_expr(ctx, e);
        }
    }
}

/// Finalizes an expression by replacing any TyVar or InferInt
/// in the expression with its final resolved type.
fn finalize_expr(ctx: &InferContext, expr: &mut TypedExpr) {
    expr.ty = ctx.finalize_type(&expr.ty);
    match &mut expr.kind {
        TypedExprKind::Literal(_) | TypedExprKind::Ident(_) => {}
        TypedExprKind::FieldAccess { base, .. } => {
            finalize_expr(ctx, base);
        }
        TypedExprKind::Index { base, index } => {
            finalize_expr(ctx, base);
            finalize_expr(ctx, index);
        }
        TypedExprKind::Unary { expr: inner, .. } => {
            finalize_expr(ctx, inner);
        }
        TypedExprKind::Binary { left, right, .. } => {
            finalize_expr(ctx, left);
            finalize_expr(ctx, right);
        }
        TypedExprKind::Call { callee, args } => {
            finalize_expr(ctx, callee);
            for arg in args {
                finalize_expr(ctx, &mut arg.value);
            }
        }
        TypedExprKind::StructInit { fields, .. } => {
            for f in fields {
                finalize_expr(ctx, &mut f.value);
            }
        }
    }
}
