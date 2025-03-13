use std::collections::HashMap;

use binius_field::{BinaryField16b, BinaryField32b, ExtensionField, Field, PackedField};
use thiserror::Error;

use super::instruction_args::{Immediate, Slot, SlotWithOffset};
use crate::{
    execution::{InterpreterInstruction, ProgramRom},
    opcodes::Opcode,
    G,
};

/// Represents the kind of instruction without the call hint property
#[derive(Debug, Clone)]
pub enum InstructionKind {
    Label(String),
    FrameSize(String, u16), // Function name, frame size
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
    AndI {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
    },
    MulI {
        dst: Slot,
        src1: Slot,
        imm: Immediate,
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
    Ret,
}

/// A wrapper that holds an instruction along with its call hint property
#[derive(Debug, Clone)]
pub struct InstructionsWithLabels {
    pub kind: InstructionKind,
    pub is_call_hint: bool,
}

impl InstructionsWithLabels {
    /// Create a new instruction with the specified kind and call hint flag
    pub fn new(kind: InstructionKind, is_call_hint: bool) -> Self {
        Self { kind, is_call_hint }
    }

    /// Create a new regular instruction (not a call hint)
    pub fn regular(kind: InstructionKind) -> Self {
        Self::new(kind, false)
    }

    /// Create a new call hint instruction
    pub fn call_hint(kind: InstructionKind) -> Self {
        Self::new(kind, true)
    }

    /// Check if this instruction is a label
    pub fn is_label(&self) -> bool {
        matches!(self.kind, InstructionKind::Label(_))
    }

    /// Get the label name if this is a label instruction
    pub fn label_name(&self) -> Option<&String> {
        if let InstructionKind::Label(name) = &self.kind {
            Some(name)
        } else {
            None
        }
    }

    /// Check if this instruction is a frame size directive
    pub fn is_framesize(&self) -> bool {
        matches!(self.kind, InstructionKind::FrameSize(_, _))
    }

