use common::test_utils::execute_test_asm;

// filepath: /home/brendan/work/predicate_labs/our_repos/PetraVM/assembly/tests/
// bit_ops.rs
pub mod common;

#[test]
fn test_bit_ops_integration() {
    // Generate the trace for the `bit_ops.asm` program
    let mut info = execute_test_asm(include_str!("../../examples/bit_ops.asm"));
    let bit_ops_frame = info.frames.add_frame("bit_ops");

    assert_eq!(
        bit_ops_frame.get_vrom_expected::<u32>(2),
        0b0011,
        "x = 0b0011"
    );

    // Verify the results of the bitwise operations
    assert_eq!(
        bit_ops_frame.get_vrom_expected::<u32>(3),
        0b0011,
        "a = 0b0011 | 0"
    );
    assert_eq!(
        bit_ops_frame.get_vrom_expected::<u32>(4),
        0b0000,
        "b = 0b0011 & 0"
    );
    assert_eq!(
        bit_ops_frame.get_vrom_expected::<u32>(5),
        0b0011,
        "c = 0b0011 ^ 0"
    );

    // Verify the results of the bitwise operations
    assert_eq!(
        bit_ops_frame.get_vrom_expected::<u32>(6),
        0b0111,
        "d = 0b0011 | 0b0101"
    );
    assert_eq!(
        bit_ops_frame.get_vrom_expected::<u32>(7),
        0b0001,
        "e = 0b0011 & 0b0101"
    );
    assert_eq!(
        bit_ops_frame.get_vrom_expected::<u32>(8),
        0b0110,
        "f = 0b0011 ^ 0b0101"
    );
}
