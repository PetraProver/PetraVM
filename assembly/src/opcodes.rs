use binius_m3::builder::B16;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pest::pratt_parser::Op;
use strum::EnumCount;
use strum_macros::EnumCount;

/// Represents the set of instructions supported by the zCrayVM.
#[derive(
    Debug, Clone, Copy, Default, EnumCount, TryFromPrimitive, IntoPrimitive, PartialEq, Eq,
)]
#[repr(u16)]
#[allow(clippy::upper_case_acronyms)]
// TODO: Adjust opcode discriminants once settled on their values.
// Consider Deref to account for aliases?
pub enum Opcode {
    // Integer instructions
    Xori = 0x02,
    Xor = 0x03,
    Andi = 0x04,
    Srli = 0x05,
    Slli = 0x06,
    Srai = 0x22,
    Addi = 0x07,
    Add = 0x08,
    Muli = 0x09,
    Mulu = 0x23,
    Mulsu = 0x24,
    Mul = 0x1f,
    B32Mul = 0x10,
    B32Muli = 0x27,
    B128Add = 0x16,
    B128Mul = 0x17,
    And = 0x13,
    Or = 0x14,
    Ori = 0x15,
    Sub = 0x19,
    Slt = 0x25,
    Slti = 0x26,
    Sltu = 0x1a,
    Sltiu = 0x1b,
    Sll = 0x1c,
    Srl = 0x1d,
    Sra = 0x1e,

    // Move instructions
    Mvvw = 0x0d,
    Mvih = 0x0e,
    Ldi = 0x0f,
    Mvvl = 0x11,

    // Jump instructions
    Jumpi = 0x20,
    Jumpv = 0x21,
    Taili = 0x0c,
    Tailv = 0x12,
    Calli = 0x18,
    Callv = 0x0a,
    Ret = 0x0b,

    // Branch instructions
    Bnz = 0x01,

    // Memory Access (RAM) instructions
    // TODO: optional ISA extension for future implementation
    // Not needed for recursion program or first version of zCrayVM
    // Design note: Considering 32-bit word-sized memory instead of byte-addressed memory
    // LW,
    // SW,
    // LB,
    // LBU,
    // LH,
    // LHU,
    // SB,
    // SH,
    #[default]
    Invalid = 0x00,
}

impl Opcode {
    pub const OP_COUNT: usize = Self::COUNT - 1;
    pub const fn get_field_elt(&self) -> B16 {
        B16::new(*self as u16)
    }

    /// Returns the number of arguments expected by the given opcode.
    pub fn num_args(&self) -> usize {
        match self {
            Opcode::Bnz => 3,     // cond, target_low, target_high
            Opcode::Jumpi => 2,   // target_low, target_high
            Opcode::Jumpv => 1,   // offset
            Opcode::Xori => 3,    // dst, src, imm
            Opcode::Xor => 3,     // dst, src1, src2
            Opcode::Ret => 0,     //
            Opcode::Slli => 3,    // dst, src, imm
            Opcode::Srli => 3,    // dst, src, imm
            Opcode::Srai => 3,    // dst, src, imm
            Opcode::Sll => 3,     // dst, src1, src2
            Opcode::Srl => 3,     // dst, src1, src2
            Opcode::Sra => 3,     // dst, src1, src2
            Opcode::Tailv => 2,   // offset, next_fp
            Opcode::Taili => 3,   // target_low, target_high, next_fp
            Opcode::Calli => 3,   // target_low, target_high, next_fp
            Opcode::Callv => 2,   // offset, next_fp
            Opcode::And => 3,     // dst, src1, src2
            Opcode::Andi => 3,    // dst, src, imm
            Opcode::Sub => 3,     // dst, src1, src2
            Opcode::Slt => 3,     // dst, src1, src2
            Opcode::Slti => 3,    // dst, src, imm
            Opcode::Sltu => 3,    // dst, src1, src2
            Opcode::Sltiu => 3,   // dst, src, imm
            Opcode::Or => 3,      // dst, src1, src2
            Opcode::Ori => 3,     // dst, src, imm
            Opcode::Muli => 3,    // dst, src, imm
            Opcode::Mulu => 3,    // dst, src1, src2
            Opcode::Mul => 3,     // dst, src1, src2
            Opcode::Mulsu => 3,   // dst, src1, src2
            Opcode::B32Mul => 3,  // dst, src1, src2
            Opcode::B32Muli => 3, // dst, src, imm
            Opcode::B128Add => 3, // dst, src1, src2
            Opcode::B128Mul => 3, // dst, src, imm
            Opcode::Add => 3,     // dst, src1, src2
            Opcode::Addi => 3,    // dst, src, imm
            Opcode::Mvvw => 3,    // dst, offset, src
            Opcode::Mvvl => 3,    // dst, offset, src
            Opcode::Mvih => 3,    // dst, offset, imm
            Opcode::Ldi => 3,     // dst, imm_low, imm_high
            Opcode::Invalid => 0, // invalid
        }
    }
}
