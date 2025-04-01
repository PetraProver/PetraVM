//! Utility functions for packing values into larger field elements for channel
//! operations.

use binius_m3::builder::{upcast_expr, Col, TableBuilder, B128, B16, B32};

macro_rules! pack_instruction_common {
    ($table:expr, $name:expr, $pc:expr, $args:expr, $opcode_expr:expr) => {
        $table.add_computed(
            $name,
            // Instruction part (lower 64 bits)
            upcast_expr($args[0].into()) * B128::from(1u128 << 16) +
            upcast_expr($args[1].into()) * B128::from(1u128 << 32) +
            upcast_expr($args[2].into()) * B128::from(1u128 << 48) +
            // PC part (upper 64 bits)
            upcast_expr($pc.into()) * B128::from(1u128 << 64) + $opcode_expr,
        )
    };
}

/// Packs an instruction with a 32-bit immediate value.
///
/// Format: [PC (32 bits) | imm (32 bits) | arg (16 bits) | opcode (16 bits)]
pub fn pack_instruction_with_32bits_imm(
    table: &mut TableBuilder,
    name: &str,
    pc: Col<B32, 1>,
    opcode: u32,
    arg: Col<B16, 1>,
    imm: Col<B32, 1>,
) -> Col<B128, 1> {
    table.add_computed(
        name,
        upcast_expr(arg.into()) * B128::from(1u128 << 16)
            + upcast_expr(imm.into()) * B128::from(1u128 << 32)
            + upcast_expr(pc.into()) * B128::from(1u128 << 64)
            + B128::new(opcode as u128),
    )
}

/// Packs an instruction with a fixed opcode value.
///
/// Format: [PC (32 bits) | arg3 (16 bits) | arg2 (16 bits) | arg1 (16 bits) |
/// opcode (16 bits)]
pub fn pack_instruction_with_fixed_opcode(
    table: &mut TableBuilder,
    name: &str,
    pc: Col<B32, 1>,
    opcode: u32,
    args: [Col<B16, 1>; 3],
) -> Col<B128, 1> {
    pack_instruction_common!(table, name, pc, args, B128::new(opcode as u128))
}

/// Packs an instruction with a variable opcode column.
///
/// Format: [PC (32 bits) | arg3 (16 bits) | arg2 (16 bits) | arg1 (16 bits) |
/// opcode (16 bits)]
pub fn pack_instruction(
    table: &mut TableBuilder,
    name: &str,
    pc: Col<B32, 1>,
    opcode: Col<B16, 1>,
    args: [Col<B16, 1>; 3],
) -> Col<B128, 1> {
    pack_instruction_common!(table, name, pc, args, upcast_expr(opcode.into()))
}

/// Creates a B128 value by packing instruction components with constant values.
///
/// Format: [PC (32 bits) | arg3 (16 bits) | arg2 (16 bits) | arg1 (16 bits) |
/// opcode (16 bits)]
pub fn pack_instruction_b128(pc: u32, opcode: u16, arg1: u16, arg2: u16, arg3: u16) -> B128 {
    let instr =
        (opcode as u64) | ((arg1 as u64) << 16) | ((arg2 as u64) << 32) | ((arg3 as u64) << 48);
    B128::from((instr as u128) | ((pc as u128) << 64))
}

/// Creates a B128 value by packing instruction components with a 32-bit
/// immediate value.
///
/// Format: [PC (32 bits) | imm (32 bits) | arg (16 bits) | opcode (16 bits)]
pub fn pack_instruction_with_32bits_imm_b128(pc: u32, opcode: u16, arg: u16, imm: u32) -> B128 {
    let instr = (opcode as u64) | ((arg as u64) << 16) | ((imm as u64) << 32);
    B128::from((instr as u128) | ((pc as u128) << 64))
}
