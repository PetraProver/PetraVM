use std::collections::HashMap;

use binius_field::{BinaryField16b, BinaryField32b, ExtensionField, Field, PackedField};
use thiserror::Error;

use super::instruction_args::{Immediate, Slot, SlotWithOffset};
use crate::memory::ProgramRom;
use crate::{execution::InterpreterInstruction, execution::G, opcodes::Opcode};

/// This is an incomplete list of instructions
/// So far, only the ones added for parsing the fibonacci example has been added
///
/// Ideally we want another pass that removes labels, and replaces label
/// references with the absolute program counter/instruction index we would jump
/// to.
#[derive(Debug)]
pub enum InstructionsWithLabels {
    Label(String, Option<u16>),
    B32Mul {
        dst: Slot,
        src1: Slot,
        src2: Slot,
    },
    B128Add {
        dst: Slot,
        src1: Slot,
        src2: Slot,
    },
    B128Mul {
        dst: Slot,
        src1: Slot,
        src2: Slot,
    },
    Mvih {
        dst: SlotWithOffset,
        imm: Immediate,
    },
    Mvvw {
        dst: SlotWithOffset,
        src: Slot,
    },
    Mvvl {
        dst: SlotWithOffset,
        src: Slot,
    },
    Taili {
        label: String,
        next_fp: Slot,
    },
    Tailv {
        offset: Slot,
        next_fp: Slot,
    },
    Calli {
        label: String,
        next_fp: Slot,
    },
    Callv {
        offset: Slot,
        next_fp: Slot,
    },
    Jumpi {
        label: String,
    },
    Jumpv {
        offset: Slot,
    },
    Ldi {
        dst: Slot,
        imm: Immediate,
    },
    Xor {
        dst: Slot,
        src1: Slot,
        src2: Slot,
    },
    Xori {
        dst: Slot,
        src: Slot,
        imm: Immediate,
    },
    Bnz {
        label: String,
        src: Slot,
    },
    Add {
        dst: Slot,
        src1: Slot,
        src2: Slot,
    },
    Addi {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
    },
    Or {
        dst: Slot,
        src1: Slot,
        src2: Slot,
    },
    Ori {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
    },
    Sub {
        dst: Slot,
        src1: Slot,
        src2: Slot,
    },
    Slt {
        dst: Slot,
        src1: Slot,
        src2: Slot,
    },
    Slti {
        dst: Slot,
        src: Slot,
        imm: Immediate,
    },
    Sltu {
        dst: Slot,
        src1: Slot,
        src2: Slot,
    },
    Sltiu {
        dst: Slot,
        src: Slot,
        imm: Immediate,
    },
    Sll {
        dst: Slot,
        src1: Slot,
        src2: Slot,
    },
    Srl {
        dst: Slot,
        src1: Slot,
        src2: Slot,
    },
    Sra {
        dst: Slot,
        src1: Slot,
        src2: Slot,
    },
    Andi {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
    },
    And {
        dst: Slot,
        src1: Slot,
        src2: Slot,
    },
    Muli {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
    },
    Mul {
        dst: Slot,
        src1: Slot,
        src2: Slot,
    },
    Mulu {
        dst: Slot,
        src1: Slot,
        src2: Slot,
    },
    Mulsu {
        dst: Slot,
        src1: Slot,
        src2: Slot,
    },
    Srli {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
    },
    Slli {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
    },
    Srai {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
    },
    Ret,
}

