use judas_ast::item::InterfaceRole;
use judas_ast::span::Span;

use crate::context::InferContext;
use crate::error::TypeError;
use crate::types::{ResolvedType, WidthExpr};

impl InferContext {
    /// Follow substitutions to get root type
    ///
    /// Recursively follows type and applying substitutions until a
    /// concrete type is reached.
    pub fn normalize(&self, ty: &ResolvedType) -> ResolvedType {
        match ty {
            ResolvedType::TyVar(id) => {
                if let Some(target) = self.substitutions.get(id) {
                    self.normalize(target)
                } else {
                    ty.clone()
                }
            }
            ResolvedType::InferInt(id) => {
                if let Some(target) = self.substitutions.get(id) {
                    self.normalize(target)
                } else {
                    ty.clone()
                }
            }
            ResolvedType::Array { element, size } => ResolvedType::Array {
                element: Box::new(self.normalize(element)),
                size: size.clone(),
            },
            _ => ty.clone(),
        }
    }

    /// Default remaining unconstrained InferInt to UInt<32>
    pub fn finalize_type(&self, ty: &ResolvedType) -> ResolvedType {
        let norm = self.normalize(ty);
        match norm {
            ResolvedType::InferInt(_) => ResolvedType::uint(32),
            ResolvedType::Array { element, size } => ResolvedType::Array {
                element: Box::new(self.finalize_type(&element)),
                size,
            },
            _ => norm,
        }
    }

    /// Checks if a type contains a given type variable.
    ///
    /// Used to detect cycles in type variable substitutions.
    pub(crate) fn occurs_in(&self, var_id: u32, ty: &ResolvedType) -> bool {
        let norm = self.normalize(ty);
        match norm {
            ResolvedType::TyVar(id) | ResolvedType::InferInt(id) => id == var_id,
            ResolvedType::Array { element, .. } => self.occurs_in(var_id, &element),
            _ => false,
        }
    }

