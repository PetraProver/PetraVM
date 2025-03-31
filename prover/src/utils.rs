//! Utility functions for packing values into larger field elements for channel
//! operations.

use binius_m3::builder::{upcast_expr, Col, TableBuilder, B128, B16, B32};

use crate::opcodes::util::{pack_b16_into_b64, pack_b64_into_b128};

pub fn pack_prom_opcode(
    table: &mut TableBuilder,
    name: &str,
    pc: Col<B32, 1>,
    opcode: u32,
    args: [Col<B16, 1>; 3],
) -> Col<B128, 1> {
    table.add_computed(
        name,
        // Instruction part (lower 64 bits)
        upcast_expr(args[0].into()) * B128::from(1u128 << 16) +
        upcast_expr(args[1].into()) * B128::from(1u128 << 32) +
        upcast_expr(args[2].into()) * B128::from(1u128 << 48) +
        // PC part (upper 64 bits)
        upcast_expr(pc.into()) * B128::from(1u128 << 64) + B128::new(opcode as u128),
    )
}

pub fn pack_prom_entry(
    table: &mut TableBuilder,
    name: &str,
    pc: Col<B32, 1>,
    opcode: Col<B16, 1>,
    args: [Col<B16, 1>; 3],
) -> Col<B128, 1> {
    table.add_computed(
        name,
        pack_b64_into_b128([
            pack_b16_into_b64([
                opcode.into(),
                args[0].into(),
                args[1].into(),
                args[2].into(),
            ]),
            upcast_expr(pc.into()),
        ]),
    )
}

pub fn pack_prom_entry_u128(pc: u32, opcode: u16, arg0: u16, arg1: u16, arg2: u16) -> u128 {
    (pc as u128) << 64
        | opcode as u128
        | (arg0 as u128) << 16
        | (arg1 as u128) << 32
        | (arg2 as u128) << 48
}
