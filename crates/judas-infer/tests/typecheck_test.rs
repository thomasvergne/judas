use judas_infer::*;
use judas_parser::parse_source;

#[test]
fn test_typecheck_counter() {
    let src = r#"
module Counter(width: usize = 8) {
    input clk: Clock
    input reset: Bool
    input enable: Bool

    output count: UInt<width>

    register count_reg: UInt<width> = 0

    count = count_reg

    on rising(clk) {
        if reset {
            count_reg = 0
        } else if enable {
            count_reg = count_reg + 1
        }
    }
}
"#;

    let source = parse_source(src).expect("Parse error");
    let typed_source = infer_source_file(&source).expect("Inference error");

    assert_eq!(typed_source.items.len(), 1);
    let TypedItemKind::Module(ref module) = typed_source.items[0].kind else {
        panic!("Expected module");
    };

    // Check register type
    let TypedModuleItemKind::Register(ref reg) = module.items[4].kind else {
        panic!("Expected register item at index 4");
    };
    assert_eq!(reg.name.name, "count_reg");
    assert_eq!(reg.ty, ResolvedType::uint(8));
    // Reset value literal 0 should have unified to UInt<8>
    assert_eq!(reg.reset_value.ty, ResolvedType::uint(8));

    // Check on block
    let TypedModuleItemKind::On(ref on_block) = module.items[6].kind else {
        panic!("Expected on block at index 6");
    };
    assert_eq!(on_block.trigger.signal.ty, ResolvedType::Clock);

    // Check binary operation `count_reg + 1` inside `else if enable`
    let TypedStmtKind::If {
        ref else_branch, ..
    } = on_block.body[0].kind
    else {
        panic!("Expected if statement");
    };
    let else_stmts = else_branch.as_ref().unwrap();
    let TypedStmtKind::If {
        ref then_branch, ..
    } = else_stmts[0].kind
    else {
        panic!("Expected else if statement");
    };
    let TypedStmtKind::Assign { ref value, .. } = then_branch[0].kind else {
        panic!("Expected assign statement");
    };
    assert_eq!(value.ty, ResolvedType::uint(8));
}

#[test]
fn test_typecheck_adder_with_extend() {
    let src = r#"
module Adder {
    input a: UInt<8>
    input b: UInt<8>

    output sum: UInt<9>

    sum = extend(a) + extend(b)
}
"#;

    let source = parse_source(src).expect("Parse error");
    let typed_source = infer_source_file(&source).expect("Inference error");

    let TypedItemKind::Module(ref module) = typed_source.items[0].kind else {
        panic!("Expected module");
    };

    let TypedModuleItemKind::Connection(ref conn) = module.items[3].kind else {
        panic!("Expected connection item");
    };
    assert_eq!(conn.target.ty, ResolvedType::uint(9));
    assert_eq!(conn.value.ty, ResolvedType::uint(9));
}

#[test]
fn test_typecheck_traffic_light_enum_and_match() {
    let src = r#"
enum Light {
    Red,
    Green,
    Yellow
}

module TrafficLight {
    input clk: Clock
    output light: Light

    register state: Light = Red

    on rising(clk) {
        match state {
            Red    => state = Green
            Green  => state = Yellow
            Yellow => state = Red
        }
    }

    light = state
}
"#;

    let source = parse_source(src).expect("Parse error");
    let typed_source = infer_source_file(&source).expect("Inference error");

    assert_eq!(typed_source.items.len(), 2);
    let TypedItemKind::Module(ref module) = typed_source.items[1].kind else {
        panic!("Expected module");
    };

    // Register `state` should have enum type `Light`
    let TypedModuleItemKind::Register(ref reg) = module.items[2].kind else {
        panic!("Expected register");
    };
    assert_eq!(reg.ty, ResolvedType::Enum("Light".into()));
    assert_eq!(reg.reset_value.ty, ResolvedType::Enum("Light".into()));
}

#[test]
fn test_typecheck_component_instantiation_and_connect() {
    let src = r#"
module Counter(width: usize = 8) {
    input clk: Clock
    input reset: Bool
    input enable: Bool
    output count: UInt<width>
}

module Top {
    input clk: Clock
    input reset: Bool

    let counter = Counter(width: 16)

    connect counter.clk = clk
    connect counter.reset = reset
    connect counter.enable = true

    output count: UInt<16> = counter.count
}
"#;

    let source = parse_source(src).expect("Parse error");
    let typed_source = infer_source_file(&source).expect("Inference error");

    let TypedItemKind::Module(ref top) = typed_source.items[1].kind else {
        panic!("Expected Top module");
    };

    // `let counter = Counter(width: 16)`
    let TypedModuleItemKind::Let(ref let_decl) = top.items[2].kind else {
        panic!("Expected let decl");
    };
    match &let_decl.ty {
        ResolvedType::ModuleInstance { module_name, ports } => {
            assert_eq!(module_name, "Counter");
            assert_eq!(ports.get("count").unwrap().1, ResolvedType::uint(16));
        }
        _ => panic!("Expected module instance type"),
    }

    // `output count: UInt<16> = counter.count`
    let TypedModuleItemKind::Port(ref port) = top.items[6].kind else {
        panic!("Expected port");
    };
    assert_eq!(port.ty, ResolvedType::uint(16));
    let init = port.initializer.as_ref().unwrap();
    assert_eq!(init.ty, ResolvedType::uint(16));
}

