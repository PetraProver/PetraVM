//! Utility functions for packing values into larger field elements for channel
//! operations.

use binius_field::ExtensionField;
use binius_m3::builder::{upcast_expr, Col, Expr, TableBuilder, B128, B16, B32, B64};

/// Pack four B16 values into a B64 value.
///
/// # Arguments
/// * `table` - The table builder
/// * `name` - The name for the packed column
/// * `values` - Array of four B16 values to pack
///
/// # Returns
/// * A Col<B64, 1> representing the packed values
pub fn pack_b16_into_b64(
    table: &mut TableBuilder,
    name: &str,
    values: [Col<B16, 1>; 4],
) -> Col<B64, 1> {
    // Get the basis for B64 extension of B16
    let basis: [_; 4] = std::array::from_fn(|i| {
        <B64 as ExtensionField<B16>>::basis(i).expect("i in range 0..4; extension degree is 4")
    });

    // Create expressions for each value
    let exprs: [Expr<B16, 1>; 4] = [
        values[0].into(),
        values[1].into(),
        values[2].into(),
        values[3].into(),
    ];

    // Compute the packed value
    table.add_computed(
        name,
        exprs
            .into_iter()
            .enumerate()
            .map(|(i, expr)| upcast_expr(expr) * basis[i])
            .reduce(|a, b| a + b)
            .expect("exprs has length 4"),
    )
}

/// Pack instruction (B64) and PC (B64) into a B128 value for PROM channel.
///
/// Format: [instruction (lower 64 bits), PC (upper 64 bits)]
///
/// # Arguments
/// * `table` - The table builder
/// * `name` - The name for the packed column
/// * `instruction` - The packed instruction (B64)
/// * `pc_upcast` - The PC value upcasted to B64
///
/// # Returns
/// * A Col<B128, 1> representing the packed values
pub fn pack_b64_into_b128(
    table: &mut TableBuilder,
    name: &str,
    instruction: Col<B64, 1>,
    pc_upcast: Col<B64, 1>,
) -> Col<B128, 1> {
    // Get the basis for B128 extension of B64
    let basis: [_; 2] = std::array::from_fn(|i| {
        <B128 as ExtensionField<B64>>::basis(i).expect("i in range 0..2; extension degree is 2")
    });

    // Compute the packed value
    table.add_computed(
        name,
        upcast_expr(instruction.into()) * basis[0] + upcast_expr(pc_upcast.into()) * basis[1],
    )
}

/// Pack a PC and instruction values into a B128 value for PROM channel.
///
/// Format: [Instruction (lower 64 bits), PC (upper 64 bits)]
///
/// # Arguments
/// * `table` - The table builder
/// * `name` - The name for the packed column
/// * `pc` - The PC value (B32)
/// * `opcode` - The opcode (B16)
/// * `args` - Array of three B16 values for the arguments
///
/// # Returns
/// * A Col<B128, 1> representing the packed values
pub fn pack_prom_entry(
    table: &mut TableBuilder,
    name: &str,
    pc: Col<B32, 1>,
    opcode: Col<B16, 1>,
    args: [Col<B16, 1>; 3],
) -> Col<B128, 1> {
    // Pack opcode and args into a B64
    let instr = pack_b16_into_b64(
        table,
        &format!("{}_instr", name),
        [opcode, args[0], args[1], args[2]],
    );

    // Upcast PC to B64
    let pc_upcast = table.add_computed(format!("{}_pc_upcast", name), upcast_expr(pc.into()));

    // Pack instruction and PC into a B128
    pack_b64_into_b128(table, name, instr, pc_upcast)
}

pub fn pack_prom_entry_b128(pc: u32, opcode: u16, arg1: u16, arg2: u16, arg3: u16) -> B128 {
    let pc_upcast = pc as u64;
    let instr =
        (opcode as u64) | ((arg1 as u64) << 16) | ((arg2 as u64) << 32) | ((arg3 as u64) << 48);
    B128::from((instr as u128) | (pc_upcast as u128) << 64)
}
