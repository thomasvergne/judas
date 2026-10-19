use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionResponse, InsertTextFormat, Position,
};

use crate::document::{position_to_offset, Document};
use judas_infer::{TypedItemKind, TypedModuleItemKind, TypedSourceFile};

/// Handles `textDocument/completion` request.
pub fn handle_completion(doc: &Document, pos: Position) -> Option<CompletionResponse> {
    let mut items = Vec::new();

    let offset = position_to_offset(&doc.text, pos);
    let prefix = get_prefix_before(&doc.text, offset);

    // 1. Check if user typed `::` (e.g. `State::` or `math::`)
    if prefix.ends_with("::") {
        let parent = prefix.trim_end_matches("::").trim();
        let tast = doc.tast.as_ref().or(doc.last_valid_tast.as_ref());
        if let Some(tast) = tast {
            add_path_completions(tast, parent, &mut items);
        }
        if items.is_empty() {
            scan_text_enum_variants(&doc.text, parent, &mut items);
        }
        return Some(CompletionResponse::Array(items));
    }

    // 2. Check if user typed `.` (e.g. `instance.` or `signal.`)
    if prefix.ends_with('.') {
        let target = prefix.trim_end_matches('.').trim();
        let tast = doc.tast.as_ref().or(doc.last_valid_tast.as_ref());
        if let Some(tast) = tast {
            add_member_completions(tast, target, &mut items);
        }
        // Always provide built-in methods on signals
        items.push(CompletionItem {
            label: "repeat".to_string(),
            kind: Some(CompletionItemKind::METHOD),
            detail: Some("repeat<COUNT>()".to_string()),
            insert_text: Some("repeat<${1:COUNT}>()".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
        items.push(CompletionItem {
            label: "slice".to_string(),
            kind: Some(CompletionItemKind::METHOD),
            detail: Some("slice<WIDTH>(offset)".to_string()),
            insert_text: Some("slice<${1:WIDTH}>(${2:offset})".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
        return Some(CompletionResponse::Array(items));
    }

    // 3. General completions: Keywords, types, snippets, and document symbols
    add_keyword_completions(&mut items);
    add_builtin_type_completions(&mut items);
    add_builtin_function_completions(&mut items);

    let tast = doc.tast.as_ref().or(doc.last_valid_tast.as_ref());
    if let Some(tast) = tast {
        add_document_symbols(tast, &mut items);
    }

    Some(CompletionResponse::Array(items))
}

fn scan_text_enum_variants(text: &str, parent: &str, items: &mut Vec<CompletionItem>) {
    let pattern = format!("enum {}", parent);
    if let Some(idx) = text.find(&pattern)
        && let Some(open) = text[idx..].find('{')
    {
        let after_open = &text[idx + open + 1..];
        if let Some(close) = after_open.find('}') {
            let body = &after_open[..close];
            for part in body.split(',') {
                let trimmed = part.trim();
                let name = trimmed.split('=').next().unwrap_or("").trim();
                if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    items.push(CompletionItem {
                        label: name.to_string(),
                        kind: Some(CompletionItemKind::ENUM_MEMBER),
                        detail: Some(format!("{}::{}", parent, name)),
                        ..Default::default()
                    });
                }
            }
        }
    }
}

fn get_prefix_before(text: &str, offset: usize) -> &str {
    let safe_offset = offset.min(text.len());
    let before = &text[..safe_offset];
    let start = before
        .rfind(|c: char| c.is_whitespace() || c == ';' || c == '(' || c == '{' || c == ',')
        .map(|idx| idx + 1)
        .unwrap_or(0);
    &before[start..]
}

fn add_path_completions(tast: &TypedSourceFile, parent: &str, items: &mut Vec<CompletionItem>) {
    for item in &tast.items {
        match &item.kind {
            TypedItemKind::Enum(e) => {
                if e.name.name == parent {
                    for v in &e.variants {
                        items.push(CompletionItem {
                            label: v.name.name.clone(),
                            kind: Some(CompletionItemKind::ENUM_MEMBER),
                            detail: Some(format!("{}::{} = {}", e.name.name, v.name.name, v.discriminant)),
                            ..Default::default()
                        });
                    }
                }
            }
            TypedItemKind::Namespace(ns) if ns.name.name == parent => {
                for ni in &ns.items {
                    match &ni.kind {
                        TypedItemKind::Module(m) => {
                            items.push(CompletionItem {
                                label: m.name.name.clone(),
                                kind: Some(CompletionItemKind::MODULE),
                                detail: Some(format!("module {}", m.name.name)),
                                ..Default::default()
                            });
                        }
                        TypedItemKind::Struct(s) => {
                            items.push(CompletionItem {
                                label: s.name.name.clone(),
                                kind: Some(CompletionItemKind::STRUCT),
                                detail: Some(format!("struct {}", s.name.name)),
                                ..Default::default()
                            });
                        }
                        TypedItemKind::Enum(e) => {
                            items.push(CompletionItem {
                                label: e.name.name.clone(),
                                kind: Some(CompletionItemKind::ENUM),
                                detail: Some(format!("enum {}", e.name.name)),
                                ..Default::default()
                            });
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

fn add_member_completions(tast: &TypedSourceFile, target: &str, items: &mut Vec<CompletionItem>) {
    // Check if target is an instance of a module or a struct
    for item in &tast.items {
        if let TypedItemKind::Module(m) = &item.kind {
            for mi in &m.items {
                if let TypedModuleItemKind::Let(l) = &mi.kind
                    && l.name.name == target
                {
                    // If type name matches a module, suggest its ports!
                    let type_str = format!("{}", l.ty);
                    for other in &tast.items {
                        if let TypedItemKind::Module(target_mod) = &other.kind
                            && (target_mod.name.name == type_str || target_mod.name.name == target)
                        {
                            for target_mi in &target_mod.items {
                                if let TypedModuleItemKind::Port(p) = &target_mi.kind {
                                    items.push(CompletionItem {
                                        label: p.name.name.clone(),
                                        kind: Some(CompletionItemKind::PROPERTY),
                                        detail: Some(format!("{} {}: {}", p.direction, p.name.name, p.ty)),
                                        ..Default::default()
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn add_keyword_completions(items: &mut Vec<CompletionItem>) {
    let keywords = [
        ("module", "module ${1:Name} {\n    $0\n}", "Declares a hardware module"),
        ("extern module", "extern module ${1:Name} \"${2:path.v}\" {\n    $0\n}", "Declares an external Verilog module"),
        ("struct", "struct ${1:Name} {\n    ${2:field}: ${3:UInt<16>},\n}", "Declares a packed struct"),
        ("enum", "enum ${1:Name} {\n    ${2:Variant1},\n    ${3:Variant2},\n}", "Declares an enumeration"),
        ("interface", "interface ${1:Name} {\n    out ${2:valid}: Bool,\n    in ${3:ready}: Bool,\n}", "Declares an interface bus"),
        ("namespace", "namespace ${1:name} {\n    $0\n}", "Declares a namespace"),
        ("import", "import \"${1:file.jd}\";", "Imports another Judas file"),
        ("pub", "pub ", "Public visibility modifier"),
        ("input", "input ${1:name}: ${2:UInt<8>}", "Declares an input port"),
        ("output", "output ${1:name}: ${2:UInt<8>}", "Declares an output port"),
        ("register", "register ${1:name}: ${2:UInt<8>} = ${3:0}", "Declares a clocked register"),
        ("let", "let ${1:name} = ${2:expr}", "Declares an internal wire/signal"),
        ("connect", "connect ${1:target} = ${2:value}", "Explicit hardware connection"),
        ("on rising", "on rising(${1:clk}) {\n    $0\n}", "Clock rising-edge sequential block"),
        ("on falling", "on falling(${1:clk}) {\n    $0\n}", "Clock falling-edge sequential block"),
        ("master", "master ${1:port}: ${2:InterfaceName}", "Master interface port"),
        ("slave", "slave ${1:port}: ${2:InterfaceName}", "Slave interface port"),
        ("assert", "assert ${1:cond}, \"${2:message}\"", "Hardware assertion"),
        ("match", "match ${1:expr} {\n    ${2:pattern} => ${3:action},\n    _ => $0,\n}", "Pattern matching"),
        ("if", "if ${1:cond} {\n    $0\n}", "Conditional statement"),
        ("else", "else {\n    $0\n}", "Else branch"),
    ];

    for (label, snippet, doc) in keywords {
        items.push(CompletionItem {
            label: label.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(doc.to_string()),
            insert_text: Some(snippet.to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
    }
}

fn add_builtin_type_completions(items: &mut Vec<CompletionItem>) {
    let types = [
        ("UInt", "UInt<${1:16}>", "Unsigned integer bit vector"),
        ("Int", "Int<${1:16}>", "Signed integer bit vector"),
        ("Bits", "Bits<${1:8}>", "Raw bit vector"),
        ("Bit", "Bit", "Single-bit raw signal"),
        ("Bool", "Bool", "Boolean signal (true/false)"),
        ("Clock", "Clock", "Hardware clock signal"),
        ("Reset", "Reset", "Hardware reset signal"),
        ("Array", "Array<${1:UInt<8>}, ${2:4}>", "Fixed-size homogeneous array"),
    ];

    for (label, snippet, doc) in types {
        items.push(CompletionItem {
            label: label.to_string(),
            kind: Some(CompletionItemKind::CLASS),
            detail: Some(doc.to_string()),
            insert_text: Some(snippet.to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
    }
}

fn add_builtin_function_completions(items: &mut Vec<CompletionItem>) {
    let funcs = [
        ("extend", "extend<${1:WIDTH}>(${2:signal})", "Widen a bit vector or integer"),
        ("truncate", "truncate<${1:WIDTH}>(${2:signal})", "Truncate higher bits"),
        ("concat", "concat(${1:msb}, ${2:lsb})", "Concatenate multiple signals"),
        ("repeat", "repeat<${1:COUNT}>(${2:signal})", "Replicate a signal"),
        ("slice", "slice<${1:WIDTH}>(${2:signal}, ${3:offset})", "Extract a bit slice"),
    ];

    for (label, snippet, doc) in funcs {
        items.push(CompletionItem {
            label: label.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(doc.to_string()),
            insert_text: Some(snippet.to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
    }
}

fn add_document_symbols(tast: &TypedSourceFile, items: &mut Vec<CompletionItem>) {
    for item in &tast.items {
        match &item.kind {
            TypedItemKind::Module(m) => {
                items.push(CompletionItem {
                    label: m.name.name.clone(),
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some(format!("module {}", m.name.name)),
                    insert_text: Some(format!("{}($0)", m.name.name)),
                    insert_text_format: Some(InsertTextFormat::SNIPPET),
                    ..Default::default()
                });
                for mi in &m.items {
                    match &mi.kind {
                        TypedModuleItemKind::Port(p) => {
                            items.push(CompletionItem {
                                label: p.name.name.clone(),
                                kind: Some(CompletionItemKind::VARIABLE),
                                detail: Some(format!("{} {}: {}", p.direction, p.name.name, p.ty)),
                                ..Default::default()
                            });
                        }
                        TypedModuleItemKind::Register(r) => {
                            items.push(CompletionItem {
                                label: r.name.name.clone(),
                                kind: Some(CompletionItemKind::VARIABLE),
                                detail: Some(format!("register {}: {}", r.name.name, r.ty)),
                                ..Default::default()
                            });
                        }
                        TypedModuleItemKind::Let(l) => {
                            items.push(CompletionItem {
                                label: l.name.name.clone(),
                                kind: Some(CompletionItemKind::VARIABLE),
                                detail: Some(format!("let {}: {}", l.name.name, l.ty)),
                                ..Default::default()
                            });
                        }
                        _ => {}
                    }
                }
            }
            TypedItemKind::Struct(s) => {
                items.push(CompletionItem {
                    label: s.name.name.clone(),
                    kind: Some(CompletionItemKind::STRUCT),
                    detail: Some(format!("struct {}", s.name.name)),
                    insert_text: Some(format!("{} {{ $0 }}", s.name.name)),
                    insert_text_format: Some(InsertTextFormat::SNIPPET),
                    ..Default::default()
                });
            }
            TypedItemKind::Interface(iface) => {
                items.push(CompletionItem {
                    label: iface.name.name.clone(),
                    kind: Some(CompletionItemKind::INTERFACE),
                    detail: Some(format!("interface {}", iface.name.name)),
                    ..Default::default()
                });
            }
            TypedItemKind::Enum(e) => {
                items.push(CompletionItem {
                    label: e.name.name.clone(),
                    kind: Some(CompletionItemKind::ENUM),
                    detail: Some(format!("enum {}", e.name.name)),
                    ..Default::default()
                });
            }
            _ => {}
        }
    }
}