#[test]
fn test_typecheck_width_mismatch_error() {
    let src = r#"
module Broken {
    input a: UInt<8>
    output b: UInt<9>

    b = a
}
"#;

    let source = parse_source(src).expect("Parse error");
    let err = infer_source_file(&source).expect_err("Should detect width mismatch");
    assert!(err.message.contains("width mismatch"));
}

#[test]
fn test_typecheck_invalid_clock_error() {
    let src = r#"
module Broken {
    input sig: Bool

    on rising(sig) {
    }
}
"#;

    let source = parse_source(src).expect("Parse error");
    let err = infer_source_file(&source).expect_err("Should detect non-clock trigger");
    assert!(err.message.contains("Type mismatch"));
}

#[test]
fn test_typecheck_generic_adder_with_extend() {
    let src = r#"
module Adder(N: usize) {
    input a: UInt<N>
    input b: UInt<N>
    output sum: UInt<N + 1>

    sum = extend(a) + extend(b)
}
"#;

    let source = parse_source(src).expect("Parse error");
    let typed_source = infer_source_file(&source).expect("Inference error");

    let TypedItemKind::Module(ref module) = typed_source.items[0].kind else {
        panic!("Expected module");
    };

    let TypedModuleItemKind::Connection(ref conn) = module.items[3].kind else {
        panic!("Expected connection item");
    };

    let expected_w = WidthExpr::Add(
        Box::new(WidthExpr::Var("N".into())),
        Box::new(WidthExpr::Const(1)),
    );
    assert_eq!(conn.target.ty, ResolvedType::UInt(expected_w));
}

#[test]
fn test_typecheck_generic_adder_width_mismatch() {
    let src = r#"
module Adder(N: usize) {
    input a: UInt<N>
    input b: UInt<N>
    output sum: UInt<N + 1>

    sum = a + b
}
"#;

    let source = parse_source(src).expect("Parse error");
    let err = infer_source_file(&source).expect_err("Should fail due to N vs N + 1");
    assert!(err.message.contains("width mismatch"));
}

#[test]
fn test_typecheck_generic_concat() {
    let src = r#"
module ConcatTest(N: usize, M: usize) {
    input a: UInt<N>
    input b: UInt<M>
    output out: UInt<N + M>

    out = concat(a, b)
}
"#;

    let source = parse_source(src).expect("Parse error");
    let typed_source = infer_source_file(&source).expect("Inference error");

    let TypedItemKind::Module(ref module) = typed_source.items[0].kind else {
        panic!("Expected module");
    };

    let TypedModuleItemKind::Connection(ref conn) = module.items[3].kind else {
        panic!("Expected connection item");
    };
    let expected_w = WidthExpr::Add(
        Box::new(WidthExpr::Var("N".into())),
        Box::new(WidthExpr::Var("M".into())),
    );
    assert_eq!(conn.target.ty, ResolvedType::UInt(expected_w));
}

#[test]
fn test_typecheck_generic_affine_equivalence() {
    let src = r#"
module CommutativeWidth(N: usize) {
    input a: UInt<1 + N>
    output b: UInt<N + 1>

    b = a
}
"#;

    let source = parse_source(src).expect("Parse error");
    let typed_source = infer_source_file(&source).expect("Inference error");

    let TypedItemKind::Module(ref module) = typed_source.items[0].kind else {
        panic!("Expected module");
    };

    let TypedModuleItemKind::Connection(ref conn) = module.items[2].kind else {
        panic!("Expected connection item");
    };
    assert!(conn.target.ty.is_equivalent_to(&conn.value.ty));
}

