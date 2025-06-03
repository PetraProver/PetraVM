use thiserror::Error;

use super::instruction_args::{Immediate, Slot, SlotWithOffset};

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
        prover_only: bool,
    },
    B32Muli {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
        prover_only: bool,
    },
    B128Add {
        dst: Slot,
        src1: Slot,
        src2: Slot,
        prover_only: bool,
    },
    B128Mul {
        dst: Slot,
        src1: Slot,
        src2: Slot,
        prover_only: bool,
    },
    Mvih {
        dst: SlotWithOffset,
        imm: Immediate,
        prover_only: bool,
    },
    Mvvw {
        dst: SlotWithOffset,
        src: Slot,
        prover_only: bool,
    },
    Mvvl {
        dst: SlotWithOffset,
        src: Slot,
        prover_only: bool,
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
        prover_only: bool,
    },
    Xor {
        dst: Slot,
        src1: Slot,
        src2: Slot,
        prover_only: bool,
    },
    Xori {
        dst: Slot,
        src: Slot,
        imm: Immediate,
        prover_only: bool,
    },
    Bnz {
        label: String,
        src: Slot,
    },
    Add {
        dst: Slot,
        src1: Slot,
        src2: Slot,
        prover_only: bool,
    },
    Addi {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
        prover_only: bool,
    },
    Or {
        dst: Slot,
        src1: Slot,
        src2: Slot,
        prover_only: bool,
    },
    Ori {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
        prover_only: bool,
    },
    Sub {
        dst: Slot,
        src1: Slot,
        src2: Slot,
        prover_only: bool,
    },
    Sle {
        dst: Slot,
        src1: Slot,
        src2: Slot,
        prover_only: bool,
    },
    Slei {
        dst: Slot,
        src: Slot,
        imm: Immediate,
        prover_only: bool,
    },
    Sleu {
        dst: Slot,
        src1: Slot,
        src2: Slot,
        prover_only: bool,
    },
    Sleiu {
        dst: Slot,
        src: Slot,
        imm: Immediate,
        prover_only: bool,
    },
    Slt {
        dst: Slot,
        src1: Slot,
        src2: Slot,
        prover_only: bool,
    },
    Slti {
        dst: Slot,
        src: Slot,
        imm: Immediate,
        prover_only: bool,
    },
    Sltu {
        dst: Slot,
        src1: Slot,
        src2: Slot,
        prover_only: bool,
    },
    Sltiu {
        dst: Slot,
        src: Slot,
        imm: Immediate,
        prover_only: bool,
    },
    Sll {
        dst: Slot,
        src1: Slot,
        src2: Slot,
        prover_only: bool,
    },
    Srl {
        dst: Slot,
        src1: Slot,
        src2: Slot,
        prover_only: bool,
    },
    Sra {
        dst: Slot,
        src1: Slot,
        src2: Slot,
        prover_only: bool,
    },
    Andi {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
        prover_only: bool,
    },
    And {
        dst: Slot,
        src1: Slot,
        src2: Slot,
        prover_only: bool,
    },
    Muli {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
        prover_only: bool,
    },
    Mul {
        dst: Slot,
        src1: Slot,
        src2: Slot,
        prover_only: bool,
    },
    Mulu {
        dst: Slot,
        src1: Slot,
        src2: Slot,
        prover_only: bool,
    },
    Mulsu {
        dst: Slot,
        src1: Slot,
        src2: Slot,
        prover_only: bool,
    },
    Srli {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
        prover_only: bool,
    },
    Slli {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
        prover_only: bool,
    },
    Srai {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
        prover_only: bool,
    },
    Alloci {
        dst: Slot,
        imm: Immediate,
    },
    Allocv {
        dst: Slot,
        src: Slot,
    },
    Ret,
}

