use std::collections::HashMap;
use std::fmt::Write;

use crate::context::{SimInstance, Simulator};
use crate::value::{SignalKind, SimValue};

impl Simulator {
    pub(crate) fn emit_vcd_header(&self, root: &SimInstance, out: &mut String) {
        let _ = writeln!(out, "$version Judas HDL Native Simulator 0.1.0 $end");
        let _ = writeln!(out, "$timescale 1ns $end");
        Self::emit_vcd_scope(root, out, 0);
        let _ = writeln!(out, "$enddefinitions $end");
    }

    fn emit_vcd_scope(inst: &SimInstance, out: &mut String, depth: usize) {
        let pad = "  ".repeat(depth);
        let _ = writeln!(out, "{}$scope module {} $end", pad, inst.instance_name);
        for meta in &inst.signal_meta {
            let vcd_type = match meta.kind {
                SignalKind::Register => "reg",
                SignalKind::Input | SignalKind::Output | SignalKind::Wire => "wire",
            };
            let _ = writeln!(
                out,
                "{}  $var {} {} {} {} $end",
                pad, vcd_type, meta.width, meta.vcd_id, meta.vcd_name
            );
        }
        for child in &inst.children {
            Self::emit_vcd_scope(child, out, depth + 1);
        }
        let _ = writeln!(out, "{}$upscope $end", pad);
    }

    pub(crate) fn dump_vcd_initial(
        inst: &SimInstance,
        out: &mut String,
        prev_values: &mut HashMap<String, SimValue>,
    ) {
        for meta in &inst.signal_meta {
            if let Some(&val) = inst.signals.get(&meta.name) {
                let _ = writeln!(out, "{}", val.format_vcd(&meta.vcd_id));
                prev_values.insert(meta.vcd_id.clone(), val);
            }
        }
        for child in &inst.children {
            Self::dump_vcd_initial(child, out, prev_values);
        }
    }

    pub(crate) fn dump_vcd_changes(
        inst: &SimInstance,
        time_ns: u64,
        out: &mut String,
        prev_values: &mut HashMap<String, SimValue>,
    ) {
        let mut changes = Vec::new();
        Self::collect_vcd_changes(inst, prev_values, &mut changes);
        if !changes.is_empty() {
            let _ = writeln!(out, "#{}", time_ns);
            for line in changes {
                let _ = writeln!(out, "{}", line);
            }
        }
    }

    fn collect_vcd_changes(
        inst: &SimInstance,
        prev_values: &mut HashMap<String, SimValue>,
        changes: &mut Vec<String>,
    ) {
        for meta in &inst.signal_meta {
            if let Some(&val) = inst.signals.get(&meta.name) {
                let changed = match prev_values.get(&meta.vcd_id) {
                    Some(prev) => prev.bits != val.bits,
                    None => true,
                };
                if changed {
                    changes.push(val.format_vcd(&meta.vcd_id));
                    prev_values.insert(meta.vcd_id.clone(), val);
                }
            }
        }
        for child in &inst.children {
            Self::collect_vcd_changes(child, prev_values, changes);
        }
    }
}