#[test]
fn test_typecheck_full_adder() {
    let src = r#"
module FullAdder {
    input a: Bit
    input b: Bit
    input cin: Bit

    output sum: Bit
    output cout: Bit

    sum = a ^ b ^ cin
    cout = (a & b) | (cin & (a ^ b))
}
"#;

    let source = parse_source(src).expect("Parse error");
    let typed_source = infer_source_file(&source).expect("Inference error");

    let TypedItemKind::Module(ref module) = typed_source.items[0].kind else {
        panic!("Expected module");
    };

    // Check sum connection
    let TypedModuleItemKind::Connection(ref sum_conn) = module.items[5].kind else {
        panic!("Expected sum connection");
    };
    assert_eq!(sum_conn.target.ty, ResolvedType::Bit);
    assert_eq!(sum_conn.value.ty, ResolvedType::Bit);

    // Check cout connection
    let TypedModuleItemKind::Connection(ref cout_conn) = module.items[6].kind else {
        panic!("Expected cout connection");
    };
    assert_eq!(cout_conn.target.ty, ResolvedType::Bit);
    assert_eq!(cout_conn.value.ty, ResolvedType::Bit);
}

#[test]
fn test_typecheck_adder_with_carry() {
    let src = r#"
module AdderWithCarry(N: usize) {
    input a: UInt<N>
    input b: UInt<N>
    input cin: Bit

    output sum: UInt<N>
    output cout: Bit

    let total: UInt<N + 1> = extend(a) + extend(b) + extend<N + 1>(cin)

    sum = truncate<N>(total)
    cout = truncate<1>(total >> N)
}
"#;

    let source = parse_source(src).expect("Parse error");
    let typed_source = infer_source_file(&source).expect("Inference error");

    let TypedItemKind::Module(ref module) = typed_source.items[0].kind else {
        panic!("Expected module");
    };

    let TypedModuleItemKind::Connection(ref sum_conn) = module.items[6].kind else {
        panic!("Expected sum connection");
    };
    assert!(
        sum_conn
            .target
            .ty
            .is_equivalent_to(&ResolvedType::UInt(WidthExpr::Var("N".into())))
    );

    let TypedModuleItemKind::Connection(ref cout_conn) = module.items[7].kind else {
        panic!("Expected cout connection");
    };
    assert_eq!(cout_conn.target.ty, ResolvedType::Bit);
}

#[test]
fn test_diagnostic_typo_suggestion() {
    let src = r#"
module TypoTest {
    input clk: Clock
    input reset: Bool

    on rising(clkk) {
    }
}
"#;

    let source = parse_source(src).expect("Parse error");
    let err = infer_source_file(&source).expect_err("Should detect undefined variable");
    assert!(err.message.contains("Undefined variable or signal `clkk`"));
    let help = err.help.expect("Expected help message");
    assert!(help.contains("did you mean `clk`?"), "Help was: {}", help);
}

#[test]
fn test_diagnostic_width_mismatch_help() {
    let src = r#"
module WidthMismatch {
    input a: UInt<8>
    output b: UInt<9>

    b = a
}
"#;

    let source = parse_source(src).expect("Parse error");
    let err = infer_source_file(&source).expect_err("Should detect width mismatch");
    let help = err.help.expect("Expected help message");
    assert!(help.contains("extend<9>"), "Help was: {}", help);
}

#[test]
fn test_diagnostic_invalid_clock_help() {
    let src = r#"
module ClockTest {
    input sig: Bool

    on rising(sig) {
    }
}
"#;

    let source = parse_source(src).expect("Parse error");
    let err = infer_source_file(&source).expect_err("Should detect invalid clock");
    let help = err.help.expect("Expected help message");
    assert!(help.contains("Clock"));
}

#[test]
fn test_typecheck_enum_custom_values() {
    let src = r#"
enum state {
    red = 0b00,
    yellow = 0b01,
    green = 0b10,
}

module FSM {
    input clk: Clock
    output out: state

    register current_state: state = red

    on rising(clk) {
        match current_state {
            red => current_state = yellow
            yellow => current_state = green
            green => current_state = red
        }
    }

    out = current_state
}
"#;

    let source = parse_source(src).expect("Parse error");
    let typed_source = infer_source_file(&source).expect("Inference error");

    let TypedItemKind::Enum(ref enum_decl) = typed_source.items[0].kind else {
        panic!("Expected enum");
    };
    assert_eq!(enum_decl.name.name, "state");
    assert_eq!(enum_decl.variants.len(), 3);
    assert_eq!(enum_decl.variants[0].name.name, "red");
    assert_eq!(enum_decl.variants[0].discriminant, 0);
    assert_eq!(enum_decl.variants[1].name.name, "yellow");
    assert_eq!(enum_decl.variants[1].discriminant, 1);
    assert_eq!(enum_decl.variants[2].name.name, "green");
    assert_eq!(enum_decl.variants[2].discriminant, 2);
}

