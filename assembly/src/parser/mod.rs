use std::str::FromStr;

use pest::{iterators::Pair, iterators::Pairs, Parser};

mod instruction_args;
mod instructions_with_labels;
mod tests;

use instruction_args::{Immediate, Slot, SlotWithOffset};
pub(crate) use instructions_with_labels::{Error, InstructionsWithLabels};
use tracing::instrument;

#[derive(pest_derive::Parser)]
#[grammar = "parser/asm.pest"]
struct AsmParser;

#[inline]
fn get_first_inner<'a>(pair: Pair<'a, Rule>, msg: &str) -> Pair<'a, Rule> {
    pair.into_inner().next().expect(msg)
}

#[inline]
fn parse_opcode<'a>(pair: Pair<'a, Rule>) -> (Rule, bool) {
    let mut pairs = pair.into_inner();
    let opcode_rule = pairs.next().expect("opcode is always present").as_rule();
    let prover_only = pairs.next().is_some();
    (opcode_rule, prover_only)
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
                        let (opcode_rule, prover_only) =
                            parse_opcode(mov_imm.next().expect("This is MVI_H"));
                        let dest = mov_imm.next().expect("MVI_H has dest");
                        let imm = mov_imm.next().expect("MVI_H has imm");
                        let dst = SlotWithOffset::from_str(dest.as_str())?;
                        let imm = Immediate::from_str(imm.as_str())?;
                        match opcode_rule {
                            Rule::MVI_H_instr => {
                                instrs.push(InstructionsWithLabels::Mvih {
                                    dst,
                                    imm,
                                    prover_only,
                                });
                            }
                            _ => {
                                unreachable!("We have implemented all mov_imm instructions");
                            }
                        }
                    }
                    Rule::binary_imm => {
                        let mut binary_imm = instruction.into_inner();
                        let (opcode_rule, prover_only) =
                            parse_opcode(binary_imm.next().expect("binary_imm has instruction"));
                        let dst = binary_imm.next().expect("binary_imm has dest");
                        let src1 = binary_imm.next().expect("binary_imm has src1");
                        let imm = Immediate::from_str(
                            binary_imm.next().expect("binary_imm has imm").as_str(),
                        )?;
                        match opcode_rule {
                            // B32_ADDI is an alias for XORI.
                            Rule::XORI_instr | Rule::B32_ADDI_instr => {
                                instrs.push(InstructionsWithLabels::Xori {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src: Slot::from_str(src1.as_str())?,
                                    imm,
                                    prover_only,
                                });
                            }
                            Rule::B32_MULI_instr => {
                                instrs.push(InstructionsWithLabels::B32Muli {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src1: Slot::from_str(src1.as_str())?,
                                    imm,
                                    prover_only,
                                });
                            }
                            Rule::ADDI_instr => {
                                instrs.push(InstructionsWithLabels::Addi {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src1: Slot::from_str(src1.as_str())?,
                                    imm,
                                    prover_only,
                                });
                            }
                            Rule::ANDI_instr => {
                                instrs.push(InstructionsWithLabels::Andi {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src1: Slot::from_str(src1.as_str())?,
                                    imm,
                                    prover_only,
                                });
                            }
                            Rule::ORI_instr => {
                                instrs.push(InstructionsWithLabels::Ori {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src1: Slot::from_str(src1.as_str())?,
                                    imm,
                                    prover_only,
                                });
                            }
                            Rule::SLEI_instr => {
                                instrs.push(InstructionsWithLabels::Slei {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src: Slot::from_str(src1.as_str())?,
                                    imm,
                                    prover_only,
                                });
                            }
                            Rule::SLEIU_instr => {
                                instrs.push(InstructionsWithLabels::Sleiu {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src: Slot::from_str(src1.as_str())?,
                                    imm,
                                    prover_only,
                                });
                            }
                            Rule::SLTI_instr => {
                                instrs.push(InstructionsWithLabels::Slti {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src: Slot::from_str(src1.as_str())?,
                                    imm,
                                    prover_only,
                                });
                            }
                            Rule::SLTIU_instr => {
                                instrs.push(InstructionsWithLabels::Sltiu {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src: Slot::from_str(src1.as_str())?,
                                    imm,
                                    prover_only,
                                });
                            }
                            Rule::MULI_instr => {
                                instrs.push(InstructionsWithLabels::Muli {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src1: Slot::from_str(src1.as_str())?,
                                    imm,
                                    prover_only,
                                });
                            }
                            Rule::SRLI_instr => {
                                instrs.push(InstructionsWithLabels::Srli {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src1: Slot::from_str(src1.as_str())?,
                                    imm,
                                    prover_only,
                                });
                            }
                            Rule::SLLI_instr => {
                                instrs.push(InstructionsWithLabels::Slli {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src1: Slot::from_str(src1.as_str())?,
                                    imm,
                                    prover_only,
                                });
                            }
                            Rule::SRAI_instr => {
                                instrs.push(InstructionsWithLabels::Srai {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src1: Slot::from_str(src1.as_str())?,
                                    imm,
                                    prover_only,
                                });
                            }
                            _ => {
                                unimplemented!("binary_imm: {:?} not implemented", opcode_rule);
                            }
                        };
                    }
                    Rule::mov_non_imm => {
                        let mut mov_non_imm = instruction.into_inner();
                        let (opcode_rule, prover_only) =
                            parse_opcode(mov_non_imm.next().expect("mov_non_imm has instruction"));
                        let dst = mov_non_imm.next().expect("mov_non_imm has dst");
                        let src = mov_non_imm.next().expect("mov_non_imm has src");
                        match opcode_rule {
                            Rule::MVV_W_instr => {
                                instrs.push(InstructionsWithLabels::Mvvw {
                                    dst: SlotWithOffset::from_str(dst.as_str())?,
                                    src: Slot::from_str(src.as_str())?,
                                    prover_only,
                                });
                            }
                            Rule::MVV_L_instr => {
                                instrs.push(InstructionsWithLabels::Mvvl {
                                    dst: SlotWithOffset::from_str(dst.as_str())?,
                                    src: Slot::from_str(src.as_str())?,
                                    prover_only,
                                });
                            }
                            _ => {
                                unimplemented!("mov_non_imm: {:?} not implemented", opcode_rule);
                            }
                        };
                    }
                    Rule::jump_with_op_imm => {
                        let mut jump_with_op_instrs_imm = instruction.into_inner();
                        let (opcode_rule, prover_only) = parse_opcode(
                            jump_with_op_instrs_imm
                                .next()
                                .expect("jump_with_op_instrs_imm has instruction"),
                        );
                        if prover_only {
                            return Err(Error::UnknownInstruction(format!("{opcode_rule:?}")));
                        }
                        let dst = jump_with_op_instrs_imm
                            .next()
                            .expect("jump_with_op_instrs_imm has dst");
                        let imm = jump_with_op_instrs_imm
                            .next()
                            .expect("jump_with_op_instrs_imm has imm");
                        match opcode_rule {
                            Rule::TAILI_instr => {
                                instrs.push(InstructionsWithLabels::Taili {
                                    label: dst.as_str().to_string(),
                                    next_fp: Slot::from_str(imm.as_str())?,
                                });
                            }
                            Rule::CALLI_instr => {
                                instrs.push(InstructionsWithLabels::Calli {
                                    label: dst.as_str().to_string(),
                                    next_fp: Slot::from_str(imm.as_str())?,
                                });
                            }
                            Rule::BNZ_instr => {
                                instrs.push(InstructionsWithLabels::Bnz {
                                    label: dst.as_str().to_string(),
                                    src: Slot::from_str(imm.as_str())?,
                                });
                            }
                            _ => {
                                unimplemented!(
                                    "jump_with_op_imm: {:?} not implemented",
                                    opcode_rule
                                );
                            }
                        };
                    }
                    Rule::jump_with_op_non_imm => {
                        let mut jump_non_imm = instruction.into_inner();
                        let (opcode_rule, prover_only) = parse_opcode(
                            jump_non_imm
                                .next()
                                .expect("jump_with_op_non_imm has instruction"),
                        );
                        if prover_only {
                            return Err(Error::UnknownInstruction(format!("{opcode_rule:?}")));
                        }
                        let op1 = jump_non_imm
                            .next()
                            .expect("jump_with_op_non_imm has first operand");
                        let op2 = jump_non_imm
                            .next()
                            .expect("jump_with_op_non_imm has second operand");
                        match opcode_rule {
                            Rule::TAILV_instr => {
                                instrs.push(InstructionsWithLabels::Tailv {
                                    offset: Slot::from_str(op1.as_str())?,
                                    next_fp: Slot::from_str(op2.as_str())?,
                                });
                            }
                            Rule::CALLV_instr => {
                                instrs.push(InstructionsWithLabels::Callv {
                                    offset: Slot::from_str(op1.as_str())?,
                                    next_fp: Slot::from_str(op2.as_str())?,
                                });
                            }
                            _ => {
                                unimplemented!(
                                    "jump_with_op_non_imm: {:?} not implemented",
                                    opcode_rule
                                );
                            }
                        }
                    }
                    Rule::load_imm => {
                        let mut load_imm = instruction.into_inner();
                        let (opcode_rule, prover_only) =
                            parse_opcode(load_imm.next().expect("load_imm has LDI.W instruction"));
                        let dst =
                            Slot::from_str(load_imm.next().expect("load_imm has dst").as_str())?;
                        let imm = Immediate::from_str(
                            load_imm.next().expect("load_imm has imm").as_str(),
                        )?;
                        match opcode_rule {
                            Rule::LDI_W_instr => {
                                instrs.push(InstructionsWithLabels::Ldi {
                                    dst,
                                    imm,
                                    prover_only,
                                });
                            }
                            _ => {
                                unreachable!("We have implemented all load_imm instructions");
                            }
                        }
                    }
                    Rule::binary_non_imm => {
                        let mut binary_op = instruction.into_inner();
                        let (opcode_rule, prover_only) =
                            parse_opcode(binary_op.next().expect("binary_op has instruction"));
                        let dst =
                            Slot::from_str(binary_op.next().expect("binary_op has dst").as_str())?;
                        let src1 =
                            Slot::from_str(binary_op.next().expect("binary_op has src1").as_str())?;
                        let src2 =
                            Slot::from_str(binary_op.next().expect("binary_op has src2").as_str())?;
                        match opcode_rule {
                            // B32_ADD is an alias for XOR.
                            Rule::XOR_instr | Rule::B32_ADD_instr => {
                                instrs.push(InstructionsWithLabels::Xor {
                                    dst,
                                    src1,
                                    src2,
                                    prover_only,
                                });
                            }
                            Rule::ADD_instr => {
                                instrs.push(InstructionsWithLabels::Add {
                                    dst,
                                    src1,
                                    src2,
                                    prover_only,
                                });
                            }
                            Rule::AND_instr => {
                                instrs.push(InstructionsWithLabels::And {
                                    dst,
                                    src1,
                                    src2,
                                    prover_only,
                                });
                            }
                            Rule::OR_instr => {
                                instrs.push(InstructionsWithLabels::Or {
                                    dst,
                                    src1,
                                    src2,
                                    prover_only,
                                });
                            }
                            Rule::SLL_instr => {
                                instrs.push(InstructionsWithLabels::Sll {
                                    dst,
                                    src1,
                                    src2,
                                    prover_only,
                                });
                            }
                            Rule::SRL_instr => {
                                instrs.push(InstructionsWithLabels::Srl {
                                    dst,
                                    src1,
                                    src2,
                                    prover_only,
                                });
                            }
                            Rule::SRA_instr => {
                                instrs.push(InstructionsWithLabels::Sra {
                                    dst,
                                    src1,
                                    src2,
                                    prover_only,
                                });
                            }
                            Rule::SLE_instr => {
                                instrs.push(InstructionsWithLabels::Sle {
                                    dst,
                                    src1,
                                    src2,
                                    prover_only,
                                });
                            }
                            Rule::SLEU_instr => {
                                instrs.push(InstructionsWithLabels::Sleu {
                                    dst,
                                    src1,
                                    src2,
                                    prover_only,
                                });
                            }
                            Rule::SLT_instr => {
                                instrs.push(InstructionsWithLabels::Slt {
                                    dst,
                                    src1,
                                    src2,
                                    prover_only,
                                });
                            }
                            Rule::SLTU_instr => {
                                instrs.push(InstructionsWithLabels::Sltu {
                                    dst,
                                    src1,
                                    src2,
                                    prover_only,
                                });
                            }
                            Rule::SUB_instr => {
                                instrs.push(InstructionsWithLabels::Sub {
                                    dst,
                                    src1,
                                    src2,
                                    prover_only,
                                });
                            }
                            Rule::B32_MUL_instr => {
                                instrs.push(InstructionsWithLabels::B32Mul {
                                    dst,
                                    src1,
                                    src2,
                                    prover_only,
                                });
                            }
                            Rule::MUL_instr => {
                                instrs.push(InstructionsWithLabels::Mul {
                                    dst,
                                    src1,
                                    src2,
                                    prover_only,
                                });
                            }
                            Rule::B128_ADD_instr => {
                                instrs.push(InstructionsWithLabels::B128Add {
                                    dst,
                                    src1,
                                    src2,
                                    prover_only,
                                });
                            }
                            Rule::B128_MUL_instr => {
                                instrs.push(InstructionsWithLabels::B128Mul {
                                    dst,
                                    src1,
                                    src2,
                                    prover_only,
                                });
                            }
                            Rule::MULU_instr => {
                                instrs.push(InstructionsWithLabels::Mulu {
                                    dst,
                                    src1,
                                    src2,
                                    prover_only,
                                });
                            }
                            Rule::MULSU_instr => {
                                instrs.push(InstructionsWithLabels::Mulsu {
                                    dst,
                                    src1,
                                    src2,
                                    prover_only,
                                });
                            }
                            _ => {
                                unimplemented!("binary_op: {opcode_rule:?} not implemented");
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
                        let (opcode_rule, prover_only) =
                            parse_opcode(simple_jump.next().expect("jump has instruction"));
                        if prover_only {
                            return Err(Error::UnknownInstruction(format!("{opcode_rule:?}")));
                        }
                        let dst = simple_jump
                            .next()
                            .expect("simple_jump expects a destination operand");
                        match dst.as_rule() {
                            Rule::label_name => {
                                // This is a jump to a label
                                instrs.push(InstructionsWithLabels::Jumpi {
                                    label: dst.as_str().to_string(),
                                });
                            }
                            Rule::slot => {
                                // This is a jump with an offset (e.g. "J @13")
                                instrs.push(InstructionsWithLabels::Jumpv {
                                    offset: Slot::from_str(dst.as_str())?,
                                });
                            }
                            _ => unreachable!("Unexpected token in simple_jump"),
                        }
                    }
                    Rule::alloc_imm => {
                        let mut alloc_imm = instruction.into_inner();
                        let (opcode_rule, prover_only) =
                            parse_opcode(alloc_imm.next().expect("alloc_imm has instruction"));
                        if !prover_only {
                            return Err(Error::UnknownInstruction(format!("{opcode_rule:?}")));
                        }
                        let dst = alloc_imm.next().expect("alloc_imm has dst");
                        let imm = alloc_imm.next().expect("alloc_imm has src");
                        match opcode_rule {
                            Rule::ALLOCI_instr => {
                                instrs.push(InstructionsWithLabels::Alloci {
                                    dst: Slot::from_str(dst.as_str())?,
                                    imm: Immediate::from_str(imm.as_str())?,
                                });
                            }
                            _ => {
                                unreachable!("We have implemented all alloc_imm instructions");
                            }
                        };
                    }
                    Rule::alloc_non_imm => {
                        let mut alloc_non_imm = instruction.into_inner();
                        let (opcode_rule, prover_only) = parse_opcode(
                            alloc_non_imm.next().expect("alloc_non_imm has instruction"),
                        );
                        if !prover_only {
                            return Err(Error::UnknownInstruction(format!("{opcode_rule:?}")));
                        }
                        let dst = alloc_non_imm.next().expect("alloc_non_imm has dst");
                        let src = alloc_non_imm.next().expect("alloc_non_imm has src");
                        match opcode_rule {
                            Rule::ALLOCV_instr => {
                                instrs.push(InstructionsWithLabels::Allocv {
                                    dst: Slot::from_str(dst.as_str())?,
                                    src: Slot::from_str(src.as_str())?,
                                });
                            }
                            _ => {
                                unreachable!("We have implemented all alloc_non_imm instructions");
                            }
                        };
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

#[instrument(level = "debug", skip_all)]
pub fn parse_program(input: &str) -> Result<Vec<InstructionsWithLabels>, Error> {
    let parser = AsmParser::parse(Rule::program, input);
    let mut instrs = Vec::<InstructionsWithLabels>::new();

    let program = parser
        .map_err(|err| Error::PestParse(Box::new(err)))?
        .next()
        .ok_or(Error::NoStartLabelOrInstructionFound)?
        .into_inner();

    for line in program {
        parse_line(&mut instrs, line.into_inner())?;
    }

    Ok(instrs)
}
