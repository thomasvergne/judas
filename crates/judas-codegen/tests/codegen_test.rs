use judas_codegen::generate_verilog;
use judas_infer::{infer_source_file, infer_source_file_with_path};
use judas_parser::parse_source;

fn compile_to_verilog(src: &str) -> String {
    let ast = parse_source(src).expect("Failed to parse Judas source");
    let tast = infer_source_file(&ast).expect("Failed to type-check Judas source");
    generate_verilog(&tast)
}

#[test]
fn test_codegen_counter() {
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

    let verilog = compile_to_verilog(src);
    assert!(verilog.contains("module Counter #("));
    assert!(verilog.contains("parameter width = 8"));
    assert!(verilog.contains("input wire clk,"));
    assert!(verilog.contains("input wire reset,"));
    assert!(verilog.contains("input wire enable,"));
    assert!(verilog.contains("output wire [7:0] count"));
    assert!(verilog.contains("reg [7:0] count_reg = 8'd0;"));
    assert!(verilog.contains("assign count = count_reg;"));
    assert!(verilog.contains("always @(posedge clk) begin"));
    assert!(verilog.contains("if (reset) begin"));
    assert!(verilog.contains("count_reg <= 8'd0;"));
    assert!(verilog.contains("end else if (enable) begin"));
    assert!(verilog.contains("count_reg <= count_reg + 8'd1;"));
}

#[test]
fn test_codegen_generic_adder_with_carry() {
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

    let verilog = compile_to_verilog(src);
    assert!(verilog.contains("module AdderWithCarry #("));
    assert!(verilog.contains("input wire [N - 1:0] a,"));
    assert!(verilog.contains("input wire [N - 1:0] b,"));
    assert!(verilog.contains("input wire cin,"));
    assert!(verilog.contains("output wire [N - 1:0] sum,"));
    assert!(verilog.contains("output wire cout"));
    assert!(verilog.contains("wire [N:0] total = ({1'b0, a} + {1'b0, b}) + {{(N){1'b0}}, cin};"));
    assert!(verilog.contains("assign sum = total[N - 1:0];"));
    assert!(verilog.contains("assign cout = total[N];"));
}

#[test]
fn test_codegen_enum_and_match() {
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

    let verilog = compile_to_verilog(src);
    assert!(verilog.contains("output wire [1:0] light"));
    assert!(verilog.contains("reg [1:0] state = 2'd0;"));
    assert!(verilog.contains("case (state)"));
    assert!(verilog.contains("2'd0: begin"));
    assert!(verilog.contains("state <= 2'd1;"));
    assert!(verilog.contains("2'd1: begin"));
    assert!(verilog.contains("state <= 2'd2;"));
    assert!(verilog.contains("2'd2: begin"));
    assert!(verilog.contains("state <= 2'd0;"));
}

#[test]
fn test_codegen_packed_struct() {
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

    let verilog = compile_to_verilog(src);
    assert!(verilog.contains("input wire [23:0] in_color,"));
    assert!(verilog.contains("output wire [7:0] red_channel,"));
    assert!(verilog.contains("output wire [23:0] out_color"));
    assert!(verilog.contains("reg [23:0] color_reg = {8'd255, 8'd0, 8'd0};"));
    assert!(verilog.contains("assign red_channel = in_color[23:16];"));
    assert!(verilog.contains("assign out_color = {in_color[23:16], in_color[15:8], in_color[7:0]};"));
}

#[test]
fn test_codegen_interface_and_bundle_connect() {
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

    connect cons.bus = prod.bus
}
"#;

    let verilog = compile_to_verilog(src);
    assert!(verilog.contains("output reg [31:0] bus_data,"));
    assert!(verilog.contains("output reg bus_valid,"));
    assert!(verilog.contains("input wire bus_ready"));
    assert!(verilog.contains("Producer prod ("));
    assert!(verilog.contains(".bus_data(prod_bus_data)"));
    assert!(verilog.contains(".bus_ready(cons_bus_ready)"));
    assert!(verilog.contains("Consumer cons ("));
}

#[test]
fn test_codegen_bit_manip_and_assertions() {
    let src = r#"
module BitManip {
    input clk: Clock
    input b: Bit
    input word: UInt<32>

    output sign_ext: UInt<8>
    output nibble: UInt<4>
    output word_part: UInt<8>

    sign_ext = repeat<8>(b)
    nibble = slice<4>(word, 0)
    word_part = word.slice<8>(16)

    assert word != 0, "Word must be non-zero"
}
"#;

    let verilog = compile_to_verilog(src);
    assert!(verilog.contains("assign sign_ext = {8{b}};"));
    assert!(verilog.contains("assign nibble = word[3:0];"));
    assert!(verilog.contains("assign word_part = word[23:16];"));
    assert!(verilog.contains("$display(\"Assertion failed: Word must be non-zero\");"));
}

#[test]
fn test_codegen_mini_cpu_example() {
    let path = std::path::Path::new("../../examples/mini_cpu.jd");
    let src = std::fs::read_to_string(path).expect("Failed to read mini_cpu.jd");
    let ast = parse_source(&src).expect("Parse error");
    let tast = infer_source_file_with_path(&ast, Some(path)).expect("Typecheck error");
    let verilog = generate_verilog(&tast);

    assert!(verilog.contains("module ProgramRom ("));
    assert!(verilog.contains("module CpuCore ("));
    assert!(verilog.contains("module MiniCpuSystem ("));
    assert!(verilog.contains("CpuCore cpu ("));
    assert!(verilog.contains("ProgramRom rom ("));
    assert!(verilog.contains("inst = {ir[15:12], ir[11:10], ir[9:8], ir[7:0]};"));
}

#[test]
fn test_codegen_file_import_main_example() {
    let path = std::path::Path::new("../../examples/main.jd");
    let src = std::fs::read_to_string(path).expect("Failed to read main.jd");
    let ast = parse_source(&src).expect("Parse error");
    let tast = infer_source_file_with_path(&ast, Some(path)).expect("Typecheck error");
    let verilog = generate_verilog(&tast);

    assert!(verilog.contains("module Counter #("));
    assert!(verilog.contains("module Top ("));
    assert!(verilog.contains("Counter #("));
    assert!(verilog.contains(".width(16)"));
    assert!(verilog.contains(") counter ("));
}