impl InstructionsWithLabels {
    pub(crate) fn prover_only(&self) -> bool {
        use InstructionsWithLabels::*;
        match self {
            B32Mul { prover_only, .. } => *prover_only,
            B32Muli { prover_only, .. } => *prover_only,
            B128Add { prover_only, .. } => *prover_only,
            B128Mul { prover_only, .. } => *prover_only,
            Mvih { prover_only, .. } => *prover_only,
            Mvvw { prover_only, .. } => *prover_only,
            Mvvl { prover_only, .. } => *prover_only,
            Ldi { prover_only, .. } => *prover_only,
            Xor { prover_only, .. } => *prover_only,
            Xori { prover_only, .. } => *prover_only,
            Add { prover_only, .. } => *prover_only,
            Addi { prover_only, .. } => *prover_only,
            Or { prover_only, .. } => *prover_only,
            Ori { prover_only, .. } => *prover_only,
            Sub { prover_only, .. } => *prover_only,
            Sle { prover_only, .. } => *prover_only,
            Slei { prover_only, .. } => *prover_only,
            Sleu { prover_only, .. } => *prover_only,
            Sleiu { prover_only, .. } => *prover_only,
            Slt { prover_only, .. } => *prover_only,
            Slti { prover_only, .. } => *prover_only,
            Sltu { prover_only, .. } => *prover_only,
            Sltiu { prover_only, .. } => *prover_only,
            Sll { prover_only, .. } => *prover_only,
            Srl { prover_only, .. } => *prover_only,
            Sra { prover_only, .. } => *prover_only,
            Andi { prover_only, .. } => *prover_only,
            And { prover_only, .. } => *prover_only,
            Muli { prover_only, .. } => *prover_only,
            Mul { prover_only, .. } => *prover_only,
            Mulu { prover_only, .. } => *prover_only,
            Mulsu { prover_only, .. } => *prover_only,
            Srli { prover_only, .. } => *prover_only,
            Slli { prover_only, .. } => *prover_only,
            Srai { prover_only, .. } => *prover_only,
            Alloci { .. } => true,
            Allocv { .. } => true,
            _ => false,
        }
    }
}

