use std::path::Path;

use judas_ast::PortDirection;
use judas_infer::{infer_source_file, infer_source_file_with_path};
use judas_parser::parse_source;
use judas_sim::{SimConfig, SimError, simulate};

#[test]
fn test_simulate_mini_cpu() {
    let path = Path::new("../../examples/mini_cpu.jd");
    let src = std::fs::read_to_string(path).expect("failed to read mini_cpu.jd");
    let ast = parse_source(&src).expect("parse failed");
    let tast = infer_source_file_with_path(&ast, Some(path)).expect("infer failed");

    let config = SimConfig {
        cycles: 100,
        ..Default::default()
    };
    let report = simulate(&tast, &config).expect("simulation failed");

    assert_eq!(report.top_module, "MiniCpuSystem");
    assert_eq!(report.cycles_simulated, 100);
    assert_eq!(report.final_time_ns, 1000);

    let find_out = |name: &str| -> u128 {
        report
            .final_ports
            .iter()
            .find(|(dir, n, _)| *dir == PortDirection::Output && n == name)
            .map(|(_, _, v)| v.bits)
            .expect("missing output port")
    };

    // 1 + 2 + 3 + 4 + 5 = 15
    assert_eq!(find_out("out_data"), 15);
    assert_eq!(find_out("pc"), 7);
    assert_eq!(find_out("is_halted"), 1);

    assert!(report.vcd_content.contains("$scope module MiniCpuSystem $end"));
    assert!(report.vcd_content.contains("$scope module cpu $end"));
    assert!(report.vcd_content.contains("$scope module rom $end"));
    assert!(report.vcd_content.contains("$dumpvars"));
}

#[test]
fn test_simulate_counter_and_main() {
    let path = Path::new("../../examples/main.jd");
    let src = std::fs::read_to_string(path).expect("failed to read main.jd");
    let ast = parse_source(&src).expect("parse failed");
    let tast = infer_source_file_with_path(&ast, Some(path)).expect("infer failed");

    let config = SimConfig {
        cycles: 10,
        ..Default::default()
    };
    let report = simulate(&tast, &config).expect("simulation failed");
    assert_eq!(report.top_module, "Top");

    let count_val = report
        .final_ports
        .iter()
        .find(|(_, n, _)| n == "count")
        .map(|(_, _, v)| v.bits)
        .expect("missing count port");
    // Cycle 0 has reset asserted, cycles 1..10 (9 cycles) increment count from 0 to 9
    assert_eq!(count_val, 9);
}

#[test]
fn test_simulate_assertion_failure() {
    let src = r#"
module FaultyCounter {
    input clk: Clock
    register count: UInt<4> = 0
    assert count < 3, "count reached 3"

    on rising(clk) {
        count = count + 1
    }
}
"#;
    let ast = parse_source(src).expect("parse failed");
    let tast = infer_source_file(&ast).expect("infer failed");

    let config = SimConfig {
        cycles: 10,
        ..Default::default()
    };
    let err = simulate(&tast, &config).expect_err("expected assertion failure");
    match err {
        SimError::AssertionFailed { message, .. } => {
            assert_eq!(message, "count reached 3");
        }
        other => panic!("unexpected error: {:?}", other),
    }
}

#[test]
fn test_simulate_dsp48e1() {
    let path = Path::new("../../examples/dsp48e1.jd");
    let src = std::fs::read_to_string(path).expect("failed to read dsp48e1.jd");
    let ast = parse_source(&src).expect("parse failed");
    let tast = infer_source_file_with_path(&ast, Some(path)).expect("infer failed");

    let config = SimConfig {
        cycles: 10,
        ..Default::default()
    };
    let report = simulate(&tast, &config).expect("simulation failed");
    assert_eq!(report.top_module, "Dsp48e1System");

    let find_out = |name: &str| -> u128 {
        report
            .final_ports
            .iter()
            .find(|(dir, n, _)| *dir == PortDirection::Output && n == name)
            .map(|(_, _, v)| v.bits)
            .expect("missing output port")
    };

    // DSP0: (3 + 7) * 4 + 10 = 50
    assert_eq!(find_out("p0_out"), 50);
    // DSP1: 50 + (2 + 8) * 5 = 100
    assert_eq!(find_out("p1_out"), 100);
    // Pattern detector matched 100 (0x64)
    assert_eq!(find_out("pattern_matched"), 1);
}
