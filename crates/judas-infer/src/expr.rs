use std::collections::HashMap;

use judas_ast::expr::{BinaryOp, Expr, ExprKind, Literal, UnaryOp};
use judas_ast::item::{InterfaceRole, InterfaceSignalDir, PortDirection};
use judas_ast::types::{Ident, TypeArg};

use crate::context::InferContext;
use crate::error::TypeError;
use crate::tast::{TypedCallArg, TypedExpr, TypedExprKind, TypedFieldInit};
use crate::types::{ResolvedType, WidthExpr};

/// Infers the type of an expression.
///
/// Returns a [`TypedExpr`] with the inferred type and the original span.
///
/// Expression type inference elaborates on the expression kind and infers
/// the type accordingly. It returns a [`TypedExpr`] with the inferred type
/// and the original span.
pub(crate) fn infer_expr(ctx: &mut InferContext, expr: &Expr) -> Result<TypedExpr, TypeError> {
    match &expr.kind {
        ExprKind::Literal(lit) => {
            let ty = match lit {
                Literal::Integer(_) => ctx.fresh_infer_int(),
                Literal::Bool(_) => ResolvedType::Bool,
                Literal::Bit(_) => ResolvedType::Bit,
                Literal::String(s) => ResolvedType::bits(s.len() as u32 * 8),
            };
            Ok(TypedExpr::new(
                TypedExprKind::Literal(lit.clone()),
                ty,
                expr.span,
            ))
        }

        ExprKind::Ident(ident) => {
            let resolved_name = ctx.resolve_symbol_name(&ident.name);
            if let Some(ty) = ctx
                .current_signals
                .get(&ident.name)
                .or_else(|| ctx.current_signals.get(&resolved_name))
            {
                Ok(TypedExpr::new(
                    TypedExprKind::Ident(ident.clone()),
                    ty.clone(),
                    expr.span,
                ))
            } else if let Some(enum_name) = ctx
                .variant_to_enum
                .get(&ident.name)
                .or_else(|| ctx.variant_to_enum.get(&resolved_name))
            {
                Ok(TypedExpr::new(
                    TypedExprKind::Ident(ident.clone()),
                    ResolvedType::Enum(enum_name.clone()),
                    expr.span,
                ))
            } else if let Some(val) = ctx
                .current_constants
                .get(&ident.name)
                .or_else(|| ctx.current_constants.get(&resolved_name))
            {
                Ok(TypedExpr::new(
                    TypedExprKind::Literal(Literal::Integer(*val as i128)),
                    ResolvedType::uint(32),
                    expr.span,
                ))
            } else {
                Err(TypeError::undefined_variable(
                    &ident.name,
                    ident.span,
                    &ctx.visible_names(),
                ))
            }
        }

        ExprKind::Path(path) => {
            let path_str = path.to_string_path();
            let resolved_name = ctx.resolve_symbol_name(&path_str);

            if let Some(vis) = ctx.visibilities.get(&resolved_name)
                && *vis == judas_ast::path::Visibility::Private
                && path.is_qualified()
            {
                return Err(TypeError::new(
                    format!(
                        "Symbol `{}` is private and cannot be accessed from outside its namespace",
                        path_str
                    ),
                    path.span,
                ));
            }

            if let Some(enum_name) = ctx.variant_to_enum.get(&resolved_name) {
                Ok(TypedExpr::new(
                    TypedExprKind::Path(path.clone()),
                    ResolvedType::Enum(enum_name.clone()),
                    expr.span,
                ))
            } else if path.segments.len() >= 2 {
                let enum_prefix = path.segments[..path.segments.len() - 1]
                    .iter()
                    .map(|s| s.name.as_str())
                    .collect::<Vec<_>>()
                    .join("::");
                let resolved_enum = ctx.resolve_symbol_name(&enum_prefix);
                let variant_name = &path.segments.last().unwrap().name;
                if let Some(variants) = ctx.enums.get(&resolved_enum)
                    && variants.contains(variant_name)
                {
                    return Ok(TypedExpr::new(
                        TypedExprKind::Path(path.clone()),
                        ResolvedType::Enum(resolved_enum),
                        expr.span,
                    ));
                }
                Err(TypeError::undefined_variable(
                    &path_str,
                    path.span,
                    &ctx.visible_names(),
                ))
            } else if let Some(ty) = ctx.current_signals.get(&resolved_name) {
                Ok(TypedExpr::new(
                    TypedExprKind::Path(path.clone()),
                    ty.clone(),
                    expr.span,
                ))
            } else if let Some(val) = ctx.current_constants.get(&resolved_name) {
                Ok(TypedExpr::new(
                    TypedExprKind::Literal(Literal::Integer(*val as i128)),
                    ResolvedType::uint(32),
                    expr.span,
                ))
            } else {
                Err(TypeError::undefined_variable(
                    &path_str,
                    path.span,
                    &ctx.visible_names(),
                ))
            }
        }

        ExprKind::FieldAccess { base, field } => {
            let typed_base = infer_expr(ctx, base)?;
            let norm_base = ctx.normalize(&typed_base.ty);

            match &norm_base {
                ResolvedType::ModuleInstance { module_name, ports } => {
                    if let Some((_dir, port_ty)) = ports.get(&field.name) {
                        Ok(TypedExpr::new(
                            TypedExprKind::FieldAccess {
                                base: Box::new(typed_base),
                                field: field.clone(),
                            },
                            port_ty.clone(),
                            expr.span,
                        ))
                    } else {
                        let available_ports: Vec<String> = ports.keys().cloned().collect();
                        Err(TypeError::unknown_port(
                            module_name,
                            &field.name,
                            field.span,
                            &available_ports,
                        ))
                    }
                }
                ResolvedType::Struct { name, fields, .. } => {
                    if let Some((_, f_ty)) = fields.iter().find(|(n, _)| n == &field.name) {
                        Ok(TypedExpr::new(
                            TypedExprKind::FieldAccess {
                                base: Box::new(typed_base),
                                field: field.clone(),
                            },
                            f_ty.clone(),
                            expr.span,
                        ))
                    } else {
                        let available: Vec<String> =
                            fields.iter().map(|(n, _)| n.clone()).collect();
                        Err(TypeError::unknown_struct_field(
                            name,
                            &field.name,
                            field.span,
                            &available,
                        ))
                    }
                }
                ResolvedType::InterfaceInstance { name, signals, .. } => {
                    if let Some((_dir, sig_ty)) = signals.get(&field.name) {
                        Ok(TypedExpr::new(
                            TypedExprKind::FieldAccess {
                                base: Box::new(typed_base),
                                field: field.clone(),
                            },
                            sig_ty.clone(),
                            expr.span,
                        ))
                    } else {
                        let available: Vec<String> = signals.keys().cloned().collect();
                        Err(TypeError::new(
                            format!("Interface `{name}` has no signal named `{}`", field.name),
                            field.span,
                        )
                        .with_help(format!(
                            "Available signals on interface `{name}`: {}",
                            available.join(", ")
                        )))
                    }
                }
                _ => Err(TypeError::new(
                    format!(
                        "Cannot access field `{}` on non-composite type `{}`",
                        field.name, norm_base
                    ),
                    expr.span,
                )),
            }
        }

        ExprKind::StructInit { path, fields } => {
            let path_str = path.to_string_path();
            let resolved_name = ctx.resolve_symbol_name(&path_str);

            if let Some(vis) = ctx.visibilities.get(&resolved_name)
                && *vis == judas_ast::path::Visibility::Private
                && path.is_qualified()
            {
                return Err(TypeError::new(
                    format!(
                        "Struct `{}` is private and cannot be accessed from outside its namespace",
                        path_str
                    ),
                    path.span,
                ));
            }

            let sig = if let Some(s) = ctx.struct_definitions.get(&resolved_name).cloned() {
                s
            } else {
                let available: Vec<String> = ctx.struct_definitions.keys().cloned().collect();
                return Err(TypeError::unknown_struct(&path_str, path.span, &available));
            };

            let mut typed_fields = Vec::new();
            let mut provided_field_names = std::collections::HashSet::new();

            for field in fields {
                if !provided_field_names.insert(field.name.name.clone()) {
                    return Err(TypeError::new(
                        format!(
                            "Duplicate field `{}` in struct `{}` initialization",
                            field.name.name, path_str
                        ),
                        field.span,
                    ));
                }

                if let Some((_, expected_ty)) =
                    sig.fields.iter().find(|(n, _)| n == &field.name.name)
                {
                    let mut typed_val = infer_expr(ctx, &field.value)?;
                    let unified = ctx.unify(expected_ty, &typed_val.ty, field.value.span)?;
                    typed_val.ty = unified;
                    typed_fields.push(TypedFieldInit {
                        name: field.name.clone(),
                        value: typed_val,
                        span: field.span,
                    });
                } else {
                    let available_fields: Vec<String> =
                        sig.fields.iter().map(|(n, _)| n.clone()).collect();
                    return Err(TypeError::unknown_struct_field(
                        &path_str,
                        &field.name.name,
                        field.span,
                        &available_fields,
                    ));
                }
            }

            // Check that all declared fields were provided
            let mut missing = Vec::new();
            for (f_name, _) in &sig.fields {
                if !provided_field_names.contains(f_name) {
                    missing.push(f_name.clone());
                }
            }
            if !missing.is_empty() {
                return Err(TypeError::missing_struct_fields(
                    &path_str, expr.span, &missing,
                ));
            }

            let ret_ty = ResolvedType::Struct {
                name: resolved_name,
                fields: sig.fields.clone(),
                total_width: sig.total_width.clone(),
            };

            Ok(TypedExpr::new(
                TypedExprKind::StructInit {
                    path: path.clone(),
                    fields: typed_fields,
                },
                ret_ty,
                expr.span,
            ))
        }

        ExprKind::Index { base, index } => {
            let typed_base = infer_expr(ctx, base)?;
            let typed_index = infer_expr(ctx, index)?;

            let norm_base = ctx.normalize(&typed_base.ty);
            match norm_base {
                ResolvedType::Array { element, .. } => {
                    // Index must be integer
                    let expected_index_ty = ctx.fresh_infer_int();
                    ctx.unify(&expected_index_ty, &typed_index.ty, index.span)?;

                    Ok(TypedExpr::new(
                        TypedExprKind::Index {
                            base: Box::new(typed_base),
                            index: Box::new(typed_index),
                        },
                        *element,
                        expr.span,
                    ))
                }
                other => Err(TypeError::new(
                    format!("Cannot index non-array type `{}`", other),
                    base.span,
                )),
            }
        }

        ExprKind::Unary { op, expr: inner } => {
            let typed_inner = infer_expr(ctx, inner)?;
            let ty = match op {
                UnaryOp::Not => {
                    ctx.unify(&ResolvedType::Bool, &typed_inner.ty, inner.span)?;
                    ResolvedType::Bool
                }
                UnaryOp::Neg => {
                    if !typed_inner.ty.is_integer() {
                        return Err(TypeError::new(
                            "Unary minus requires integer operand",
                            expr.span,
                        ));
                    }
                    typed_inner.ty.clone()
                }
                UnaryOp::BitNot => {
                    if !typed_inner.ty.is_numeric_or_bits() {
                        return Err(TypeError::new(
                            "Bitwise not requires integer or bits operand",
                            expr.span,
                        ));
                    }
                    typed_inner.ty.clone()
                }
            };
            Ok(TypedExpr::new(
                TypedExprKind::Unary {
                    op: *op,
                    expr: Box::new(typed_inner),
                },
                ty,
                expr.span,
            ))
        }

        ExprKind::Binary { op, left, right } => {
            let typed_left = infer_expr(ctx, left)?;
            let typed_right = infer_expr(ctx, right)?;

            let ty = match op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                    let unified = ctx.unify(&typed_left.ty, &typed_right.ty, expr.span)?;
                    if !unified.is_integer() {
                        return Err(TypeError::arithmetic_requires_integers(unified, expr.span));
                    }
                    unified
                }
                BinaryOp::Eq | BinaryOp::Ne => {
                    ctx.unify(&typed_left.ty, &typed_right.ty, expr.span)?;
                    ResolvedType::Bool
                }
                BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                    let unified = ctx.unify(&typed_left.ty, &typed_right.ty, expr.span)?;
                    if !unified.is_numeric_or_bits() {
                        return Err(TypeError::comparison_requires_numeric(unified, expr.span));
                    }
                    ResolvedType::Bool
                }
                BinaryOp::And | BinaryOp::Or => {
                    ctx.unify(&ResolvedType::Bool, &typed_left.ty, left.span)?;
                    ctx.unify(&ResolvedType::Bool, &typed_right.ty, right.span)?;
                    ResolvedType::Bool
                }
                BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
                    let unified = ctx.unify(&typed_left.ty, &typed_right.ty, expr.span)?;
                    if !unified.is_numeric_or_bits() && unified != ResolvedType::Bool {
                        return Err(TypeError::bitwise_requires_bits(unified, expr.span));
                    }
                    unified
                }
                BinaryOp::Shl | BinaryOp::Shr => {
                    if !typed_left.ty.is_numeric_or_bits() {
                        return Err(TypeError::new(
                            "Shift left operand must be integer or bits",
                            left.span,
                        ));
                    }
                    if !typed_right.ty.is_integer() {
                        return Err(TypeError::shift_amount_must_be_integer(right.span));
                    }
                    typed_left.ty.clone()
                }
            };

            Ok(TypedExpr::new(
                TypedExprKind::Binary {
                    op: *op,
                    left: Box::new(typed_left),
                    right: Box::new(typed_right),
                },
                ty,
                expr.span,
            ))
        }

        ExprKind::Call {
            callee,
            generic_args,
            args,
        } => {
            // Check for builtin `repeat`: repeat<N>(val) or val.repeat<N>()
            let is_repeat = match &callee.kind {
                ExprKind::Ident(id) if id.name == "repeat" => Some(true),
                ExprKind::FieldAccess { field, .. } if field.name == "repeat" => Some(false),
                _ => None,
            };
            if let Some(is_fn_call) = is_repeat {
                if generic_args.len() != 1 {
                    return Err(TypeError::repeat_arg_count(expr.span));
                }
                let val_expr = if is_fn_call {
                    if args.len() != 1 {
                        return Err(TypeError::repeat_arg_count(expr.span));
                    }
                    &args[0].value
                } else {
                    if !args.is_empty() {
                        return Err(TypeError::repeat_arg_count(expr.span));
                    }
                    if let ExprKind::FieldAccess { base, .. } = &callee.kind {
                        base.as_ref()
                    } else {
                        unreachable!()
                    }
                };

                let count = ctx.resolve_width_arg(&generic_args[0], expr.span)?;
                let typed_val = infer_expr(ctx, val_expr)?;
                let norm = ctx.normalize(&typed_val.ty);
                let val_w = match &norm {
                    ResolvedType::Bit | ResolvedType::Bool => WidthExpr::Const(1),
                    ResolvedType::UInt(w) | ResolvedType::Int(w) | ResolvedType::Bits(w) => {
                        w.clone()
                    }
                    ResolvedType::InferInt(_) => WidthExpr::Const(1),
                    ResolvedType::Struct { total_width, .. } => total_width.clone(),
                    other => {
                        return Err(TypeError::new(
                            format!("Cannot repeat non-bit/integer type `{other}`"),
                            val_expr.span,
                        ));
                    }
                };
                let total_w = match (&count, &val_w) {
                    (WidthExpr::Const(c1), WidthExpr::Const(c2)) => WidthExpr::Const(c1 * c2),
                    _ => WidthExpr::Mul(Box::new(count), Box::new(val_w)),
                };
                let ret_ty = if matches!(norm, ResolvedType::Bits(_)) {
                    ResolvedType::Bits(total_w)
                } else {
                    ResolvedType::UInt(total_w)
                };

                let typed_callee = TypedExpr::new(
                    TypedExprKind::Ident(Ident::new("repeat", callee.span)),
                    ret_ty.clone(),
                    callee.span,
                );
                return Ok(TypedExpr::new(
                    TypedExprKind::Call {
                        callee: Box::new(typed_callee),
                        args: vec![TypedCallArg {
                            name: None,
                            value: typed_val,
                            span: val_expr.span,
                        }],
                    },
                    ret_ty,
                    expr.span,
                ));
            }

            // Check for builtin `slice`: slice<W>(signal, offset) or signal.slice<W>(offset)
            let is_slice = match &callee.kind {
                ExprKind::Ident(id) if id.name == "slice" => Some(true),
                ExprKind::FieldAccess { field, .. } if field.name == "slice" => Some(false),
                _ => None,
            };
            if let Some(is_fn_call) = is_slice {
                if generic_args.len() != 1 {
                    return Err(TypeError::slice_arg_count(expr.span));
                }
                let (sig_expr, offset_expr) = if is_fn_call {
                    if args.len() != 2 {
                        return Err(TypeError::slice_arg_count(expr.span));
                    }
                    (&args[0].value, &args[1].value)
                } else {
                    if args.len() != 1 {
                        return Err(TypeError::slice_arg_count(expr.span));
                    }
                    if let ExprKind::FieldAccess { base, .. } = &callee.kind {
                        (base.as_ref(), &args[0].value)
                    } else {
                        unreachable!()
                    }
                };

                let slice_w = ctx.resolve_width_arg(&generic_args[0], expr.span)?;
                let typed_sig = infer_expr(ctx, sig_expr)?;
                let norm_sig = ctx.normalize(&typed_sig.ty);
                if !norm_sig.is_numeric_or_bits()
                    && !matches!(norm_sig, ResolvedType::Struct { .. })
                {
                    return Err(TypeError::new(
                        format!("Cannot slice non-bit/integer type `{norm_sig}`"),
                        sig_expr.span,
                    ));
                }

                let mut typed_offset = infer_expr(ctx, offset_expr)?;
                let offset_req = ctx.fresh_infer_int();
                let offset_unified = ctx.unify(&offset_req, &typed_offset.ty, offset_expr.span)?;
                typed_offset.ty = offset_unified;

                let ret_ty = if matches!(norm_sig, ResolvedType::Bits(_)) {
                    ResolvedType::Bits(slice_w)
                } else {
                    ResolvedType::UInt(slice_w)
                };

                let typed_callee = TypedExpr::new(
                    TypedExprKind::Ident(Ident::new("slice", callee.span)),
                    ret_ty.clone(),
                    callee.span,
                );
                return Ok(TypedExpr::new(
                    TypedExprKind::Call {
                        callee: Box::new(typed_callee),
                        args: vec![
                            TypedCallArg {
                                name: None,
                                value: typed_sig,
                                span: sig_expr.span,
                            },
                            TypedCallArg {
                                name: None,
                                value: typed_offset,
                                span: offset_expr.span,
                            },
                        ],
                    },
                    ret_ty,
                    expr.span,
                ));
            }

            // Check for builtins: extend, truncate, zero, concat
            if let ExprKind::Ident(fn_ident) = &callee.kind {
                if fn_ident.name == "extend" {
                    if generic_args.len() > 1 {
                        return Err(TypeError::new(
                            "`extend` takes at most 1 generic argument",
                            expr.span,
                        ));
                    }
                    if args.len() != 1 {
                        return Err(TypeError::extend_arg_count(expr.span));
                    }
                    let typed_arg = infer_expr(ctx, &args[0].value)?;
                    let norm = ctx.normalize(&typed_arg.ty);

                    let ret_ty = if generic_args.len() == 1 {
                        let target_w = ctx.resolve_width_arg(&generic_args[0], expr.span)?;
                        match &norm {
                            ResolvedType::Bits(_) => ResolvedType::Bits(target_w),
                            ResolvedType::Int(_) => ResolvedType::Int(target_w),
                            ResolvedType::UInt(_)
                            | ResolvedType::Bit
                            | ResolvedType::Bool
                            | ResolvedType::InferInt(_) => ResolvedType::UInt(target_w),
                            other => {
                                return Err(TypeError::cannot_extend(other, args[0].value.span));
                            }
                        }
                    } else {
                        match norm {
                            ResolvedType::UInt(w) => {
                                let new_w = match &w {
                                    WidthExpr::Const(c) => WidthExpr::Const(c + 1),
                                    _ => WidthExpr::Add(
                                        Box::new(w.clone()),
                                        Box::new(WidthExpr::Const(1)),
                                    ),
                                };
                                ResolvedType::UInt(new_w)
                            }
                            ResolvedType::Int(w) => {
                                let new_w = match &w {
                                    WidthExpr::Const(c) => WidthExpr::Const(c + 1),
                                    _ => WidthExpr::Add(
                                        Box::new(w.clone()),
                                        Box::new(WidthExpr::Const(1)),
                                    ),
                                };
                                ResolvedType::Int(new_w)
                            }
                            ResolvedType::Bits(w) => {
                                let new_w = match &w {
                                    WidthExpr::Const(c) => WidthExpr::Const(c + 1),
                                    _ => WidthExpr::Add(
                                        Box::new(w.clone()),
                                        Box::new(WidthExpr::Const(1)),
                                    ),
                                };
                                ResolvedType::Bits(new_w)
                            }
                            ResolvedType::Bit | ResolvedType::Bool => {
                                ResolvedType::UInt(WidthExpr::Const(2))
                            }
                            ResolvedType::InferInt(_) => ctx.fresh_infer_int(),
                            other => {
                                return Err(TypeError::cannot_extend(other, args[0].value.span));
                            }
                        }
                    };

                    let typed_callee = TypedExpr::new(
                        TypedExprKind::Ident(fn_ident.clone()),
                        ret_ty.clone(),
                        callee.span,
                    );
                    return Ok(TypedExpr::new(
                        TypedExprKind::Call {
                            callee: Box::new(typed_callee),
                            args: vec![TypedCallArg {
                                name: args[0].name.clone(),
                                value: typed_arg,
                                span: args[0].span,
                            }],
                        },
                        ret_ty,
                        expr.span,
                    ));
                } else if fn_ident.name == "concat" {
                    if args.len() < 2 {
                        return Err(TypeError::concat_min_args(expr.span));
                    }
                    let mut typed_args = Vec::new();
                    let mut total_width: Option<WidthExpr> = None;
                    let mut is_all_bits = true;

                    for arg in args {
                        let typed_val = infer_expr(ctx, &arg.value)?;
                        let norm = ctx.normalize(&typed_val.ty);
                        let w = match &norm {
                            ResolvedType::Bool | ResolvedType::Bit => WidthExpr::Const(1),
                            ResolvedType::Bits(bw) => bw.clone(),
                            ResolvedType::UInt(uw) => {
                                is_all_bits = false;
                                uw.clone()
                            }
                            ResolvedType::Int(iw) => {
                                is_all_bits = false;
                                iw.clone()
                            }
                            ResolvedType::InferInt(_) => {
                                is_all_bits = false;
                                WidthExpr::Const(1)
                            }
                            other => {
                                return Err(TypeError::cannot_concatenate(other, arg.value.span));
                            }
                        };

                        total_width = match total_width {
                            None => Some(w),
                            Some(acc) => {
                                let combined = match (&acc, &w) {
                                    (WidthExpr::Const(c1), WidthExpr::Const(c2)) => {
                                        WidthExpr::Const(c1 + c2)
                                    }
                                    _ => WidthExpr::Add(Box::new(acc), Box::new(w)),
                                };
                                Some(combined)
                            }
                        };

                        typed_args.push(TypedCallArg {
                            name: arg.name.clone(),
                            value: typed_val,
                            span: arg.span,
                        });
                    }

                    let final_width = total_width.unwrap_or(WidthExpr::Const(0));
                    let ret_ty = if is_all_bits {
                        ResolvedType::Bits(final_width)
                    } else {
                        ResolvedType::UInt(final_width)
                    };

                    let typed_callee = TypedExpr::new(
                        TypedExprKind::Ident(fn_ident.clone()),
                        ret_ty.clone(),
                        callee.span,
                    );
                    return Ok(TypedExpr::new(
                        TypedExprKind::Call {
                            callee: Box::new(typed_callee),
                            args: typed_args,
                        },
                        ret_ty,
                        expr.span,
                    ));
                } else if fn_ident.name == "truncate" {
                    if generic_args.len() != 1 {
                        return Err(TypeError::missing_truncate_arg(expr.span));
                    }
                    if args.len() != 1 {
                        return Err(TypeError::truncate_arg_count(expr.span));
                    }
                    let target_w = ctx.resolve_width_arg(&generic_args[0], expr.span)?;
                    let typed_arg = infer_expr(ctx, &args[0].value)?;
                    let target_ty = ResolvedType::UInt(target_w);
                    let typed_callee = TypedExpr::new(
                        TypedExprKind::Ident(fn_ident.clone()),
                        target_ty.clone(),
                        callee.span,
                    );

                    return Ok(TypedExpr::new(
                        TypedExprKind::Call {
                            callee: Box::new(typed_callee),
                            args: vec![TypedCallArg {
                                name: args[0].name.clone(),
                                value: typed_arg,
                                span: args[0].span,
                            }],
                        },
                        target_ty,
                        expr.span,
                    ));
                } else if fn_ident.name == "zero" {
                    let ret_ty = if !generic_args.is_empty() {
                        match &generic_args[0] {
                            TypeArg::Type(t) => ctx.resolve_type(t)?,
                            _ => ctx.fresh_tyvar(),
                        }
                    } else {
                        ctx.fresh_tyvar()
                    };

                    let typed_callee = TypedExpr::new(
                        TypedExprKind::Ident(fn_ident.clone()),
                        ret_ty.clone(),
                        callee.span,
                    );
                    return Ok(TypedExpr::new(
                        TypedExprKind::Call {
                            callee: Box::new(typed_callee),
                            args: Vec::new(),
                        },
                        ret_ty,
                        expr.span,
                    ));
                }
            }

            let callee_target = match &callee.kind {
                ExprKind::Ident(id) => Some((id.name.clone(), id.span)),
                ExprKind::Path(p) => Some((p.to_string_path(), p.span)),
                _ => None,
            };

            if let Some((target_name, callee_span)) = callee_target {
                let resolved_name = ctx.resolve_symbol_name(&target_name);

                if let Some(vis) = ctx.visibilities.get(&resolved_name)
                    && *vis == judas_ast::path::Visibility::Private
                    && (target_name.contains("::") || ctx.aliases.contains_key(&target_name))
                {
                    return Err(TypeError::new(
                        format!(
                            "Module `{}` is private and cannot be accessed from outside its namespace",
                            target_name
                        ),
                        callee_span,
                    ));
                }

                if let Some(sig) = ctx.module_definitions.get(&resolved_name).cloned() {
                    // Component instantiation: Counter(width: 16) or math::Counter(width: 16)
                    let mut instance_constants = HashMap::new();
                    let mut typed_args = Vec::new();

                    for arg in args {
                        let typed_val = infer_expr(ctx, &arg.value)?;
                        if let Some(arg_name) = &arg.name
                            && sig.params.contains_key(&arg_name.name)
                            && let Ok(val) = ctx.eval_const_expr(&arg.value)
                        {
                            instance_constants.insert(arg_name.name.clone(), val as i64);
                        }
                        typed_args.push(TypedCallArg {
                            name: arg.name.clone(),
                            value: typed_val,
                            span: arg.span,
                        });
                    }

                    // Fill defaults for missing params
                    for (param_name, (_pty, pdef)) in &sig.params {
                        if !instance_constants.contains_key(param_name)
                            && let Some(def_val) = pdef
                        {
                            instance_constants.insert(param_name.clone(), *def_val);
                        }
                    }

                    // Resolve instance ports
                    let mut inst_ports = HashMap::new();
                    let old_consts =
                        std::mem::replace(&mut ctx.current_constants, instance_constants);
                    for (p_name, (dir, ast_ty)) in &sig.ports {
                        let res_ty = ctx.resolve_type(ast_ty)?;
                        inst_ports.insert(p_name.clone(), (*dir, res_ty));
                    }
                    for (iface_port_name, (role, iface_name)) in &sig.interface_ports {
                        let resolved_iface_name = ctx.resolve_symbol_name(iface_name);
                        if let Some(iface_sig) = ctx.interface_definitions.get(&resolved_iface_name) {
                            let mut instance_signals = HashMap::new();
                            for (sig_dir, sig_name, sig_ty) in &iface_sig.signals {
                                let dir = match (role, sig_dir) {
                                    (InterfaceRole::Master, InterfaceSignalDir::Out) => {
                                        PortDirection::Output
                                    }
                                    (InterfaceRole::Master, InterfaceSignalDir::In) => {
                                        PortDirection::Input
                                    }
                                    (InterfaceRole::Slave, InterfaceSignalDir::Out) => {
                                        PortDirection::Input
                                    }
                                    (InterfaceRole::Slave, InterfaceSignalDir::In) => {
                                        PortDirection::Output
                                    }
                                };
                                instance_signals.insert(sig_name.clone(), (dir, sig_ty.clone()));
                            }
                            inst_ports.insert(
                                iface_port_name.clone(),
                                (
                                    PortDirection::Output,
                                    ResolvedType::InterfaceInstance {
                                        name: iface_name.clone(),
                                        role: *role,
                                        signals: instance_signals,
                                    },
                                ),
                            );
                        }
                    }
                    ctx.current_constants = old_consts;

                    let inst_ty = ResolvedType::ModuleInstance {
                        module_name: resolved_name,
                        ports: inst_ports,
                    };

                    let typed_callee = match &callee.kind {
                        ExprKind::Ident(fn_ident) => TypedExpr::new(
                            TypedExprKind::Ident(fn_ident.clone()),
                            inst_ty.clone(),
                            callee.span,
                        ),
                        ExprKind::Path(path) => TypedExpr::new(
                            TypedExprKind::Path(path.clone()),
                            inst_ty.clone(),
                            callee.span,
                        ),
                        _ => unreachable!(),
                    };

                    return Ok(TypedExpr::new(
                        TypedExprKind::Call {
                            callee: Box::new(typed_callee),
                            args: typed_args,
                        },
                        inst_ty,
                        expr.span,
                    ));
                }
            }

            // General call fallback
            let typed_callee = infer_expr(ctx, callee)?;
            let mut typed_args = Vec::new();
            for arg in args {
                typed_args.push(TypedCallArg {
                    name: arg.name.clone(),
                    value: infer_expr(ctx, &arg.value)?,
                    span: arg.span,
                });
            }

            Ok(TypedExpr::new(
                TypedExprKind::Call {
                    callee: Box::new(typed_callee),
                    args: typed_args,
                },
                ctx.fresh_tyvar(),
                expr.span,
            ))
        }
    }
}