impl std::fmt::Display for InstructionsWithLabels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use InstructionsWithLabels::*;
        let bang = if self.prover_only() { "!" } else { "" };
        match self {
            Label(label, frame_size) => {
                if let Some(size) = frame_size {
                    write!(f, "#[framesize(0x{size:x})]\n{label}:")
                } else {
                    write!(f, "{label}:")
                }
            }
            B32Mul {
                dst, src1, src2, ..
            } => {
                write!(f, "B32_MUL{bang} {dst} {src1} {src2}")
            }
            B32Muli { dst, src1, imm, .. } => {
                write!(f, "B32_MULI{bang} {dst} {src1} {imm}")
            }
            B128Add {
                dst, src1, src2, ..
            } => {
                write!(f, "B128_ADD{bang} {dst} {src1} {src2}")
            }
            B128Mul {
                dst, src1, src2, ..
            } => {
                write!(f, "B128_MUL{bang} {dst} {src1} {src2}")
            }
            Mvih { dst, imm, .. } => {
                write!(f, "MVI.H{bang} {dst} {imm}")
            }
            Mvvw { dst, src, .. } => {
                write!(f, "MVV.W{bang} {dst} {src}")
            }
            Mvvl { dst, src, .. } => {
                write!(f, "MVV.L{bang} {dst} {src}")
            }
            Taili { label, next_fp } => {
                write!(f, "TAILI {label} {next_fp}")
            }
            Tailv { offset, next_fp } => {
                write!(f, "TAILV {offset} {next_fp}")
            }
            Calli { label, next_fp } => {
                write!(f, "CALLI {label} {next_fp}")
            }
            Callv { offset, next_fp } => {
                write!(f, "CALLV {offset} {next_fp}")
            }
            Jumpi { label } => write!(f, "J {label}"),
            Jumpv { offset } => write!(f, "J {offset}"),
            Ldi { dst, imm, .. } => write!(f, "LDI{bang} {dst} {imm}"),
            Xor {
                dst, src1, src2, ..
            } => write!(f, "XOR{bang} {dst} {src1} {src2}"),
            Xori { dst, src, imm, .. } => {
                write!(f, "XORI{bang} {dst} {src} {imm}")
            }
            Bnz { label, src } => write!(f, "BNZ {label} {src}"),
            Add {
                dst, src1, src2, ..
            } => write!(f, "ADD{bang} {dst} {src1} {src2}"),
            Addi { dst, src1, imm, .. } => {
                write!(f, "ADDI{bang} {dst} {src1} {imm}")
            }
            Or {
                dst, src1, src2, ..
            } => write!(f, "OR{bang} {dst} {src1} {src2}"),
            Ori { dst, src1, imm, .. } => {
                write!(f, "ORI{bang} {dst} {src1} {imm}")
            }
            Sub {
                dst, src1, src2, ..
            } => write!(f, "SUB{bang} {dst} {src1} {src2}"),
            Sle {
                dst, src1, src2, ..
            } => {
                write!(f, "SLE{bang} {dst} {src1} {src2}")
            }
            Slei { dst, src, imm, .. } => {
                write!(f, "SLEI{bang} {dst} {src} {imm}")
            }
            Sleu {
                dst, src1, src2, ..
            } => {
                write!(f, "SLEU{bang} {dst} {src1} {src2}")
            }
            Sleiu { dst, src, imm, .. } => {
                write!(f, "SLEIU{bang} {dst} {src} {imm}")
            }
            Slt {
                dst, src1, src2, ..
            } => {
                write!(f, "SLT{bang} {dst} {src1} {src2}")
            }
            Slti { dst, src, imm, .. } => {
                write!(f, "SLTI{bang} {dst} {src} {imm}")
            }
            Sltu {
                dst, src1, src2, ..
            } => {
                write!(f, "SLTU{bang} {dst} {src1} {src2}")
            }
            Sltiu { dst, src, imm, .. } => {
                write!(f, "SLTIU{bang} {dst} {src} {imm}")
            }
            Sll {
                dst, src1, src2, ..
            } => {
                write!(f, "SLL{bang} {dst} {src1} {src2}")
            }
            Srl {
                dst, src1, src2, ..
            } => {
                write!(f, "SRL{bang} {dst} {src1} {src2}")
            }
            Sra {
                dst, src1, src2, ..
            } => {
                write!(f, "SRA{bang} {dst} {src1} {src2}")
            }
            Andi { dst, src1, imm, .. } => {
                write!(f, "ANDI{bang} {dst} {src1} {imm}")
            }
            And {
                dst, src1, src2, ..
            } => {
                write!(f, "AND{bang} {dst} {src1} {src2}")
            }
            Muli { dst, src1, imm, .. } => {
                write!(f, "MULI{bang} {dst} {src1} {imm}")
            }
            Mul {
                dst, src1, src2, ..
            } => write!(f, "MUL{bang} {dst} {src1} {src2}"),
            Mulu {
                dst, src1, src2, ..
            } => {
                write!(f, "MULU{bang} {dst} {src1} {src2}")
            }
            Mulsu {
                dst, src1, src2, ..
            } => {
                write!(f, "MULSU{bang} {dst} {src1} {src2}")
            }
            Srli { dst, src1, imm, .. } => {
                write!(f, "SRLI{bang} {dst} {src1} {imm}")
            }
            Slli { dst, src1, imm, .. } => {
                write!(f, "SLLI{bang} {dst} {src1} {imm}")
            }
            Srai { dst, src1, imm, .. } => {
                write!(f, "SRAI{bang} {dst} {src1} {imm}")
            }
            Ret => write!(f, "RET"),
            Alloci { dst, imm } => {
                write!(f, "ALLOCI! {dst} {imm}")
            }
            Allocv { dst, src } => {
                write!(f, "ALLOCV! {dst} {src}")
            }
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

    #[error(transparent)]
    PestParse(#[from] Box<pest::error::Error<super::Rule>>),
}
