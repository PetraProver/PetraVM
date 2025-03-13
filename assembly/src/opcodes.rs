use std::ops::Deref;

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
    Addi = 0x07,
    Add = 0x08,
    Muli = 0x09,
    B32Muli = 0x0a,
    B32Mul = 0x10,
    // B32Add, // TODO
    B128Add = 0x16,
    B128Mul = 0x17,
    // Srai, // TODO
    // Slti, // TODO
    // Sltiu, // TODO
    // Sub, // TODO
    // Slt, // TODO
    // Sltu, // TODO
    And = 0x13,
    Or = 0x14,
    Ori = 0x15,
    // Sll, // TODO
    // Srl, // TODO
    // Sra, // TODO
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
    TailV = 0x12,
    Ret = 0x0b,

    // Branch instructions
    #[default]
    Bnz = 0x01,
    // Memory Access (RAM) instructions
    // LW, // TODO
    // SW, // TODO
    // LB, // TODO, low-priority, see specs
    // LBU, // TODO, low-priority, see specs
    // LH, // TODO, low-priority, see specs
    // LHU, // TODO, low-priority, see specs
    // SB, // TODO, low-priority, see specs
    // SU, // TODO, low-priority, see specs
}

impl Opcode {
    pub const fn get_field_elt(&self) -> BinaryField16b {
        BinaryField16b::new(*self as u16)
    }
}
