use std::collections::HashMap;
use std::str::FromStr;

use pest::{iterators::Pair, iterators::Pairs, Parser};

mod instruction_args;
mod instructions_with_labels;
mod tests;

use instruction_args::{BadArgumentError, Immediate, Slot, SlotWithOffset};
pub(crate) use instructions_with_labels::{
    get_full_prom_and_labels, Error, InstructionKind, InstructionsWithLabels, LabelsFrameSizes,
};

#[derive(pest_derive::Parser)]
#[grammar = "parser/asm.pest"]
struct AsmParser;

#[inline]
fn get_first_inner<'a>(pair: Pair<'a, Rule>, msg: &str) -> Pair<'a, Rule> {
    pair.into_inner().next().expect(msg)
}

// A line may have a label and an instruction
fn parse_line(
    instrs: &mut Vec<InstructionsWithLabels>,
    pairs: Pairs<'_, Rule>,
    current_framesize: &mut Option<u16>,
) -> Result<(), Error> {
    let mut has_callhint = false;

    for instr_or_label in pairs {
        match instr_or_label.as_rule() {
            Rule::framesize_directive => {
                let mut inner = instr_or_label.into_inner();
                let size = inner
                    .next()
                    .expect("framesize directive must have a size")
                    .as_str();
                let size = u16::from_str(size).map_err(|_| {
                    Error::BadArgument(BadArgumentError::Immediate(size.to_string()))
                })?;

                // Store the framesize for the next label
                *current_framesize = Some(size);
            }
            Rule::label => {
                let label_name = get_first_inner(instr_or_label, "label must have label_name");
                let label_str = label_name.as_span().as_str().to_string();

                // If we have a pending framesize, associate it with this label
                if let Some(size) = current_framesize.take() {
                    instrs.push(InstructionsWithLabels::regular(InstructionKind::FrameSize(
                        label_str.clone(),
                        size,
                    )));
                }

                instrs.push(InstructionsWithLabels::regular(InstructionKind::Label(
                    label_str,
                )));
            }
            Rule::instruction => {
                parse_instruction(instrs, instr_or_label, has_callhint)?;
                has_callhint = false; // Reset call hint flag
            }
            Rule::instruction_with_hint => {
                let mut inner = instr_or_label.into_inner();

                // Check if first item is a callhint directive
                if let Some(first) = inner.next() {
                    if first.as_rule() == Rule::callhint_directive {
                        has_callhint = true;
                    } else {
                        // It's the instruction itself
                        parse_instruction(instrs, first, has_callhint)?;
                        has_callhint = false; // Reset call hint flag
                    }
                }

                // If there's another item, it must be the instruction
                if let Some(instruction) = inner.next() {
                    parse_instruction(instrs, instruction, has_callhint)?;
                    has_callhint = false; // Reset call hint flag
                }
            }
            Rule::EOI => (),
            Rule::line => parse_line(instrs, instr_or_label.into_inner(), current_framesize)?,
            _ => {
                return Err(Error::UnknownInstruction(
                    instr_or_label.as_span().as_str().to_string(),
                ));
            }
        }
    }

    Ok(())
}

