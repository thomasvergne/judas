use std::collections::HashMap;
use std::fmt::Write;

use judas_ast::item::PortDirection;
use judas_ast::stmt::EdgeKind;
use judas_infer::{
    ResolvedType, TypedExpr, TypedExprKind, TypedModuleItemKind, TypedSourceFile,
};

use crate::context::{SimConfig, SimInstance, SimReport, Simulator};
use crate::error::SimError;
use crate::value::SimValue;

impl Simulator {
    pub fn run(
        &mut self,
        source: &TypedSourceFile,
        config: &SimConfig,
    ) -> Result<SimReport, SimError> {
        // 1. Collect all declarations
        self.collect_items(&source.imported_items, "");
        self.collect_items(&source.items, "");

        // 2. Find top-level module
        let top_decl = self.find_top_module(&source.items, config.top_module.as_deref())?;
        let top_name = top_decl.name.name.clone();

        // 3. Elaborate top module and its hierarchy
        let mut root = self.elaborate_instance(&top_name, &top_decl, HashMap::new());

        // 4. Identify top-level clock, reset, and enable ports
        let mut clock_ports = Vec::new();
        let mut reset_ports = Vec::new();
        let mut enable_ports = Vec::new();

        for item in &root.decl.items {
            if let TypedModuleItemKind::Port(p) = &item.kind
                && p.direction == PortDirection::Input
            {
                let name_lower = p.name.name.to_lowercase();
                if p.ty == ResolvedType::Clock || name_lower == "clk" || name_lower == "clock" {
                    clock_ports.push(p.name.name.clone());
                } else if name_lower == "rst" || name_lower == "reset" {
                    reset_ports.push((p.name.name.clone(), true));
                } else if name_lower == "rst_n" || name_lower == "reset_n" {
                    reset_ports.push((p.name.name.clone(), false));
                } else if (name_lower == "enable" || name_lower == "en")
                    && matches!(p.ty, ResolvedType::Bool | ResolvedType::Bit)
                {
                    enable_ports.push(p.name.name.clone());
                }
            }
        }

        // 5. Initialize VCD buffer & header
        let mut vcd = String::new();
        self.emit_vcd_header(&root, &mut vcd);

        // 6. Initial state at t = 0
        for clk in &clock_ports {
            root.set_signal(clk, SimValue::from_bool(false));
        }
        for (rst, active_high) in &reset_ports {
            root.set_signal(rst, SimValue::from_bool(*active_high));
        }
        for en in &enable_ports {
            root.set_signal(en, SimValue::from_bool(true));
        }
        for (port_name, &val) in &config.initial_inputs {
            if let Some(existing) = root.signals.get(port_name).copied() {
                root.set_signal(
                    port_name,
                    SimValue::new(val, existing.width, existing.is_signed),
                );
            }
        }

        self.propagate_combinational(&mut root);
        self.record_clocks(&mut root);

        // Dump initial values at #0
        let mut prev_vcd_values: HashMap<String, SimValue> = HashMap::new();
        let _ = writeln!(vcd, "#0");
        let _ = writeln!(vcd, "$dumpvars");
        Self::dump_vcd_initial(&root, &mut vcd, &mut prev_vcd_values);
        let _ = writeln!(vcd, "$end");

        let half = config.half_period_ns.max(1);
        let mut current_time_ns = 0;

        // 7. Cycle-accurate simulation loop
        for cycle in 0..config.cycles {
            // --- First half-cycle: Rising clock edge (clk = 0 -> 1) ---
            current_time_ns += half;
            for clk in &clock_ports {
                root.set_signal(clk, SimValue::from_bool(true));
            }
            self.propagate_combinational(&mut root);
            self.step_clock_edges(&mut root, cycle, current_time_ns)?;
            self.propagate_combinational(&mut root);
            self.check_module_assertions(&root, cycle, current_time_ns)?;
            Self::dump_vcd_changes(&root, current_time_ns, &mut vcd, &mut prev_vcd_values);

            // --- Second half-cycle: Falling clock edge (clk = 1 -> 0) ---
            current_time_ns += half;
            for clk in &clock_ports {
                root.set_signal(clk, SimValue::from_bool(false));
            }
            // De-assert reset on the falling edge of cycle 0
            if cycle == 0 && !reset_ports.is_empty() {
                for (rst, active_high) in &reset_ports {
                    root.set_signal(rst, SimValue::from_bool(!*active_high));
                }
            }
            self.propagate_combinational(&mut root);
            self.step_clock_edges(&mut root, cycle, current_time_ns)?;
            self.propagate_combinational(&mut root);
            self.check_module_assertions(&root, cycle, current_time_ns)?;
            Self::dump_vcd_changes(&root, current_time_ns, &mut vcd, &mut prev_vcd_values);
        }

        // 8. Collect final port values on the top module
        let mut final_ports = Vec::new();
        for item in &root.decl.items {
            if let TypedModuleItemKind::Port(p) = &item.kind
                && let Some(val) = root.signals.get(&p.name.name)
            {
                final_ports.push((p.direction, p.name.name.clone(), *val));
            }
        }

        Ok(SimReport {
            top_module: top_name,
            cycles_simulated: config.cycles,
            final_time_ns: current_time_ns,
            final_ports,
            vcd_content: vcd,
        })
    }

