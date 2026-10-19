use std::sync::Arc;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range, Url};

use judas_ast::{SourceFile, Span};
use judas_infer::{infer_source_file_with_path, TypedSourceFile};
use judas_parser::parse_source;

/// Represents an in-memory document managed by the language server.
#[derive(Debug, Clone)]
pub struct Document {
    pub uri: Url,
    pub text: String,
    pub version: i32,
    pub ast: Option<Arc<SourceFile>>,
    pub tast: Option<Arc<TypedSourceFile>>,
    pub last_valid_ast: Option<Arc<SourceFile>>,
    pub last_valid_tast: Option<Arc<TypedSourceFile>>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Document {
    pub fn new(uri: Url, text: String, version: i32) -> Self {
        let mut doc = Self {
            uri,
            text,
            version,
            ast: None,
            tast: None,
            last_valid_ast: None,
            last_valid_tast: None,
            diagnostics: Vec::new(),
        };
        doc.reanalyze();
        doc
    }

    pub fn update_text(&mut self, text: String, version: i32) {
        self.text = text;
        self.version = version;
        self.reanalyze();
    }

    pub fn reanalyze(&mut self) {
        self.diagnostics.clear();
        self.ast = None;
        self.tast = None;

        // 1. Parse phase
        let ast = match parse_source(&self.text) {
            Ok(ast) => Arc::new(ast),
            Err(err) => {
                let range = span_to_range(&self.text, &err.span);
                let message = if let Some(help) = &err.help {
                    format!("{}\n\nHelp: {}", err.message, help)
                } else {
                    err.message.clone()
                };

                self.diagnostics.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(NumberOrString::String("syntax_error".to_string())),
                    code_description: None,
                    source: Some("judas".to_string()),
                    message,
                    related_information: None,
                    tags: None,
                    data: None,
                });
                return;
            }
        };

        self.ast = Some(ast.clone());
        self.last_valid_ast = Some(ast.clone());

        // 2. Type-checking / Inference phase
        let file_path = self.uri.to_file_path().ok();
        match infer_source_file_with_path(&ast, file_path.as_deref()) {
            Ok(tast) => {
                let tast_arc = Arc::new(tast);
                self.tast = Some(tast_arc.clone());
                self.last_valid_tast = Some(tast_arc);
            }
            Err(err) => {
                let range = span_to_range(&self.text, &err.span);
                let message = if let Some(help) = &err.help {
                    format!("{}\n\nHelp: {}", err.message, help)
                } else {
                    err.message.clone()
                };

                self.diagnostics.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(NumberOrString::String("type_error".to_string())),
                    code_description: None,
                    source: Some("judas".to_string()),
                    message,
                    related_information: None,
                    tags: None,
                    data: None,
                });
            }
        }
    }

    /// Finds the word / identifier at the specified position.
    pub fn word_at_position(&self, pos: Position) -> Option<String> {
        let offset = position_to_offset(&self.text, pos);
        let chars: Vec<char> = self.text.chars().collect();
        if offset > chars.len() {
            return None;
        }

        // Search backward for start of identifier
        let mut start = offset;
        while start > 0 && is_ident_char(chars[start - 1]) {
            start -= 1;
        }

        // Search forward for end of identifier
        let mut end = offset;
        while end < chars.len() && is_ident_char(chars[end]) {
            end += 1;
        }

        if start < end {
            Some(chars[start..end].iter().collect())
        } else {
            None
        }
    }
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Converts a Judas `Span` to an LSP `Range`.
pub fn span_to_range(text: &str, span: &Span) -> Range {
    let start_pos = offset_to_position(text, span.start);
    let end_pos = offset_to_position(text, span.end);

    if start_pos == end_pos && span.line > 0 {
        let line = (span.line - 1) as u32;
        let col = (span.column.saturating_sub(1)) as u32;
        let len = span.end.saturating_sub(span.start).max(1) as u32;
        Range {
            start: Position::new(line, col),
            end: Position::new(line, col + len),
        }
    } else {
        Range {
            start: start_pos,
            end: end_pos,
        }
    }
}

/// Converts a 0-based byte offset in `text` to an LSP 0-based `Position`.
pub fn offset_to_position(text: &str, offset: usize) -> Position {
    let mut current_offset = 0;
    let mut line = 0;
    let mut col = 0;

    for ch in text.chars() {
        if current_offset >= offset {
            break;
        }
        current_offset += ch.len_utf8();
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += ch.len_utf16() as u32;
        }
    }

    Position::new(line, col)
}

/// Converts an LSP 0-based `Position` to a byte offset in `text`.
pub fn position_to_offset(text: &str, pos: Position) -> usize {
    let mut current_line = 0;
    let mut current_col = 0;
    let mut offset = 0;

    for ch in text.chars() {
        if current_line == pos.line && current_col >= pos.character {
            return offset;
        }
        if ch == '\n' {
            if current_line == pos.line {
                return offset;
            }
            current_line += 1;
            current_col = 0;
        } else {
            current_col += ch.len_utf16() as u32;
        }
        offset += ch.len_utf8();
    }

    offset
}
