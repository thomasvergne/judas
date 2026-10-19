use crate::span::Span;
use crate::types::Ident;
use std::fmt;

/// Represents the visibility of an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
}

impl Visibility {
    pub fn is_pub(&self) -> bool {
        matches!(self, Visibility::Public)
    }
}

/// Represents a qualified path (e.g. `foo`, `math::Adder`, `a::b::c`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    pub segments: Vec<Ident>,
    pub span: Span,
}

impl Path {
    pub fn new(segments: Vec<Ident>, span: Span) -> Self {
        Self { segments, span }
    }

    pub fn from_ident(ident: Ident) -> Self {
        let span = ident.span;
        Self {
            segments: vec![ident],
            span,
        }
    }

    pub fn to_string_path(&self) -> String {
        self.segments
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
            .join("::")
    }

    pub fn last_ident(&self) -> &Ident {
        self.segments
            .last()
            .expect("Path must have at least one segment")
    }

    pub fn is_single(&self) -> bool {
        self.segments.len() == 1
    }

    pub fn is_qualified(&self) -> bool {
        self.segments.len() > 1
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string_path())
    }
}
