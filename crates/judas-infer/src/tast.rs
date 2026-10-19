use crate::types::{ResolvedType, WidthExpr};
use judas_ast::*;

/// Represents a field initialization in a typed struct.
///
/// # Examples
///
/// ```text
/// Foo { var: expr }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedFieldInit {
    pub name: Ident,
    pub value: TypedExpr,
    pub span: Span,
}

/// Represents a call argument in a typed call expression.
///
/// # Examples
///
/// ```text
/// foo(bar: expr, expr)
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedCallArg {
    pub name: Option<Ident>,
    pub value: TypedExpr,
    pub span: Span,
}

/// Represents a typed expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedExprKind {
    Literal(Literal),
    Ident(Ident),
    FieldAccess {
        base: Box<TypedExpr>,
        field: Ident,
    },
    Call {
        callee: Box<TypedExpr>,
        args: Vec<TypedCallArg>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<TypedExpr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<TypedExpr>,
        right: Box<TypedExpr>,
    },
    Index {
        base: Box<TypedExpr>,
        index: Box<TypedExpr>,
    },
    StructInit {
        name: Ident,
        fields: Vec<TypedFieldInit>,
    },
}

/// Represents a typed expression.
///
/// # Examples
///
/// ```text
/// foo(bar, baz: qux)
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedExpr {
    pub kind: TypedExprKind,
    pub ty: ResolvedType,
    pub span: Span,
}

impl TypedExpr {
    pub fn new(kind: TypedExprKind, ty: ResolvedType, span: Span) -> Self {
        Self { kind, ty, span }
    }
}

/// Represents a typed match arm.
///
/// # Examples
///
/// ```text
/// match x {
///     Foo => bar(),
///     _ => baz(),
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedMatchArm {
    pub pattern: Pattern,
    pub body: Vec<TypedStmt>,
    pub span: Span,
}

/// Represents a typed statement.
///
/// # Examples
///
/// ```text
/// let x = 42
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedStmtKind {
    Assign {
        target: TypedExpr,
        value: TypedExpr,
    },
    If {
        condition: TypedExpr,
        then_branch: Vec<TypedStmt>,
        else_branch: Option<Vec<TypedStmt>>,
    },
    Match {
        scrutinee: TypedExpr,
        arms: Vec<TypedMatchArm>,
    },
    Let {
        name: Ident,
        ty: ResolvedType,
        value: TypedExpr,
    },
    Connect {
        target: TypedExpr,
        value: TypedExpr,
    },
    Assert {
        condition: TypedExpr,
        message: Option<String>,
    },
    Expr(TypedExpr),
}

/// Represents a typed statement.
///
/// # Examples
///
/// ```text
/// let x = 42
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedStmt {
    pub kind: TypedStmtKind,
    pub span: Span,
}

impl TypedStmt {
    pub fn new(kind: TypedStmtKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// Represents a typed trigger.
///
/// # Examples
///
/// ```text
/// rising(clk)
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedTrigger {
    pub edge: EdgeKind,
    pub signal: TypedExpr,
    pub span: Span,
}

/// Represents a typed on block.
///
/// # Examples
///
/// ```text
/// on rising(clk) {
///     ...
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedOnBlock {
    pub trigger: TypedTrigger,
    pub body: Vec<TypedStmt>,
    pub span: Span,
}

/// Represents a typed port declaration.
///
/// # Examples
///
/// ```text
/// input clk: Bits<8>
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedPortDecl {
    pub direction: PortDirection,
    pub name: Ident,
    pub ty: ResolvedType,
    pub initializer: Option<TypedExpr>,
    pub span: Span,
}

/// Represents a typed register declaration.
///
/// # Examples
///
/// ```text
/// register clk: Bits<8> = 0
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedRegisterDecl {
    pub name: Ident,
    pub ty: ResolvedType,
    pub reset_value: TypedExpr,
    pub span: Span,
}

/// Represents a typed let declaration.
///
/// # Examples
///
/// ```text
/// let x = 42
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedLetDecl {
    pub name: Ident,
    pub ty: ResolvedType,
    pub value: TypedExpr,
    pub span: Span,
}

/// Represents a typed connection declaration.
///
/// # Examples
///
/// ```text
/// connect clk.field = input1
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedConnectionDecl {
    pub target: TypedExpr,
    pub value: TypedExpr,
    pub is_explicit_connect: bool,
    pub span: Span,
}

/// Represents a typed assert declaration.
///
/// # Examples
///
/// ```text
/// assert clk.field == input1, "clk.field should be equal to input1"
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedAssertDecl {
    pub condition: TypedExpr,
    pub message: Option<String>,
    pub span: Span,
}

/// Represents a typed interface port declaration.
///
/// # Examples
///
/// ```text
/// master i2c: I2C
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedInterfacePortDecl {
    pub role: InterfaceRole,
    pub name: Ident,
    pub interface_name: Ident,
    pub span: Span,
}

/// Represents a typed module item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedModuleItemKind {
    Port(TypedPortDecl),
    Register(TypedRegisterDecl),
    Let(TypedLetDecl),
    Connection(TypedConnectionDecl),
    On(TypedOnBlock),
    Assert(TypedAssertDecl),
    InterfacePort(TypedInterfacePortDecl),
}

/// Represents a typed module item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedModuleItem {
    pub kind: TypedModuleItemKind,
    pub span: Span,
}

impl TypedModuleItem {
    pub fn new(kind: TypedModuleItemKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// Represents a typed module declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedModuleDecl {
    pub name: Ident,
    pub items: Vec<TypedModuleItem>,
    pub span: Span,
}

/// Represents a typed enum variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedEnumVariant {
    pub name: Ident,
    pub value: Option<TypedExpr>,
    pub discriminant: i128,
    pub span: Span,
}

/// Represents a typed enum declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedEnumDecl {
    pub name: Ident,
    pub variants: Vec<TypedEnumVariant>,
    pub span: Span,
}

/// Represents a typed struct field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedStructField {
    pub name: Ident,
    pub ty: ResolvedType,
    pub span: Span,
}

/// Represents a typed struct declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedStructDecl {
    pub name: Ident,
    pub fields: Vec<TypedStructField>,
    pub total_width: WidthExpr,
    pub span: Span,
}

/// Represents a typed interface signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedInterfaceSignal {
    pub dir: InterfaceSignalDir,
    pub name: Ident,
    pub ty: ResolvedType,
    pub span: Span,
}

/// Represents a typed interface declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedInterfaceDecl {
    pub name: Ident,
    pub signals: Vec<TypedInterfaceSignal>,
    pub span: Span,
}

/// Represents a typed item kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedItemKind {
    Module(TypedModuleDecl),
    Enum(TypedEnumDecl),
    Struct(TypedStructDecl),
    Interface(TypedInterfaceDecl),
}

/// Represents a typed item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedItem {
    pub kind: TypedItemKind,
    pub span: Span,
}

impl TypedItem {
    pub fn new(kind: TypedItemKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// Represents a typed source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedSourceFile {
    pub items: Vec<TypedItem>,
    pub span: Span,
}
