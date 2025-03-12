use std::{cmp::max, collections::HashMap};

use binius_field::{BinaryField16b, BinaryField32b, ExtensionField, Field, PackedField};
use thiserror::Error;

use crate::{
    emulator::{InterpreterInstruction, ProgramRom},
    instruction_args::{Immediate, Slot, SlotWithOffset},
    opcodes::Opcode,
    G,
};

/// This is an incomplete list of instructions
/// So far, only the ones added for parsing the fibonacci example has been added
///
/// Ideally we want another pass that removes labels, and replaces label
/// references with the absolute program counter/instruction index we would jump
/// to.
#[derive(Debug)]
pub enum InstructionsWithLabels {
    Label(String),
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
    // Add more instructions as needed
}

fn incr_pc(pc: u32) -> u32 {
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
        InstructionsWithLabels::Label(s) => {
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

// Labels hold the labels in the code, with their associated integer and binary
// field PCs.
type Labels = HashMap<String, BinaryField32b>;
// Binary field PC as the key. Values are: (Frame size, size of args
// and return values).
pub(crate) type LabelsFrameSizes = HashMap<BinaryField32b, u16>;
// Gives the field PC associated to an integer PC. Only conatins the PCs that
// can be called by the PROM.
pub(crate) type PCFieldToInt = HashMap<BinaryField32b, u32>;

pub fn get_frame_size_for_label(
    prom: &ProgramRom,
    label_pc: u32,
    labels_fps: &mut LabelsFrameSizes,
    pc_field_to_int: &PCFieldToInt,
) -> u16 {
    let mut cur_pc = label_pc;
    let mut interp_instruction = &prom[cur_pc as usize - 1];
    let mut instruction = interp_instruction.instruction;
    let field_pc = interp_instruction.field_pc;
    if let Some(frame_size) = labels_fps.get(&field_pc) {
        return *frame_size;
    }

    let mut cur_offset = 0;
    let mut opcode =
        Opcode::try_from(instruction[0].val()).expect("PROM should be correct at this point");
    while opcode != Opcode::Taili && opcode != Opcode::TailV && opcode != Opcode::Ret {
        match opcode {
            Opcode::Bnz => {
                let [op, src, target_low, target_high] = instruction;
                let target = BinaryField32b::from_bases([target_low, target_high]).unwrap();
                let int_target = pc_field_to_int.get(&target).unwrap_or_else(|| {
                    panic!(
                        "The provided field PC to PC mapping is incomplete. PC {:?} not found.",
                        target
                    )
                });
                let sub_offset =
                    get_frame_size_for_label(prom, *int_target, labels_fps, pc_field_to_int);
                let max_accessed_addr = max(sub_offset, src.val());
                cur_offset = max(cur_offset, max_accessed_addr);
            }
            Opcode::Addi
            | Opcode::Andi
            | Opcode::Muli
            | Opcode::Slli
            | Opcode::Srli
            | Opcode::Ori
            | Opcode::Xori => {
                let [_, dst, src, _] = instruction;
                let max_accessed_addr = max(dst, src);
                cur_offset = max(max_accessed_addr.val(), cur_offset);
            }
            Opcode::B32Muli => {
                let [_, dst, src, _] = instruction;
                let max_accessed_addr = max(dst, src);
                cur_offset = max(max_accessed_addr.val(), cur_offset);
                // B32Muli needs two rows.
                cur_pc = incr_pc(cur_pc);
            }
            Opcode::Add
            | Opcode::And
            | Opcode::Or
            | Opcode::Xor
            | Opcode::B32Mul
            | Opcode::B128Add
            | Opcode::B128Mul => {
                let [_, dst, src1, src2] = instruction;
                let max_accessed_addr = max(dst, src1);
                let max_accessed_addr = max(max_accessed_addr, src2);
                cur_offset = max(max_accessed_addr.val(), cur_offset);
            }
            Opcode::MVVW | Opcode::MVVL => {
                let [_, dst, _, src] = instruction;
                let max_accessed_addr = max(dst, src);
                cur_offset = max(max_accessed_addr.val(), cur_offset);
            }
            Opcode::LDI | Opcode::MVIH => {
                let [_, dst, _, _] = instruction;
                cur_offset = max(dst.val(), cur_offset);
            }
            Opcode::Ret | Opcode::Taili | Opcode::TailV => {
                unreachable!("This is explicitely skipped.")
            }
        }

        cur_pc = incr_pc(cur_pc);
        instruction = prom[cur_pc as usize - 1].instruction;

        opcode =
            Opcode::try_from(instruction[0].val()).expect("PROM should be correct at this point");
    }

    // We know that there was no key `label_pc` before, since it was the first thing
    // we checked in this method.
    let field_pc = prom[label_pc as usize - 1].field_pc;
    labels_fps.insert(field_pc, cur_offset);

    cur_offset
}

pub fn get_frame_sizes_all_labels(
    prom: &ProgramRom,
    labels: Labels,
    pc_field_to_int: &PCFieldToInt,
) -> LabelsFrameSizes {
    let mut labels_frame_sizes = HashMap::new();
    for (_, field_pc) in labels {
        let &pc = pc_field_to_int.get(&field_pc).unwrap();
        let _ = get_frame_size_for_label(prom, pc, &mut labels_frame_sizes, pc_field_to_int);
    }
    labels_frame_sizes
}

fn get_labels(instructions: &[InstructionsWithLabels]) -> Result<(Labels, PCFieldToInt), String> {
    let mut labels = HashMap::new();
    let mut pc_field_to_int = HashMap::new();
    let mut field_pc = BinaryField32b::ONE;
    let mut pc = 1;
    for instruction in instructions {
        match instruction {
            InstructionsWithLabels::Label(s) => {
                if labels.insert(s.clone(), field_pc).is_some()
                    || pc_field_to_int.insert(field_pc, pc).is_some()
                {
                    return Err(format!("Label {} already exists.", s));
                }
                // We do not increment the PC if we found a label.
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
    is_call_procedure_hints: &[bool],
) -> Result<(ProgramRom, Labels, PCFieldToInt), String> {
    let (labels, pc_field_to_int) = get_labels(instructions)?;
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
    Ok((prom, labels, pc_field_to_int))
}

impl std::fmt::Display for InstructionsWithLabels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstructionsWithLabels::Label(label) => write!(f, "{}:", label),
            InstructionsWithLabels::B32Muli { dst, src1, imm } => {
                write!(f, "B32_MULI {dst} {src1} {imm}")
            }
            InstructionsWithLabels::MviH { dst, imm } => write!(f, "MVI.H {dst} {imm}"),
            InstructionsWithLabels::MvvW { dst, src } => write!(f, "MVV.W {dst} {src}"),
            InstructionsWithLabels::Taili { label, arg } => write!(f, "TAILI {label} {arg}"),
            InstructionsWithLabels::Ldi { dst, imm } => write!(f, "LDI {dst} {imm}"),
            InstructionsWithLabels::Xor { dst, src1, src2 } => write!(f, "XOR {dst} {src1} {src2}"),
            InstructionsWithLabels::XorI { dst, src, imm } => write!(f, "XORI {dst} {src} {imm}"),
            InstructionsWithLabels::Bnz { label, src } => write!(f, "BNZ {label} {src}"),
            InstructionsWithLabels::Add { dst, src1, src2 } => write!(f, "ADD {dst} {src1} {src2}"),
            InstructionsWithLabels::AddI { dst, src1, imm } => write!(f, "ADDI {dst} {src1} {imm}"),
            InstructionsWithLabels::AndI { dst, src1, imm } => write!(f, "ANDI {dst} {src1} {imm}"),
            InstructionsWithLabels::MulI { dst, src1, imm } => write!(f, "MULI {dst} {src1} {imm}"),
            InstructionsWithLabels::SrlI { dst, src1, imm } => write!(f, "SRLI {dst} {src1} {imm}"),
            InstructionsWithLabels::SllI { dst, src1, imm } => write!(f, "SLLI {dst} {src1} {imm}"),
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
    BadArgument(#[from] crate::instruction_args::BadArgumentError),

    #[error("You must have at least one label and one instruction")]
    NoStartLabelOrInstructionFound,
}
