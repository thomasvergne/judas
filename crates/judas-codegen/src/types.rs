use judas_ast::expr::Literal;
use judas_infer::{AffineForm, ResolvedType, WidthExpr};

use crate::context::VerilogGenerator;

impl VerilogGenerator {
    pub(crate) fn compute_repeat_count(&self, out_ty: &ResolvedType, in_ty: &ResolvedType) -> String {
        let out_w = out_ty.width_expr().unwrap_or(WidthExpr::Const(1));
        let in_w = in_ty.width_expr().unwrap_or(WidthExpr::Const(1));
        if let (Some(oc), Some(ic)) = (out_w.eval_concrete(), in_w.eval_concrete())
            && let Some(ratio) = oc.checked_div(ic)
        {
            return ratio.to_string();
        }
        format!("{}", out_w)
    }

    pub(crate) fn concrete_width_of(&self, ty: &ResolvedType) -> Option<u32> {
        match ty {
            ResolvedType::Clock | ResolvedType::Bool | ResolvedType::Bit => Some(1),
            ResolvedType::UInt(w) | ResolvedType::Int(w) | ResolvedType::Bits(w) => {
                w.eval_concrete()
            }
            ResolvedType::Struct { total_width, .. } => total_width.eval_concrete(),
            ResolvedType::Enum(name) => self.enums.get(name).map(|e| e.width),
            _ => None,
        }
    }

    pub(crate) fn format_literal(&self, lit: &Literal, width: Option<u32>) -> String {
        match lit {
            Literal::Bool(b) => {
                if *b {
                    "1'b1".to_string()
                } else {
                    "1'b0".to_string()
                }
            }
            Literal::Bit(b) => format!("1'b{}", b),
            Literal::Integer(val) => {
                if let Some(w) = width {
                    if *val < 0 {
                        format!("-{}'d{}", w, val.abs())
                    } else {
                        format!("{}'d{}", w, val)
                    }
                } else {
                    val.to_string()
                }
            }
            Literal::String(s) => format!("\"{}\"", s),
        }
    }

    pub(crate) fn format_type_spec(&self, ty: &ResolvedType) -> String {
        match ty {
            ResolvedType::Clock | ResolvedType::Bool | ResolvedType::Bit => String::new(),
            ResolvedType::UInt(w) | ResolvedType::Bits(w) => self.format_range(w),
            ResolvedType::Int(w) => {
                let r = self.format_range(w);
                if r.is_empty() {
                    "signed".to_string()
                } else {
                    format!("signed {}", r)
                }
            }
            ResolvedType::Struct { total_width, .. } => self.format_range(total_width),
            ResolvedType::Enum(name) => {
                let w = self.enums.get(name).map(|e| e.width).unwrap_or(1);
                if w <= 1 {
                    String::new()
                } else {
                    format!("[{}:0]", w - 1)
                }
            }
            _ => String::new(),
        }
    }

    pub(crate) fn format_range(&self, w: &WidthExpr) -> String {
        if let Some(c) = w.eval_concrete() {
            if c <= 1 {
                String::new()
            } else {
                format!("[{}:0]", c - 1)
            }
        } else {
            format!("[{}:0]", self.format_msb(w))
        }
    }

    pub(crate) fn format_msb(&self, w: &WidthExpr) -> String {
        if let Some(c) = w.eval_concrete() {
            c.saturating_sub(1).to_string()
        } else if let Some(mut aff) = w.to_affine() {
            aff.constant -= 1;
            format_affine(&aff)
        } else {
            format!("{} - 1", w)
        }
    }
}

pub(crate) fn format_affine(aff: &AffineForm) -> String {
    let mut parts = Vec::new();
    for (var, coeff) in &aff.terms {
        if *coeff == 1 {
            parts.push(var.clone());
        } else if *coeff == -1 {
            parts.push(format!("-{}", var));
        } else {
            parts.push(format!("{} * {}", coeff, var));
        }
    }
    if aff.constant > 0 {
        if parts.is_empty() {
            parts.push(aff.constant.to_string());
        } else {
            parts.push(format!("+ {}", aff.constant));
        }
    } else if aff.constant < 0 {
        if parts.is_empty() {
            parts.push(aff.constant.to_string());
        } else {
            parts.push(format!("- {}", aff.constant.abs()));
        }
    } else if parts.is_empty() {
        return "0".to_string();
    }
    parts.join(" ")
}

pub(crate) fn compute_struct_field_range(
    fields: &[(String, ResolvedType)],
    total_width: &WidthExpr,
    target_field: &str,
) -> Option<(u32, u32)> {
    let mut current_msb = total_width.eval_concrete()?;
    for (f_name, f_ty) in fields {
        let f_w = f_ty.width_expr()?.eval_concrete()?;
        let msb = current_msb.saturating_sub(1);
        let lsb = current_msb.saturating_sub(f_w);
        if f_name == target_field {
            return Some((msb, lsb));
        }
        current_msb = lsb;
    }
    None
}
