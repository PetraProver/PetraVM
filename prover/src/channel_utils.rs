//! Utility functions for packing values into larger field elements for channel
//! operations.

use binius_field::ExtensionField;
use binius_m3::builder::{upcast_expr, Col, Expr, TableBuilder, B128, B16, B32, B64};

/// Pack two B32 values into a B64 value.
///
/// Format: [val1 (lower 32 bits), val2 (upper 32 bits)]
///
/// # Arguments
/// * `table` - The table builder
/// * `name` - The name for the packed column
/// * `val1` - Value for lower 32 bits (e.g., FP)
/// * `val2` - Value for upper 32 bits (e.g., PC)
///
/// # Returns
/// * A Col<B64, 1> representing the packed values
pub fn pack_b32_into_b64(
    table: &mut TableBuilder,
    name: &str,
    val1: Col<B32, 1>,
    val2: Col<B32, 1>,
) -> Col<B64, 1> {
    // Get the basis for B64 extension of B32
    let basis: [_; 2] = std::array::from_fn(|i| {
        <B64 as ExtensionField<B32>>::basis(i).expect("i in range 0..2; extension degree is 2")
    });

    // Compute the packed value
    table.add_computed(
        name,
        upcast_expr(val1.into()) * basis[0] + upcast_expr(val2.into()) * basis[1],
    )
}

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

/// Pack PC and FP values into a B128 value with zeros in upper positions.
///
/// Format: [zero (0-31), zero (32-63), pc (64-95), fp (96-127)]
///
/// # Arguments
/// * `table` - The table builder
/// * `name` - The name for the packed column
/// * `pc` - The PC value (B32)
/// * `fp` - The FP value (B32)
///
/// # Returns
/// * A Col<B128, 1> representing the packed state
pub fn pack_state_b32_into_b128(
    table: &mut TableBuilder,
    name: &str,
    pc: Col<B32, 1>,
    fp: Col<B32, 1>,
) -> Col<B128, 1> {
    // Create zero constants - using the same format as seen in other code
    let zero_b32 = table.add_constant("zero_b32", [B32::from(0)]);

    // Get the basis for B128 extension of B32
    let basis: [_; 4] = std::array::from_fn(|i| {
        <B128 as ExtensionField<B32>>::basis(i).expect("i in range 0..4; extension degree is 4")
    });

    // Create expressions for each value in the format [zero, zero, pc, fp]
    let exprs: [Expr<B32, 1>; 4] = [zero_b32.into(), zero_b32.into(), pc.into(), fp.into()];

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
