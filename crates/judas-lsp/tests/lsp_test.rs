use tower_lsp::lsp_types::{Position, Url};

use judas_lsp::completion::handle_completion;
use judas_lsp::definition::handle_goto_definition;
use judas_lsp::document::Document;
use judas_lsp::hover::handle_hover;
use judas_lsp::symbols::handle_document_symbols;

#[test]
fn test_lsp_diagnostics_valid_code() {
    let src = r#"
module Counter {
    input clk: Clock
    input reset: Bool
    output count: UInt<8>

    register count_reg: UInt<8> = 0
    count = count_reg

    on rising(clk) {
        if reset {
            count_reg = 0
        } else {
            count_reg = count_reg + 1
        }
    }
}
"#;

    let uri = Url::parse("file:///test/counter.jd").unwrap();
    let doc = Document::new(uri, src.to_string(), 1);

    assert!(doc.ast.is_some());
    assert!(doc.tast.is_some());
    assert!(doc.diagnostics.is_empty(), "Expected 0 diagnostics, found {:?}", doc.diagnostics);
}

#[test]
fn test_lsp_diagnostics_syntax_error() {
    let src = r#"
module Incomplete {
    input clk: Clock
    output out_val
}
"#;

    let uri = Url::parse("file:///test/bad_syntax.jd").unwrap();
    let doc = Document::new(uri, src.to_string(), 1);

    assert!(doc.ast.is_none());
    assert!(!doc.diagnostics.is_empty());
    assert_eq!(doc.diagnostics[0].source.as_deref(), Some("judas"));
}

#[test]
fn test_lsp_diagnostics_type_error() {
    let src = r#"
module WidthMismatch {
    input in_a: UInt<8>
    output out_b: UInt<16>

    out_b = in_a
}
"#;

    let uri = Url::parse("file:///test/bad_types.jd").unwrap();
    let doc = Document::new(uri, src.to_string(), 1);

    assert!(doc.ast.is_some());
    assert!(doc.tast.is_none());
    assert!(!doc.diagnostics.is_empty());
    let diag = &doc.diagnostics[0];
    assert!(diag.message.contains("width mismatch") || diag.message.contains("Width"));
}

#[test]
fn test_lsp_hover_builtin_and_symbols() {
    let src = r#"
struct Point {
    x: UInt<16>,
    y: UInt<16>,
}

module Top {
    input clk: Clock
    output out_val: UInt<16>

    let p = Point { x: 10, y: 20 }
    out_val = p.x
}
"#;

    let uri = Url::parse("file:///test/hover.jd").unwrap();
    let doc = Document::new(uri, src.to_string(), 1);

    // Hover on 'UInt'
    let hover_uint = handle_hover(&doc, Position::new(2, 8));
    assert!(hover_uint.is_some());
    if let tower_lsp::lsp_types::HoverContents::Markup(content) = hover_uint.unwrap().contents {
        assert!(content.value.contains("UInt<WIDTH>"));
    }

    // Hover on 'Clock'
    let hover_clock = handle_hover(&doc, Position::new(7, 16));
    assert!(hover_clock.is_some());
    if let tower_lsp::lsp_types::HoverContents::Markup(content) = hover_clock.unwrap().contents {
        assert!(content.value.contains("Clock"));
    }

    // Hover on module 'Top'
    let hover_top = handle_hover(&doc, Position::new(6, 8));
    assert!(hover_top.is_some());
    if let tower_lsp::lsp_types::HoverContents::Markup(content) = hover_top.unwrap().contents {
        assert!(content.value.contains("module Top"));
    }

    // Hover on struct 'Point'
    let hover_point = handle_hover(&doc, Position::new(1, 8));
    assert!(hover_point.is_some());
    if let tower_lsp::lsp_types::HoverContents::Markup(content) = hover_point.unwrap().contents {
        assert!(content.value.contains("struct Point"));
    }
}

