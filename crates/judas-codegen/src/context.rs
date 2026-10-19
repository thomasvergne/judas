use std::collections::{HashMap, HashSet};

use judas_ast::item::{InterfaceRole, InterfaceSignalDir, PortDirection};
use judas_infer::{
    ResolvedType, TypedEnumDecl, TypedInterfaceDecl, TypedItem, TypedItemKind, TypedModuleDecl,
    TypedModuleItemKind,
};

/// Information about an enum declaration for Verilog generation.
#[derive(Debug, Clone)]
pub struct EnumInfo {
    pub width: u32,
    pub variants: HashMap<String, i128>,
}

/// Information about an interface port on a module.
#[derive(Debug, Clone)]
pub struct InterfacePortInfo {
    pub port_name: String,
    pub signals: Vec<(PortDirection, String, ResolvedType)>,
}

/// Information about a module signature for instantiation in Verilog.
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub verilog_name: String,
    pub params: Vec<(String, Option<i64>)>,
    pub ports: Vec<(PortDirection, String, ResolvedType)>,
    pub interface_ports: Vec<InterfacePortInfo>,
}

/// Main code generator translating a [`judas_infer::TypedSourceFile`] into synthesizable Verilog-2001.
pub struct VerilogGenerator {
    pub(crate) enums: HashMap<String, EnumInfo>,
    /// Maps variant name (both short `Red` and qualified `Light::Red`) to `(bit_width, discriminant)`
    pub(crate) variant_lookup: HashMap<String, (u32, i128)>,
    pub(crate) interfaces: HashMap<String, Vec<(InterfaceSignalDir, String, ResolvedType)>>,
    pub(crate) modules: HashMap<String, ModuleInfo>,
    /// Tracks signals in the current module that are procedural `let` variables inside `on` blocks
    pub(crate) procedural_lets: HashSet<String>,
    /// Tracks interface ports of the current module so `bus.pc` becomes `bus_pc`
    pub(crate) current_interface_ports: HashSet<String>,
    /// Tracks submodule instances in the current module so `cpu.io_data` becomes `cpu_io_data`
    pub(crate) current_instances: HashSet<String>,
}

impl Default for VerilogGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl VerilogGenerator {
    pub fn new() -> Self {
        Self {
            enums: HashMap::new(),
            variant_lookup: HashMap::new(),
            interfaces: HashMap::new(),
            modules: HashMap::new(),
            procedural_lets: HashSet::new(),
            current_interface_ports: HashSet::new(),
            current_instances: HashSet::new(),
        }
    }

    pub(crate) fn collect_module_names(
        items: &[TypedItem],
        prefix: &str,
        names: &mut HashSet<String>,
    ) {
        for item in items {
            match &item.kind {
                TypedItemKind::Module(m) => {
                    let full_name = qualify_name(prefix, &m.name.name);
                    names.insert(sanitize_ident(&full_name));
                }
                TypedItemKind::Namespace(ns) => {
                    let next_prefix = qualify_name(prefix, &ns.name.name);
                    Self::collect_module_names(&ns.items, &next_prefix, names);
                }
                _ => {}
            }
        }
    }

    pub(crate) fn collect_items(&mut self, items: &[TypedItem], prefix: &str) {
        for item in items {
            match &item.kind {
                TypedItemKind::Enum(enum_decl) => {
                    self.register_enum(enum_decl, prefix);
                }
                TypedItemKind::Interface(iface_decl) => {
                    self.register_interface(iface_decl, prefix);
                }
                TypedItemKind::Module(mod_decl) => {
                    self.register_module(mod_decl, prefix);
                }
                TypedItemKind::Namespace(ns_decl) => {
                    let next_prefix = qualify_name(prefix, &ns_decl.name.name);
                    self.collect_items(&ns_decl.items, &next_prefix);
                }
                TypedItemKind::Struct(_) | TypedItemKind::Import(_) => {}
            }
        }
    }

