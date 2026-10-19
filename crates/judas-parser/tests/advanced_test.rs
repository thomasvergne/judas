use judas_parser::parse_source;

#[test]
fn test_complex_types_and_operators() {
    let src = r#"
module Shifter {
    input a: Bits<32>
    input shift_amount: UInt<5>
    output result: Bits<32>

    // Bitwise and shift operations
    result = (a << shift_amount) | (a >> (32 - shift_amount))
}
"#;

    let sf = parse_source(src).expect("Failed to parse Shifter");
    assert_eq!(sf.items.len(), 1);
}

#[test]
fn test_array_types() {
    let src = r#"
module Memory {
    input clk: Clock
    input addr: UInt<4>
    output data: UInt<8>

    register mem: Array<UInt<8>, 16> = zero()

    data = mem[addr]
}
"#;

    let sf = parse_source(src).expect("Failed to parse Memory");
    assert_eq!(sf.items.len(), 1);
}

#[test]
fn test_literals_and_comments() {
    let src = r#"
/* Multi-line block comment
   defining Constants module */
module Constants {
    // Binary literal
    output bin_val: Bits<8> = 0b1010_0101
    // Hex literal
    output hex_val: Bits<8> = 0xFF
}
"#;

    let sf = parse_source(src).expect("Failed to parse Constants");
    assert_eq!(sf.items.len(), 1);
}

#[test]
fn test_error_reporting() {
    let src = r#"
module Incomplete {
    input clk: Clock
    output missing_type:
}
"#;

    let err = parse_source(src).expect_err("Should fail to parse");
    // Line 5 contains `}` where a type was expected
    assert_eq!(err.span.line, 5);
}
