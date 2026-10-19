use judas_ast::*;
use judas_parser::parse_source;

#[test]
fn test_counter() {
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

    let source_file = parse_source(src).expect("Failed to parse Counter");
    assert_eq!(source_file.items.len(), 1);

    let ItemKind::Module(ref module) = source_file.items[0].kind else {
        panic!("Expected module");
    };

    assert_eq!(module.name.name, "Counter");
    assert_eq!(module.params.len(), 1);
    assert_eq!(module.params[0].name.name, "width");

    // Check items
    assert_eq!(module.items.len(), 7); // clk, reset, enable, count, count_reg, count=count_reg, on rising
}

#[test]
fn test_adder() {
    let src = r#"
module Adder {
    input a: UInt<8>
    input b: UInt<8>

    output sum: UInt<9>

    sum = extend(a) + extend(b)
}
"#;

    let source_file = parse_source(src).expect("Failed to parse Adder");
    assert_eq!(source_file.items.len(), 1);

    let ItemKind::Module(ref module) = source_file.items[0].kind else {
        panic!("Expected module");
    };
    assert_eq!(module.name.name, "Adder");
    assert_eq!(module.items.len(), 4);
}

#[test]
fn test_d_flip_flop() {
    let src = r#"
module DFlipFlop {
    input clk: Clock
    input d: Bool
    output q: Bool

    register state: Bool = false

    on rising(clk) {
        state = d
    }

    q = state
}
"#;

    let source_file = parse_source(src).expect("Failed to parse DFlipFlop");
    assert_eq!(source_file.items.len(), 1);

    let ItemKind::Module(ref module) = source_file.items[0].kind else {
        panic!("Expected module");
    };
    assert_eq!(module.name.name, "DFlipFlop");
    assert_eq!(module.items.len(), 6);
}

#[test]
fn test_traffic_light() {
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

    let source_file = parse_source(src).expect("Failed to parse TrafficLight");
    assert_eq!(source_file.items.len(), 2);

    let ItemKind::Enum(ref enum_decl) = source_file.items[0].kind else {
        panic!("Expected enum");
    };
    assert_eq!(enum_decl.name.name, "Light");
    assert_eq!(enum_decl.variants.len(), 3);
    assert_eq!(enum_decl.variants[0].name.name, "Red");
    assert_eq!(enum_decl.variants[1].name.name, "Green");
    assert_eq!(enum_decl.variants[2].name.name, "Yellow");

    let ItemKind::Module(ref module) = source_file.items[1].kind else {
        panic!("Expected module");
    };
    assert_eq!(module.name.name, "TrafficLight");
}

#[test]
fn test_top_component_connect() {
    let src = r#"
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

    let source_file = parse_source(src).expect("Failed to parse Top");
    assert_eq!(source_file.items.len(), 1);

    let ItemKind::Module(ref module) = source_file.items[0].kind else {
        panic!("Expected module");
    };
    assert_eq!(module.name.name, "Top");
    assert_eq!(module.items.len(), 7);

    // Verify compact output with initializer
    let ModuleItemKind::Port(ref port) = module.items[6].kind else {
        panic!("Expected port");
    };
    assert_eq!(port.name.name, "count");
    assert!(port.initializer.is_some());
}

#[test]
fn test_generic_register() {
    let src = r#"
module Register<T, width: usize> {
    input clk: Clock
    input value: T

    output result: T

    register stored: T = zero<T>()

    on rising(clk) {
        stored = value
    }

    result = stored
}
"#;

    let source_file = parse_source(src).expect("Failed to parse Register");
    assert_eq!(source_file.items.len(), 1);

    let ItemKind::Module(ref module) = source_file.items[0].kind else {
        panic!("Expected module");
    };
    assert_eq!(module.name.name, "Register");
    assert_eq!(module.generics.len(), 2);

    match &module.generics[0].kind {
        GenericParamKind::Type { name } => assert_eq!(name.name, "T"),
        _ => panic!("Expected type generic"),
    }

    match &module.generics[1].kind {
        GenericParamKind::Const { name, .. } => assert_eq!(name.name, "width"),
        _ => panic!("Expected const generic"),
    }
}