    fn register_enum(&mut self, enum_decl: &TypedEnumDecl, prefix: &str) {
        let full_name = qualify_name(prefix, &enum_decl.name.name);

        let max_disc = enum_decl
            .variants
            .iter()
            .map(|v| v.discriminant)
            .max()
            .unwrap_or(0)
            .max(0) as u64;

        let width = if max_disc <= 1 {
            1
        } else {
            64 - max_disc.leading_zeros()
        };

        let mut variants = HashMap::new();
        for v in &enum_decl.variants {
            variants.insert(v.name.name.clone(), v.discriminant);
            self.variant_lookup
                .insert(v.name.name.clone(), (width, v.discriminant));
            self.variant_lookup.insert(
                format!("{}::{}", enum_decl.name.name, v.name.name),
                (width, v.discriminant),
            );
            self.variant_lookup
                .insert(format!("{}::{}", full_name, v.name.name), (width, v.discriminant));
        }

        let info = EnumInfo { width, variants };
        self.enums.insert(enum_decl.name.name.clone(), info.clone());
        self.enums.insert(full_name, info);
    }

    fn register_interface(&mut self, iface_decl: &TypedInterfaceDecl, prefix: &str) {
        let full_name = qualify_name(prefix, &iface_decl.name.name);

        let signals: Vec<_> = iface_decl
            .signals
            .iter()
            .map(|s| (s.dir, s.name.name.clone(), s.ty.clone()))
            .collect();

        self.interfaces
            .insert(iface_decl.name.name.clone(), signals.clone());
        self.interfaces.insert(full_name, signals);
    }

    fn register_module(&mut self, mod_decl: &TypedModuleDecl, prefix: &str) {
        let full_name = qualify_name(prefix, &mod_decl.name.name);
        let verilog_name = sanitize_ident(&full_name);

        let params: Vec<_> = mod_decl
            .params
            .iter()
            .map(|p| (p.name.name.clone(), p.default))
            .collect();

        let mut ports = Vec::new();
        let mut interface_ports = Vec::new();

        for item in &mod_decl.items {
            match &item.kind {
                TypedModuleItemKind::Port(p) => {
                    ports.push((p.direction, p.name.name.clone(), p.ty.clone()));
                }
                TypedModuleItemKind::InterfacePort(ip) => {
                    let iface_name = ip.interface_name.to_string_path();
                    let signals = if !ip.signals.is_empty() {
                        ip.signals.clone()
                    } else if let Some(sigs) = self.interfaces.get(&iface_name) {
                        sigs.iter()
                            .map(|(sdir, sname, sty)| {
                                let dir = resolve_interface_port_dir(ip.role, *sdir);
                                (dir, sname.clone(), sty.clone())
                            })
                            .collect()
                    } else {
                        Vec::new()
                    };

                    interface_ports.push(InterfacePortInfo {
                        port_name: ip.name.name.clone(),
                        signals,
                    });
                }
                _ => {}
            }
        }

        let info = ModuleInfo {
            verilog_name,
            params,
            ports,
            interface_ports,
        };
        self.modules.insert(mod_decl.name.name.clone(), info.clone());
        self.modules.insert(full_name, info);
    }
}

pub(crate) fn resolve_interface_port_dir(
    role: InterfaceRole,
    sig_dir: InterfaceSignalDir,
) -> PortDirection {
    match (role, sig_dir) {
        (InterfaceRole::Master, InterfaceSignalDir::Out) => PortDirection::Output,
        (InterfaceRole::Master, InterfaceSignalDir::In) => PortDirection::Input,
        (InterfaceRole::Slave, InterfaceSignalDir::Out) => PortDirection::Input,
        (InterfaceRole::Slave, InterfaceSignalDir::In) => PortDirection::Output,
    }
}

pub(crate) fn qualify_name(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{}::{}", prefix, name)
    }
}

pub(crate) fn sanitize_ident(name: &str) -> String {
    name.replace("::", "_")
}
