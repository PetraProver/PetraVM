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
    B32Muli {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
    },
    MviH {
        dst: SlotWithOffset,
        imm: Immediate,
    },
    MvvW {
        dst: SlotWithOffset,
        src: Slot,
    },
    Taili {
        label: String,
        arg: Slot,
    },
    Calli {
        label: String,
        arg: Slot,
    },
    Tailv {
        offset: Slot,
        arg: Slot,
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
    XorI {
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
    AddI {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
    },
    Sub {
        dst: Slot,
        src1: Slot,
        src2: Slot,
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
    AndI {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
    },
    And {
        dst: Slot,
        src1: Slot,
        src2: Slot,
    },
    MulI {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
    },
    Mul {
        dst: Slot,
        src1: Slot,
        src2: Slot,
    },
    SrlI {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
    },
    SllI {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
    },
    SraI {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
    },
    Ret,
    // Add more instructions as needed
}

const fn incr_pc(pc: u32) -> u32 {
    if pc == u32::MAX {
        // We skip over 0, as it is inaccessible in the multiplicative group.
        return 1;
    }

    pc + 1
}

pub fn get_prom_inst_from_inst_with_label(
    prom: &mut ProgramRom,
    labels: &Labels,
    field_pc: &mut BinaryField32b,
    instruction: &InstructionsWithLabels,
    is_call_hint: bool,
) -> Result<(), String> {
    match instruction {
        InstructionsWithLabels::Label(s, _) => {
            if labels.get(s).is_none() {
                return Err(format!("Label {} not found in the HashMap of labels.", s));
            }
        }
        InstructionsWithLabels::AddI { dst, src1, imm } => {
            let instruction = [
                Opcode::Addi.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionsWithLabels::Add { dst, src1, src2 } => {
            let instruction = [
                Opcode::Add.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionsWithLabels::Sub { dst, src1, src2 } => {
            let instruction = [
                Opcode::Sub.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionsWithLabels::Sltu { dst, src1, src2 } => {
            let instruction = [
                Opcode::Sltu.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionsWithLabels::Sltiu { dst, src, imm } => {
            let instruction = [
                Opcode::Sltiu.get_field_elt(),
                dst.get_16bfield_val(),
                src.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionsWithLabels::AndI { dst, src1, imm } => {
            let instruction = [
                Opcode::Andi.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                imm.get_field_val(),
            ];

            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionsWithLabels::And { dst, src1, src2 } => {
            let instruction = [
                Opcode::And.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];

            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;
        }
        // TODO: To change
        InstructionsWithLabels::B32Muli { dst, src1, imm } => {
            let instruction = [
                Opcode::B32Muli.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;

            let instruction = [
                Opcode::B32Muli.get_field_elt(),
                imm.get_high_field_val(),
                BinaryField16b::zero(),
                BinaryField16b::zero(),
            ];
            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionsWithLabels::Bnz { label, src } => {
            if let Some(target) = labels.get(label) {
                let targets_16b =
                    ExtensionField::<BinaryField16b>::iter_bases(target).collect::<Vec<_>>();
                let instruction = [
                    Opcode::Bnz.get_field_elt(),
                    src.get_16bfield_val(),
                    targets_16b[0],
                    targets_16b[1],
                ];

                prom.push(InterpreterInstruction::new(
                    instruction,
                    *field_pc,
                    is_call_hint,
                ));
            } else {
                return Err(format!("Label in BNZ instruction, {}, nonexistent.", label));
            }
            *field_pc *= G;
        }
        InstructionsWithLabels::Jumpv { offset } => {
            let instruction = [
                Opcode::Jumpv.get_field_elt(),
                offset.get_16bfield_val(),
                BinaryField16b::zero(),
                BinaryField16b::zero(),
            ];

            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionsWithLabels::Jumpi { label } => {
            if let Some(target) = labels.get(label) {
                let targets_16b =
                    ExtensionField::<BinaryField16b>::iter_bases(target).collect::<Vec<_>>();
                let instruction = [
                    Opcode::Jumpi.get_field_elt(),
                    targets_16b[0],
                    targets_16b[1],
                    BinaryField16b::zero(),
                ];

                prom.push(InterpreterInstruction::new(
                    instruction,
                    *field_pc,
                    is_call_hint,
                ));
            } else {
                return Err(format!("Label in BNZ instruction, {}, nonexistent.", label));
            }
            *field_pc *= G;
        }
        InstructionsWithLabels::MulI { dst, src1, imm } => {
            let instruction = [
                Opcode::Muli.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionsWithLabels::Mul { dst, src1, src2 } => {
            let instruction = [
                Opcode::Mul.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionsWithLabels::MvvW { dst, src } => {
            let instruction = [
                Opcode::MVVW.get_field_elt(),
                dst.get_slot_16bfield_val(),
                dst.get_offset_field_val(),
                src.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionsWithLabels::SllI { dst, src1, imm } => {
            let instruction = [
                Opcode::Slli.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionsWithLabels::SrlI { dst, src1, imm } => {
            let instruction = [
                Opcode::Srli.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionsWithLabels::SraI { dst, src1, imm } => {
            let instruction = [
                Opcode::Srai.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionsWithLabels::Ret => {
            let instruction = [
                Opcode::Ret.get_field_elt(),
                BinaryField16b::zero(),
                BinaryField16b::zero(),
                BinaryField16b::zero(),
            ];
            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionsWithLabels::Taili { label, arg } => {
            if let Some(target) = labels.get(label) {
                let targets_16b =
                    ExtensionField::<BinaryField16b>::iter_bases(target).collect::<Vec<_>>();
                let instruction = [
                    Opcode::Taili.get_field_elt(),
                    targets_16b[0],
                    targets_16b[1],
                    arg.get_16bfield_val(),
                ];

                prom.push(InterpreterInstruction::new(
                    instruction,
                    *field_pc,
                    is_call_hint,
                ));
            } else {
                return Err(format!(
                    "Label in Taili instruction, {}, nonexistent.",
                    label
                ));
            }

            *field_pc *= G;
        }
        InstructionsWithLabels::Calli { label, arg } => {
            if let Some(target) = labels.get(label) {
                let targets_16b =
                    ExtensionField::<BinaryField16b>::iter_bases(target).collect::<Vec<_>>();
                let instruction = [
                    Opcode::Calli.get_field_elt(),
                    targets_16b[0],
                    targets_16b[1],
                    arg.get_16bfield_val(),
                ];

                prom.push(InterpreterInstruction::new(
                    instruction,
                    *field_pc,
                    is_call_hint,
                ));
            } else {
                return Err(format!(
                    "Label in Taili instruction, {}, nonexistent.",
                    label
                ));
            }

            *field_pc *= G;
        }
        InstructionsWithLabels::Tailv { offset, arg } => {
            let instruction = [
                Opcode::TailV.get_field_elt(),
                offset.get_16bfield_val(),
                arg.get_16bfield_val(),
                BinaryField16b::zero(),
            ];

            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionsWithLabels::XorI { dst, src, imm } => {
            let instruction = [
                Opcode::Xori.get_field_elt(),
                dst.get_16bfield_val(),
                src.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionsWithLabels::Xor { dst, src1, src2 } => {
            let instruction = [
                Opcode::Xor.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionsWithLabels::MviH { dst, imm } => {
            let instruction = [
                Opcode::MVIH.get_field_elt(),
                dst.get_slot_16bfield_val(),
                dst.get_offset_field_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionsWithLabels::Ldi { dst, imm } => {
            let instruction = [
                Opcode::LDI.get_field_elt(),
                dst.get_16bfield_val(),
                imm.get_field_val(),
                imm.get_high_field_val(),
            ];
            prom.push(InterpreterInstruction::new(
                instruction,
                *field_pc,
                is_call_hint,
            ));

            *field_pc *= G;
        }
    }
    Ok(())
}

// Labels hold the labels in the code, with their associated binary field PCs.
type Labels = HashMap<String, BinaryField32b>;
// Binary field PC as the key. Values are: frame size.
pub type LabelsFrameSizes = HashMap<BinaryField32b, u16>;
// Gives the field PC associated to an integer PC. Only contains the PCs that
// can be called by the PROM.
pub(crate) type PCFieldToInt = HashMap<BinaryField32b, u32>;

fn get_labels(
    instructions: &[InstructionsWithLabels],
) -> Result<(Labels, PCFieldToInt, LabelsFrameSizes), String> {
    let mut labels = HashMap::new();
    let mut pc_field_to_int = HashMap::new();
    let mut frame_sizes = HashMap::new();
    let mut field_pc = BinaryField32b::ONE;
    let mut pc = 1;

    for instruction in instructions {
        match instruction {
            InstructionsWithLabels::Label(s, frame_size) => {
                if labels.insert(s.clone(), field_pc).is_some()
                    || pc_field_to_int.insert(field_pc, pc).is_some()
                {
                    return Err(format!("Label {} already exists.", s));
                }

                // If we have a frame size for this label, add it to our frame_sizes map
                if let Some(size) = frame_size {
                    frame_sizes.insert(field_pc, *size);
                }

                // We do not increment the PC if we found a label.
            }
            _ => {
                field_pc *= G;
                pc = incr_pc(pc);
            }
        }
    }
    Ok((labels, pc_field_to_int, frame_sizes))
}

pub fn get_full_prom_and_labels(
    instructions: &[InstructionsWithLabels],
    is_call_procedure_hints: &[bool],
) -> Result<(ProgramRom, Labels, PCFieldToInt, LabelsFrameSizes), String> {
    let (labels, pc_field_to_int, frame_sizes) = get_labels(instructions)?;
    let mut prom = ProgramRom::new();
    let mut field_pc = BinaryField32b::ONE;

    assert_eq!(
        instructions.len(),
        is_call_procedure_hints.len(),
        "The instructions have length {} but the call procedure hints have length {}",
        instructions.len(),
        is_call_procedure_hints.len()
    );

    for (instruction, &is_call_procedure) in instructions.iter().zip(is_call_procedure_hints) {
        get_prom_inst_from_inst_with_label(
            &mut prom,
            &labels,
            &mut field_pc,
            instruction,
            is_call_procedure,
        )?;
    }

    Ok((prom, labels, pc_field_to_int, frame_sizes))
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
            InstructionsWithLabels::B32Muli { dst, src1, imm } => {
                write!(f, "B32_MULI {dst} {src1} {imm}")
            }
            InstructionsWithLabels::MviH { dst, imm } => write!(f, "MVI.H {dst} {imm}"),
            InstructionsWithLabels::MvvW { dst, src } => write!(f, "MVV.W {dst} {src}"),
            InstructionsWithLabels::Taili { label, arg } => write!(f, "TAILI {label} {arg}"),
            InstructionsWithLabels::Calli { label, arg } => write!(f, "CALLI {label} {arg}"),
            InstructionsWithLabels::Tailv { offset, arg } => write!(f, "TAILV {offset} {arg}"),
            InstructionsWithLabels::Ldi { dst, imm } => write!(f, "LDI {dst} {imm}"),
            InstructionsWithLabels::Xor { dst, src1, src2 } => write!(f, "XOR {dst} {src1} {src2}"),
            InstructionsWithLabels::XorI { dst, src, imm } => write!(f, "XORI {dst} {src} {imm}"),
            InstructionsWithLabels::Bnz { label, src } => write!(f, "BNZ {label} {src}"),
            InstructionsWithLabels::Jumpv { offset } => write!(f, "JUMPV {offset}"),
            InstructionsWithLabels::Jumpi { label } => write!(f, "JUMPI {label}"),
            InstructionsWithLabels::Add { dst, src1, src2 } => write!(f, "ADD {dst} {src1} {src2}"),
            InstructionsWithLabels::And { dst, src1, src2 } => {
                write!(f, "ANDI {dst} {src1} {src2}")
            }
            InstructionsWithLabels::Mul { dst, src1, src2 } => write!(f, "MUL {dst} {src1} {src2}"),
            InstructionsWithLabels::Sub { dst, src1, src2 } => write!(f, "SUB {dst} {src1} {src2}"),
            InstructionsWithLabels::Sltu { dst, src1, src2 } => {
                write!(f, "SLTU {dst} {src1} {src2}")
            }
            InstructionsWithLabels::Sltiu { dst, src, imm } => {
                write!(f, "SLTIU {dst} {src} {imm}")
            }
            InstructionsWithLabels::AddI { dst, src1, imm } => write!(f, "ADDI {dst} {src1} {imm}"),
            InstructionsWithLabels::AndI { dst, src1, imm } => write!(f, "ANDI {dst} {src1} {imm}"),
            InstructionsWithLabels::MulI { dst, src1, imm } => write!(f, "MULI {dst} {src1} {imm}"),
            InstructionsWithLabels::SrlI { dst, src1, imm } => write!(f, "SRLI {dst} {src1} {imm}"),
            InstructionsWithLabels::SllI { dst, src1, imm } => write!(f, "SLLI {dst} {src1} {imm}"),
            InstructionsWithLabels::SraI { dst, src1, imm } => write!(f, "SLLI {dst} {src1} {imm}"),
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
