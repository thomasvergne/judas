use std::collections::HashMap;

use judas_ast::expr::{BinaryOp, Expr, ExprKind, Literal};
use judas_ast::item::{InterfaceRole, InterfaceSignalDir, PortDirection};
use judas_ast::span::Span;
use judas_ast::types::{Type, TypeArg, TypeKind};

use crate::error::TypeError;
use crate::types::{ResolvedType, WidthExpr};

/// Represents the signature of a struct type.
///
/// A struct signature contains the fields of the struct and
/// the total width of the struct.
#[derive(Debug, Clone)]
pub struct StructSignature {
    pub fields: Vec<(String, ResolvedType)>,
    pub total_width: WidthExpr,
}

/// Represents the signature of an interface type.
///
/// An interface signature contains the signals of the interface.
#[derive(Debug, Clone)]
pub struct InterfaceSignature {
    pub signals: Vec<(InterfaceSignalDir, String, ResolvedType)>,
}

/// Represents the signature of a module type.
///
/// A module signature contains the parameters, ports, and
/// interface ports of the module.
#[derive(Debug, Clone)]
pub(crate) struct ModuleSignature {
    pub(crate) params: HashMap<String, (ResolvedType, Option<i64>)>,
    pub(crate) ports: HashMap<String, (PortDirection, Type)>,
    pub(crate) interface_ports: HashMap<String, (InterfaceRole, String)>,
}

/// Represents the context for type inference.
///
/// The infer context holds the unification table, symbol tables,
/// and current module scope.
pub struct InferContext {
    // Unification / type variable table
    pub(crate) substitutions: HashMap<u32, ResolvedType>,
    pub(crate) next_var_id: u32,

    // Global symbol tables
    pub(crate) enums: HashMap<String, Vec<String>>,
    pub(crate) variant_to_enum: HashMap<String, String>,
    pub(crate) struct_definitions: HashMap<String, StructSignature>,
    pub(crate) interface_definitions: HashMap<String, InterfaceSignature>,
    pub(crate) module_definitions: HashMap<String, ModuleSignature>,

    // Current module scope
    pub(crate) current_constants: HashMap<String, i64>,
    pub(crate) current_signals: HashMap<String, ResolvedType>,
    pub(crate) current_instances: HashMap<String, HashMap<String, (PortDirection, ResolvedType)>>,
}

impl Default for InferContext {
    fn default() -> Self {
        Self::new()
    }
}

impl InferContext {
    pub fn new() -> Self {
        Self {
            substitutions: HashMap::new(),
            next_var_id: 0,
            enums: HashMap::new(),
            variant_to_enum: HashMap::new(),
            struct_definitions: HashMap::new(),
            interface_definitions: HashMap::new(),
            module_definitions: HashMap::new(),
            current_constants: HashMap::new(),
            current_signals: HashMap::new(),
            current_instances: HashMap::new(),
        }
    }

