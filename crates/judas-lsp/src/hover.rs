use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position};

use crate::document::Document;
use judas_infer::{
    TypedItemKind, TypedModuleItemKind,
};

/// Handles `textDocument/hover` request.
pub fn handle_hover(doc: &Document, pos: Position) -> Option<Hover> {
    let word = doc.word_at_position(pos)?;

    // 1. Check built-in documentation
    if let Some(doc_str) = get_builtin_doc(&word) {
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: doc_str.to_string(),
            }),
            range: None,
        });
    }

    // 2. Check AST / TAST symbols
    if let Some(tast) = &doc.tast
        && let Some(hover_info) = search_tast_symbol(tast, &word)
    {
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: hover_info,
            }),
            range: None,
        });
    }

    None
}

fn search_tast_symbol(tast: &judas_infer::TypedSourceFile, name: &str) -> Option<String> {
    for item in &tast.items {
        match &item.kind {
            TypedItemKind::Module(m) => {
                if m.name.name == name {
                    let mut md = format!("```judas\nmodule {}", m.name.name);
                    if m.is_extern {
                        md = format!("```judas\nextern module {}", m.name.name);
                        if let Some(ref path) = m.foreign_path {
                            md.push_str(&format!(" \"{}\"", path));
                        }
                    }
                    md.push_str(" {\n");
                    for mi in &m.items {
                        match &mi.kind {
                            TypedModuleItemKind::Port(p) => {
                                md.push_str(&format!("    {} {}: {}\n", p.direction, p.name.name, p.ty));
                            }
                            TypedModuleItemKind::Register(r) => {
                                md.push_str(&format!("    register {}: {} = ...\n", r.name.name, r.ty));
                            }
                            TypedModuleItemKind::Let(l) => {
                                md.push_str(&format!("    let {}: {} = ...\n", l.name.name, l.ty));
                            }
                            TypedModuleItemKind::InterfacePort(ip) => {
                                md.push_str(&format!("    {} {}: {}\n", ip.role, ip.name.name, ip.interface_name.to_string_path()));
                            }
                            _ => {}
                        }
                    }
                    md.push_str("}\n```\n");
                    return Some(md);
                }

                // Check inside module items
                for mi in &m.items {
                    match &mi.kind {
                        TypedModuleItemKind::Port(p) => {
                            if p.name.name == name {
                                return Some(format!(
                                    "```judas\n{} {}: {}\n```\n*Port of module `{}`*",
                                    p.direction, p.name.name, p.ty, m.name.name
                                ));
                            }
                        }
                        TypedModuleItemKind::Register(r) => {
                            if r.name.name == name {
                                return Some(format!(
                                    "```judas\nregister {}: {}\n```\n*Sequential register in module `{}`*",
                                    r.name.name, r.ty, m.name.name
                                ));
                            }
                        }
                        TypedModuleItemKind::Let(l) => {
                            if l.name.name == name {
                                return Some(format!(
                                    "```judas\nlet {}: {}\n```\n*Internal signal in module `{}`*",
                                    l.name.name, l.ty, m.name.name
                                ));
                            }
                        }
                        TypedModuleItemKind::InterfacePort(ip) if ip.name.name == name => {
                            return Some(format!(
                                "```judas\n{} {}: {}\n```\n*Interface port on module `{}`*",
                                ip.role, ip.name.name, ip.interface_name.to_string_path(), m.name.name
                            ));
                        }
                        _ => {}
                    }
                }
            }
            TypedItemKind::Struct(s) => {
                if s.name.name == name {
                    let mut md = format!("```judas\nstruct {} {{\n", s.name.name);
                    for f in &s.fields {
                        md.push_str(&format!("    {}: {},\n", f.name.name, f.ty));
                    }
                    md.push_str("}\n```\n");
                    md.push_str(&format!("*Total bit width: {} bits*", s.total_width));
                    return Some(md);
                }
                for f in &s.fields {
                    if f.name.name == name {
                        return Some(format!(
                            "```judas\n{}: {}\n```\n*Field of struct `{}`*",
                            f.name.name, f.ty, s.name.name
                        ));
                    }
                }
            }
            TypedItemKind::Interface(iface) => {
                if iface.name.name == name {
                    let mut md = format!("```judas\ninterface {} {{\n", iface.name.name);
                    for sig in &iface.signals {
                        md.push_str(&format!("    {} {}: {},\n", sig.dir, sig.name.name, sig.ty));
                    }
                    md.push_str("}\n```\n");
                    return Some(md);
                }
                for sig in &iface.signals {
                    if sig.name.name == name {
                        return Some(format!(
                            "```judas\n{} {}: {}\n```\n*Signal of interface `{}`*",
                            sig.dir, sig.name.name, sig.ty, iface.name.name
                        ));
                    }
                }
            }
            TypedItemKind::Enum(e) => {
                if e.name.name == name {
                    let mut md = format!("```judas\nenum {} {{\n", e.name.name);
                    for v in &e.variants {
                        md.push_str(&format!("    {} = {},\n", v.name.name, v.discriminant));
                    }
                    md.push_str("}\n```\n");
                    return Some(md);
                }
                for v in &e.variants {
                    if v.name.name == name {
                        return Some(format!(
                            "```judas\n{}::{} = {}\n```\n*Variant of enum `{}`*",
                            e.name.name, v.name.name, v.discriminant, e.name.name
                        ));
                    }
                }
            }
            TypedItemKind::Namespace(ns) if ns.name.name == name => {
                return Some(format!(
                    "```judas\nnamespace {}\n```\n*Contains {} item(s)*",
                    ns.name.name, ns.items.len()
                ));
            }
            _ => {}
        }
    }

    None
}