impl std::fmt::Display for InstructionsWithLabels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstructionsWithLabels::Label(label, frame_size) => {
                if let Some(size) = frame_size {
                    write!(f, "#[framesize(0x{:x})]\n{}:", size, label)
                } else {
                    write!(f, "{}:", label)
                }
            }
            InstructionsWithLabels::B32Mul { dst, src1, src2 } => {
                write!(f, "B32_MUL {dst} {src1} {src2}")
            }
            InstructionsWithLabels::B128Add { dst, src1, src2 } => {
                write!(f, "B128_ADD {dst} {src1} {src2}")
            }
            InstructionsWithLabels::B128Mul { dst, src1, src2 } => {
                write!(f, "B128_MUL {dst} {src1} {src2}")
            }
            InstructionsWithLabels::Mvih { dst, imm } => write!(f, "MVI.H {dst} {imm}"),
            InstructionsWithLabels::Mvvw { dst, src } => write!(f, "MVV.W {dst} {src}"),
            InstructionsWithLabels::Mvvl { dst, src } => write!(f, "MVV.L {dst} {src}"),
            InstructionsWithLabels::Taili { label, next_fp } => {
                write!(f, "TAILI {label} {next_fp}")
            }
            InstructionsWithLabels::Tailv { offset, next_fp } => {
                write!(f, "TAILV {offset} {next_fp}")
            }
            InstructionsWithLabels::Calli { label, next_fp } => {
                write!(f, "CALLI {label} {next_fp}")
            }
            InstructionsWithLabels::Callv { offset, next_fp } => {
                write!(f, "CALLV {offset} {next_fp}")
            }
            InstructionsWithLabels::Jumpi { label } => write!(f, "JUMPI {label}"),
            InstructionsWithLabels::Jumpv { offset } => write!(f, "JUMPV {offset}"),
            InstructionsWithLabels::Ldi { dst, imm } => write!(f, "LDI {dst} {imm}"),
            InstructionsWithLabels::Xor { dst, src1, src2 } => write!(f, "XOR {dst} {src1} {src2}"),
            InstructionsWithLabels::Xori { dst, src, imm } => write!(f, "XORI {dst} {src} {imm}"),
            InstructionsWithLabels::Bnz { label, src } => write!(f, "BNZ {label} {src}"),
            InstructionsWithLabels::Add { dst, src1, src2 } => write!(f, "ADD {dst} {src1} {src2}"),
            InstructionsWithLabels::Addi { dst, src1, imm } => write!(f, "ADDI {dst} {src1} {imm}"),
            InstructionsWithLabels::Or { dst, src1, src2 } => write!(f, "OR {dst} {src1} {src2}"),
            InstructionsWithLabels::Ori { dst, src1, imm } => {
                write!(f, "ORI {dst} {src1} {imm}")
            }
            InstructionsWithLabels::Sub { dst, src1, src2 } => write!(f, "SUB {dst} {src1} {src2}"),
            InstructionsWithLabels::Slt { dst, src1, src2 } => {
                write!(f, "SLT {dst} {src1} {src2}")
            }
            InstructionsWithLabels::Slti { dst, src, imm } => {
                write!(f, "SLTI {dst} {src} {imm}")
            }
            InstructionsWithLabels::Sltu { dst, src1, src2 } => {
                write!(f, "SLTU {dst} {src1} {src2}")
            }
            InstructionsWithLabels::Sltiu { dst, src, imm } => {
                write!(f, "SLTIU {dst} {src} {imm}")
            }
            InstructionsWithLabels::Sll { dst, src1, src2 } => {
                write!(f, "SLL {dst} {src1} {src2}")
            }
            InstructionsWithLabels::Srl { dst, src1, src2 } => {
                write!(f, "SRL {dst} {src1} {src2}")
            }
            InstructionsWithLabels::Sra { dst, src1, src2 } => {
                write!(f, "SRA {dst} {src1} {src2}")
            }
            InstructionsWithLabels::Andi { dst, src1, imm } => write!(f, "ANDI {dst} {src1} {imm}"),
            InstructionsWithLabels::And { dst, src1, src2 } => {
                write!(f, "AND {dst} {src1} {src2}")
            }
            InstructionsWithLabels::Muli { dst, src1, imm } => write!(f, "MULI {dst} {src1} {imm}"),
            InstructionsWithLabels::Mul { dst, src1, src2 } => write!(f, "MUL {dst} {src1} {src2}"),
            InstructionsWithLabels::Mulu { dst, src1, src2 } => {
                write!(f, "MULU {dst} {src1} {src2}")
            }
            InstructionsWithLabels::Mulsu { dst, src1, src2 } => {
                write!(f, "MULSU {dst} {src1} {src2}")
            }
            InstructionsWithLabels::Srli { dst, src1, imm } => write!(f, "SRLI {dst} {src1} {imm}"),
            InstructionsWithLabels::Slli { dst, src1, imm } => write!(f, "SLLI {dst} {src1} {imm}"),
            InstructionsWithLabels::Srai { dst, src1, imm } => write!(f, "SRAI {dst} {src1} {imm}"),
            InstructionsWithLabels::Ret => write!(f, "RET"),
        }
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Unknown instruction: {0}")]
    UnknownInstruction(String),

    #[error(
        "Wrong number of arguments on line {line_number} for instruction: {instruction} {args:?}"
    )]
    WrongNumberOfArguments {
        line_number: usize,
        instruction: String,
        args: Vec<String>,
    },

    #[error("Bad argument: {0}")]
    BadArgument(#[from] super::instruction_args::BadArgumentError),

    #[error("You must have at least one label and one instruction")]
    NoStartLabelOrInstructionFound,
}
