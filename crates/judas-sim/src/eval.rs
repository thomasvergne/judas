use std::collections::HashMap;

use judas_ast::expr::{BinaryOp, Literal, UnaryOp};
use judas_ast::stmt::PatternKind;
use judas_infer::{
    ResolvedType, TypedCallArg, TypedExpr, TypedExprKind, TypedStmt, TypedStmtKind,
};

use crate::context::{SimInstance, Simulator};
use crate::error::SimError;
use crate::value::{SimValue, bit_mask};

impl Simulator {
    pub(crate) fn exec_stmt(
        &self,
        inst: &mut SimInstance,
        stmt: &TypedStmt,
        staged_signals: &mut HashMap<String, SimValue>,
        staged_arrays: &mut HashMap<(String, usize), SimValue>,
        cycle: u64,
        time_ns: u64,
    ) -> Result<(), SimError> {
        match &stmt.kind {
            TypedStmtKind::Let { name, value, .. } => {
                // Procedural `let` updates immediately (blocking semantics)
                let val = self.eval_expr(inst, value);
                inst.set_signal(&name.name, val);
            }
            TypedStmtKind::Assign { target, value } | TypedStmtKind::Connect { target, value } => {
                let val = self.eval_expr(inst, value);

                // Array element assignment: `mem[idx] = val`
                if let TypedExprKind::Index { base, index } = &target.kind
                    && let TypedExprKind::Ident(arr_id) = &base.kind
                    && inst.arrays.contains_key(&arr_id.name)
                {
                    let idx_val = self.eval_expr(inst, index).bits as usize;
                    staged_arrays.insert((arr_id.name.clone(), idx_val), val);
                    return Ok(());
                }

                if let Some(sig_key) = Self::resolve_local_target_key(inst, target) {
                    if inst.procedural_lets.contains(&sig_key) {
                        // Blocking update for local procedural variable
                        inst.set_signal(&sig_key, val);
                    } else {
                        // Non-blocking staged update for registers and output ports
                        staged_signals.insert(sig_key, val);
                    }
                }
            }
            TypedStmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond = self.eval_expr(inst, condition);
                if cond.as_bool() {
                    for s in then_branch {
                        self.exec_stmt(inst, s, staged_signals, staged_arrays, cycle, time_ns)?;
                    }
                } else if let Some(else_stmts) = else_branch {
                    for s in else_stmts {
                        self.exec_stmt(inst, s, staged_signals, staged_arrays, cycle, time_ns)?;
                    }
                }
            }
            TypedStmtKind::Match { scrutinee, arms } => {
                let scrut_val = self.eval_expr(inst, scrutinee);
                for arm in arms {
                    let matched = match &arm.pattern.kind {
                        PatternKind::Wildcard => true,
                        PatternKind::Literal(lit) => {
                            let pat_val = self.eval_literal(lit, Some(scrut_val.width));
                            scrut_val.bits == pat_val.bits
                        }
                        PatternKind::Ident(id) => {
                            if let Some((_, disc)) = self.variant_lookup.get(&id.name) {
                                scrut_val.bits == (*disc as u128)
                            } else if let Some(sig) = inst.signals.get(&id.name) {
                                scrut_val.bits == sig.bits
                            } else {
                                false
                            }
                        }
                    };
                    if matched {
                        for s in &arm.body {
                            self.exec_stmt(inst, s, staged_signals, staged_arrays, cycle, time_ns)?;
                        }
                        break;
                    }
                }
            }
            TypedStmtKind::Assert { condition, message } => {
                let cond = self.eval_expr(inst, condition);
                if !cond.as_bool() {
                    return Err(SimError::AssertionFailed {
                        instance: inst.instance_name.clone(),
                        cycle,
                        time_ns,
                        message: message
                            .clone()
                            .unwrap_or_else(|| "Assertion failed".to_string()),
                    });
                }
            }
            TypedStmtKind::Expr(expr) => {
                let _ = self.eval_expr(inst, expr);
            }
        }
        Ok(())
    }

    pub(crate) fn eval_expr(&self, inst: &SimInstance, expr: &TypedExpr) -> SimValue {
        let (target_w, target_signed) = self.eval_type_width(&expr.ty, &inst.params);
        match &expr.kind {
            TypedExprKind::Literal(lit) => self.eval_literal(lit, Some(target_w)),
            TypedExprKind::Ident(id) => {
                if let ResolvedType::Enum(enum_name) = &expr.ty
                    && let Some(enum_info) = self.enums.get(enum_name)
                    && let Some(&disc) = enum_info.variants.get(&id.name)
                {
                    return SimValue::new(disc as u128, enum_info.width, false);
                }
                if let Some(&(w, disc)) = self.variant_lookup.get(&id.name) {
                    return SimValue::new(disc as u128, w, false);
                }
                if let Some(&val) = inst.signals.get(&id.name) {
                    return val;
                }
                if let Some(&param_val) = inst.params.get(&id.name) {
                    return SimValue::new(u128::from(param_val), target_w, target_signed);
                }
                SimValue::zero(target_w, target_signed)
            }
            TypedExprKind::Path(path) => {
                let path_str = path.to_string_path();
                if let Some(&(w, disc)) = self.variant_lookup.get(&path_str) {
                    return SimValue::new(disc as u128, w, false);
                }
                if let Some(last) = path.segments.last()
                    && let Some(&(w, disc)) = self.variant_lookup.get(&last.name)
                {
                    return SimValue::new(disc as u128, w, false);
                }
                SimValue::zero(target_w, target_signed)
            }
            TypedExprKind::FieldAccess { base, field } => {
                if let TypedExprKind::Ident(base_id) = &base.kind {
                    // 1. Local interface port signal: `bus.pc`
                    if inst.interface_ports.contains(&base_id.name) {
                        let key = format!("{}.{}", base_id.name, field.name);
                        if let Some(&val) = inst.signals.get(&key) {
                            return val;
                        }
                    }
                    // 2. Child module instance output port: `cpu.io_data`
                    if let Some(&child_idx) = inst.child_indices.get(&base_id.name)
                        && let Some(&val) = inst.children[child_idx].signals.get(&field.name)
                    {
                        return val;
                    }
                }

                // 3. Packed struct field extraction: `inst.opcode`
                if let ResolvedType::Struct {
                    fields,
                    total_width,
                    ..
                } = &base.ty
                {
                    let base_val = self.eval_expr(inst, base);
                    if let Some((msb, lsb)) = self.compute_struct_field_range(
                        fields,
                        total_width,
                        &field.name,
                        &inst.params,
                    ) {
                        let f_width = msb - lsb + 1;
                        let extracted = (base_val.bits >> lsb) & bit_mask(f_width);
                        return SimValue::new(extracted, f_width, target_signed);
                    }
                }

                SimValue::zero(target_w, target_signed)
            }
            TypedExprKind::Index { base, index } => {
                let idx = self.eval_expr(inst, index).bits as usize;
                if let TypedExprKind::Ident(arr_id) = &base.kind
                    && let Some(arr) = inst.arrays.get(&arr_id.name)
                {
                    return arr
                        .get(idx)
                        .copied()
                        .unwrap_or_else(|| SimValue::zero(target_w, target_signed));
                }
                let base_val = self.eval_expr(inst, base);
                let bit = (base_val.bits >> idx) & 1;
                SimValue::new(bit, 1, false)
            }
            TypedExprKind::Unary { op, expr: inner } => {
                let v = self.eval_expr(inst, inner);
                match op {
                    UnaryOp::Not => SimValue::from_bool(!v.as_bool()),
                    UnaryOp::Neg => SimValue::new(v.bits.wrapping_neg(), target_w, target_signed),
                    UnaryOp::BitNot => SimValue::new(!v.bits, target_w, target_signed),
                }
            }
            TypedExprKind::Binary { op, left, right } => {
                let l = self.eval_expr(inst, left);
                let r = self.eval_expr(inst, right);
                let bits = match op {
                    BinaryOp::Add => l.bits.wrapping_add(r.bits),
                    BinaryOp::Sub => l.bits.wrapping_sub(r.bits),
                    BinaryOp::Mul => l.bits.wrapping_mul(r.bits),
                    BinaryOp::Div => {
                        if r.bits == 0 {
                            0
                        } else if l.is_signed && r.is_signed {
                            (l.as_signed().wrapping_div(r.as_signed())) as u128
                        } else {
                            l.bits / r.bits
                        }
                    }
                    BinaryOp::Mod => {
                        if r.bits == 0 {
                            0
                        } else if l.is_signed && r.is_signed {
                            (l.as_signed().wrapping_rem(r.as_signed())) as u128
                        } else {
                            l.bits % r.bits
                        }
                    }
                    BinaryOp::Eq => u128::from(l.bits == r.bits),
                    BinaryOp::Ne => u128::from(l.bits != r.bits),
                    BinaryOp::Lt => {
                        if l.is_signed && r.is_signed {
                            u128::from(l.as_signed() < r.as_signed())
                        } else {
                            u128::from(l.bits < r.bits)
                        }
                    }
                    BinaryOp::Le => {
                        if l.is_signed && r.is_signed {
                            u128::from(l.as_signed() <= r.as_signed())
                        } else {
                            u128::from(l.bits <= r.bits)
                        }
                    }
                    BinaryOp::Gt => {
                        if l.is_signed && r.is_signed {
                            u128::from(l.as_signed() > r.as_signed())
                        } else {
                            u128::from(l.bits > r.bits)
                        }
                    }
                    BinaryOp::Ge => {
                        if l.is_signed && r.is_signed {
                            u128::from(l.as_signed() >= r.as_signed())
                        } else {
                            u128::from(l.bits >= r.bits)
                        }
                    }
                    BinaryOp::And => u128::from(l.as_bool() && r.as_bool()),
                    BinaryOp::Or => u128::from(l.as_bool() || r.as_bool()),
                    BinaryOp::BitAnd => l.bits & r.bits,
                    BinaryOp::BitOr => l.bits | r.bits,
                    BinaryOp::BitXor => l.bits ^ r.bits,
                    BinaryOp::Shl => {
                        let shift = (r.bits as u32).min(127);
                        l.bits.wrapping_shl(shift)
                    }
                    BinaryOp::Shr => {
                        let shift = (r.bits as u32).min(127);
                        if l.is_signed {
                            (l.as_signed() >> shift) as u128
                        } else {
                            l.bits.wrapping_shr(shift)
                        }
                    }
                };
                SimValue::new(bits, target_w, target_signed)
            }
            TypedExprKind::StructInit { fields, .. } => {
                if let ResolvedType::Struct {
                    fields: decl_fields,
                    ..
                } = &expr.ty
                {
                    let mut packed: u128 = 0;
                    for (decl_name, decl_ty) in decl_fields {
                        let (f_w, _) = self.eval_type_width(decl_ty, &inst.params);
                        let f_val = fields
                            .iter()
                            .find(|f| &f.name.name == decl_name)
                            .map(|f| self.eval_expr(inst, &f.value).bits)
                            .unwrap_or(0)
                            & bit_mask(f_w);
                        packed = (packed << f_w) | f_val;
                    }
                    SimValue::new(packed, target_w, false)
                } else {
                    SimValue::zero(target_w, false)
                }
            }
            TypedExprKind::Call { callee, args } => {
                self.eval_call(inst, expr, callee, args, target_w, target_signed)
            }
        }
    }

    fn eval_call(
        &self,
        inst: &SimInstance,
        _expr: &TypedExpr,
        callee: &TypedExpr,
        args: &[TypedCallArg],
        target_w: u32,
        target_signed: bool,
    ) -> SimValue {
        if let TypedExprKind::Ident(fn_id) = &callee.kind {
            match fn_id.name.as_str() {
                "concat" => {
                    let mut acc: u128 = 0;
                    for arg in args {
                        let v = self.eval_expr(inst, &arg.value);
                        acc = (acc << v.width) | (v.bits & bit_mask(v.width));
                    }
                    return SimValue::new(acc, target_w, target_signed);
                }
                "repeat" => {
                    if let Some(first) = args.first() {
                        let v = self.eval_expr(inst, &first.value);
                        let count = target_w.checked_div(v.width).unwrap_or(0);
                        let mut acc: u128 = 0;
                        for _ in 0..count {
                            acc = (acc << v.width) | (v.bits & bit_mask(v.width));
                        }
                        return SimValue::new(acc, target_w, target_signed);
                    }
                }
                "slice" => {
                    if args.len() == 2 {
                        let base_val = self.eval_expr(inst, &args[0].value);
                        let offset = self.eval_expr(inst, &args[1].value).bits as u32;
                        let sliced = (base_val.bits >> offset) & bit_mask(target_w);
                        return SimValue::new(sliced, target_w, target_signed);
                    }
                }
                "extend" => {
                    if let Some(first) = args.first() {
                        let v = self.eval_expr(inst, &first.value);
                        if v.is_signed {
                            return SimValue::new(v.as_signed() as u128, target_w, target_signed);
                        } else {
                            return SimValue::new(v.bits, target_w, target_signed);
                        }
                    }
                }
                "truncate" => {
                    if let Some(first) = args.first() {
                        let v = self.eval_expr(inst, &first.value);
                        return SimValue::new(v.bits, target_w, target_signed);
                    }
                }
                "zero" => {
                    return SimValue::zero(target_w, target_signed);
                }
                _ => {}
            }
        }
        SimValue::zero(target_w, target_signed)
    }

    pub(crate) fn eval_literal(&self, lit: &Literal, width: Option<u32>) -> SimValue {
        let w = width.unwrap_or(32).max(1);
        match lit {
            Literal::Bool(b) => SimValue::from_bool(*b),
            Literal::Bit(b) => SimValue::new(u128::from(*b), 1, false),
            Literal::Integer(i) => SimValue::new(*i as u128, w, *i < 0),
            Literal::String(_) => SimValue::zero(w, false),
        }
    }

    pub(crate) fn resolve_local_target_key(
        inst: &SimInstance,
        target: &TypedExpr,
    ) -> Option<String> {
        match &target.kind {
            TypedExprKind::Ident(id) => Some(id.name.clone()),
            TypedExprKind::FieldAccess { base, field } => {
                if let TypedExprKind::Ident(base_id) = &base.kind
                    && inst.interface_ports.contains(&base_id.name)
                {
                    return Some(format!("{}.{}", base_id.name, field.name));
                }
                None
            }
            _ => None,
        }
    }

    pub(crate) fn expr_key(expr: &TypedExpr) -> String {
        match &expr.kind {
            TypedExprKind::Ident(id) => id.name.clone(),
            TypedExprKind::FieldAccess { base, field } => {
                format!("{}.{}", Self::expr_key(base), field.name)
            }
            _ => "clk".to_string(),
        }
    }
}