#[test]
fn test_lsp_goto_definition() {
    let src = r#"
module Target {
    input a: UInt<8>
    output b: UInt<8>
    b = a
}
"#;

    let uri = Url::parse("file:///test/def.jd").unwrap();
    let doc = Document::new(uri, src.to_string(), 1);

    // Click on 'Target'
    let def_target = handle_goto_definition(&doc, Position::new(1, 8));
    assert!(def_target.is_some());

    // Click on 'a'
    let def_a = handle_goto_definition(&doc, Position::new(2, 11));
    assert!(def_a.is_some());
}

#[test]
fn test_lsp_completion() {
    let src = r#"
enum State {
    Idle,
    Running,
}

module Top {
    input clk: Clock
    let st = State::
}
"#;

    let uri = Url::parse("file:///test/comp.jd").unwrap();
    let doc = Document::new(uri, src.to_string(), 1);

    // General completion (at beginning of line)
    let comp_general = handle_completion(&doc, Position::new(0, 0));
    assert!(comp_general.is_some());
    if let tower_lsp::lsp_types::CompletionResponse::Array(items) = comp_general.unwrap() {
        assert!(items.iter().any(|i| i.label == "module"));
        assert!(items.iter().any(|i| i.label == "struct"));
        assert!(items.iter().any(|i| i.label == "UInt"));
    }

    // Enum variant completion after State::
    let comp_state = handle_completion(&doc, Position::new(8, 20));
    assert!(comp_state.is_some());
    if let tower_lsp::lsp_types::CompletionResponse::Array(items) = comp_state.unwrap() {
        assert!(items.iter().any(|i| i.label == "Idle"));
        assert!(items.iter().any(|i| i.label == "Running"));
    }
}

#[test]
fn test_lsp_document_symbols() {
    let src = r#"
struct Complex {
    real: Int<16>,
    imag: Int<16>,
}

module Top {
    input clk: Clock
    register count: UInt<8> = 0
    let temp = 42
}
"#;

    let uri = Url::parse("file:///test/symbols.jd").unwrap();
    let doc = Document::new(uri, src.to_string(), 1);

    let symbols_resp = handle_document_symbols(&doc);
    assert!(symbols_resp.is_some());
    if let tower_lsp::lsp_types::DocumentSymbolResponse::Nested(symbols) = symbols_resp.unwrap() {
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "Complex");
        assert_eq!(symbols[1].name, "Top");
        assert!(symbols[1].children.is_some());
        let top_children = symbols[1].children.as_ref().unwrap();
        assert!(top_children.iter().any(|c| c.name == "clk"));
        assert!(top_children.iter().any(|c| c.name == "count"));
        assert!(top_children.iter().any(|c| c.name == "temp"));
    }
}

#[test]
fn test_lsp_semantic_tokens() {
    let src = r#"
// Counter module
module Counter {
    input clk: Clock
    register count: UInt<8> = 0
}
"#;

    let uri = Url::parse("file:///test/tokens.jd").unwrap();
    let doc = Document::new(uri, src.to_string(), 1);

    let params = tower_lsp::lsp_types::SemanticTokensParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: doc.uri.clone() },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let resp = judas_lsp::semantic_tokens::handle_semantic_tokens(&doc, params);
    assert!(resp.is_some());
    if let tower_lsp::lsp_types::SemanticTokensResult::Tokens(tokens) = resp.unwrap() {
        assert!(!tokens.data.is_empty());
        // Verify we extracted keyword tokens, comment tokens, type tokens
        assert!(tokens.data.iter().any(|t| t.token_type == judas_lsp::semantic_tokens::TOKEN_TYPE_KEYWORD));
        assert!(tokens.data.iter().any(|t| t.token_type == judas_lsp::semantic_tokens::TOKEN_TYPE_COMMENT));
        assert!(tokens.data.iter().any(|t| t.token_type == judas_lsp::semantic_tokens::TOKEN_TYPE_TYPE));
    }
}

