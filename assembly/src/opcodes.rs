use binius_field::BinaryField16b;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pest::pratt_parser::Op;
use strum::EnumCount;
use strum_macros::EnumCount;

#[derive(
    Debug, Clone, Copy, Default, EnumCount, TryFromPrimitive, IntoPrimitive, PartialEq, Eq,
)]
#[repr(u16)]
#[allow(clippy::upper_case_acronyms)]
// TODO: Add missing opcodes
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
    pub const fn get_field_elt(&self) -> BinaryField16b {
        BinaryField16b::new(*self as u16)
    }

    /// Returns the number of arguments expected by the given opcode.
    pub fn num_args(&self) -> usize {
        match self {
            Opcode::Bnz => 3,
            Opcode::Jumpi => 3,
            Opcode::Jumpv => 2,
            Opcode::Xori => 3,
            Opcode::Xor => 3,
            Opcode::Ret => 0,
            Opcode::Slli => 3,
            Opcode::Srli => 3,
            Opcode::Srai => 3,
            Opcode::Sll => 3,
            Opcode::Srl => 3,
            Opcode::Sra => 3,
            Opcode::Tailv => 2,
            Opcode::Taili => 3,
            Opcode::Calli => 3,
            Opcode::Callv => 2,
            Opcode::And => 3,
            Opcode::Andi => 3,
            Opcode::Sub => 3,
            Opcode::Slt => 3,
            Opcode::Slti => 3,
            Opcode::Sltu => 3,
            Opcode::Sltiu => 3,
            Opcode::Or => 3,
            Opcode::Ori => 3,
            Opcode::Muli => 3,
            Opcode::Mulu => 3,
            Opcode::Mul => 3,
            Opcode::Mulsu => 3,
            Opcode::B32Mul => 3,
            Opcode::B32Muli => 3,
            Opcode::B128Add => 3,
            Opcode::B128Mul => 3,
            Opcode::Add => 3,
            Opcode::Addi => 3,
            Opcode::Mvvw => 3,
            Opcode::Mvvl => 3,
            Opcode::Mvih => 3,
            Opcode::Ldi => 3,
            Opcode::Invalid => 0, // invalid
        }
    }
}
