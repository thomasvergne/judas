use judas_ast::expr::Literal;
use judas_ast::stmt::{PatternKind, Stmt, StmtKind};

use crate::context::InferContext;
use crate::error::TypeError;
use crate::expr::infer_expr;
use crate::tast::{TypedMatchArm, TypedStmt, TypedStmtKind};
use crate::types::ResolvedType;

/// Infers a statement and returns a `TypedStmt`.
///
/// # Errors
///
/// Returns an error if the statement cannot be inferred.
pub(crate) fn infer_stmt(ctx: &mut InferContext, stmt: &Stmt) -> Result<TypedStmt, TypeError> {
    match &stmt.kind {
        StmtKind::Assign { target, value } => {
            let typed_target = infer_expr(ctx, target)?;
            let typed_val = infer_expr(ctx, value)?;
            ctx.unify(&typed_target.ty, &typed_val.ty, stmt.span)?;

            Ok(TypedStmt::new(
                TypedStmtKind::Assign {
                    target: typed_target,
                    value: typed_val,
                },
                stmt.span,
            ))
        }

        StmtKind::Connect { target, value } => {
            let typed_target = infer_expr(ctx, target)?;
            let typed_val = infer_expr(ctx, value)?;
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
                        stmt.span,
                    ));
                }
            } else {
                ctx.unify(&typed_target.ty, &typed_val.ty, stmt.span)?;
            }

            Ok(TypedStmt::new(
                TypedStmtKind::Connect {
                    target: typed_target,
                    value: typed_val,
                },
                stmt.span,
            ))
        }

        StmtKind::Assert { condition, message } => {
            let typed_cond = infer_expr(ctx, condition)?;
            ctx.unify(&ResolvedType::Bool, &typed_cond.ty, condition.span)?;
            Ok(TypedStmt::new(
                TypedStmtKind::Assert {
                    condition: typed_cond,
                    message: message.clone(),
                },
                stmt.span,
            ))
        }

        StmtKind::Let { name, ty, value } => {
            let typed_val = infer_expr(ctx, value)?;
            let final_ty = if let Some(ty_ast) = ty {
                let expected = ctx.resolve_type(ty_ast)?;
                ctx.unify(&expected, &typed_val.ty, value.span)?
            } else {
                typed_val.ty.clone()
            };

            ctx.current_signals
                .insert(name.name.clone(), final_ty.clone());

            Ok(TypedStmt::new(
                TypedStmtKind::Let {
                    name: name.clone(),
                    ty: final_ty,
                    value: typed_val,
                },
                stmt.span,
            ))
        }

        StmtKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let typed_cond = infer_expr(ctx, condition)?;
            ctx.unify(&ResolvedType::Bool, &typed_cond.ty, condition.span)?;

            let mut typed_then = Vec::new();
            for s in then_branch {
                typed_then.push(infer_stmt(ctx, s)?);
            }

            let typed_else = if let Some(else_stmts) = else_branch {
                let mut elses = Vec::new();
                for s in else_stmts {
                    elses.push(infer_stmt(ctx, s)?);
                }
                Some(elses)
            } else {
                None
            };

            Ok(TypedStmt::new(
                TypedStmtKind::If {
                    condition: typed_cond,
                    then_branch: typed_then,
                    else_branch: typed_else,
                },
                stmt.span,
            ))
        }

        StmtKind::Match { scrutinee, arms } => {
            let typed_scrutinee = infer_expr(ctx, scrutinee)?;

            let mut typed_arms = Vec::new();
            for arm in arms {
                // Match pattern against scrutinee type
                match &arm.pattern.kind {
                    PatternKind::Ident(ident) => {
                        // Check if it's an enum variant
                        if let Some(enum_name) = ctx.variant_to_enum.get(&ident.name) {
                            ctx.unify(
                                &ResolvedType::Enum(enum_name.clone()),
                                &typed_scrutinee.ty,
                                arm.pattern.span,
                            )?;
                        } else {
                            // Variable pattern binds scrutinee type
                            ctx.current_signals
                                .insert(ident.name.clone(), typed_scrutinee.ty.clone());
                        }
                    }
                    PatternKind::Literal(Literal::Integer(_)) => {
                        let lit_ty = ctx.fresh_infer_int();
                        ctx.unify(&lit_ty, &typed_scrutinee.ty, arm.pattern.span)?;
                    }
                    PatternKind::Literal(Literal::Bool(_)) => {
                        ctx.unify(&ResolvedType::Bool, &typed_scrutinee.ty, arm.pattern.span)?;
                    }
                    PatternKind::Literal(Literal::Bit(_)) => {
                        ctx.unify(&ResolvedType::Bit, &typed_scrutinee.ty, arm.pattern.span)?;
                    }
                    PatternKind::Literal(Literal::String(_)) => {}
                    PatternKind::Wildcard => {}
                }

                let mut arm_body = Vec::new();
                for s in &arm.body {
                    arm_body.push(infer_stmt(ctx, s)?);
                }

                typed_arms.push(TypedMatchArm {
                    pattern: arm.pattern.clone(),
                    body: arm_body,
                    span: arm.span,
                });
            }

            Ok(TypedStmt::new(
                TypedStmtKind::Match {
                    scrutinee: typed_scrutinee,
                    arms: typed_arms,
                },
                stmt.span,
            ))
        }

        StmtKind::Expr(expr) => {
            let typed_expr = infer_expr(ctx, expr)?;
            Ok(TypedStmt::new(TypedStmtKind::Expr(typed_expr), stmt.span))
        }
    }
}
