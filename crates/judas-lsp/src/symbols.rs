use tower_lsp::lsp_types::{
    DocumentSymbol, DocumentSymbolResponse, SymbolKind,
};

use crate::document::{span_to_range, Document};
use judas_infer::{
    TypedItemKind, TypedModuleItemKind, TypedSourceFile,
};

/// Handles `textDocument/documentSymbol` request.
pub fn handle_document_symbols(doc: &Document) -> Option<DocumentSymbolResponse> {
    let tast = doc.tast.as_ref()?;
    let symbols = collect_symbols(doc, tast);
    Some(DocumentSymbolResponse::Nested(symbols))
}

#[allow(deprecated)]
fn collect_symbols(doc: &Document, tast: &TypedSourceFile) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    for item in &tast.items {
        match &item.kind {
            TypedItemKind::Module(m) => {
                let range = span_to_range(&doc.text, &m.span);
                let selection_range = span_to_range(&doc.text, &m.name.span);

                let mut children = Vec::new();
                for mi in &m.items {
                    match &mi.kind {
                        TypedModuleItemKind::Port(p) => {
                            children.push(DocumentSymbol {
                                name: p.name.name.clone(),
                                detail: Some(format!("{} {}", p.direction, p.ty)),
                                kind: SymbolKind::PROPERTY,
                                tags: None,
                                deprecated: None,
                                range: span_to_range(&doc.text, &mi.span),
                                selection_range: span_to_range(&doc.text, &p.name.span),
                                children: None,
                            });
                        }
                        TypedModuleItemKind::Register(r) => {
                            children.push(DocumentSymbol {
                                name: r.name.name.clone(),
                                detail: Some(format!("register: {}", r.ty)),
                                kind: SymbolKind::VARIABLE,
                                tags: None,
                                deprecated: None,
                                range: span_to_range(&doc.text, &mi.span),
                                selection_range: span_to_range(&doc.text, &r.name.span),
                                children: None,
                            });
                        }
                        TypedModuleItemKind::Let(l) => {
                            children.push(DocumentSymbol {
                                name: l.name.name.clone(),
                                detail: Some(format!("let: {}", l.ty)),
                                kind: SymbolKind::VARIABLE,
                                tags: None,
                                deprecated: None,
                                range: span_to_range(&doc.text, &mi.span),
                                selection_range: span_to_range(&doc.text, &l.name.span),
                                children: None,
                            });
                        }
                        TypedModuleItemKind::InterfacePort(ip) => {
                            children.push(DocumentSymbol {
                                name: ip.name.name.clone(),
                                detail: Some(format!("{} {}", ip.role, ip.interface_name.to_string_path())),
                                kind: SymbolKind::INTERFACE,
                                tags: None,
                                deprecated: None,
                                range: span_to_range(&doc.text, &mi.span),
                                selection_range: span_to_range(&doc.text, &ip.name.span),
                                children: None,
                            });
                        }
                        TypedModuleItemKind::On(on_block) => {
                            children.push(DocumentSymbol {
                                name: format!("on {}(...)", on_block.trigger.edge),
                                detail: None,
                                kind: SymbolKind::EVENT,
                                tags: None,
                                deprecated: None,
                                range: span_to_range(&doc.text, &on_block.span),
                                selection_range: span_to_range(&doc.text, &on_block.span),
                                children: None,
                            });
                        }
                        _ => {}
                    }
                }

                symbols.push(DocumentSymbol {
                    name: m.name.name.clone(),
                    detail: Some(if m.is_extern { "extern module" } else { "module" }.to_string()),
                    kind: SymbolKind::MODULE,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range,
                    children: if children.is_empty() { None } else { Some(children) },
                });
            }
            TypedItemKind::Struct(s) => {
                let range = span_to_range(&doc.text, &s.span);
                let selection_range = span_to_range(&doc.text, &s.name.span);

                let mut children = Vec::new();
                for f in &s.fields {
                    children.push(DocumentSymbol {
                        name: f.name.name.clone(),
                        detail: Some(format!("{}", f.ty)),
                        kind: SymbolKind::FIELD,
                        tags: None,
                        deprecated: None,
                        range: span_to_range(&doc.text, &f.span),
                        selection_range: span_to_range(&doc.text, &f.name.span),
                        children: None,
                    });
                }

                symbols.push(DocumentSymbol {
                    name: s.name.name.clone(),
                    detail: Some(format!("struct ({} bits)", s.total_width)),
                    kind: SymbolKind::STRUCT,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range,
                    children: if children.is_empty() { None } else { Some(children) },
                });
            }
            TypedItemKind::Interface(iface) => {
                let range = span_to_range(&doc.text, &iface.span);
                let selection_range = span_to_range(&doc.text, &iface.name.span);

                let mut children = Vec::new();
                for sig in &iface.signals {
                    children.push(DocumentSymbol {
                        name: sig.name.name.clone(),
                        detail: Some(format!("{} {}", sig.dir, sig.ty)),
                        kind: SymbolKind::FIELD,
                        tags: None,
                        deprecated: None,
                        range: span_to_range(&doc.text, &sig.span),
                        selection_range: span_to_range(&doc.text, &sig.name.span),
                        children: None,
                    });
                }

                symbols.push(DocumentSymbol {
                    name: iface.name.name.clone(),
                    detail: Some("interface".to_string()),
                    kind: SymbolKind::INTERFACE,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range,
                    children: if children.is_empty() { None } else { Some(children) },
                });
            }
            TypedItemKind::Enum(e) => {
                let range = span_to_range(&doc.text, &e.span);
                let selection_range = span_to_range(&doc.text, &e.name.span);

                let mut children = Vec::new();
                for v in &e.variants {
                    children.push(DocumentSymbol {
                        name: v.name.name.clone(),
                        detail: Some(format!("= {}", v.discriminant)),
                        kind: SymbolKind::ENUM_MEMBER,
                        tags: None,
                        deprecated: None,
                        range: span_to_range(&doc.text, &v.span),
                        selection_range: span_to_range(&doc.text, &v.name.span),
                        children: None,
                    });
                }

                symbols.push(DocumentSymbol {
                    name: e.name.name.clone(),
                    detail: Some("enum".to_string()),
                    kind: SymbolKind::ENUM,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range,
                    children: if children.is_empty() { None } else { Some(children) },
                });
            }
            TypedItemKind::Namespace(ns) => {
                let range = span_to_range(&doc.text, &ns.span);
                let selection_range = span_to_range(&doc.text, &ns.name.span);

                symbols.push(DocumentSymbol {
                    name: ns.name.name.clone(),
                    detail: Some("namespace".to_string()),
                    kind: SymbolKind::NAMESPACE,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range,
                    children: None,
                });
            }
            _ => {}
        }
    }

    symbols
}