    /// Returns the visible names in the current context.
    ///
    /// This includes signals, constants, variants, enums, structs,
    /// interfaces, and modules.
    ///
    /// Useful for debugging and error reporting.
    pub fn visible_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.current_signals.keys().cloned().collect();
        names.extend(self.current_constants.keys().cloned());
        names.extend(self.variant_to_enum.keys().cloned());
        names.extend(self.enums.keys().cloned());
        names.extend(self.struct_definitions.keys().cloned());
        names.extend(self.interface_definitions.keys().cloned());
        names.extend(self.module_definitions.keys().cloned());
        names
    }

    /// Creates a fresh type variable.
    ///
    /// This is used for unification and type inference.
    pub(crate) fn fresh_tyvar(&mut self) -> ResolvedType {
        let id = self.next_var_id;
        self.next_var_id += 1;
        ResolvedType::TyVar(id)
    }

    /// Creates a fresh infer integer type variable.
    ///
    /// This is used for unification and type inference.
    pub(crate) fn fresh_infer_int(&mut self) -> ResolvedType {
        let id = self.next_var_id;
        self.next_var_id += 1;
        ResolvedType::InferInt(id)
    }

    /// Resolves a type to its canonical form.
    ///
    /// This is used for unification and type inference.
    pub(crate) fn resolve_type(&self, ty: &Type) -> Result<ResolvedType, TypeError> {
        match &ty.kind {
            TypeKind::Clock => Ok(ResolvedType::Clock),
            TypeKind::Bool => Ok(ResolvedType::Bool),
            TypeKind::Bit => Ok(ResolvedType::Bit),
            TypeKind::UInt(arg) => {
                let w = self.resolve_width_arg(arg, ty.span)?;
                Ok(ResolvedType::UInt(w))
            }
            TypeKind::Int(arg) => {
                let w = self.resolve_width_arg(arg, ty.span)?;
                Ok(ResolvedType::Int(w))
            }
            TypeKind::Bits(arg) => {
                let w = self.resolve_width_arg(arg, ty.span)?;
                Ok(ResolvedType::Bits(w))
            }
            TypeKind::Array { element, size } => {
                let elem_res = self.resolve_type(element)?;
                let sz = self.resolve_width_arg(size, ty.span)?;
                Ok(ResolvedType::Array {
                    element: Box::new(elem_res),
                    size: sz,
                })
            }
            TypeKind::Named { name, args } => {
                if self.enums.contains_key(&name.name) {
                    if !args.is_empty() {
                        return Err(TypeError::new(
                            format!("Enum `{}` does not take generic arguments", name.name),
                            ty.span,
                        ));
                    }
                    Ok(ResolvedType::Enum(name.name.clone()))
                } else if let Some(sig) = self.struct_definitions.get(&name.name) {
                    if !args.is_empty() {
                        return Err(TypeError::new(
                            format!("Struct `{}` does not take generic arguments", name.name),
                            ty.span,
                        ));
                    }
                    Ok(ResolvedType::Struct {
                        name: name.name.clone(),
                        fields: sig.fields.clone(),
                        total_width: sig.total_width.clone(),
                    })
                } else if name.name == "usize" || name.name == "int" {
                    Ok(ResolvedType::uint(32))
                } else {
                    // Could be a generic type parameter (like T)
                    Ok(ResolvedType::Enum(name.name.clone()))
                }
            }
        }
    }

    /// Resolves a width argument to a width expression.
    pub(crate) fn resolve_width_arg(
        &self,
        arg: &TypeArg,
        span: Span,
    ) -> Result<WidthExpr, TypeError> {
        match arg {
            TypeArg::Expr(expr) => self.resolve_width_expr(expr),
            TypeArg::Type(ty) => match &ty.kind {
                TypeKind::Named { name, .. } => {
                    if let Some(val) = self.current_constants.get(&name.name) {
                        Ok(WidthExpr::Const(*val as u32))
                    } else {
                        Ok(WidthExpr::Var(name.name.clone()))
                    }
                }
                _ => Err(TypeError::new("Expected valid width expression", span)),
            },
        }
    }

    /// Resolves a width expression to a width expression.
    ///
    /// For instance, `10` is resolved to `WidthExpr::Const(10)`,
    /// and `width` is resolved to `WidthExpr::Var("width")`.
    pub(crate) fn resolve_width_expr(&self, expr: &Expr) -> Result<WidthExpr, TypeError> {
        match &expr.kind {
            ExprKind::Literal(Literal::Integer(val)) => Ok(WidthExpr::Const(*val as u32)),
            ExprKind::Ident(ident) => {
                if let Some(val) = self.current_constants.get(&ident.name) {
                    Ok(WidthExpr::Const(*val as u32))
                } else {
                    Ok(WidthExpr::Var(ident.name.clone()))
                }
            }
            ExprKind::Binary { op, left, right } => {
                let l = self.resolve_width_expr(left)?;
                let r = self.resolve_width_expr(right)?;
                match op {
                    BinaryOp::Add => match (&l, &r) {
                        (WidthExpr::Const(c1), WidthExpr::Const(c2)) => {
                            Ok(WidthExpr::Const(c1 + c2))
                        }
                        _ => Ok(WidthExpr::Add(Box::new(l), Box::new(r))),
                    },
                    BinaryOp::Sub => match (&l, &r) {
                        (WidthExpr::Const(c1), WidthExpr::Const(c2)) => {
                            Ok(WidthExpr::Const(c1.saturating_sub(*c2)))
                        }
                        _ => Ok(WidthExpr::Sub(Box::new(l), Box::new(r))),
                    },
                    BinaryOp::Mul => match (&l, &r) {
                        (WidthExpr::Const(c1), WidthExpr::Const(c2)) => {
                            Ok(WidthExpr::Const(c1 * c2))
                        }
                        _ => Ok(WidthExpr::Mul(Box::new(l), Box::new(r))),
                    },
                    _ => Err(TypeError::new(
                        "Unsupported operator in width expression",
                        expr.span,
                    )),
                }
            }
            _ => Err(TypeError::new("Expected valid width expression", expr.span)),
        }
    }

    /// Evaluates a constant expression to a concrete value.
    pub(crate) fn eval_const_expr(&self, expr: &Expr) -> Result<u32, TypeError> {
        let width = self.resolve_width_expr(expr)?;
        if let Some(c) = width.eval_concrete() {
            Ok(c)
        } else {
            Err(TypeError::new(
                "Expected constant integer expression",
                expr.span,
            ))
        }
    }
}
