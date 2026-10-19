use std::collections::HashSet;
use std::fmt::Write;

use judas_ast::stmt::PatternKind;
use judas_infer::{ResolvedType, TypedExpr, TypedExprKind, TypedStmt, TypedStmtKind};

use crate::context::VerilogGenerator;

impl VerilogGenerator {
    pub(crate) fn collect_on_block_targets(
        &self,
        stmts: &[TypedStmt],
        assigned_in_on: &mut HashSet<String>,
        procedural_lets: &mut Vec<(String, ResolvedType)>,
    ) {
        for stmt in stmts {
            match &stmt.kind {
                TypedStmtKind::Assign { target, .. } | TypedStmtKind::Connect { target, .. } => {
                    if let Some(name) = self.extract_target_root_name(target) {
                        assigned_in_on.insert(name);
                    }
                }
                TypedStmtKind::Let { name, ty, .. } => {
                    if !procedural_lets.iter().any(|(n, _)| n == &name.name) {
                        procedural_lets.push((name.name.clone(), ty.clone()));
                    }
                }
                TypedStmtKind::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    self.collect_on_block_targets(then_branch, assigned_in_on, procedural_lets);
                    if let Some(else_stmts) = else_branch {
                        self.collect_on_block_targets(else_stmts, assigned_in_on, procedural_lets);
                    }
                }
                TypedStmtKind::Match { arms, .. } => {
                    for arm in arms {
                        self.collect_on_block_targets(&arm.body, assigned_in_on, procedural_lets);
                    }
                }
                TypedStmtKind::Assert { .. } | TypedStmtKind::Expr(_) => {}
            }
        }
    }

    pub(crate) fn extract_target_root_name(&self, target: &TypedExpr) -> Option<String> {
        match &target.kind {
            TypedExprKind::Ident(id) => Some(id.name.clone()),
            TypedExprKind::FieldAccess { base, field } => {
                if let TypedExprKind::Ident(base_id) = &base.kind
                    && self.current_interface_ports.contains(&base_id.name)
                {
                    return Some(format!("{}_{}", base_id.name, field.name));
                }
                self.extract_target_root_name(base)
            }
            TypedExprKind::Index { base, .. } => self.extract_target_root_name(base),
            _ => None,
        }
    }

    pub(crate) fn emit_stmt(&self, stmt: &TypedStmt, indent: usize, out: &mut String) {
        let pad = "    ".repeat(indent);
        match &stmt.kind {
            TypedStmtKind::Let { name, value, .. } => {
                let val_str = self.emit_expr(value);
                let _ = writeln!(out, "{}{} = {};", pad, name.name, val_str);
            }
            TypedStmtKind::Assign { target, value } | TypedStmtKind::Connect { target, value } => {
                let target_str = self.emit_expr(target);
                let val_str = self.emit_expr(value);
                let is_blocking = if let Some(root) = self.extract_target_root_name(target) {
                    self.procedural_lets.contains(&root)
                } else {
                    false
                };
                let op = if is_blocking { "=" } else { "<=" };
                let _ = writeln!(out, "{}{} {} {};", pad, target_str, op, val_str);
            }
            TypedStmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond_str = self.emit_expr(condition);
                let _ = writeln!(out, "{}if ({}) begin", pad, cond_str);
                for s in then_branch {
                    self.emit_stmt(s, indent + 1, out);
                }
                if let Some(else_stmts) = else_branch {
                    if else_stmts.len() == 1 && matches!(else_stmts[0].kind, TypedStmtKind::If { .. })
                    {
                        let _ = write!(out, "{}end else ", pad);
                        self.emit_if_continuation(&else_stmts[0], indent, out);
                    } else {
                        let _ = writeln!(out, "{}end else begin", pad);
                        for s in else_stmts {
                            self.emit_stmt(s, indent + 1, out);
                        }
                        let _ = writeln!(out, "{}end", pad);
                    }
                } else {
                    let _ = writeln!(out, "{}end", pad);
                }
            }
            TypedStmtKind::Match { scrutinee, arms } => {
                let scrut_str = self.emit_expr(scrutinee);
                let scrut_width = self.concrete_width_of(&scrutinee.ty);
                let _ = writeln!(out, "{}case ({})", pad, scrut_str);
                for arm in arms {
                    let pat_str = match &arm.pattern.kind {
                        PatternKind::Wildcard => "default".to_string(),
                        PatternKind::Literal(lit) => self.format_literal(lit, scrut_width),
                        PatternKind::Ident(id) => {
                            if let Some((w, disc)) = self.variant_lookup.get(&id.name) {
                                format!("{}'d{}", w, disc)
                            } else {
                                id.name.clone()
                            }
                        }
                    };
                    let _ = writeln!(out, "{}    {}: begin", pad, pat_str);
                    for s in &arm.body {
                        self.emit_stmt(s, indent + 2, out);
                    }
                    let _ = writeln!(out, "{}    end", pad);
                }
                let _ = writeln!(out, "{}endcase", pad);
            }
            TypedStmtKind::Assert { condition, message } => {
                let cond_str = self.emit_expr(condition);
                let msg = message.as_deref().unwrap_or("Assertion failed");
                let _ = writeln!(out, "{}`ifndef SYNTHESIS", pad);
                let _ = writeln!(out, "{}if (!({})) begin", pad, cond_str);
                let _ = writeln!(out, "{}    $display(\"Assertion failed: {}\");", pad, msg);
                let _ = writeln!(out, "{}    $finish;", pad);
                let _ = writeln!(out, "{}end", pad);
                let _ = writeln!(out, "{}`endif", pad);
            }
            TypedStmtKind::Expr(expr) => {
                let e_str = self.emit_expr(expr);
                let _ = writeln!(out, "{}{};", pad, e_str);
            }
        }
    }

    fn emit_if_continuation(&self, stmt: &TypedStmt, indent: usize, out: &mut String) {
        let pad = "    ".repeat(indent);
        if let TypedStmtKind::If {
            condition,
            then_branch,
            else_branch,
        } = &stmt.kind
        {
            let cond_str = self.emit_expr(condition);
            let _ = writeln!(out, "if ({}) begin", cond_str);
            for s in then_branch {
                self.emit_stmt(s, indent + 1, out);
            }
            if let Some(else_stmts) = else_branch {
                if else_stmts.len() == 1 && matches!(else_stmts[0].kind, TypedStmtKind::If { .. }) {
                    let _ = write!(out, "{}end else ", pad);
                    self.emit_if_continuation(&else_stmts[0], indent, out);
                } else {
                    let _ = writeln!(out, "{}end else begin", pad);
                    for s in else_stmts {
                        self.emit_stmt(s, indent + 1, out);
                    }
                    let _ = writeln!(out, "{}end", pad);
                }
            } else {
                let _ = writeln!(out, "{}end", pad);
            }
        }
    }
}
