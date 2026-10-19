use judas_ast::{InterfaceRole, PortDirection};
use std::collections::{BTreeMap, HashMap};
use std::fmt;

/// Represents a width expression, which can be a constant, a variable,
/// or an arithmetic expression. Used to represent a wide variety of
/// width expressions.
///
/// # Examples
///
/// ```text
/// module test(N: usize) {
///     ...
///     input entry: UInt<N>
///
///     output exit: UInt<N + 1>
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WidthExpr {
    Const(u32),
    Var(String),
    Add(Box<WidthExpr>, Box<WidthExpr>),
    Sub(Box<WidthExpr>, Box<WidthExpr>),
    Mul(Box<WidthExpr>, Box<WidthExpr>),
}

/// Represents an affine form, which is a linear combination of
/// variables and a constant.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AffineForm {
    pub constant: i64,
    pub terms: BTreeMap<String, i64>,
}

impl AffineForm {
    pub fn new_const(val: i64) -> Self {
        Self {
            constant: val,
            terms: BTreeMap::new(),
        }
    }

    pub fn new_var(name: impl Into<String>) -> Self {
        let mut terms = BTreeMap::new();
        terms.insert(name.into(), 1);
        Self { constant: 0, terms }
    }

    pub fn add(&self, other: &Self) -> Self {
        let mut terms = self.terms.clone();
        for (k, v) in &other.terms {
            *terms.entry(k.clone()).or_insert(0) += v;
        }
        terms.retain(|_, v| *v != 0);
        Self {
            constant: self.constant + other.constant,
            terms,
        }
    }

    pub fn sub(&self, other: &Self) -> Self {
        let mut terms = self.terms.clone();
        for (k, v) in &other.terms {
            *terms.entry(k.clone()).or_insert(0) -= v;
        }
        terms.retain(|_, v| *v != 0);
        Self {
            constant: self.constant - other.constant,
            terms,
        }
    }

    pub fn mul_const(&self, k: i64) -> Self {
        let mut terms = self.terms.clone();
        for v in terms.values_mut() {
            *v *= k;
        }
        terms.retain(|_, v| *v != 0);
        Self {
            constant: self.constant * k,
            terms,
        }
    }

    pub fn is_concrete(&self) -> bool {
        self.terms.is_empty()
    }
}

impl WidthExpr {
    pub fn to_affine(&self) -> Option<AffineForm> {
        match self {
            WidthExpr::Const(c) => Some(AffineForm::new_const(*c as i64)),
            WidthExpr::Var(name) => Some(AffineForm::new_var(name)),
            WidthExpr::Add(a, b) => {
                let l = a.to_affine()?;
                let r = b.to_affine()?;
                Some(l.add(&r))
            }
            WidthExpr::Sub(a, b) => {
                let l = a.to_affine()?;
                let r = b.to_affine()?;
                Some(l.sub(&r))
            }
            WidthExpr::Mul(a, b) => {
                let l = a.to_affine()?;
                let r = b.to_affine()?;
                if l.is_concrete() {
                    Some(r.mul_const(l.constant))
                } else if r.is_concrete() {
                    Some(l.mul_const(r.constant))
                } else {
                    None
                }
            }
        }
    }

    pub fn is_equivalent_to(&self, other: &WidthExpr) -> bool {
        if self == other {
            return true;
        }
        match (self.to_affine(), other.to_affine()) {
            (Some(a1), Some(a2)) => a1 == a2,
            _ => false,
        }
    }

    pub fn substitute(&self, var: &str, val: u32) -> Self {
        match self {
            WidthExpr::Const(c) => WidthExpr::Const(*c),
            WidthExpr::Var(name) => {
                if name == var {
                    WidthExpr::Const(val)
                } else {
                    WidthExpr::Var(name.clone())
                }
            }
            WidthExpr::Add(a, b) => {
                let l = a.substitute(var, val);
                let r = b.substitute(var, val);
                match (&l, &r) {
                    (WidthExpr::Const(c1), WidthExpr::Const(c2)) => WidthExpr::Const(c1 + c2),
                    _ => WidthExpr::Add(Box::new(l), Box::new(r)),
                }
            }
            WidthExpr::Sub(a, b) => {
                let l = a.substitute(var, val);
                let r = b.substitute(var, val);
                match (&l, &r) {
                    (WidthExpr::Const(c1), WidthExpr::Const(c2)) => {
                        WidthExpr::Const(c1.saturating_sub(*c2))
                    }
                    _ => WidthExpr::Sub(Box::new(l), Box::new(r)),
                }
            }
            WidthExpr::Mul(a, b) => {
                let l = a.substitute(var, val);
                let r = b.substitute(var, val);
                match (&l, &r) {
                    (WidthExpr::Const(c1), WidthExpr::Const(c2)) => WidthExpr::Const(c1 * c2),
                    _ => WidthExpr::Mul(Box::new(l), Box::new(r)),
                }
            }
        }
    }

    pub fn eval_concrete(&self) -> Option<u32> {
        let aff = self.to_affine()?;
        if aff.is_concrete() && aff.constant >= 0 {
            Some(aff.constant as u32)
        } else {
            None
        }
    }
}

impl fmt::Display for WidthExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WidthExpr::Const(c) => write!(f, "{}", c),
            WidthExpr::Var(name) => write!(f, "{}", name),
            WidthExpr::Add(a, b) => write!(f, "{} + {}", a, b),
            WidthExpr::Sub(a, b) => write!(f, "{} - {}", a, b),
            WidthExpr::Mul(a, b) => write!(f, "{} * {}", a, b),
        }
    }
}

