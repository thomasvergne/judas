use std::collections::{HashMap, HashSet};

use judas_ast::item::{InterfaceRole, InterfaceSignalDir, PortDirection};
use judas_infer::{
    ResolvedType, TypedExprKind, TypedModuleDecl, TypedModuleItemKind, TypedStmt, TypedStmtKind,
};

use crate::context::{SimInstance, Simulator};
use crate::value::{SignalKind, SignalMeta, SimValue};

impl Simulator {
    pub(crate) fn next_vcd_id(&mut self) -> String {
        // Generate compact Printable ASCII VCD identifiers ('!' = 33 .. '~' = 126)
        let mut n = self.next_vcd_code;
        self.next_vcd_code += 1;
        let mut id = String::new();
        loop {
            let ch = (33 + (n % 94)) as u8 as char;
            id.push(ch);
            n /= 94;
            if n == 0 {
                break;
            }
            n -= 1;
        }
        id
    }

    pub(crate) fn elaborate_instance(
        &mut self,
        instance_name: &str,
        decl: &TypedModuleDecl,
        mut params: HashMap<String, u32>,
    ) -> SimInstance {
        for p in &decl.params {
            params
                .entry(p.name.name.clone())
                .or_insert_with(|| p.default.unwrap_or(8).max(1) as u32);
        }

        let mut inst = SimInstance {
            instance_name: instance_name.to_string(),
            decl: decl.clone(),
            params,
            signals: HashMap::new(),
            arrays: HashMap::new(),
            signal_meta: Vec::new(),
            registers: HashSet::new(),
            interface_ports: HashSet::new(),
            procedural_lets: HashSet::new(),
            prev_clocks: HashMap::new(),
            children: Vec::new(),
            child_indices: HashMap::new(),
        };

        // 1. Elaborate ports and interface ports
        for item in &decl.items {
            match &item.kind {
                TypedModuleItemKind::Port(port) => {
                    let (w, is_signed) = self.eval_type_width(&port.ty, &inst.params);
                    let kind = match port.direction {
                        PortDirection::Input => SignalKind::Input,
                        PortDirection::Output => SignalKind::Output,
                    };
                    self.register_signal(
                        &mut inst,
                        &port.name.name,
                        &port.name.name,
                        w,
                        is_signed,
                        kind,
                        SimValue::zero(w, is_signed),
                    );
                }
                TypedModuleItemKind::InterfacePort(ip) => {
                    inst.interface_ports.insert(ip.name.name.clone());
                    let iface_name = ip.interface_name.to_string_path();
                    let signals = if !ip.signals.is_empty() {
                        ip.signals.clone()
                    } else if let Some(sigs) = self.interfaces.get(&iface_name) {
                        sigs.iter()
                            .map(|(sdir, sname, sty)| {
                                let dir = match (ip.role, sdir) {
                                    (InterfaceRole::Master, InterfaceSignalDir::Out) => {
                                        PortDirection::Output
                                    }
                                    (InterfaceRole::Master, InterfaceSignalDir::In) => {
                                        PortDirection::Input
                                    }
                                    (InterfaceRole::Slave, InterfaceSignalDir::Out) => {
                                        PortDirection::Input
                                    }
                                    (InterfaceRole::Slave, InterfaceSignalDir::In) => {
                                        PortDirection::Output
                                    }
                                };
                                (dir, sname.clone(), sty.clone())
                            })
                            .collect()
                    } else {
                        Vec::new()
                    };

                    for (dir, s_name, s_ty) in signals {
                        let sig_key = format!("{}.{}", ip.name.name, s_name);
                        let vcd_name = format!("{}_{}", ip.name.name, s_name);
                        let (w, is_signed) = self.eval_type_width(&s_ty, &inst.params);
                        let kind = match dir {
                            PortDirection::Input => SignalKind::Input,
                            PortDirection::Output => SignalKind::Output,
                        };
                        self.register_signal(
                            &mut inst,
                            &sig_key,
                            &vcd_name,
                            w,
                            is_signed,
                            kind,
                            SimValue::zero(w, is_signed),
                        );
                    }
                }
                _ => {}
            }
        }

        // 2. Elaborate registers (and initialize them to their declared reset_value)
        for item in &decl.items {
            if let TypedModuleItemKind::Register(reg) = &item.kind {
                inst.registers.insert(reg.name.name.clone());
                if let ResolvedType::Array { element, size } = &reg.ty {
                    let (elem_w, is_signed) = self.eval_type_width(element, &inst.params);
                    let arr_len = self.eval_width_expr(size, &inst.params).max(1) as usize;
                    inst.arrays.insert(
                        reg.name.name.clone(),
                        vec![SimValue::zero(elem_w, is_signed); arr_len],
                    );
                } else {
                    let (w, is_signed) = self.eval_type_width(&reg.ty, &inst.params);
                    let init_val = self.eval_expr(&inst, &reg.reset_value);
                    let init_masked = SimValue::new(init_val.bits, w, is_signed);
                    self.register_signal(
                        &mut inst,
                        &reg.name.name,
                        &reg.name.name,
                        w,
                        is_signed,
                        SignalKind::Register,
                        init_masked,
                    );
                }
            }
        }

        // 3. Elaborate combinational `let` wires and child module instances
        for item in &decl.items {
            if let TypedModuleItemKind::Let(let_decl) = &item.kind {
                if let ResolvedType::ModuleInstance { module_name, .. } = &let_decl.ty {
                    if let Some(child_decl) = self.modules.get(module_name).cloned() {
                        let mut child_params = HashMap::new();
                        let param_names: HashSet<String> = child_decl
                            .params
                            .iter()
                            .map(|p| p.name.name.clone())
                            .collect();

                        if let TypedExprKind::Call { args, .. } = &let_decl.value.kind {
                            for arg in args {
                                if let Some(arg_id) = &arg.name
                                    && param_names.contains(&arg_id.name)
                                {
                                    let val = self.eval_expr(&inst, &arg.value);
                                    child_params.insert(arg_id.name.clone(), val.bits as u32);
                                }
                            }
                        }

                        let child_inst =
                            self.elaborate_instance(&let_decl.name.name, &child_decl, child_params);
                        let idx = inst.children.len();
                        inst.child_indices.insert(let_decl.name.name.clone(), idx);
                        inst.children.push(child_inst);
                    }
                } else {
                    let (w, is_signed) = self.eval_type_width(&let_decl.ty, &inst.params);
                    self.register_signal(
                        &mut inst,
                        &let_decl.name.name,
                        &let_decl.name.name,
                        w,
                        is_signed,
                        SignalKind::Wire,
                        SimValue::zero(w, is_signed),
                    );
                }
            }
        }

        // 4. Elaborate procedural `let` variables inside `on` blocks
        let mut proc_lets = Vec::new();
        for item in &decl.items {
            if let TypedModuleItemKind::On(on_block) = &item.kind {
                Self::collect_procedural_lets(&on_block.body, &mut proc_lets);
            }
        }
        for (let_name, let_ty) in proc_lets {
            inst.procedural_lets.insert(let_name.clone());
            let (w, is_signed) = self.eval_type_width(&let_ty, &inst.params);
            self.register_signal(
                &mut inst,
                &let_name,
                &let_name,
                w,
                is_signed,
                SignalKind::Wire,
                SimValue::zero(w, is_signed),
            );
        }

        inst
    }

