use tower_lsp::lsp_types::{
    SemanticToken, SemanticTokenType, SemanticTokens, SemanticTokensLegend,
    SemanticTokensParams, SemanticTokensResult,
};

use crate::document::{offset_to_position, Document};
use judas_parser::token::TokenKind;
use judas_parser::Lexer;

pub const TOKEN_TYPE_KEYWORD: u32 = 0;
pub const TOKEN_TYPE_TYPE: u32 = 1;
pub const TOKEN_TYPE_FUNCTION: u32 = 2;
pub const TOKEN_TYPE_VARIABLE: u32 = 3;
pub const TOKEN_TYPE_PROPERTY: u32 = 4;
pub const TOKEN_TYPE_NUMBER: u32 = 5;
pub const TOKEN_TYPE_STRING: u32 = 6;
pub const TOKEN_TYPE_COMMENT: u32 = 7;
pub const TOKEN_TYPE_OPERATOR: u32 = 8;

pub fn get_legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::KEYWORD,    // 0
            SemanticTokenType::TYPE,       // 1
            SemanticTokenType::FUNCTION,   // 2
            SemanticTokenType::VARIABLE,   // 3
            SemanticTokenType::PROPERTY,   // 4
            SemanticTokenType::NUMBER,     // 5
            SemanticTokenType::STRING,     // 6
            SemanticTokenType::COMMENT,    // 7
            SemanticTokenType::OPERATOR,   // 8
        ],
        token_modifiers: vec![],
    }
}

struct RawToken {
    line: u32,
    col: u32,
    length: u32,
    token_type: u32,
}

pub fn handle_semantic_tokens(
    doc: &Document,
    _params: SemanticTokensParams,
) -> Option<SemanticTokensResult> {
    let mut raw_tokens = Vec::new();

    // 1. Extract comments from source text
    collect_comments(&doc.text, &mut raw_tokens);

    // 2. Extract tokens from Lexer
    let mut lexer = Lexer::new(&doc.text);
    if let Ok(tokens) = lexer.tokenize() {
        for tok in tokens {
            if tok.kind == TokenKind::Eof {
                continue;
            }

            let token_type = match &tok.kind {
                TokenKind::Module
                | TokenKind::Extern
                | TokenKind::Enum
                | TokenKind::Input
                | TokenKind::Output
                | TokenKind::Register
                | TokenKind::Connect
                | TokenKind::On
                | TokenKind::Rising
                | TokenKind::Falling
                | TokenKind::If
                | TokenKind::Else
                | TokenKind::Match
                | TokenKind::Let
                | TokenKind::Assert
                | TokenKind::Struct
                | TokenKind::Interface
                | TokenKind::Master
                | TokenKind::Slave
                | TokenKind::Namespace
                | TokenKind::Import
                | TokenKind::Pub
                | TokenKind::As
                | TokenKind::BoolLiteral(_) => TOKEN_TYPE_KEYWORD,

                TokenKind::Ident(name) => {
                    if is_builtin_type(name) || name.chars().next().is_some_and(char::is_uppercase) {
                        TOKEN_TYPE_TYPE
                    } else if is_builtin_function(name) {
                        TOKEN_TYPE_FUNCTION
                    } else {
                        TOKEN_TYPE_VARIABLE
                    }
                }

                TokenKind::IntLiteral(_) => TOKEN_TYPE_NUMBER,
                TokenKind::StringLiteral(_) => TOKEN_TYPE_STRING,

                TokenKind::Eq
                | TokenKind::EqEq
                | TokenKind::NotEq
                | TokenKind::Lt
                | TokenKind::LtEq
                | TokenKind::Gt
                | TokenKind::GtEq
                | TokenKind::Plus
                | TokenKind::Minus
                | TokenKind::Star
                | TokenKind::Slash
                | TokenKind::Percent
                | TokenKind::AmpAmp
                | TokenKind::PipePipe
                | TokenKind::Amp
                | TokenKind::Pipe
                | TokenKind::Caret
                | TokenKind::Shl
                | TokenKind::Shr
                | TokenKind::Bang
                | TokenKind::Tilde
                | TokenKind::FatArrow => TOKEN_TYPE_OPERATOR,

                _ => continue,
            };

            let pos = offset_to_position(&doc.text, tok.span.start);
            let len = tok.span.end.saturating_sub(tok.span.start) as u32;

            if len > 0 {
                raw_tokens.push(RawToken {
                    line: pos.line,
                    col: pos.character,
                    length: len,
                    token_type,
                });
            }
        }
    }

    // 3. Sort raw tokens by line, then column
    raw_tokens.sort_by(|a, b| a.line.cmp(&b.line).then_with(|| a.col.cmp(&b.col)));

    // 4. Delta-encode tokens for LSP
    let mut data = Vec::new();
    let mut prev_line = 0;
    let mut prev_col = 0;

    for raw in raw_tokens {
        let delta_line = raw.line.saturating_sub(prev_line);
        let delta_start = if delta_line == 0 {
            raw.col.saturating_sub(prev_col)
        } else {
            raw.col
        };

        data.push(SemanticToken {
            delta_line,
            delta_start,
            length: raw.length,
            token_type: raw.token_type,
            token_modifiers_bitset: 0,
        });

        prev_line = raw.line;
        prev_col = raw.col;
    }

    Some(SemanticTokensResult::Tokens(SemanticTokens {
        result_id: None,
        data,
    }))
}

fn is_builtin_type(name: &str) -> bool {
    matches!(
        name,
        "UInt" | "Int" | "Bits" | "Bit" | "Bool" | "Clock" | "Reset" | "Array" | "usize"
    )
}

fn is_builtin_function(name: &str) -> bool {
    matches!(
        name,
        "extend" | "truncate" | "concat" | "repeat" | "slice"
    )
}

fn collect_comments(text: &str, raw_tokens: &mut Vec<RawToken>) {
    let mut offset = 0;
    let bytes = text.as_bytes();

    while offset < bytes.len() {
        if offset + 1 < bytes.len() && bytes[offset] == b'/' && bytes[offset + 1] == b'/' {
            // Line comment
            let start = offset;
            offset += 2;
            while offset < bytes.len() && bytes[offset] != b'\n' {
                offset += 1;
            }
            let pos = offset_to_position(text, start);
            raw_tokens.push(RawToken {
                line: pos.line,
                col: pos.character,
                length: (offset - start) as u32,
                token_type: TOKEN_TYPE_COMMENT,
            });
        } else if offset + 1 < bytes.len() && bytes[offset] == b'/' && bytes[offset + 1] == b'*' {
            // Block comment: split line by line
            let start = offset;
            offset += 2;
            while offset + 1 < bytes.len() && !(bytes[offset] == b'*' && bytes[offset + 1] == b'/') {
                offset += 1;
            }
            if offset + 1 < bytes.len() {
                offset += 2; // skip */
            }

            let comment_text = &text[start..offset];
            let mut line_offset = start;
            for line in comment_text.lines() {
                let pos = offset_to_position(text, line_offset);
                raw_tokens.push(RawToken {
                    line: pos.line,
                    col: pos.character,
                    length: line.len() as u32,
                    token_type: TOKEN_TYPE_COMMENT,
                });
                line_offset += line.len() + 1; // +1 for newline
            }
        } else {
            offset += 1;
        }
    }
}
