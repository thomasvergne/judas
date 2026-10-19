use crate::expr::Expr;
use crate::span::Span;
use crate::stmt::{Stmt, Trigger};
use crate::types::{Ident, Type};

/// Represents the direction of a port.
///
/// A `PortDirection` is either `Input` or `Output`.
/// `Input` ports are read-only, `Output` ports are write-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDirection {
    Input,
    Output,
}

/// Represents a port declaration.
///
/// A `PortDecl` contains the direction, name, type, and
/// initializer of a port. It is used to declare the interface
/// of a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortDecl {
    pub direction: PortDirection,
    pub name: Ident,
    pub ty: Type,
    pub initializer: Option<Expr>,
    pub span: Span,
}

/// Represents a register declaration.
///
/// A `RegisterDecl` contains the name, type, and reset value
/// of a register. It is used to declare an independent
/// state variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterDecl {
    pub name: Ident,
    pub ty: Type,
    pub reset_value: Expr,
    pub span: Span,
}

/// Represents a `let` declaration.
///
/// A `LetDecl` contains the name, type, and value of a `let`
/// expression. It is used to declare a local and temporary
/// variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LetDecl {
    pub name: Ident,
    pub ty: Option<Type>,
    pub value: Expr,
    pub span: Span,
}

/// Represents a connection declaration.
///
/// A `ConnectionDecl` contains the target and value of a
/// connection. It is used to connect ports and registers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionDecl {
    pub target: Expr,
    pub value: Expr,
    pub is_explicit_connect: bool,
    pub span: Span,
}

/// Represents an `on` block.
///
/// An `OnBlock` contains a trigger and a body of statements.
/// It is used to define the behavior of a module when a
/// trigger condition is met.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnBlock {
    pub trigger: Trigger,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// Represents an assertion in a module body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssertDecl {
    pub condition: Expr,
    pub message: Option<String>,
    pub span: Span,
}

/// Represents the role of an interface port in a module (Master or Slave).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceRole {
    Master,
    Slave,
}

/// Represents an interface port declaration on a module (e.g. `master bus: AxiStream`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfacePortDecl {
    pub role: InterfaceRole,
    pub name: Ident,
    pub interface_name: Ident,
    pub span: Span,
}

/// Represents a module item.
///
/// A `ModuleItem` is a top-level item in a module, such as a
/// port, register, `let` declaration, connection, `on`
/// block, assertion, or interface port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleItemKind {
    Port(PortDecl),
    Register(RegisterDecl),
    Let(LetDecl),
    Connection(ConnectionDecl),
    On(OnBlock),
    Assert(AssertDecl),
    InterfacePort(InterfacePortDecl),
}

/// Represents a module item.
///
/// A `ModuleItem` is a top-level item in a module, such as a
/// port, register, `let` declaration, connection, or `on`
/// block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleItem {
    pub kind: ModuleItemKind,
    pub span: Span,
}

impl ModuleItem {
    pub fn new(kind: ModuleItemKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// Represents a generic parameter kind.
///
/// A `GenericParamKind` is either a type parameter or a const
/// parameter. It both specifies the kind of a generic parameter and
/// the name of the parameter.
///
/// It is used to differentiate between type parameters which are basically
/// generic type variables, and const parameters which are generic
/// constant values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenericParamKind {
    Type {
        name: Ident,
    },
    Const {
        name: Ident,
        ty: Type,
        default: Option<Box<Expr>>,
    },
}

/// Represents a generic parameter.
///
/// A `GenericParam` is a generic parameter that can be used in a
/// generic type or function. It contains the kind of the parameter
/// and the span of the parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericParam {
    pub kind: GenericParamKind,
    pub span: Span,
}

/// Represents a module parameter.
///
/// A `ModuleParam` is a parameter that can be used in a module
/// declaration. It contains the name, type, default value, and span
/// of the parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleParam {
    pub name: Ident,
    pub ty: Type,
    pub default: Option<Expr>,
    pub span: Span,
}

/// Represents a module declaration.
///
/// A `ModuleDecl` is a module declaration that contains the name,
/// generic parameters, parameters, items, whether it is an external
/// module (e.g. Verilog FFI), optional foreign file path, and span
/// of the module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleDecl {
    pub name: Ident,
    pub generics: Vec<GenericParam>,
    pub params: Vec<ModuleParam>,
    pub items: Vec<ModuleItem>,
    pub is_extern: bool,
    pub foreign_path: Option<String>,
    pub span: Span,
}

/// Represents an enum declaration.
/// Represents an enum variant.
///
/// An `EnumVariant` has a name, an optional custom value expression, and a span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariant {
    pub name: Ident,
    pub value: Option<Expr>,
    pub span: Span,
}

impl EnumVariant {
    pub fn new(name: Ident, value: Option<Expr>, span: Span) -> Self {
        Self { name, value, span }
    }
}

/// Represents an enum declaration.
///
/// An `EnumDecl` is an enum declaration that contains the name and
/// variants of the enum.
///
/// Enumerations are like C-style enums, where each variant is an
/// identifier with an optional custom value (e.g. `red = 0b00`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDecl {
    pub name: Ident,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

/// Represents a field in a packed struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructField {
    pub name: Ident,
    pub ty: Type,
    pub span: Span,
}

impl StructField {
    pub fn new(name: Ident, ty: Type, span: Span) -> Self {
        Self { name, ty, span }
    }
}

/// Represents a hardware packed struct declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructDecl {
    pub name: Ident,
    pub fields: Vec<StructField>,
    pub span: Span,
}

/// Represents the direction of a signal in an interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceSignalDir {
    In,
    Out,
}

/// Represents an individual signal declaration inside an interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceSignal {
    pub dir: InterfaceSignalDir,
    pub name: Ident,
    pub ty: Type,
    pub span: Span,
}

impl InterfaceSignal {
    pub fn new(dir: InterfaceSignalDir, name: Ident, ty: Type, span: Span) -> Self {
        Self {
            dir,
            name,
            ty,
            span,
        }
    }
}

/// Represents an interface declaration (bus abstraction).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceDecl {
    pub name: Ident,
    pub signals: Vec<InterfaceSignal>,
    pub span: Span,
}

/// Represents the kind of an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemKind {
    Module(ModuleDecl),
    Enum(EnumDecl),
    Struct(StructDecl),
    Interface(InterfaceDecl),
}

/// Represents an item.
///
/// An `Item` is an item that can be a module, enum, struct, or interface declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub kind: ItemKind,
    pub span: Span,
}

impl Item {
    pub fn new(kind: ItemKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// A source file consists of a list of items bound by a
/// global span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub items: Vec<Item>,
    pub span: Span,
}