#[test]
fn test_typecheck_repeat_and_slice() {
    let src = r#"
module BitManip {
    input b: Bit
    input word: UInt<32>

    output sign_ext: UInt<8>
    output nibble: UInt<4>
    output word_part: UInt<8>

    // repeat<N>(val) and val.repeat<N>()
    sign_ext = repeat<8>(b)

    // slice<W>(signal, offset) and signal.slice<W>(offset)
    nibble = slice<4>(word, 0)
    word_part = word.slice<8>(16)
}
"#;

    let source = parse_source(src).expect("Parse error");
    let typed_source = infer_source_file(&source).expect("Inference error");

    let TypedItemKind::Module(ref module) = typed_source.items[0].kind else {
        panic!("Expected module");
    };

    assert_eq!(module.items.len(), 8);
}

#[test]
fn test_typecheck_assertions() {
    let src = r#"
module CheckedCounter {
    input clk: Clock
    input enable: Bool
    output count: UInt<8>

    register val: UInt<8> = 0
    count = val

    // Module body assertion
    assert val < 255, "Counter overflow invariant"

    on rising(clk) {
        if enable {
            // Procedural assertion
            assert enable, "Enable must be asserted inside this branch"
            val = val + 1
        }
    }
}
"#;

    let source = parse_source(src).expect("Parse error");
    let typed_source = infer_source_file(&source).expect("Inference error");

    let TypedItemKind::Module(ref module) = typed_source.items[0].kind else {
        panic!("Expected module");
    };

    // Index 5 should be the Assert item
    let TypedModuleItemKind::Assert(ref assert_decl) = module.items[5].kind else {
        panic!("Expected assert module item");
    };
    assert_eq!(assert_decl.condition.ty, ResolvedType::Bool);
    assert_eq!(
        assert_decl.message.as_deref(),
        Some("Counter overflow invariant")
    );
}

#[test]
fn test_typecheck_struct() {
    let src = r#"
struct RGBColor {
    r: UInt<8>,
    g: UInt<8>,
    b: UInt<8>,
}

module ColorProcessor {
    input in_color: RGBColor
    output red_channel: UInt<8>
    output out_color: RGBColor

    register color_reg: RGBColor = RGBColor { r: 255, g: 0, b: 0 }

    red_channel = in_color.r
    out_color = RGBColor {
        r: in_color.r,
        g: in_color.g,
        b: in_color.b,
    }
}
"#;

    let source = parse_source(src).expect("Parse error");
    let typed_source = infer_source_file(&source).expect("Inference error");

    assert_eq!(typed_source.items.len(), 2);

    let TypedItemKind::Struct(ref struct_decl) = typed_source.items[0].kind else {
        panic!("Expected struct declaration");
    };
    assert_eq!(struct_decl.name.name, "RGBColor");
    assert_eq!(struct_decl.fields.len(), 3);
    assert_eq!(struct_decl.total_width, WidthExpr::Const(24));
}

#[test]
fn test_typecheck_interface() {
    let src = r#"
interface HandshakeBus {
    out data: UInt<32>,
    out valid: Bit,
    in ready: Bit,
}

module Producer {
    input clk: Clock
    master bus: HandshakeBus

    on rising(clk) {
        bus.data = 42
        bus.valid = 1
    }
}

module Consumer {
    input clk: Clock
    slave bus: HandshakeBus

    on rising(clk) {
        bus.ready = 1
    }
}

module Top {
    input clk: Clock

    let prod = Producer(clk: clk)
    let cons = Consumer(clk: clk)

    // Direct interface bundle connection!
    connect cons.bus = prod.bus
}
"#;

    let source = parse_source(src).expect("Parse error");
    let typed_source = infer_source_file(&source).expect("Inference error");

    assert_eq!(typed_source.items.len(), 4);
    let TypedItemKind::Interface(ref iface_decl) = typed_source.items[0].kind else {
        panic!("Expected interface declaration");
    };
    assert_eq!(iface_decl.name.name, "HandshakeBus");
    assert_eq!(iface_decl.signals.len(), 3);
}

#[test]
fn test_typecheck_struct_unknown_field_error() {
    let src = r#"
struct Point {
    x: UInt<16>,
    y: UInt<16>,
}

module Test {
    input p: Point
    output z: UInt<16>

    z = p.z
}
"#;

    let source = parse_source(src).expect("Parse error");
    let err = infer_source_file(&source).unwrap_err();
    assert!(err.to_string().contains("has no field named `z`"));
}

#[test]
fn test_typecheck_mini_cpu() {
    let src = include_str!("../../../examples/mini_cpu.jd");
    let source = parse_source(src).expect("Parse error in mini_cpu.jd");
    let typed_source = infer_source_file(&source).expect("Type inference error in mini_cpu.jd");
    assert_eq!(typed_source.items.len(), 7);
}
