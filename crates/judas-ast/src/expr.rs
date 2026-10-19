use crate::path::Path;
use crate::span::Span;
use crate::types::{Ident, TypeArg};

/// Represents a literal value.
///
/// A `Literal` is a literal value such as an integer,
/// boolean, bit, or string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Literal {
    Integer(i128),
    Bool(bool),
    Bit(u8),
    String(String),
}

/// Represents a binary operation.
///
/// A `BinaryOp` is an operator such as `+`, `-`, `*`, `/`, `%`,
///  `==`, `!=`, `<`, `<=`, `>`, `>=`, `&&`, `||`, `&`, `|`, `^`,
///  `<<`, `>>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

/// Represents a unary operation.
///
/// A `UnaryOp` is an operator such as `!`, `-`, `~`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Neg,
    BitNot,
}

/// Represents a call argument.
///
/// A `CallArg` is a name-value pair used as an argument
/// to a function call. For instance, the following call
/// to `foo` has two arguments: `bar` and `baz`.
///
/// ```text
/// foo(bar, baz);
/// ```
///
/// Whereas the following has one named argument `bar`:
///
/// ```text
/// foo(bar: 10);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallArg {
    pub name: Option<Ident>,
    pub value: Expr,
    pub span: Span,
}

/// Represents the kind of an expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprKind {
    /// As specified above, a literal value such as `10`,
    /// `true`, or `"hello"`.
    Literal(Literal),

    /// A reference to an identifier such as `foo`.
    Ident(Ident),

    /// A reference to a qualified path such as `math::Adder` or `a::b::c`.
    Path(Path),

    /// A field access expression such as `foo.bar`.
    FieldAccess { base: Box<Expr>, field: Ident },

    /// A call expression such as `foo(bar, baz)`, or
    /// `foo(bar: 10, baz, foo: 20)`, or even `extend<N + 1>(cin)`.
    Call {
        callee: Box<Expr>,
        generic_args: Vec<TypeArg>,
        args: Vec<CallArg>,
    },

    /// A unary expression such as `!foo` or `-bar`.
    Unary { op: UnaryOp, expr: Box<Expr> },

    /// A binary expression such as `foo + bar` or `foo * bar`.
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },

    /// An index expression such as `foo[bar]`.
    Index { base: Box<Expr>, index: Box<Expr> },

    /// A struct initialization expression such as `Point { x: 1, y: 2 }`
    /// or `math::Point { x: 1, y: 2 }`.
    StructInit { path: Path, fields: Vec<FieldInit> },
}

/// Represents a field initialization in a struct literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldInit {
    pub name: Ident,
    pub value: Expr,
    pub span: Span,
}

impl FieldInit {
    pub fn new(name: Ident, value: Expr, span: Span) -> Self {
        Self { name, value, span }
    }
}

/// Represents an expression associated with a span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

impl Expr {
    pub fn new(kind: ExprKind, span: Span) -> Self {
        Self { kind, span }
    }
}
