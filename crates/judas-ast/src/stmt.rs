use crate::expr::Expr;
use crate::span::Span;
use crate::types::{Ident, Type};

/// A pattern used in a `match` expression.
///
/// A pattern can be an identifier, a literal, or a wildcard.
/// It represents data that might be matched against a value,
/// called a `scrutinee`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternKind {
    Ident(Ident),
    Literal(crate::expr::Literal),
    Wildcard,
}

/// A fully and well-defined pattern.
/// It contains a `PatternKind` and a `Span`.
///
/// `PatternKind` represents the kind of the given pattern.
/// `Span` represents the source code location of the pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    pub kind: PatternKind,
    pub span: Span,
}

impl Pattern {
    pub fn new(kind: PatternKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// An arm of a `match` expression.
/// It contains a `Pattern` and a `Vec<Stmt>` body.
/// `Pattern` represents the pattern to match against.
/// `Vec<Stmt>` represents the body of the arm.
/// `Span` represents the source code location of the arm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// Represents the kind of edge for a `Trigger`.
/// `Rising` represents a rising edge, `Falling` represents
/// a falling edge.
///
/// In Verilog, `Rising` corresponds to `posedge` and
/// `Falling` corresponds to `negedge`. Both are edge triggers
/// used to detect changes in a signal and synchronize with
/// the clock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeKind {
    Rising,
    Falling,
}

impl std::fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rising => write!(f, "rising"),
            Self::Falling => write!(f, "falling"),
        }
    }
}

/// Represents a `Trigger` that synchronizes with the clock.
/// `edge` represents the kind of edge to detect, `signal`
/// represents the signal to detect changes on, and `span`
/// represents the source code location of the trigger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    pub edge: EdgeKind,
    pub signal: Expr,
    pub span: Span,
}

/// Represents the kind of statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StmtKind {
    /// Represents an assignment statement.
    /// Respectively `target` is the left-hand side of
    /// the assignment, and `value` is the right-hand side.
    Assign { target: Expr, value: Expr },

    /// Represents a conditional statement.
    /// `condition` is the condition to evaluate, `then_branch`
    /// is the branch to execute if the condition is true, and
    /// `else_branch` is the branch to execute if the condition
    /// is false.
    If {
        condition: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Option<Vec<Stmt>>,
    },

    /// Represents a match statement.
    /// `scrutinee` is the value to match on, and `arms` are
    /// the branches to execute based on the value.
    Match {
        scrutinee: Expr,
        arms: Vec<MatchArm>,
    },

    /// Represents a let statement.
    /// `name` is the name of the variable, `ty` is the type of
    /// the variable, and `value` is the value to assign to it.
    Let {
        name: Ident,
        ty: Option<Type>,
        value: Expr,
    },

    /// Represents a connect statement.
    /// `target` is the target to connect to, and `value` is the
    /// value to connect.
    ///
    /// It is used to connect a value (respectively a arbitrary
    /// expression) to a target (respectively a port on a module).
    ///
    /// Ports are basically inputs and outputs on a module.
    Connect { target: Expr, value: Expr },

    /// Represents an assertion statement.
    /// `condition` must evaluate to a boolean.
    /// `message` is an optional failure message.
    Assert {
        condition: Expr,
        message: Option<String>,
    },

    /// Represents an expression statement.
    /// The basic fallback for any statement that does not fit
    /// into the other categories.
    Expr(Expr),
}

/// Represents a statement.
/// A statement is a single line of code that can be executed.
/// It is composed of a `StmtKind` and a `Span`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

impl Stmt {
    pub fn new(kind: StmtKind, span: Span) -> Self {
        Self { kind, span }
    }
}
