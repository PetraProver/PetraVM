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
    Srai = 0x18,
    Addi = 0x07,
    Add = 0x08,
    Muli = 0x09,
    B32Muli = 0x0a,
    B32Mul = 0x10,
    // B32Add, // TODO
    B128Add = 0x16,
    B128Mul = 0x17,
    // Slti, // TODO
    // Sltiu, // TODO
    // Sub, // TODO
    // Slt, // TODO
    // Sltu, // TODO
    And = 0x13,
    Or = 0x14,
    Ori = 0x15,
    Sll = 0x19,
    Srl = 0x1A,
    Sra = 0x1B,
    // Mul, // TODO
    // Mulu, // TODO
    // Mulsu, // TODO

    // Move instructions
    MVVW = 0x0d,
    MVIH = 0x0e,
    LDI = 0x0f,
    MVVL = 0x11,

    // Jump instructions
    // Jumpi, // TODO
    // JumpV, // TODO
    // Calli, // TODO,
    // CallV, // TODO,
    Taili = 0x0c,
    Tailv = 0x12,
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