fn get_builtin_doc(word: &str) -> Option<&'static str> {
    match word {
        // Built-in types
        "UInt" => Some(
            "```judas\nUInt<WIDTH>\n```\n\
            Unsigned integer bit vector of `WIDTH` bits.\n\n\
            Supports arithmetic (`+`, `-`, `*`), comparisons, and bitwise operations.",
        ),
        "Int" => Some(
            "```judas\nInt<WIDTH>\n```\n\
            Signed integer bit vector of `WIDTH` bits (two's complement representation).",
        ),
        "Bits" => Some(
            "```judas\nBits<WIDTH>\n```\n\
            Raw bit vector of `WIDTH` bits.\n\n\
            Supports bitwise operations (`&`, `|`, `^`, `~`) and bit slicing.",
        ),
        "Bit" => Some(
            "```judas\nBit\n```\n\
            Single-bit raw signal (equivalent to `Bits<1>`).",
        ),
        "Bool" => Some(
            "```judas\nBool\n```\n\
            Single-bit boolean signal (`true` or `false`). Used for conditions and control flow.",
        ),
        "Clock" => Some(
            "```judas\nClock\n```\n\
            Hardware clock signal. Can be used as trigger in `on rising(clk)` or `on falling(clk)`.",
        ),
        "Reset" => Some(
            "```judas\nReset\n```\n\
            Hardware reset signal.",
        ),
        "Array" => Some(
            "```judas\nArray<TYPE, SIZE>\n```\n\
            Fixed-size homogeneous hardware array of `SIZE` elements of type `TYPE`.",
        ),

        // Built-in functions
        "extend" => Some(
            "```judas\nextend<WIDTH>(signal) -> TYPE<WIDTH>\n```\n\
            Widens an integer or bit vector to target `WIDTH`.\n\n\
            Zero-extends for `UInt` / `Bits`, sign-extends for `Int`.",
        ),
        "truncate" => Some(
            "```judas\ntruncate<WIDTH>(signal) -> TYPE<WIDTH>\n```\n\
            Truncates higher bits of a signal to keep only the lower `WIDTH` bits.",
        ),
        "concat" => Some(
            "```judas\nconcat(msb, ..., lsb) -> Bits<TOTAL_WIDTH>\n```\n\
            Concatenates multiple signals into a single continuous bus MSB to LSB.",
        ),
        "repeat" => Some(
            "```judas\nrepeat<COUNT>(signal) -> Bits<WIDTH * COUNT>\nsignal.repeat<COUNT>()\n```\n\
            Replicates a signal `COUNT` times.",
        ),
        "slice" => Some(
            "```judas\nslice<WIDTH>(signal, offset) -> Bits<WIDTH>\nsignal.slice<WIDTH>(offset)\n```\n\
            Extracts a bit slice of length `WIDTH` starting at bit index `offset`.",
        ),
        "rising" => Some(
            "```judas\nrising(clk: Clock)\n```\n\
            Triggers sequential logic on the rising (positive) clock edge.",
        ),
        "falling" => Some(
            "```judas\nfalling(clk: Clock)\n```\n\
            Triggers sequential logic on the falling (negative) clock edge.",
        ),

        // Language keywords
        "module" => Some(
            "```judas\nmodule Name(params...) { ... }\n```\n\
            Defines a hardware entity with ports, internal signals, registers, and concurrent logic.",
        ),
        "register" => Some(
            "```judas\nregister name: Type = reset_value\n```\n\
            Declares a sequential clocked D-flip-flop register with an initial reset value.",
        ),
        "connect" => Some(
            "```judas\nconnect target = driver\n```\n\
            Connects a driver expression or instance port to a target signal.",
        ),
        "interface" => Some(
            "```judas\ninterface Name { out valid: Bool, in ready: Bool, ... }\n```\n\
            Defines a standardized bundle of directed signals representing a communication bus.",
        ),
        "master" => Some(
            "```judas\nmaster port_name: InterfaceName\n```\n\
            Declares an interface port adopting the master role (out signals driven, in signals received).",
        ),
        "slave" => Some(
            "```judas\nslave port_name: InterfaceName\n```\n\
            Declares an interface port adopting the slave role (in signals driven, out signals received).",
        ),
        "struct" => Some(
            "```judas\nstruct Name { field1: Type1, field2: Type2 }\n```\n\
            Defines a bit-accurate packed composite structure without padding.",
        ),
        "enum" => Some(
            "```judas\nenum Name { Variant1, Variant2 = 0x5 }\n```\n\
            Defines an enumeration with sequential or explicit bit discriminants.",
        ),
        "namespace" => Some(
            "```judas\nnamespace name { ... }\n```\n\
            Groups declarations in a logical module scope.",
        ),
        "import" => Some(
            "```judas\nimport \"file.jd\" as alias;\nimport namespace::Symbol;\n```\n\
            Imports declarations from another file or namespace.",
        ),
        "pub" => Some(
            "```judas\npub module ... / pub struct ...\n```\n\
            Declares an item public to outside namespaces and files.",
        ),
        "assert" => Some(
            "```judas\nassert condition, \"message\"\n```\n\
            Hardware invariant check or compile-time static verification.",
        ),

        _ => None,
    }
}
