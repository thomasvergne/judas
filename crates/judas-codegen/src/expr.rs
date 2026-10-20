use judas_ast::expr::{BinaryOp, Literal, UnaryOp};
use judas_infer::{ResolvedType, TypedCallArg, TypedExpr, TypedExprKind, WidthExpr};

use crate::context::{VerilogGenerator, sanitize_ident};
use crate::types::{compute_struct_field_range, format_affine};

impl VerilogGenerator {
    pub fn emit_expr(&self, expr: &TypedExpr) -> String {
        match &expr.kind {
            TypedExprKind::Literal(lit) => {
                let width = self.concrete_width_of(&expr.ty);
                self.format_literal(lit, width)
            }
            TypedExprKind::Ident(id) => {
                if let ResolvedType::Enum(enum_name) = &expr.ty
                    && let Some(enum_info) = self.enums.get(enum_name)
                    && let Some(disc) = enum_info.variants.get(&id.name)
                {
                    return format!("{}'d{}", enum_info.width, disc);
                }
                if let Some((w, disc)) = self.variant_lookup.get(&id.name) {
                    return format!("{}'d{}", w, disc);
                }
                id.name.clone()
            }
            TypedExprKind::Path(path) => {
                let path_str = path.to_string_path();
                if let Some((w, disc)) = self.variant_lookup.get(&path_str) {
                    return format!("{}'d{}", w, disc);
                }
                if let Some(last) = path.segments.last()
                    && let Some((w, disc)) = self.variant_lookup.get(&last.name)
                {
                    return format!("{}'d{}", w, disc);
                }
                sanitize_ident(&path_str)
            }
            TypedExprKind::FieldAccess { base, field } => {
                // 1. Interface port signal access: `bus.pc` -> `bus_pc`
                if let TypedExprKind::Ident(base_id) = &base.kind {
                    if self.current_interface_ports.contains(&base_id.name) {
                        return format!("{}_{}", base_id.name, field.name);
                    }
                    // 2. Submodule instance port access: `cpu.io_data` -> `cpu_io_data`
                    if self.current_instances.contains(&base_id.name) {
                        return format!("{}_{}", base_id.name, field.name);
                    }
                }

                // 3. Packed struct field extraction: `inst.opcode` -> `inst[15:12]`
                if let ResolvedType::Struct {
                    fields,
                    total_width,
                    ..
                } = &base.ty
                {
                    let base_str = self.emit_expr(base);
                    if let Some((msb, lsb)) =
                        compute_struct_field_range(fields, total_width, &field.name)
                    {
                        if msb == lsb {
                            return format!("{}[{}]", base_str, lsb);
                        } else {
                            return format!("{}[{}:{}]", base_str, msb, lsb);
                        }
                    }
                }

                format!("{}_{}", self.emit_expr(base), field.name)
            }
            TypedExprKind::Index { base, index } => {
                format!("{}[{}]", self.emit_expr(base), self.emit_expr(index))
            }
            TypedExprKind::Unary { op, expr: inner } => {
                let op_str = match op {
                    UnaryOp::Not => "!",
                    UnaryOp::Neg => "-",
                    UnaryOp::BitNot => "~",
                };
                let inner_str = self.emit_expr(inner);
                if matches!(inner.kind, TypedExprKind::Binary { .. }) {
                    format!("{}({})", op_str, inner_str)
                } else {
                    format!("{}{}", op_str, inner_str)
                }
            }
            TypedExprKind::Binary { op, left, right } => {
                let op_str = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                    BinaryOp::Mod => "%",
                    BinaryOp::Eq => "==",
                    BinaryOp::Ne => "!=",
                    BinaryOp::Lt => "<",
                    BinaryOp::Le => "<=",
                    BinaryOp::Gt => ">",
                    BinaryOp::Ge => ">=",
                    BinaryOp::And => "&&",
                    BinaryOp::Or => "||",
                    BinaryOp::BitAnd => "&",
                    BinaryOp::BitOr => "|",
                    BinaryOp::BitXor => "^",
                    BinaryOp::Shl => "<<",
                    BinaryOp::Shr => ">>",
                };
                let l_str = self.emit_operand(left);
                let r_str = self.emit_operand(right);
                format!("{} {} {}", l_str, op_str, r_str)
            }
            TypedExprKind::StructInit { fields, .. } => {
                // Pack fields in the declaration order of the struct
                if let ResolvedType::Struct {
                    fields: decl_fields,
                    ..
                } = &expr.ty
                {
                    let mut parts = Vec::new();
                    for (decl_name, _) in decl_fields {
                        if let Some(f_init) = fields.iter().find(|f| &f.name.name == decl_name) {
                            parts.push(self.emit_expr(&f_init.value));
                        }
                    }
                    format!("{{{}}}", parts.join(", "))
                } else {
                    let parts: Vec<String> =
                        fields.iter().map(|f| self.emit_expr(&f.value)).collect();
                    format!("{{{}}}", parts.join(", "))
                }
            }
            TypedExprKind::Call { callee, args } => self.emit_call(expr, callee, args),
        }
    }

    fn emit_operand(&self, expr: &TypedExpr) -> String {
        let s = self.emit_expr(expr);
        if matches!(expr.kind, TypedExprKind::Binary { .. }) {
            format!("({})", s)
        } else {
            s
        }
    }

    fn emit_call(&self, expr: &TypedExpr, callee: &TypedExpr, args: &[TypedCallArg]) -> String {
        if let TypedExprKind::Ident(fn_id) = &callee.kind {
            match fn_id.name.as_str() {
                "concat" => {
                    let parts: Vec<String> =
                        args.iter().map(|a| self.emit_expr(&a.value)).collect();
                    return format!("{{{}}}", parts.join(", "));
                }
                "repeat" => {
                    if let Some(arg) = args.first() {
                        let val_str = self.emit_expr(&arg.value);
                        let count_str = self.compute_repeat_count(&expr.ty, &arg.value.ty);
                        return format!("{{{}{{{}}}}}", count_str, val_str);
                    }
                }
                "slice" => {
                    if args.len() == 2 {
                        let sig_str = self.emit_expr(&args[0].value);
                        let slice_w = expr.ty.width_expr().unwrap_or(WidthExpr::Const(1));

                        if let TypedExprKind::Literal(Literal::Integer(offset)) =
                            &args[1].value.kind
                            && let Some(w_const) = slice_w.eval_concrete()
                        {
                            if w_const <= 1 {
                                return format!("{}[{}]", sig_str, offset);
                            } else {
                                let msb = *offset as u32 + w_const - 1;
                                return format!("{}[{}:{}]", sig_str, msb, offset);
                            }
                        }

                        let offset_str = self.emit_expr(&args[1].value);
                        return format!("{}[{} +: {}]", sig_str, offset_str, slice_w);
                    }
                }
                "extend" => {
                    if let Some(arg) = args.first() {
                        let inner_str = self.emit_expr(&arg.value);
                        let in_w = arg.value.ty.width_expr().unwrap_or(WidthExpr::Const(1));
                        let out_w = expr.ty.width_expr().unwrap_or(WidthExpr::Const(1));

                        if let (Some(out_aff), Some(in_aff)) = (out_w.to_affine(), in_w.to_affine())
                        {
                            let diff = out_aff.sub(&in_aff);
                            if diff.is_concrete() {
                                if diff.constant <= 0 {
                                    return inner_str;
                                } else if diff.constant == 1 {
                                    return format!("{{1'b0, {}}}", inner_str);
                                } else {
                                    return format!(
                                        "{{{{{} {{1'b0}}}}, {}}}",
                                        diff.constant, inner_str
                                    );
                                }
                            } else {
                                let diff_str = format_affine(&diff);
                                return format!("{{{{({}){{1'b0}}}}, {}}}", diff_str, inner_str);
                            }
                        }
                        return inner_str;
                    }
                }
                "truncate" => {
                    if let Some(arg) = args.first() {
                        let out_w = expr.ty.width_expr().unwrap_or(WidthExpr::Const(1));

                        // Special case: `truncate<1>(total >> N)` -> `total[N]`
                        if out_w.eval_concrete() == Some(1)
                            && let TypedExprKind::Binary {
                                op: BinaryOp::Shr,
                                left,
                                right,
                            } = &arg.value.kind
                        {
                            let base_str = self.emit_expr(left);
                            let idx_str = match &right.kind {
                                TypedExprKind::Literal(Literal::Integer(i)) => i.to_string(),
                                _ => self.emit_expr(right),
                            };
                            return format!("{}[{}]", base_str, idx_str);
                        }

                        let inner_str = self.emit_expr(&arg.value);
                        if matches!(
                            arg.value.kind,
                            TypedExprKind::Ident(_) | TypedExprKind::FieldAccess { .. }
                        ) {
                            if out_w.eval_concrete() == Some(1) {
                                return format!("{}[0]", inner_str);
                            } else {
                                let msb = self.format_msb(&out_w);
                                return format!("{}[{}:0]", inner_str, msb);
                            }
                        }
                        return inner_str;
                    }
                }
                "zero" => {
                    let w = self.concrete_width_of(&expr.ty);
                    return self.format_literal(&Literal::Integer(0), w);
                }
                _ => {}
            }
        }

        let callee_str = self.emit_expr(callee);
        let arg_strs: Vec<String> = args.iter().map(|a| self.emit_expr(&a.value)).collect();
        format!("{}({})", callee_str, arg_strs.join(", "))
    }
}
