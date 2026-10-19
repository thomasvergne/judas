use tower_lsp::lsp_types::{GotoDefinitionResponse, Location, Position};

use crate::document::{span_to_range, Document};
use judas_infer::{TypedItemKind, TypedModuleItemKind, TypedSourceFile};

/// Handles `textDocument/definition` request.
pub fn handle_goto_definition(doc: &Document, pos: Position) -> Option<GotoDefinitionResponse> {
    let word = doc.word_at_position(pos)?;
    let tast = doc.tast.as_ref()?;

    if let Some(loc) = find_symbol_location(doc, tast, &word) {
        return Some(GotoDefinitionResponse::Scalar(loc));
    }

    None
}

fn find_symbol_location(doc: &Document, tast: &TypedSourceFile, name: &str) -> Option<Location> {
    for item in &tast.items {
        match &item.kind {
            TypedItemKind::Module(m) => {
                if m.name.name == name {
                    return Some(Location {
                        uri: doc.uri.clone(),
                        range: span_to_range(&doc.text, &m.name.span),
                    });
                }
                for mi in &m.items {
                    match &mi.kind {
                        TypedModuleItemKind::Port(p) => {
                            if p.name.name == name {
                                return Some(Location {
                                    uri: doc.uri.clone(),
                                    range: span_to_range(&doc.text, &p.name.span),
                                });
                            }
                        }
                        TypedModuleItemKind::Register(r) => {
                            if r.name.name == name {
                                return Some(Location {
                                    uri: doc.uri.clone(),
                                    range: span_to_range(&doc.text, &r.name.span),
                                });
                            }
                        }
                        TypedModuleItemKind::Let(l) => {
                            if l.name.name == name {
                                return Some(Location {
                                    uri: doc.uri.clone(),
                                    range: span_to_range(&doc.text, &l.name.span),
                                });
                            }
                        }
                        TypedModuleItemKind::InterfacePort(ip) if ip.name.name == name => {
                            return Some(Location {
                                uri: doc.uri.clone(),
                                range: span_to_range(&doc.text, &ip.name.span),
                            });
                        }
                        _ => {}
                    }
                }
            }
            TypedItemKind::Struct(s) => {
                if s.name.name == name {
                    return Some(Location {
                        uri: doc.uri.clone(),
                        range: span_to_range(&doc.text, &s.name.span),
                    });
                }
                for f in &s.fields {
                    if f.name.name == name {
                        return Some(Location {
                            uri: doc.uri.clone(),
                            range: span_to_range(&doc.text, &f.name.span),
                        });
                    }
                }
            }
            TypedItemKind::Interface(iface) => {
                if iface.name.name == name {
                    return Some(Location {
                        uri: doc.uri.clone(),
                        range: span_to_range(&doc.text, &iface.name.span),
                    });
                }
                for sig in &iface.signals {
                    if sig.name.name == name {
                        return Some(Location {
                            uri: doc.uri.clone(),
                            range: span_to_range(&doc.text, &sig.name.span),
                        });
                    }
                }
            }
            TypedItemKind::Enum(e) => {
                if e.name.name == name {
                    return Some(Location {
                        uri: doc.uri.clone(),
                        range: span_to_range(&doc.text, &e.name.span),
                    });
                }
                for v in &e.variants {
                    if v.name.name == name {
                        return Some(Location {
                            uri: doc.uri.clone(),
                            range: span_to_range(&doc.text, &v.name.span),
                        });
                    }
                }
            }
            TypedItemKind::Namespace(ns) if ns.name.name == name => {
                return Some(Location {
                    uri: doc.uri.clone(),
                    range: span_to_range(&doc.text, &ns.name.span),
                });
            }
            _ => {}
        }
    }

    None
}
