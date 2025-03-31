//! Utility functions for packing values into larger field elements for channel
//! operations.

use binius_m3::builder::{upcast_expr, Col, TableBuilder, B128, B16, B32};

macro_rules! pack_prom_common {
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

pub fn pack_prom_opcode(
    table: &mut TableBuilder,
    name: &str,
    pc: Col<B32, 1>,
    opcode: u32,
    args: [Col<B16, 1>; 3],
) -> Col<B128, 1> {
    pack_prom_common!(table, name, pc, args, B128::new(opcode as u128))
}

pub fn pack_prom_entry(
    table: &mut TableBuilder,
    name: &str,
    pc: Col<B32, 1>,
    opcode: Col<B16, 1>,
    args: [Col<B16, 1>; 3],
) -> Col<B128, 1> {
    pack_prom_common!(table, name, pc, args, upcast_expr(opcode.into()))
}

pub fn pack_prom_entry_b128(pc: u32, opcode: u16, arg1: u16, arg2: u16, arg3: u16) -> B128 {
    let pc_upcast = pc as u64;
    let instr =
        (opcode as u64) | ((arg1 as u64) << 16) | ((arg2 as u64) << 32) | ((arg3 as u64) << 48);
    B128::from((instr as u128) | ((pc_upcast as u128) << 64))
}