/// Represents a resolved type, which is a type that has been fully resolved
/// and does not contain any type variables.
///
/// # Examples
///
/// ```text
/// module test {
///     ...
///     input entry: UInt<N>
///     input test: Bits<8>
///     input field: UInt<N + 1>
///
///     output exit: UInt<N + 1>
///     output exit2: UInt<N - 1>
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedType {
    Clock,
    Bool,
    Bit,
    UInt(WidthExpr),
    Int(WidthExpr),
    Bits(WidthExpr),
    Array {
        element: Box<ResolvedType>,
        size: WidthExpr,
    },
    Enum(String),
    Struct {
        name: String,
        fields: Vec<(String, ResolvedType)>,
        total_width: WidthExpr,
    },
    InterfaceInstance {
        name: String,
        role: InterfaceRole,
        signals: HashMap<String, (PortDirection, ResolvedType)>,
    },
    ModuleInstance {
        module_name: String,
        ports: HashMap<String, (PortDirection, ResolvedType)>,
    },
    TyVar(u32),
    InferInt(u32),
    Void,
}

impl ResolvedType {
    pub fn uint(w: u32) -> Self {
        ResolvedType::UInt(WidthExpr::Const(w))
    }

    pub fn int(w: u32) -> Self {
        ResolvedType::Int(WidthExpr::Const(w))
    }

    pub fn bits(w: u32) -> Self {
        ResolvedType::Bits(WidthExpr::Const(w))
    }

    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            ResolvedType::UInt(_) | ResolvedType::Int(_) | ResolvedType::InferInt(_)
        )
    }

    pub fn is_numeric_or_bits(&self) -> bool {
        matches!(
            self,
            ResolvedType::UInt(_)
                | ResolvedType::Int(_)
                | ResolvedType::Bits(_)
                | ResolvedType::Bit
                | ResolvedType::InferInt(_)
                | ResolvedType::Struct { .. }
        )
    }

    pub fn width_expr(&self) -> Option<WidthExpr> {
        match self {
            ResolvedType::Bool | ResolvedType::Bit => Some(WidthExpr::Const(1)),
            ResolvedType::UInt(w) | ResolvedType::Int(w) | ResolvedType::Bits(w) => Some(w.clone()),
            ResolvedType::Struct { total_width, .. } => Some(total_width.clone()),
            _ => None,
        }
    }

    pub fn is_equivalent_to(&self, other: &Self) -> bool {
        match (self, other) {
            (ResolvedType::Clock, ResolvedType::Clock)
            | (ResolvedType::Bool, ResolvedType::Bool)
            | (ResolvedType::Bit, ResolvedType::Bit)
            | (ResolvedType::Void, ResolvedType::Void) => true,
            (ResolvedType::UInt(w1), ResolvedType::UInt(w2))
            | (ResolvedType::Int(w1), ResolvedType::Int(w2))
            | (ResolvedType::Bits(w1), ResolvedType::Bits(w2)) => w1.is_equivalent_to(w2),
            (
                ResolvedType::Array {
                    element: e1,
                    size: s1,
                },
                ResolvedType::Array {
                    element: e2,
                    size: s2,
                },
            ) => e1.is_equivalent_to(e2) && s1.is_equivalent_to(s2),
            (ResolvedType::Enum(name1), ResolvedType::Enum(name2)) => name1 == name2,
            (ResolvedType::Struct { name: n1, .. }, ResolvedType::Struct { name: n2, .. }) => {
                n1 == n2
            }
            (
                ResolvedType::InterfaceInstance {
                    name: n1, role: r1, ..
                },
                ResolvedType::InterfaceInstance {
                    name: n2, role: r2, ..
                },
            ) => n1 == n2 && r1 == r2,
            (ResolvedType::TyVar(id1), ResolvedType::TyVar(id2)) => id1 == id2,
            (ResolvedType::InferInt(id1), ResolvedType::InferInt(id2)) => id1 == id2,
            _ => false,
        }
    }
}

impl fmt::Display for ResolvedType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolvedType::Clock => write!(f, "Clock"),
            ResolvedType::Bool => write!(f, "Bool"),
            ResolvedType::Bit => write!(f, "Bit"),
            ResolvedType::UInt(w) => write!(f, "UInt<{}>", w),
            ResolvedType::Int(w) => write!(f, "Int<{}>", w),
            ResolvedType::Bits(w) => write!(f, "Bits<{}>", w),
            ResolvedType::Array { element, size } => write!(f, "Array<{}, {}>", element, size),
            ResolvedType::Enum(name) => write!(f, "{}", name),
            ResolvedType::Struct { name, .. } => write!(f, "struct {}", name),
            ResolvedType::InterfaceInstance { name, role, .. } => write!(
                f,
                "{} interface {}",
                match role {
                    InterfaceRole::Master => "master",
                    InterfaceRole::Slave => "slave",
                },
                name
            ),
            ResolvedType::ModuleInstance { module_name, .. } => write!(f, "module {}", module_name),
            ResolvedType::TyVar(id) => write!(f, "?T{}", id),
            ResolvedType::InferInt(id) => write!(f, "{{integer#{}}}", id),
            ResolvedType::Void => write!(f, "()"),
        }
    }
}
