/// Represents a span of text in the source code.
///
/// A `Span` is composed of `start` and `end` indices,
/// as well as `line` and `column` numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

impl Span {
    pub fn new(start: usize, end: usize, line: usize, column: usize) -> Self {
        Self {
            start,
            end,
            line,
            column,
        }
    }

    /// Merge two spans into a single span.
    /// Useful for typechecking and LSP operations.
    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            line: self.line.min(other.line),
            column: if self.start <= other.start {
                self.column
            } else {
                other.column
            },
        }
    }
}

/// A node with a `Span` attached.
///
/// A `Spanned` node is used to attach a `Span` to a node in the AST.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }
}

impl From<Span> for miette::SourceSpan {
    fn from(span: Span) -> Self {
        let len = span.end.saturating_sub(span.start).max(1);
        miette::SourceSpan::new(span.start.into(), len)
    }
}