    /// Propagates all combinational wires, port initializers, continuous connections,
    /// and submodule input/output bindings to a fixed point across the hierarchy.
    pub(crate) fn propagate_combinational(&self, inst: &mut SimInstance) {
        for _ in 0..4 {
            self.propagate_instance_once(inst);
        }
    }

    fn propagate_instance_once(&self, inst: &mut SimInstance) {
        // 1. Evaluate combinational `let` wires and inline child instantiation port bindings
        let items = inst.decl.items.clone();
        for item in &items {
            match &item.kind {
                TypedModuleItemKind::Let(let_decl) => {
                    if matches!(let_decl.ty, ResolvedType::ModuleInstance { .. }) {
                        if let Some(&child_idx) = inst.child_indices.get(&let_decl.name.name)
                            && let TypedExprKind::Call { args, .. } = &let_decl.value.kind
                        {
                            let mut port_updates = Vec::new();
                            for arg in args {
                                if let Some(arg_id) = &arg.name
                                    && inst.children[child_idx].signals.contains_key(&arg_id.name)
                                {
                                    let val = self.eval_expr(inst, &arg.value);
                                    port_updates.push((arg_id.name.clone(), val));
                                }
                            }
                            for (p_name, val) in port_updates {
                                inst.children[child_idx].set_signal(&p_name, val);
                            }
                        }
                    } else {
                        let val = self.eval_expr(inst, &let_decl.value);
                        inst.set_signal(&let_decl.name.name, val);
                    }
                }
                TypedModuleItemKind::Port(port) => {
                    if let Some(init) = &port.initializer {
                        let val = self.eval_expr(inst, init);
                        inst.set_signal(&port.name.name, val);
                    }
                }
                TypedModuleItemKind::Connection(conn) => {
                    self.apply_connection(inst, &conn.target, &conn.value);
                }
                _ => {}
            }
        }

        // 2. Propagate inside all child instances
        for child in &mut inst.children {
            self.propagate_instance_once(child);
        }

        // 3. Re-evaluate parent connections & port initializers that depend on child outputs
        for item in &items {
            match &item.kind {
                TypedModuleItemKind::Port(port) => {
                    if let Some(init) = &port.initializer {
                        let val = self.eval_expr(inst, init);
                        inst.set_signal(&port.name.name, val);
                    }
                }
                TypedModuleItemKind::Connection(conn) => {
                    self.apply_connection(inst, &conn.target, &conn.value);
                }
                _ => {}
            }
        }
    }

    fn apply_connection(&self, inst: &mut SimInstance, target: &TypedExpr, value: &TypedExpr) {
        // Case 1: Interface bundle connection between two submodule instances: `connect rom.bus = cpu.bus`
        if let (
            TypedExprKind::FieldAccess {
                base: t_base,
                field: t_field,
            },
            TypedExprKind::FieldAccess {
                base: v_base,
                field: v_field,
            },
        ) = (&target.kind, &value.kind)
            && let (TypedExprKind::Ident(t_inst), TypedExprKind::Ident(v_inst)) =
                (&t_base.kind, &v_base.kind)
            && let ResolvedType::InterfaceInstance { signals, .. } = &target.ty
            && let (Some(&t_idx), Some(&v_idx)) = (
                inst.child_indices.get(&t_inst.name),
                inst.child_indices.get(&v_inst.name),
            )
        {
            for (sig_name, (dir, _)) in signals {
                let t_sig_key = format!("{}.{}", t_field.name, sig_name);
                let v_sig_key = format!("{}.{}", v_field.name, sig_name);
                match dir {
                    PortDirection::Input => {
                        if let Some(&val) = inst.children[v_idx].signals.get(&v_sig_key) {
                            inst.children[t_idx].set_signal(&t_sig_key, val);
                        }
                    }
                    PortDirection::Output => {
                        if let Some(&val) = inst.children[t_idx].signals.get(&t_sig_key) {
                            inst.children[v_idx].set_signal(&v_sig_key, val);
                        }
                    }
                }
            }
            return;
        }

        // Case 2: Single port connection targeting a child instance: `connect counter.clk = clk`
        if let TypedExprKind::FieldAccess { base, field } = &target.kind
            && let TypedExprKind::Ident(child_id) = &base.kind
            && let Some(&child_idx) = inst.child_indices.get(&child_id.name)
        {
            let val = self.eval_expr(inst, value);
            inst.children[child_idx].set_signal(&field.name, val);
            return;
        }

        // Case 3: Local signal or local interface signal connection
        let val = self.eval_expr(inst, value);
        if let Some(sig_key) = Self::resolve_local_target_key(inst, target) {
            inst.set_signal(&sig_key, val);
        }
    }