    fn collect_procedural_lets(stmts: &[TypedStmt], out: &mut Vec<(String, ResolvedType)>) {
        for stmt in stmts {
            match &stmt.kind {
                TypedStmtKind::Let { name, ty, .. } => {
                    if !out.iter().any(|(n, _)| n == &name.name) {
                        out.push((name.name.clone(), ty.clone()));
                    }
                }
                TypedStmtKind::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    Self::collect_procedural_lets(then_branch, out);
                    if let Some(else_stmts) = else_branch {
                        Self::collect_procedural_lets(else_stmts, out);
                    }
                }
                TypedStmtKind::Match { arms, .. } => {
                    for arm in arms {
                        Self::collect_procedural_lets(&arm.body, out);
                    }
                }
                _ => {}
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn register_signal(
        &mut self,
        inst: &mut SimInstance,
        key: &str,
        vcd_name: &str,
        width: u32,
        is_signed: bool,
        kind: SignalKind,
        initial: SimValue,
    ) {
        let vcd_id = self.next_vcd_id();
        inst.signals.insert(key.to_string(), initial);
        inst.signal_meta.push(SignalMeta {
            name: key.to_string(),
            vcd_name: vcd_name.to_string(),
            width,
            is_signed,
            kind,
            vcd_id,
        });
    }
}
