use std::collections::{HashMap, HashSet};

use judas_ast::item::{InterfaceSignalDir, PortDirection};
use judas_infer::{
    ResolvedType, TypedEnumDecl, TypedInterfaceDecl, TypedItem, TypedItemKind, TypedModuleDecl,
    WidthExpr,
};

use crate::error::SimError;
use crate::value::{SignalMeta, SimValue};

/// Configuration for running a simulation.
#[derive(Debug, Clone)]
pub struct SimConfig {
    /// Number of clock cycles to simulate.
    pub cycles: u64,
    /// Optional explicit top-level module name. Defaults to the last non-extern module.
    pub top_module: Option<String>,
    /// Clock half-period in nanoseconds (default: 5ns -> 10ns full period / 100MHz).
    pub half_period_ns: u64,
    /// Initial values for top-level input ports (`port_name -> value`).
    pub initial_inputs: HashMap<String, u128>,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            cycles: 100,
            top_module: None,
            half_period_ns: 5,
            initial_inputs: HashMap::new(),
        }
    }
}

/// Summary and results of a completed simulation run.
#[derive(Debug, Clone)]
pub struct SimReport {
    /// Name of the simulated top-level module.
    pub top_module: String,
    /// Total clock cycles simulated.
    pub cycles_simulated: u64,
    /// Final simulation timestamp in nanoseconds.
    pub final_time_ns: u64,
    /// Final values of the top-level module's ports (`(direction, port_name, SimValue)`).
    pub final_ports: Vec<(PortDirection, String, SimValue)>,
    /// Complete IEEE 1364 VCD waveform file content.
    pub vcd_content: String,
}

#[derive(Debug, Clone)]
pub(crate) struct EnumInfo {
    pub width: u32,
    pub variants: HashMap<String, i128>,
}

/// An elaborated hardware module instance in the simulation hierarchy.
#[derive(Debug, Clone)]
pub(crate) struct SimInstance {
    pub instance_name: String,
    pub decl: TypedModuleDecl,
    pub params: HashMap<String, u32>,
    pub signals: HashMap<String, SimValue>,
    pub arrays: HashMap<String, Vec<SimValue>>,
    pub signal_meta: Vec<SignalMeta>,
    pub registers: HashSet<String>,
    pub interface_ports: HashSet<String>,
    pub procedural_lets: HashSet<String>,
    pub prev_clocks: HashMap<String, u128>,
    pub children: Vec<SimInstance>,
    pub child_indices: HashMap<String, usize>,
}

impl SimInstance {
    pub(crate) fn set_signal(&mut self, key: &str, val: SimValue) {
        if let Some(existing) = self.signals.get_mut(key) {
            *existing = SimValue::new(val.bits, existing.width, existing.is_signed);
        }
    }
}

/// Global context holding type/module definitions and elaborated hierarchy.
pub struct Simulator {
    pub(crate) enums: HashMap<String, EnumInfo>,
    pub(crate) variant_lookup: HashMap<String, (u32, i128)>,
    pub(crate) interfaces: HashMap<String, Vec<(InterfaceSignalDir, String, ResolvedType)>>,
    pub(crate) modules: HashMap<String, TypedModuleDecl>,
    pub(crate) next_vcd_code: usize,
}

impl Default for Simulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Simulator {
    pub fn new() -> Self {
        Self {
            enums: HashMap::new(),
            variant_lookup: HashMap::new(),
            interfaces: HashMap::new(),
            modules: HashMap::new(),
            next_vcd_code: 0,
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
                    if !mod_decl.is_extern {
                        let full_name = qualify_name(prefix, &mod_decl.name.name);
                        self.modules
                            .insert(mod_decl.name.name.clone(), mod_decl.clone());
                        self.modules.insert(full_name, mod_decl.clone());
                    }
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

    pub(crate) fn find_top_module(
        &self,
        items: &[TypedItem],
        requested: Option<&str>,
    ) -> Result<TypedModuleDecl, SimError> {
        if let Some(req_name) = requested {
            if let Some(m) = self.modules.get(req_name) {
                return Ok(m.clone());
            }
            return Err(SimError::TopModuleNotFound(req_name.to_string()));
        }

        let mut last_mod = None;
        Self::find_last_module_in_items(items, &mut last_mod);
        last_mod.ok_or(SimError::NoTopModule)
    }

    fn find_last_module_in_items(items: &[TypedItem], last: &mut Option<TypedModuleDecl>) {
        for item in items {
            match &item.kind {
                TypedItemKind::Module(m) if !m.is_extern => {
                    *last = Some(m.clone());
                }
                TypedItemKind::Namespace(ns) => {
                    Self::find_last_module_in_items(&ns.items, last);
                }
                _ => {}
            }
        }
    }

    pub(crate) fn eval_type_width(
        &self,
        ty: &ResolvedType,
        params: &HashMap<String, u32>,
    ) -> (u32, bool) {
        match ty {
            ResolvedType::Clock | ResolvedType::Bool | ResolvedType::Bit => (1, false),
            ResolvedType::UInt(w) | ResolvedType::Bits(w) => {
                (self.eval_width_expr(w, params), false)
            }
            ResolvedType::Int(w) => (self.eval_width_expr(w, params), true),
            ResolvedType::Struct { total_width, .. } => {
                (self.eval_width_expr(total_width, params), false)
            }
            ResolvedType::Enum(name) => {
                let w = self.enums.get(name).map(|e| e.width).unwrap_or(1);
                (w, false)
            }
            _ => (32, false),
        }
    }

    pub(crate) fn eval_width_expr(&self, w: &WidthExpr, params: &HashMap<String, u32>) -> u32 {
        let mut subst = w.clone();
        for (k, v) in params {
            subst = subst.substitute(k, *v);
        }
        subst.eval_concrete().unwrap_or(8).max(1)
    }

    pub(crate) fn compute_struct_field_range(
        &self,
        fields: &[(String, ResolvedType)],
        total_width: &WidthExpr,
        target_field: &str,
        params: &HashMap<String, u32>,
    ) -> Option<(u32, u32)> {
        let mut current_msb = self.eval_width_expr(total_width, params);
        for (f_name, f_ty) in fields {
            let (f_w, _) = self.eval_type_width(f_ty, params);
            let msb = current_msb.saturating_sub(1);
            let lsb = current_msb.saturating_sub(f_w);
            if f_name == target_field {
                return Some((msb, lsb));
            }
            current_msb = lsb;
        }
        None
    }
}

fn qualify_name(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{}::{}", prefix, name)
    }
}
