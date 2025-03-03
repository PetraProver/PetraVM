use std::{cmp::max, collections::HashMap, str::FromStr};

use binius_field::{BinaryField16b, PackedField};
use thiserror::Error;

use crate::{
    emulator::{Instruction, Opcode},
    instruction_args::{Immediate, Slot, SlotWithOffset},
};

/// This is an incomplete list of instructions
/// So far, only the ones added for parsing the fibonacci example has been added
///
/// Ideally we want another pass that removes labels, and replaces label references with
/// the absolute program counter/instruction index we would jump to.
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

pub fn get_prom_inst_from_inst_with_label(
    prom: &mut Vec<Instruction>,
    labels: &Labels,
    instruction: &InstructionsWithLabels,
) -> Result<(), String> {
    match instruction {
        InstructionsWithLabels::Label(s) => {
            if labels.get(s).is_none() {
                return Err(format!("Label {} not found in the HashMap of labels.", s));
            }
        }
        InstructionsWithLabels::AddI { dst, src1, imm } => prom.push([
            Opcode::Addi.get_field_elt(),
            dst.get_16bfield_val(),
            src1.get_16bfield_val(),
            imm.get_field_val(),
        ]),
        InstructionsWithLabels::AndI { dst, src1, imm } => prom.push([
            Opcode::Andi.get_field_elt(),
            dst.get_16bfield_val(),
            src1.get_16bfield_val(),
            imm.get_field_val(),
        ]),
        // To change
        InstructionsWithLabels::B32Muli { dst, src1, imm } => prom.push([
            Opcode::Muli.get_field_elt(),
            dst.get_16bfield_val(),
            src1.get_16bfield_val(),
            imm.get_field_val(),
        ]),
        InstructionsWithLabels::Bnz { label, src } => {
            if let Some(target) = labels.get(label) {
                let target_high = BinaryField16b::new((target >> 16) as u16);
                let target_low = BinaryField16b::new(*target as u16);
                prom.push([
                    Opcode::Bnz.get_field_elt(),
                    src.get_16bfield_val(),
                    target_high,
                    target_low,
                ]);
            } else {
                return Err(format!("Label in BNZ instruction, {}, nonexistent.", label));
            }
        }
        InstructionsWithLabels::MulI { dst, src1, imm } => prom.push([
            Opcode::Muli.get_field_elt(),
            dst.get_16bfield_val(),
            src1.get_16bfield_val(),
            imm.get_field_val(),
        ]),
        InstructionsWithLabels::MvvW { dst, src } => prom.push([
            Opcode::MVVW.get_field_elt(),
            dst.get_slot_16bfield_val(),
            dst.get_offset_field_val(),
            src.get_16bfield_val(),
        ]),
        InstructionsWithLabels::SllI { dst, src1, imm } => prom.push([
            Opcode::Slli.get_field_elt(),
            dst.get_16bfield_val(),
            src1.get_16bfield_val(),
            imm.get_field_val(),
        ]),
        InstructionsWithLabels::SrlI { dst, src1, imm } => prom.push([
            Opcode::Srli.get_field_elt(),
            dst.get_16bfield_val(),
            src1.get_16bfield_val(),
            imm.get_field_val(),
        ]),
        InstructionsWithLabels::Ret => prom.push([
            Opcode::Ret.get_field_elt(),
            BinaryField16b::zero(),
            BinaryField16b::zero(),
            BinaryField16b::zero(),
        ]),
        InstructionsWithLabels::Taili { label, arg } => {
            if let Some(target) = labels.get(label) {
                let target_high = BinaryField16b::new((target >> 16) as u16);
                let target_low = BinaryField16b::new(*target as u16);
                prom.push([
                    Opcode::Taili.get_field_elt(),
                    target_high,
                    target_low,
                    arg.get_16bfield_val(),
                ]);
            } else {
                return Err(format!(
                    "Label in Taili instruction, {}, nonexistent.",
                    label
                ));
            }
        }
        InstructionsWithLabels::XorI { dst, src, imm } => prom.push([
            Opcode::Xori.get_field_elt(),
            dst.get_16bfield_val(),
            src.get_16bfield_val(),
            imm.get_field_val(),
        ]),
        _ => unimplemented!(),
    }
    Ok(())
}

type Labels = HashMap<String, u32>;
type LabelsFrameSizes = HashMap<u32, u16>;

