use std::str::FromStr;

use pest::{iterators::Pair, iterators::Pairs, Parser};

mod instruction_args;
mod instructions_with_labels;
mod tests;

use instruction_args::{Immediate, Slot, SlotWithOffset};
pub use instructions_with_labels::get_full_prom_and_labels;
pub(crate) use instructions_with_labels::{Error, InstructionsWithLabels, LabelsFrameSizes};

#[derive(pest_derive::Parser)]
#[grammar = "parser/asm.pest"]
struct AsmParser;

#[inline]
fn get_first_inner<'a>(pair: Pair<'a, Rule>, msg: &str) -> Pair<'a, Rule> {
    pair.into_inner().next().expect(msg)
}

// A line may have a frame size annotation, a label and an instruction
fn parse_line(
    instrs: &mut Vec<InstructionsWithLabels>,
    pairs: Pairs<'_, Rule>,
) -> Result<(), Error> {
    let mut current_frame_size: Option<u16> = None;

    for instr_or_label in pairs {
        match instr_or_label.as_rule() {
            Rule::frame_size_annotation => {
                let frame_size_hex =
                    get_first_inner(instr_or_label, "frame_size_annotation must have frame_size");
                let hex_str = frame_size_hex.as_str().trim_start_matches("0x");
                let frame_size = u16::from_str_radix(hex_str, 16).map_err(|_| {
                    Error::BadArgument(instruction_args::BadArgumentError::FrameSize(
                        hex_str.to_string(),
                    ))
                })?;
                current_frame_size = Some(frame_size);
            }
            Rule::label => {
                let label_name = get_first_inner(instr_or_label, "label must have label_name");
                instrs.push(InstructionsWithLabels::Label(
                    label_name.as_span().as_str().to_string(),
                    current_frame_size, // Include the frame size with the label
                ));
                current_frame_size = None; // Reset after using it
            }
            Rule::instruction => {
                let instruction = get_first_inner(instr_or_label, "Instruction has inner tokens");
                match instruction.as_rule() {
                    Rule::mov_imm => {
                        let mut mov_imm = instruction.into_inner();
                        // Since we know this has to be MVI_H instruction
                        let rule = get_first_inner(
                            mov_imm.next().expect("This instruction is MVI_H"),
                            "MVI_H has instruction",
                        )
                        .as_rule();
                        let dest = mov_imm.next().expect("MVI_H has dest");
                        let imm = mov_imm.next().expect("MVI_H has imm");
                        let dst = SlotWithOffset::from_str(dest.as_str())?;
                        let imm = Immediate::from_str(imm.as_str())?;
                        match rule {
                            Rule::MVI_H_instr => {
                                instrs.push(InstructionsWithLabels::MviH { dst, imm });
                            }
                            _ => {
                                unreachable!("We have implemented all mov_imm instructions");
                            }
                        }
                    }
                    Rule::binary_imm => {
                        let mut binary_imm = instruction.into_inner();
                        let rule = get_first_inner(
                            binary_imm.next().unwrap(),
                            "binary_imm has instruction",
                        )
                        .as_rule();
                        let dst = binary_imm.next().expect("binary_imm has dest");
                        let src1 = binary_imm.next().expect("binary_imm has src1");
                        let imm = Immediate::from_str(
                            binary_imm.next().expect("binary_imm has imm").as_str(),
                        )?;
                        match rule {
                            Rule::B32_MULI_instr => {
                                instrs.push(InstructionsWithLabels::B32Muli {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src1: Slot::from_str(src1.as_str())?,
                                    imm,
                                });
                            }
                            Rule::XORI_instr => {
                                instrs.push(InstructionsWithLabels::XorI {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src: Slot::from_str(src1.as_str())?,
                                    imm,
                                });
                            }
                            Rule::ADDI_instr => {
                                instrs.push(InstructionsWithLabels::AddI {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src1: Slot::from_str(src1.as_str())?,
                                    imm,
                                });
                            }
                            Rule::ANDI_instr => {
                                instrs.push(InstructionsWithLabels::AndI {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src1: Slot::from_str(src1.as_str())?,
                                    imm,
                                });
                            }
                            Rule::SLTIU_instr => {
                                instrs.push(InstructionsWithLabels::Sltiu {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src: Slot::from_str(src1.as_str())?,
                                    imm,
                                });
                            }

                            Rule::MULI_instr => {
                                instrs.push(InstructionsWithLabels::MulI {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src1: Slot::from_str(src1.as_str())?,
                                    imm,
                                });
                            }
                            Rule::SRLI_instr => {
                                instrs.push(InstructionsWithLabels::SrlI {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src1: Slot::from_str(src1.as_str())?,
                                    imm,
                                });
                            }
                            Rule::SLLI_instr => {
                                instrs.push(InstructionsWithLabels::SllI {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src1: Slot::from_str(src1.as_str())?,
                                    imm,
                                });
                            }
                            Rule::SRAI_instr => {
                                instrs.push(InstructionsWithLabels::SraI {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src1: Slot::from_str(src1.as_str())?,
                                    imm,
                                });
                            }
                            _ => {
                                unimplemented!("binary_imm: {:?} not implemented", rule);
                            }
                        };
                    }
                    Rule::mov_non_imm => {
                        let mut mov_non_imm = instruction.into_inner();
                        let rule = get_first_inner(
                            mov_non_imm.next().unwrap(),
                            "mov_non_imm has instruction",
                        )
                        .as_rule();
                        let dst = mov_non_imm.next().expect("mov_non_imm has dst");
                        let src = mov_non_imm.next().expect("mov_non_imm has src");
                        match rule {
                            Rule::MVV_W_instr => {
                                instrs.push(InstructionsWithLabels::MvvW {
                                    dst: SlotWithOffset::from_str(dst.as_str())?,
                                    src: Slot::from_str(src.as_str())?,
                                });
                            }
                            Rule::MVV_L_instr => {
                                unimplemented!("MVV_L_instr not implemented");
                            }
                            _ => {
                                unimplemented!("mov_non_imm: {:?} not implemented", rule);
                            }
                        };
                    }
                    Rule::jump_with_op_imm => {
                        let mut jump_with_op_instrs_imm = instruction.into_inner();
                        let rule = get_first_inner(
                            jump_with_op_instrs_imm.next().unwrap(),
                            "jump_with_op_instrs_imm has instruction",
                        )
                        .as_rule();
                        let dst = jump_with_op_instrs_imm
                            .next()
                            .expect("jump_with_op_instrs_imm has dst");
                        let imm = jump_with_op_instrs_imm
                            .next()
                            .expect("jump_with_op_instrs_imm has imm");
                        match rule {
                            Rule::TAILI_instr => {
                                instrs.push(InstructionsWithLabels::Taili {
                                    label: dst.as_str().to_string(),
                                    arg: Slot::from_str(imm.as_str())?,
                                });
                            }
                            Rule::CALLI_instr => {
                                instrs.push(InstructionsWithLabels::Calli {
                                    label: dst.as_str().to_string(),
                                    arg: Slot::from_str(imm.as_str())?,
                                });
                            }
                            Rule::BNZ_instr => {
                                instrs.push(InstructionsWithLabels::Bnz {
                                    label: dst.as_str().to_string(),
                                    src: Slot::from_str(imm.as_str())?,
                                });
                            }
                            _ => {
                                unimplemented!("jump_with_op_imm: {:?} not implemented", rule);
                            }
                        };
                    }
                    Rule::load_imm => {
                        let mut load_imm = instruction.into_inner();
                        let rule = get_first_inner(
                            load_imm.next().expect("load_imm has LDI.W instruction"),
                            "load_imm has LDI.W instruction",
                        )
                        .as_rule();
                        let dst =
                            Slot::from_str(load_imm.next().expect("load_imm has dst").as_str())?;
                        let imm = Immediate::from_str(
                            load_imm.next().expect("load_imm has imm").as_str(),
                        )?;
                        match rule {
                            Rule::LDI_W_instr => {
                                instrs.push(InstructionsWithLabels::Ldi { dst, imm });
                            }
                            _ => {
                                unreachable!("We have implemented all load_imm instructions");
                            }
                        }
                    }
                    Rule::binary_non_imm => {
                        let mut binary_op = instruction.into_inner();
                        let rule =
                            get_first_inner(binary_op.next().unwrap(), "binary_op has instruction")
                                .as_rule();
                        let dst =
                            Slot::from_str(binary_op.next().expect("binary_op has dst").as_str())?;
                        let src1 =
                            Slot::from_str(binary_op.next().expect("binary_op has src1").as_str())?;
                        let src2 =
                            Slot::from_str(binary_op.next().expect("binary_op has src2").as_str())?;
                        match rule {
                            Rule::XOR_instr => {
                                instrs.push(InstructionsWithLabels::Xor { dst, src1, src2 });
                            }
                            Rule::ADD_instr => {
                                instrs.push(InstructionsWithLabels::Add { dst, src1, src2 });
                            }
                            Rule::AND_instr => {
                                instrs.push(InstructionsWithLabels::And { dst, src1, src2 });
                            }
                            Rule::SLTU_instr => {
                                instrs.push(InstructionsWithLabels::Sltu { dst, src1, src2 });
                            }
                            Rule::SUB_instr => {
                                instrs.push(InstructionsWithLabels::Sub { dst, src1, src2 });
                            }
                            Rule::MUL_instr => {
                                instrs.push(InstructionsWithLabels::Mul { dst, src1, src2 });
                            }
                            _ => {
                                unimplemented!("binary_op: {:?} not implemented", rule);
                            }
                        };
                    }
                    Rule::nullary => {
                        let mut nullary = instruction.into_inner();
                        let rule =
                            get_first_inner(nullary.next().unwrap(), "nullary has instruction")
                                .as_rule();
                        match rule {
                            Rule::RET_instr => {
                                instrs.push(InstructionsWithLabels::Ret);
                            }
                            _ => unreachable!("All nullary instructions are implemented"),
                        }
                    }
                    Rule::simple_jump => {
                        let mut simple_jump = instruction.into_inner();
                        let rule =
                            get_first_inner(simple_jump.next().unwrap(), "jump has instruction")
                                .as_rule();
                        let dst = simple_jump.next().expect("jump_with_op_instrs_imm has dst");

                        match rule {
                            Rule::J_instr => {
                                let first_char =
                                    dst.as_str().chars().next().expect("simple jump as target");
                                if first_char == '_' || first_char.is_ascii() {
                                    instrs.push(InstructionsWithLabels::Jumpi {
                                        label: dst.as_str().to_string(),
                                    });
                                } else {
                                    instrs.push(InstructionsWithLabels::Jumpv {
                                        offset: Slot::from_str(dst.as_str())?,
                                    })
                                }
                            }
                            _ => unreachable!("All nullary instructions are implemented"),
                        }
                    }
                    _ => {
                        return Err(Error::UnknownInstruction(
                            instruction.as_span().as_str().to_string(),
                        ));
                    }
                };
            }
            Rule::EOI => (),
            Rule::line => parse_line(instrs, instr_or_label.into_inner())?,
            _ => {
                return Err(Error::UnknownInstruction(
                    instr_or_label.as_span().as_str().to_string(),
                ));
            }
        }
    }
    Ok(())
}

pub fn parse_program(input: &str) -> Result<Vec<InstructionsWithLabels>, Error> {
    let parser = AsmParser::parse(Rule::program, input);
    let mut instrs = Vec::<InstructionsWithLabels>::new();

    let program = parser
        .map_err(|_| Error::NoStartLabelOrInstructionFound)?
        .next()
        .ok_or(Error::NoStartLabelOrInstructionFound)?
        .into_inner();

    for line in program {
        parse_line(&mut instrs, line.into_inner())?;
    }

    Ok(instrs)
}
