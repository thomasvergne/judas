use judas_ast::Span;
use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Diagnostic, Debug, Clone, PartialEq, Eq)]
#[error("{message}")]
#[diagnostic(code(judas::parser::syntax_error))]
pub struct ParseError {
    pub message: String,

    #[label("{label}")]
    pub span: Span,

    pub label: String,

    #[help]
    pub help: Option<String>,
}

impl ParseError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        let msg = message.into();
        Self {
            message: msg.clone(),
            span,
            label: msg,
            help: None,
        }
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }
}
