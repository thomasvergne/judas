use crate::expr::Expr;
use crate::span::Span;

/// Represents an identifier.
/// An identifier is a name used to refer to a value or variable.
/// It may also refer to a type identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

impl Ident {
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
            span,
        }
    }
}

/// Represents a type argument.
/// A type argument can be a `Type` or an `Expr`.
///
/// For instance, the following are valid type arguments:
/// `UInt<N + 1>` where `N + 1` is an `Expr`, and
/// `UInt` is a `Type`, representing a unsigned integer type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeArg {
    Type(Type),
    Expr(Expr),
}

/// Represents a type kind.
/// The sub-kinds classifies the kind of type being represented.
/// For instance, `UInt` is a `TypeKind`, representing a
/// unsigned integer type, whereas `Array<T, N>` is a `TypeKind`,
/// representing an array type with element type `T` and size `N`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKind {
    /// Represents a clock type. Used to represent a clock signal.
    Clock,

    /// Represents a boolean type.
    Bool,

    /// Represents an unsigned integer type.
    UInt(Box<TypeArg>),

    /// Represents a signed integer type.
    Int(Box<TypeArg>),

    /// Represents a bit type.
    Bit,

    /// Represents a bits type.
    Bits(Box<TypeArg>),

    /// Represents an array type where the element type is `T`
    /// and the size is `N`.
    Array {
        element: Box<Type>,
        size: Box<TypeArg>,
    },

    /// Represents a named type, such as `Vec<T>` or `HashMap<K, V>`.
    Named { name: Ident, args: Vec<TypeArg> },
}

/// Full type representation.
/// A type is composed of a `TypeKind` and a `Span`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Type {
    pub kind: TypeKind,
    pub span: Span,
}

impl Type {
    pub fn new(kind: TypeKind, span: Span) -> Self {
        Self { kind, span }
    }
}
