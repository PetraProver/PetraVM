//! Utility functions for packing values into larger field elements for channel
//! operations.

use binius_field::ExtensionField;
use binius_m3::builder::{upcast_col, upcast_expr, Col, Expr, TableBuilder, B128, B16, B32, B64};

/// Get a B128 basis element by index
#[inline]
fn b128_basis(index: usize) -> B128 {
    <B128 as ExtensionField<B16>>::basis(index)
}

/// Convenience macro to "pack" an instruction with its arguments and the
/// program counter into a single 128-bit element.
///
/// The resulting packed instruction is defined as (in big-endian notation):
///
/// [ 0_32b || pc_32b || arg2_16b || arg1_16b || arg0_16b || opcode_16b ]
///
/// # Example
///
/// ```ignore
/// pack_instruction_common!(&mut table, "instruction", pc, [arg0, arg1, arg2], opcode);
/// ```
macro_rules! pack_instruction_common {
    ($table:expr, $name:expr, $pc:expr, $args:expr, $opcode_expr:expr) => {
        $table.add_computed(
            $name,
            // Instruction part (lower 64 bits)
            upcast_expr($args[0].into()) * b128_basis(1) +
            upcast_expr($args[1].into()) * b128_basis(2) +
            upcast_expr($args[2].into()) * b128_basis(3) +
            // PC part (upper 64 bits)
            upcast_expr($pc.into()) * b128_basis(4) + $opcode_expr,
        )
    };
}

/// Packs an instruction with a 32-bit immediate value.
///
/// Format: [PC (32 bits) | imm (32 bits) | arg (16 bits) | opcode (16 bits)]
///
/// The immediate value is stored as a full 32-bit value, not split into
/// high/low parts.
pub fn pack_instruction_with_32bits_imm(
    table: &mut TableBuilder,
    name: &str,
    pc: Col<B32>,
    opcode: u16,
    arg: Col<B16>,
    imm: Col<B32>,
) -> Col<B128> {
    table.add_computed(
        name,
        upcast_expr(arg.into()) * b128_basis(1)
            + upcast_expr(imm.into()) * b128_basis(2)
            + upcast_expr(pc.into()) * b128_basis(4)
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
    pc: Col<B32>,
    opcode: u16,
    args: [Col<B16>; 3],
) -> Col<B128> {
    pack_instruction_common!(table, name, pc, args, B128::new(opcode as u128))
}

/// Packs an instruction with a variable opcode column.
///
/// Format: [PC (32 bits) | arg3 (16 bits) | arg2 (16 bits) | arg1 (16 bits) |
/// opcode (16 bits)]
pub fn pack_instruction(
    table: &mut TableBuilder,
    name: &str,
    pc: Col<B32>,
    opcode: Col<B16>,
    args: [Col<B16>; 3],
) -> Col<B128> {
    pack_instruction_common!(table, name, pc, args, upcast_expr(opcode.into()))
}

/// Adds a computed column that packs an instruction with just PC and opcode
/// (zeroes for all arguments) in a table builder context.
///
/// Format: [PC (32 bits) | 0 | 0 | 0 | opcode (16 bits)]
pub fn pack_instruction_no_args(
    table: &mut TableBuilder,
    name: &str,
    pc: Col<B32>,
    opcode: u16,
) -> Col<B128> {
    table.add_computed(
        name,
        upcast_expr(pc.into()) * b128_basis(4) + B128::new(opcode as u128),
    )
}

/// Packs an instruction with a single argument.
///
/// Format: [PC (32 bits) | 0 | 0 | arg (16 bits) | opcode (16 bits)]
pub fn pack_instruction_one_arg(
    table: &mut TableBuilder,
    name: &str,
    pc: Col<B32>,
    opcode: u16,
    arg: Col<B16>,
) -> Col<B128> {
    table.add_computed(
        name,
        upcast_expr(arg.into()) * b128_basis(1)
            + upcast_expr(pc.into()) * b128_basis(4)
            + B128::new(opcode as u128),
    )
}

/// Creates a B128 value by packing instruction components with constant values.
///
/// Format: [PC (32 bits) | arg3 (16 bits) | arg2 (16 bits) | arg1 (16 bits) |
/// opcode (16 bits)]
pub fn pack_instruction_b128(pc: B32, opcode: B16, arg1: B16, arg2: B16, arg3: B16) -> B128 {
    let b1 = B128::new(opcode.val() as u128);
    let b2 = b128_basis(1) * arg1;
    let b3 = b128_basis(2) * arg2;
    let b4 = b128_basis(3) * arg3;
    let b5 = b128_basis(4) * pc;
    b1 + b2 + b3 + b4 + b5
}

/// Creates a u128 value by packing instruction components with constant values.
///
/// Format: [PC (32 bits) | arg3 (16 bits) | arg2 (16 bits) | arg1 (16 bits) |
/// opcode (16 bits)]
#[inline(always)]
pub const fn pack_instruction_u128(pc: u32, opcode: u16, arg1: u16, arg2: u16, arg3: u16) -> B128 {
    B128::new(
        opcode as u128
            | (arg1 as u128) << 16
            | (arg2 as u128) << 32
            | (arg3 as u128) << 48
            | (pc as u128) << 64,
    )
}

/// Creates a B128 value by packing instruction components with a 32-bit
/// immediate value.
///
/// Format: [PC (32 bits) | imm (32 bits) | arg (16 bits) | opcode (16 bits)]
///
/// The immediate value is stored as a full 32-bit value, not split into
/// high/low parts.
pub fn pack_instruction_with_32bits_imm_b128(pc: B32, opcode: B16, arg: B16, imm: B32) -> B128 {
    let b1 = B128::new(opcode.val() as u128);
    let b2 = b128_basis(1) * arg;
    let b3 = b128_basis(2) * imm;
    let b4 = b128_basis(4) * pc;
    b1 + b2 + b3 + b4
}

/// Packs two 16-bit limbs into a single 32-bit value.
pub(crate) fn pack_b16_into_b32(low: Col<B16, 1>, high: Col<B16, 1>) -> Expr<B32, 1> {
    upcast_expr(high.into()) * <B32 as ExtensionField<B16>>::basis(1) + upcast_expr(low.into())
}

pub(crate) fn pack_vrom_entry(addr: Col<B32>, value: Col<B32>) -> Expr<B64, 1> {
    upcast_col(addr) * <B64 as ExtensionField<B32>>::basis(0)
        + upcast_col(value) * <B64 as ExtensionField<B32>>::basis(1)
}

pub(crate) fn pack_vrom_entry_u64(addr: u32, value: u32) -> u64 {
    ((value as u64) << 32) + addr as u64
}