pub fn get_frame_size_for_label(
    prom: &[Instruction],
    label_pc: u32,
    labels_fps: &mut LabelsFrameSizes,
) -> u16 {
    if let Some(frame_size) = labels_fps.get(&label_pc) {
        return *frame_size;
    }

    let mut cur_pc = label_pc;
    let mut instruction = prom[cur_pc as usize];
    let mut cur_offset = 0;
    let mut opcode =
        Opcode::try_from(instruction[0].val()).expect("PROM should be correct at this point");
    while opcode != Opcode::Taili && opcode != Opcode::Ret {
        match opcode {
            Opcode::Bnz => {
                let [_, src, target_high, target_low] = instruction;
                println!(
                    "target high {:?} low {:?}",
                    target_high.val(),
                    target_low.val() as u32
                );
                let target = ((target_high.val() as u32) << 16) + target_low.val() as u32;
                println!("target {:?}", target);
                let sub_offset = get_frame_size_for_label(prom, target, labels_fps);
                let max_accessed_addr = max(sub_offset, src.val());
                cur_offset = max(cur_offset, max_accessed_addr);
            }
            Opcode::Addi
            | Opcode::Andi
            | Opcode::Muli
            | Opcode::Slli
            | Opcode::Srli
            | Opcode::Xori => {
                let [_, dst, src, _] = instruction;
                let max_accessed_addr = max(dst, src);
                cur_offset = max(max_accessed_addr.val(), cur_offset);
            }
            Opcode::MVVW => {
                let [_, dst, _, src] = instruction;
                let max_accessed_addr = max(dst, src);
                cur_offset = max(max_accessed_addr.val(), cur_offset);
            }
            _ => panic!(), // incaccessible: either Ret or Taili
        }

        cur_pc += 1;
        instruction = prom[cur_pc as usize];
        opcode =
            Opcode::try_from(instruction[0].val()).expect("PROM should be correct at this point");
    }

    // We know that there was no key `label_pc` before, since it was the first thing we checked in this method.
    labels_fps.insert(label_pc, cur_offset);

    cur_offset
}

pub fn get_frame_sizes_all_labels(prom: &[Instruction], labels: Labels) -> LabelsFrameSizes {
    let mut labels_frame_sizes = HashMap::new();

    for (_, pc) in labels {
        let _ = get_frame_size_for_label(prom, pc, &mut labels_frame_sizes);
    }
    labels_frame_sizes
}

fn get_labels(instructions: &[InstructionsWithLabels]) -> Result<Labels, String> {
    let mut labels = HashMap::new();
    let mut pc = 1;
    for instruction in instructions {
        match instruction {
            InstructionsWithLabels::Label(s) => {
                if let Some(_) = labels.insert(s.clone(), pc) {
                    return Err(format!("Label {} already exists.", s));
                }
                // We do not increment the PC if we found a label.
            }
            _ => pc += 1,
        }
    }
    Ok(labels)
}

pub(crate) fn get_full_prom_and_labels(
    instructions: &[InstructionsWithLabels],
) -> Result<(Vec<Instruction>, Labels), String> {
    let labels = get_labels(instructions)?;
    let mut prom = vec![];
    for instruction in instructions {
        get_prom_inst_from_inst_with_label(&mut prom, &labels, instruction)?;
    }
    Ok((prom, labels))
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

pub fn parse_instructions(input: &str) -> Result<Vec<InstructionsWithLabels>, Error> {
    input
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let line = line
                .split_once(";;")
                .map(|(before_comment, _)| before_comment)
                .unwrap_or(line)
                .trim();
            if line.is_empty() {
                return None;
            }
            Some((i + 1, line))
        })
        .map(|(line_number, line)| parse_instruction(line, line_number))
        .collect::<Result<Vec<_>, _>>()
}