    /// Get the framesize info if this is a framesize directive
    pub fn framesize_info(&self) -> Option<(&String, u16)> {
        if let InstructionKind::FrameSize(name, size) = &self.kind {
            Some((name, *size))
        } else {
            None
        }
    }
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
) -> Result<(), String> {
    match &instruction.kind {
        InstructionKind::Label(s) => {
            if labels.get(s).is_none() {
                return Err(format!("Label {} not found in the HashMap of labels.", s));
            }
        }
        InstructionKind::FrameSize(_, _) => {
            // No operation needed for FrameSize, it's processed separately
        }
        InstructionKind::AddI { dst, src1, imm } => {
            let inst = [
                Opcode::Addi.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(
                inst,
                *field_pc,
                instruction.is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionKind::Add { dst, src1, src2 } => {
            let inst = [
                Opcode::Add.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(
                inst,
                *field_pc,
                instruction.is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionKind::AndI { dst, src1, imm } => {
            let inst = [
                Opcode::Andi.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                imm.get_field_val(),
            ];

            prom.push(InterpreterInstruction::new(
                inst,
                *field_pc,
                instruction.is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionKind::B32Muli { dst, src1, imm } => {
            let inst = [
                Opcode::B32Muli.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(
                inst,
                *field_pc,
                instruction.is_call_hint,
            ));

            *field_pc *= G;

            let inst = [
                Opcode::B32Muli.get_field_elt(),
                imm.get_high_field_val(),
                BinaryField16b::zero(),
                BinaryField16b::zero(),
            ];
            prom.push(InterpreterInstruction::new(
                inst,
                *field_pc,
                instruction.is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionKind::Bnz { label, src } => {
            if let Some(target) = labels.get(label) {
                let targets_16b =
                    ExtensionField::<BinaryField16b>::iter_bases(target).collect::<Vec<_>>();
                let inst = [
                    Opcode::Bnz.get_field_elt(),
                    src.get_16bfield_val(),
                    targets_16b[0],
                    targets_16b[1],
                ];

                prom.push(InterpreterInstruction::new(
                    inst,
                    *field_pc,
                    instruction.is_call_hint,
                ));
            } else {
                return Err(format!("Label in BNZ instruction, {}, nonexistent.", label));
            }
            *field_pc *= G;
        }
        InstructionKind::MulI { dst, src1, imm } => {
            let inst = [
                Opcode::Muli.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(
                inst,
                *field_pc,
                instruction.is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionKind::MvvW { dst, src } => {
            let inst = [
                Opcode::MVVW.get_field_elt(),
                dst.get_slot_16bfield_val(),
                dst.get_offset_field_val(),
                src.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(
                inst,
                *field_pc,
                instruction.is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionKind::SllI { dst, src1, imm } => {
            let inst = [
                Opcode::Slli.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(
                inst,
                *field_pc,
                instruction.is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionKind::SrlI { dst, src1, imm } => {
            let inst = [
                Opcode::Srli.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(
                inst,
                *field_pc,
                instruction.is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionKind::Ret => {
            let inst = [
                Opcode::Ret.get_field_elt(),
                BinaryField16b::zero(),
                BinaryField16b::zero(),
                BinaryField16b::zero(),
            ];
            prom.push(InterpreterInstruction::new(
                inst,
                *field_pc,
                instruction.is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionKind::Taili { label, arg } => {
            if let Some(target) = labels.get(label) {
                let targets_16b =
                    ExtensionField::<BinaryField16b>::iter_bases(target).collect::<Vec<_>>();
                let inst = [
                    Opcode::Taili.get_field_elt(),
                    targets_16b[0],
                    targets_16b[1],
                    arg.get_16bfield_val(),
                ];

                prom.push(InterpreterInstruction::new(
                    inst,
                    *field_pc,
                    instruction.is_call_hint,
                ));
            } else {
                return Err(format!(
                    "Label in Taili instruction, {}, nonexistent.",
                    label
                ));
            }

            *field_pc *= G;
        }
        InstructionKind::XorI { dst, src, imm } => {
            let inst = [
                Opcode::Xori.get_field_elt(),
                dst.get_16bfield_val(),
                src.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(
                inst,
                *field_pc,
                instruction.is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionKind::Xor { dst, src1, src2 } => {
            let inst = [
                Opcode::Xor.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(
                inst,
                *field_pc,
                instruction.is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionKind::MviH { dst, imm } => {
            let inst = [
                Opcode::MVIH.get_field_elt(),
                dst.get_slot_16bfield_val(),
                dst.get_offset_field_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(
                inst,
                *field_pc,
                instruction.is_call_hint,
            ));

            *field_pc *= G;
        }
        InstructionKind::Ldi { dst, imm } => {
            let inst = [
                Opcode::LDI.get_field_elt(),
                dst.get_16bfield_val(),
                imm.get_field_val(),
                imm.get_high_field_val(),
            ];
            prom.push(InterpreterInstruction::new(
                inst,
                *field_pc,
                instruction.is_call_hint,
            ));

            *field_pc *= G;
        }
    }
    Ok(())
}

// Labels hold the labels in the code, with their associated integer and binary
// field PCs.
type Labels = HashMap<String, BinaryField32b>;
// Binary field PC as the key. Values are: (Frame size, size of args
// and return values).
pub(crate) type LabelsFrameSizes = HashMap<BinaryField32b, u16>;
// Gives the field PC associated to an integer PC. Only conatins the PCs that
// can be called by the PROM.
pub(crate) type PCFieldToInt = HashMap<BinaryField32b, u32>;

fn get_labels(instructions: &[InstructionsWithLabels]) -> Result<(Labels, PCFieldToInt), String> {
    let mut labels = HashMap::new();
    let mut pc_field_to_int = HashMap::new();
    let mut field_pc = BinaryField32b::ONE;
    let mut pc = 1;
    for instruction in instructions {
        match &instruction.kind {
            InstructionKind::Label(s) => {
                if labels.insert(s.clone(), field_pc).is_some()
                    || pc_field_to_int.insert(field_pc, pc).is_some()
                {
                    return Err(format!("Label {} already exists.", s));
                }
                // We do not increment the PC if we found a label.
            }
            InstructionKind::FrameSize(_, _) => {
                // Skip FrameSize entries when calculating PC addresses
            }
            _ => {
                field_pc *= G;
                pc = incr_pc(pc);
            }
        }
    }
    Ok((labels, pc_field_to_int))
}

pub(crate) fn get_full_prom_and_labels(
    instructions: &[InstructionsWithLabels],
) -> Result<(ProgramRom, Labels, PCFieldToInt, LabelsFrameSizes), String> {
    // First, get the labels and their PC values
    let (labels, pc_field_to_int) = get_labels(instructions)?;

    // Now we can build the frame size map indexed by binary field PC
    let mut label_framesizes = HashMap::new();

    // Extract frame sizes from the instructions
    for instruction in instructions {
        if let Some((func_name, size)) = instruction.framesize_info() {
            // Look up the PC for this function label
            if let Some(&pc) = labels.get(func_name) {
                label_framesizes.insert(pc, size);
            }
        }
    }

    // Build the program ROM
    let mut prom = ProgramRom::new();
    let mut field_pc = BinaryField32b::ONE;

    for instruction in instructions {
        // Skip FrameSize directives when building the ROM
        if !instruction.is_framesize() {
            get_prom_inst_from_inst_with_label(&mut prom, &labels, &mut field_pc, instruction)?;
        }
    }

    Ok((prom, labels, pc_field_to_int, label_framesizes))
}

impl std::fmt::Display for InstructionsWithLabels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Add call hint indicator if applicable
        let call_hint_str = if self.is_call_hint {
            "#[callhint] "
        } else {
            ""
        };

        match &self.kind {
            InstructionKind::Label(label) => write!(f, "{}:", label),
            InstructionKind::FrameSize(label, size) => {
                write!(f, "#[framesize({})] {}:", size, label)
            }
            InstructionKind::B32Muli { dst, src1, imm } => {
                write!(f, "{}B32_MULI {} {} {}", call_hint_str, dst, src1, imm)
            }
            InstructionKind::MviH { dst, imm } => {
                write!(f, "{}MVI.H {} {}", call_hint_str, dst, imm)
            }
            InstructionKind::MvvW { dst, src } => {
                write!(f, "{}MVV.W {} {}", call_hint_str, dst, src)
            }
            InstructionKind::Taili { label, arg } => {
                write!(f, "{}TAILI {} {}", call_hint_str, label, arg)
            }
            InstructionKind::Ldi { dst, imm } => write!(f, "{}LDI {} {}", call_hint_str, dst, imm),
            InstructionKind::Xor { dst, src1, src2 } => {
                write!(f, "{}XOR {} {} {}", call_hint_str, dst, src1, src2)
            }
            InstructionKind::XorI { dst, src, imm } => {
                write!(f, "{}XORI {} {} {}", call_hint_str, dst, src, imm)
            }
            InstructionKind::Bnz { label, src } => {
                write!(f, "{}BNZ {} {}", call_hint_str, label, src)
            }
            InstructionKind::Add { dst, src1, src2 } => {
                write!(f, "{}ADD {} {} {}", call_hint_str, dst, src1, src2)
            }
            InstructionKind::AddI { dst, src1, imm } => {
                write!(f, "{}ADDI {} {} {}", call_hint_str, dst, src1, imm)
            }
            InstructionKind::AndI { dst, src1, imm } => {
                write!(f, "{}ANDI {} {} {}", call_hint_str, dst, src1, imm)
            }
            InstructionKind::MulI { dst, src1, imm } => {
                write!(f, "{}MULI {} {} {}", call_hint_str, dst, src1, imm)
            }
            InstructionKind::SrlI { dst, src1, imm } => {
                write!(f, "{}SRLI {} {} {}", call_hint_str, dst, src1, imm)
            }
            InstructionKind::SllI { dst, src1, imm } => {
                write!(f, "{}SLLI {} {} {}", call_hint_str, dst, src1, imm)
            }
            InstructionKind::Ret => write!(f, "{}RET", call_hint_str),
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