// Helper function to parse instructions
fn parse_instruction(
    instrs: &mut Vec<InstructionsWithLabels>,
    instruction: Pair<'_, Rule>,
    has_callhint: bool,
) -> Result<(), Error> {
    let instruction = get_first_inner(instruction, "Instruction has inner tokens");
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
                    let instr = InstructionKind::MviH { dst, imm };
                    if has_callhint {
                        instrs.push(InstructionsWithLabels::call_hint(instr));
                    } else {
                        instrs.push(InstructionsWithLabels::regular(instr));
                    }
                }
                _ => {
                    unreachable!("We have implemented all mov_imm instructions");
                }
            }
        }
        Rule::binary_imm => {
            let mut binary_imm = instruction.into_inner();
            let rule =
                get_first_inner(binary_imm.next().unwrap(), "binary_imm has instruction").as_rule();
            let dst = binary_imm.next().expect("binary_imm has dest");
            let src1 = binary_imm.next().expect("binary_imm has src1");
            let imm = Immediate::from_str(binary_imm.next().expect("binary_imm has imm").as_str())?;

            let instr = match rule {
                Rule::B32_MULI_instr => InstructionKind::B32Muli {
                    dst: Slot::from_str(dst.as_str())?,
                    src1: Slot::from_str(src1.as_str())?,
                    imm,
                },
                Rule::XORI_instr => InstructionKind::XorI {
                    dst: Slot::from_str(dst.as_str())?,
                    src: Slot::from_str(src1.as_str())?,
                    imm,
                },
                Rule::ADDI_instr => InstructionKind::AddI {
                    dst: Slot::from_str(dst.as_str())?,
                    src1: Slot::from_str(src1.as_str())?,
                    imm,
                },
                Rule::ANDI_instr => InstructionKind::AndI {
                    dst: Slot::from_str(dst.as_str())?,
                    src1: Slot::from_str(src1.as_str())?,
                    imm,
                },
                Rule::MULI_instr => InstructionKind::MulI {
                    dst: Slot::from_str(dst.as_str())?,
                    src1: Slot::from_str(src1.as_str())?,
                    imm,
                },
                Rule::SRLI_instr => InstructionKind::SrlI {
                    dst: Slot::from_str(dst.as_str())?,
                    src1: Slot::from_str(src1.as_str())?,
                    imm,
                },
                Rule::SLLI_instr => InstructionKind::SllI {
                    dst: Slot::from_str(dst.as_str())?,
                    src1: Slot::from_str(src1.as_str())?,
                    imm,
                },
                _ => {
                    unimplemented!("binary_imm: {:?} not implemented", rule);
                }
            };

            if has_callhint {
                instrs.push(InstructionsWithLabels::call_hint(instr));
            } else {
                instrs.push(InstructionsWithLabels::regular(instr));
            }
        }
        Rule::mov_non_imm => {
            let mut mov_non_imm = instruction.into_inner();
            let rule = get_first_inner(mov_non_imm.next().unwrap(), "mov_non_imm has instruction")
                .as_rule();
            let dst = mov_non_imm.next().expect("mov_non_imm has dst");
            let src = mov_non_imm.next().expect("mov_non_imm has src");
            match rule {
                Rule::MVV_W_instr => {
                    let instr = InstructionKind::MvvW {
                        dst: SlotWithOffset::from_str(dst.as_str())?,
                        src: Slot::from_str(src.as_str())?,
                    };

                    if has_callhint {
                        instrs.push(InstructionsWithLabels::call_hint(instr));
                    } else {
                        instrs.push(InstructionsWithLabels::regular(instr));
                    }
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

            let instr = match rule {
                Rule::TAILI_instr => InstructionKind::Taili {
                    label: dst.as_str().to_string(),
                    arg: Slot::from_str(imm.as_str())?,
                },
                Rule::BNZ_instr => InstructionKind::Bnz {
                    label: dst.as_str().to_string(),
                    src: Slot::from_str(imm.as_str())?,
                },
                _ => {
                    unimplemented!("jump_with_op_imm: {:?} not implemented", rule);
                }
            };

            if has_callhint {
                instrs.push(InstructionsWithLabels::call_hint(instr));
            } else {
                instrs.push(InstructionsWithLabels::regular(instr));
            }
        }
        Rule::load_imm => {
            let mut load_imm = instruction.into_inner();
            let rule = get_first_inner(
                load_imm.next().expect("load_imm has LDI.W instruction"),
                "load_imm has LDI.W instruction",
            )
            .as_rule();
            let dst = Slot::from_str(load_imm.next().expect("load_imm has dst").as_str())?;
            let imm = Immediate::from_str(load_imm.next().expect("load_imm has imm").as_str())?;
            match rule {
                Rule::LDI_W_instr => {
                    let instr = InstructionKind::Ldi { dst, imm };

                    if has_callhint {
                        instrs.push(InstructionsWithLabels::call_hint(instr));
                    } else {
                        instrs.push(InstructionsWithLabels::regular(instr));
                    }
                }
                _ => {
                    unreachable!("We have implemented all load_imm instructions");
                }
            }
        }
        Rule::binary_non_imm => {
            let mut binary_op = instruction.into_inner();
            let rule =
                get_first_inner(binary_op.next().unwrap(), "binary_op has instruction").as_rule();
            let dst = Slot::from_str(binary_op.next().expect("binary_op has dst").as_str())?;
            let src1 = Slot::from_str(binary_op.next().expect("binary_op has src1").as_str())?;
            let src2 = Slot::from_str(binary_op.next().expect("binary_op has src2").as_str())?;

            let instr = match rule {
                Rule::XOR_instr => InstructionKind::Xor { dst, src1, src2 },
                Rule::ADD_instr => InstructionKind::Add { dst, src1, src2 },
                _ => {
                    unimplemented!("binary_op: {:?} not implemented", rule);
                }
            };

            if has_callhint {
                instrs.push(InstructionsWithLabels::call_hint(instr));
            } else {
                instrs.push(InstructionsWithLabels::regular(instr));
            }
        }
        Rule::nullary => {
            let mut nullary = instruction.into_inner();
            let rule =
                get_first_inner(nullary.next().unwrap(), "nullary has instruction").as_rule();
            match rule {
                Rule::RET_instr => {
                    let instr = InstructionKind::Ret;

                    if has_callhint {
                        instrs.push(InstructionsWithLabels::call_hint(instr));
                    } else {
                        instrs.push(InstructionsWithLabels::regular(instr));
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
    }

    Ok(())
}

pub fn parse_program(
    input: &str,
) -> Result<(Vec<InstructionsWithLabels>, HashMap<String, u16>), Error> {
    let parser = AsmParser::parse(Rule::program, input);
    let mut instrs = Vec::<InstructionsWithLabels>::new();
    let mut framesize_map = HashMap::new();
    let mut current_framesize = None;

    let program = parser
        .map_err(|_| Error::NoStartLabelOrInstructionFound)?
        .next()
        .ok_or(Error::NoStartLabelOrInstructionFound)?
        .into_inner();

    for line in program {
        parse_line(&mut instrs, line.into_inner(), &mut current_framesize)?;
    }

    // Process framesize directives
    let mut i = 0;
    while i < instrs.len() {
        if let Some((name, size)) = instrs[i].framesize_info() {
            framesize_map.insert(name.clone(), size);
            // Remove the FrameSize directive
            instrs.remove(i);
            // Don't increment i since we removed an item
        } else {
            i += 1;
        }
    }

    Ok((instrs, framesize_map))
}