    pub(crate) fn record_clocks(&self, inst: &mut SimInstance) {
        for item in &inst.decl.items {
            if let TypedModuleItemKind::On(on_block) = &item.kind {
                let clk_val = self.eval_expr(inst, &on_block.trigger.signal);
                let clk_key = Self::expr_key(&on_block.trigger.signal);
                inst.prev_clocks.insert(clk_key, clk_val.bits & 1);
            }
        }
        for child in &mut inst.children {
            self.record_clocks(child);
        }
    }

    pub(crate) fn step_clock_edges(
        &self,
        root: &mut SimInstance,
        cycle: u64,
        time_ns: u64,
    ) -> Result<(), SimError> {
        // Phase 1: Execute `on` blocks that only drive output ports (no `register` declarations)
        self.eval_triggered_on_blocks(root, true, cycle, time_ns)?;
        self.propagate_combinational(root);

        // Phase 2: Execute `on` blocks in modules with `register`s using non-blocking staging
        self.eval_triggered_on_blocks(root, false, cycle, time_ns)?;

        // Update prev_clocks for all instances
        self.record_clocks(root);
        Ok(())
    }

    fn eval_triggered_on_blocks(
        &self,
        inst: &mut SimInstance,
        port_only_modules: bool,
        cycle: u64,
        time_ns: u64,
    ) -> Result<(), SimError> {
        for child in &mut inst.children {
            self.eval_triggered_on_blocks(child, port_only_modules, cycle, time_ns)?;
        }

        let is_port_only = inst.registers.is_empty() && inst.arrays.is_empty();
        if is_port_only != port_only_modules {
            return Ok(());
        }

        let items = inst.decl.items.clone();
        let mut staged_signals: HashMap<String, SimValue> = HashMap::new();
        let mut staged_arrays: HashMap<(String, usize), SimValue> = HashMap::new();

        for item in &items {
            if let TypedModuleItemKind::On(on_block) = &item.kind {
                let clk_key = Self::expr_key(&on_block.trigger.signal);
                let prev_bit = inst.prev_clocks.get(&clk_key).copied().unwrap_or(0);
                let curr_bit = self.eval_expr(inst, &on_block.trigger.signal).bits & 1;

                let triggered = match on_block.trigger.edge {
                    EdgeKind::Rising => prev_bit == 0 && curr_bit == 1,
                    EdgeKind::Falling => prev_bit == 1 && curr_bit == 0,
                };

                if triggered {
                    for stmt in &on_block.body {
                        self.exec_stmt(
                            inst,
                            stmt,
                            &mut staged_signals,
                            &mut staged_arrays,
                            cycle,
                            time_ns,
                        )?;
                    }
                }
            }
        }

        // Commit non-blocking staged updates
        for (sig_key, val) in staged_signals {
            inst.set_signal(&sig_key, val);
        }
        for ((arr_name, idx), val) in staged_arrays {
            if let Some(arr) = inst.arrays.get_mut(&arr_name)
                && idx < arr.len()
            {
                let elem_w = arr[idx].width;
                let is_signed = arr[idx].is_signed;
                arr[idx] = SimValue::new(val.bits, elem_w, is_signed);
            }
        }

        Ok(())
    }

    pub(crate) fn check_module_assertions(
        &self,
        inst: &SimInstance,
        cycle: u64,
        time_ns: u64,
    ) -> Result<(), SimError> {
        for item in &inst.decl.items {
            if let TypedModuleItemKind::Assert(assert_decl) = &item.kind {
                let cond = self.eval_expr(inst, &assert_decl.condition);
                if !cond.as_bool() {
                    return Err(SimError::AssertionFailed {
                        instance: inst.instance_name.clone(),
                        cycle,
                        time_ns,
                        message: assert_decl
                            .message
                            .clone()
                            .unwrap_or_else(|| "Assertion failed".to_string()),
                    });
                }
            }
        }
        for child in &inst.children {
            self.check_module_assertions(child, cycle, time_ns)?;
        }
        Ok(())
    }
}
