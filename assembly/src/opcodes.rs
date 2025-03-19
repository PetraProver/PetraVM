use binius_field::BinaryField16b;
use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Debug, Clone, Copy, Default, TryFromPrimitive, IntoPrimitive, PartialEq, Eq)]
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
    B32Muli = 0x0a,
    B32Mul = 0x10,
    // B32Add, // TODO
    B128Add = 0x16,
    B128Mul = 0x17,
    // Slti, // TODO
    // Slt, // TODO
    And = 0x13,
    Or = 0x14,
    Ori = 0x15,
    Sub = 0x19,
    Sltu = 0x1a,
    Sltiu = 0x1b,
    Sll = 0x1c,
    Srl = 0x1d,
    Sra = 0x1e,

    // Move instructions
    MVVW = 0x0d,
    MVIH = 0x0e,
    LDI = 0x0f,
    MVVL = 0x11,

    // Jump instructions
    Jumpi = 0x20,
    Jumpv = 0x21,
    // CallV, // TODO,
    Taili = 0x0c,
    Tailv = 0x12,
    Calli = 0x18,
    Ret = 0x0b,

    // Branch instructions
    #[default]
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
}

impl Opcode {
    pub const fn get_field_elt(&self) -> BinaryField16b {
        BinaryField16b::new(*self as u16)
    }
}