    /// Unifies two types, returning the resulting type.
    ///
    /// Recursively unifies type variables and applies substitutions as needed.
    pub fn unify(
        &mut self,
        t1: &ResolvedType,
        t2: &ResolvedType,
        span: Span,
    ) -> Result<ResolvedType, TypeError> {
        let norm1 = self.normalize(t1);
        let norm2 = self.normalize(t2);

        if norm1 == norm2 {
            return Ok(norm1);
        }

        match (&norm1, &norm2) {
            (ResolvedType::TyVar(id), other) | (other, ResolvedType::TyVar(id)) => {
                if self.occurs_in(*id, other) {
                    return Err(TypeError::new(
                        format!(
                            "Infinite type detected during unification of ?T{} with {}",
                            id, other
                        ),
                        span,
                    ));
                }
                self.substitutions.insert(*id, other.clone());
                Ok(other.clone())
            }

            (ResolvedType::InferInt(id), other) | (other, ResolvedType::InferInt(id)) => {
                match other {
                    ResolvedType::UInt(_)
                    | ResolvedType::Int(_)
                    | ResolvedType::Bits(_)
                    | ResolvedType::Bit => {
                        self.substitutions.insert(*id, other.clone());
                        Ok(other.clone())
                    }
                    ResolvedType::InferInt(other_id) => {
                        if id != other_id {
                            self.substitutions.insert(*id, other.clone());
                        }
                        Ok(other.clone())
                    }
                    _ => Err(TypeError::new(
                        format!("Cannot unify integer literal with type `{}`", other),
                        span,
                    )),
                }
            }

            (
                ResolvedType::Array {
                    element: e1,
                    size: s1,
                },
                ResolvedType::Array {
                    element: e2,
                    size: s2,
                },
            ) => {
                if !s1.is_equivalent_to(s2) {
                    return Err(TypeError::new(
                        format!(
                            "Array size mismatch: expected Array<_, {}>, found Array<_, {}>",
                            s1, s2
                        ),
                        span,
                    ));
                }
                let elem_unified = self.unify(e1, e2, span)?;
                Ok(ResolvedType::Array {
                    element: Box::new(elem_unified),
                    size: s1.clone(),
                })
            }

            (ResolvedType::UInt(w1), ResolvedType::UInt(w2)) => {
                if w1.is_equivalent_to(w2) {
                    Ok(ResolvedType::UInt(w1.clone()))
                } else {
                    Err(TypeError::width_mismatch(
                        format!("UInt<{}>", w1),
                        format!("UInt<{}>", w2),
                        w1,
                        w2,
                        span,
                    ))
                }
            }

            (ResolvedType::Int(w1), ResolvedType::Int(w2)) => {
                if w1.is_equivalent_to(w2) {
                    Ok(ResolvedType::Int(w1.clone()))
                } else {
                    Err(TypeError::width_mismatch(
                        format!("Int<{}>", w1),
                        format!("Int<{}>", w2),
                        w1,
                        w2,
                        span,
                    ))
                }
            }

            (ResolvedType::Bits(w1), ResolvedType::Bits(w2)) => {
                if w1.is_equivalent_to(w2) {
                    Ok(ResolvedType::Bits(w1.clone()))
                } else {
                    Err(TypeError::width_mismatch(
                        format!("Bits<{}>", w1),
                        format!("Bits<{}>", w2),
                        w1,
                        w2,
                        span,
                    ))
                }
            }

            (ResolvedType::Enum(e1), ResolvedType::Enum(e2)) => {
                if e1 == e2 {
                    Ok(ResolvedType::Enum(e1.clone()))
                } else {
                    Err(TypeError::new(
                        format!("Enum mismatch: expected enum `{}`, found enum `{}`", e1, e2),
                        span,
                    ))
                }
            }

            (ResolvedType::Struct { name: n1, .. }, ResolvedType::Struct { name: n2, .. }) => {
                if n1 == n2 {
                    Ok(norm1)
                } else {
                    Err(TypeError::new(
                        format!("Struct type mismatch: expected `{}`, found `{}`", n1, n2),
                        span,
                    ))
                }
            }

            (
                ResolvedType::Struct {
                    total_width: sw, ..
                },
                ResolvedType::Bits(bw),
            )
            | (
                ResolvedType::Bits(bw),
                ResolvedType::Struct {
                    total_width: sw, ..
                },
            ) => {
                if sw.is_equivalent_to(bw) {
                    Ok(norm1)
                } else {
                    Err(TypeError::width_mismatch(
                        format!("{}", norm1),
                        format!("{}", norm2),
                        sw,
                        bw,
                        span,
                    ))
                }
            }

            (
                ResolvedType::InterfaceInstance {
                    name: n1, role: r1, ..
                },
                ResolvedType::InterfaceInstance {
                    name: n2, role: r2, ..
                },
            ) => {
                if n1 == n2 && r1 == r2 {
                    Ok(norm1)
                } else if n1 != n2 {
                    Err(TypeError::new(
                        format!(
                            "Interface mismatch: expected interface `{}`, found `{}`",
                            n1, n2
                        ),
                        span,
                    ))
                } else {
                    Err(TypeError::interface_role_mismatch(
                        n1,
                        match r1 {
                            InterfaceRole::Master => "master",
                            InterfaceRole::Slave => "slave",
                        },
                        match r2 {
                            InterfaceRole::Master => "master",
                            InterfaceRole::Slave => "slave",
                        },
                        span,
                    ))
                }
            }

            (ResolvedType::Bit, ResolvedType::Bool) | (ResolvedType::Bool, ResolvedType::Bit) => {
                Ok(ResolvedType::Bool)
            }

            (ResolvedType::Bit, ResolvedType::UInt(w))
            | (ResolvedType::UInt(w), ResolvedType::Bit)
                if w.is_equivalent_to(&WidthExpr::Const(1)) =>
            {
                Ok(ResolvedType::Bit)
            }

            (ResolvedType::Bit, ResolvedType::Bits(w))
            | (ResolvedType::Bits(w), ResolvedType::Bit)
                if w.is_equivalent_to(&WidthExpr::Const(1)) =>
            {
                Ok(ResolvedType::Bit)
            }

            _ => Err(TypeError::new(
                format!("Type mismatch: expected `{}`, found `{}`", norm1, norm2),
                span,
            )),
        }
    }
}