pub fn parse_instruction(line: &str, line_number: usize) -> Result<InstructionsWithLabels, Error> {
    let (instruction, args) = line.split_once(' ').unwrap_or((line, ""));
    if args.is_empty() && instruction.ends_with(':') {
        return Ok(InstructionsWithLabels::Label(
            instruction.strip_suffix(':').unwrap().to_string(),
        ));
    }

    match instruction {
        "B32_MULI" => {
            let [dst, src1, imm] = get_args(instruction, args, line_number)?;
            Ok(InstructionsWithLabels::B32Muli {
                dst: FromStr::from_str(&dst)?,
                src1: FromStr::from_str(&src1)?,
                imm: FromStr::from_str(&imm)?,
            })
        }
        "MVI.H" => {
            let [dst, imm] = get_args(instruction, args, line_number)?;
            Ok(InstructionsWithLabels::MviH {
                dst: FromStr::from_str(&dst)?,
                imm: FromStr::from_str(&imm)?,
            })
        }
        "MVV.W" => {
            let [dst, src] = get_args(instruction, args, line_number)?;
            Ok(InstructionsWithLabels::MvvW {
                dst: FromStr::from_str(&dst)?,
                src: FromStr::from_str(&src)?,
            })
        }
        "TAILI" => {
            let [label, arg] = get_args(instruction, args, line_number)?;
            Ok(InstructionsWithLabels::Taili {
                label: label.to_string(),
                arg: FromStr::from_str(&arg)?,
            })
        }
        "LDI" => {
            let [dst, imm] = get_args(instruction, args, line_number)?;
            Ok(InstructionsWithLabels::Ldi {
                dst: FromStr::from_str(&dst)?,
                imm: FromStr::from_str(&imm)?,
            })
        }
        "XOR" => {
            let [dst, src1, src2] = get_args(instruction, args, line_number)?;
            Ok(InstructionsWithLabels::Xor {
                dst: FromStr::from_str(&dst)?,
                src1: FromStr::from_str(&src1)?,
                src2: FromStr::from_str(&src2)?,
            })
        }
        "XORI" => {
            let [dst, src, imm] = get_args(instruction, args, line_number)?;
            Ok(InstructionsWithLabels::XorI {
                dst: FromStr::from_str(&dst)?,
                src: FromStr::from_str(&src)?,
                imm: FromStr::from_str(&imm)?,
            })
        }
        "BNZ" => {
            let [label, src] = get_args(instruction, args, line_number)?;
            Ok(InstructionsWithLabels::Bnz {
                label: label.to_string(),
                src: FromStr::from_str(&src)?,
            })
        }
        "ADD" => {
            let [dst, src1, src2] = get_args(instruction, args, line_number)?;
            Ok(InstructionsWithLabels::Add {
                dst: FromStr::from_str(&dst)?,
                src1: FromStr::from_str(&src1)?,
                src2: FromStr::from_str(&src2)?,
            })
        }
        "ADDI" => {
            let [dst, src1, imm] = get_args(instruction, args, line_number)?;
            Ok(InstructionsWithLabels::AddI {
                dst: FromStr::from_str(&dst)?,
                src1: FromStr::from_str(&src1)?,
                imm: FromStr::from_str(&imm)?,
            })
        }
        "ANDI" => {
            let [dst, src1, imm] = get_args(instruction, args, line_number)?;
            Ok(InstructionsWithLabels::AndI {
                dst: FromStr::from_str(&dst)?,
                src1: FromStr::from_str(&src1)?,
                imm: FromStr::from_str(&imm)?,
            })
        }
        "MULI" => {
            let [dst, src1, imm] = get_args(instruction, args, line_number)?;
            Ok(InstructionsWithLabels::MulI {
                dst: FromStr::from_str(&dst)?,
                src1: FromStr::from_str(&src1)?,
                imm: FromStr::from_str(&imm)?,
            })
        }
        "SRLI" => {
            let [dst, src1, imm] = get_args(instruction, args, line_number)?;
            Ok(InstructionsWithLabels::SrlI {
                dst: FromStr::from_str(&dst)?,
                src1: FromStr::from_str(&src1)?,
                imm: FromStr::from_str(&imm)?,
            })
        }
        "SLLI" => {
            let [dst, src1, imm] = get_args(instruction, args, line_number)?;
            Ok(InstructionsWithLabels::SllI {
                dst: FromStr::from_str(&dst)?,
                src1: FromStr::from_str(&src1)?,
                imm: FromStr::from_str(&imm)?,
            })
        }
        "RET" => Ok(InstructionsWithLabels::Ret),
        _ => Err(Error::UnknownInstruction(instruction.to_string())),
    }
}

fn get_args<const N: usize>(
    instruction: &str,
    args: &str,
    line_number: usize,
) -> Result<[String; N], Error> {
    let args = args
        .split(',')
        .map(|s| s.trim().to_string())
        .collect::<Vec<_>>();
    if args.len() != N {
        return Err(Error::WrongNumberOfArguments {
            line_number,
            instruction: instruction.to_string(),
            args,
        });
    }
    Ok(args.try_into().unwrap())
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
}
