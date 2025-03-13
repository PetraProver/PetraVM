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

/// Helper to get the first inner pair with a custom error message.
#[inline]
fn get_first_inner<'a>(pair: Pair<'a, Rule>, msg: &str) -> Pair<'a, Rule> {
    pair.into_inner().next().expect(msg)
}

/// Helper function to push an instruction into the list, respecting the call
/// hint flag.
fn push_instruction(
    instrs: &mut Vec<InstructionsWithLabels>,
    instr: InstructionKind,
    has_callhint: bool,
) {
    if has_callhint {
        instrs.push(InstructionsWithLabels::call_hint(instr));
    } else {
        instrs.push(InstructionsWithLabels::regular(instr));
    }
}

/// Parses a single line (which may contain a label and/or an instruction).
fn parse_line(
    instrs: &mut Vec<InstructionsWithLabels>,
    pairs: Pairs<'_, Rule>,
    current_framesize: &mut Option<u16>,
) -> Result<(), Error> {
    // Flag to track a pending call hint for the next instruction.
    let mut has_callhint = false;

    for pair in pairs {
        match pair.as_rule() {
            Rule::framesize_directive => {
                // Parse and store framesize for the next label.
                let mut inner = pair.into_inner();
                let size_str = inner
                    .next()
                    .expect("framesize directive must have a size")
                    .as_str();
                let size = u16::from_str(size_str).map_err(|_| {
                    Error::BadArgument(BadArgumentError::Immediate(size_str.to_string()))
                })?;
                *current_framesize = Some(size);
            }
            Rule::label => {
                let label_name = get_first_inner(pair, "label must have label_name");
                let label_str = label_name.as_span().as_str().to_string();

                // If a framesize is pending, attach it to the label.
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
                parse_instruction(instrs, pair, has_callhint)?;
                has_callhint = false;
            }
            Rule::instruction_with_hint => {
                let mut inner = pair.into_inner();
                // Check if the first token is a callhint directive.
                if let Some(first) = inner.next() {
                    if first.as_rule() == Rule::callhint_directive {
                        has_callhint = true;
                    } else {
                        // First token is the instruction itself.
                        parse_instruction(instrs, first, has_callhint)?;
                        has_callhint = false;
                    }
                }
                // If there is a second token, it must be the instruction.
                if let Some(instruction) = inner.next() {
                    parse_instruction(instrs, instruction, has_callhint)?;
                    has_callhint = false;
                }
            }
            Rule::line => {
                // Recursively parse inner lines.
                parse_line(instrs, pair.into_inner(), current_framesize)?;
            }
            Rule::EOI => {}
            _ => {
                return Err(Error::UnknownInstruction(
                    pair.as_span().as_str().to_string(),
                ));
            }
        }
    }
    Ok(())
}

/// Helper function to parse an instruction and push it into the vector.
fn parse_instruction(
    instrs: &mut Vec<InstructionsWithLabels>,
    instruction: Pair<'_, Rule>,
    has_callhint: bool,
) -> Result<(), Error> {
    // Get the first inner token to determine the instruction type.
    let instr_token = get_first_inner(instruction, "Instruction has inner tokens");
    match instr_token.as_rule() {
        Rule::mov_imm => {
            let mut inner = instr_token.into_inner();
            let instr_rule = get_first_inner(
                inner.next().expect("Expected MVI_H instruction"),
                "MVI_H missing",
            )
            .as_rule();
            let dest = inner.next().expect("MVI_H missing destination");
            let imm = inner.next().expect("MVI_H missing immediate");
            let dst = SlotWithOffset::from_str(dest.as_str())?;
            let imm = Immediate::from_str(imm.as_str())?;
            match instr_rule {
                Rule::MVI_H_instr => {
                    push_instruction(instrs, InstructionKind::MviH { dst, imm }, has_callhint);
                }
                _ => unreachable!("Unexpected mov_imm instruction type"),
            }
        }
        Rule::binary_imm => {
            let mut inner = instr_token.into_inner();
            let instr_rule =
                get_first_inner(inner.next().unwrap(), "binary_imm missing instruction").as_rule();
            let dst = inner.next().expect("binary_imm missing dest");
            let src1 = inner.next().expect("binary_imm missing src1");
            let imm = Immediate::from_str(inner.next().expect("binary_imm missing imm").as_str())?;
            let instr = match instr_rule {
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
                _ => unimplemented!("binary_imm: {:?} not implemented", instr_rule),
            };
            push_instruction(instrs, instr, has_callhint);
        }
        Rule::mov_non_imm => {
            let mut inner = instr_token.into_inner();
            let instr_rule =
                get_first_inner(inner.next().unwrap(), "mov_non_imm missing instruction").as_rule();
            let dst = inner.next().expect("mov_non_imm missing destination");
            let src = inner.next().expect("mov_non_imm missing source");
            match instr_rule {
                Rule::MVV_W_instr => {
                    push_instruction(
                        instrs,
                        InstructionKind::MvvW {
                            dst: SlotWithOffset::from_str(dst.as_str())?,
                            src: Slot::from_str(src.as_str())?,
                        },
                        has_callhint,
                    );
                }
                Rule::MVV_L_instr => unimplemented!("MVV_L_instr not implemented"),
                _ => unimplemented!("mov_non_imm: {:?} not implemented", instr_rule),
            }
        }
        Rule::jump_with_op_imm => {
            let mut inner = instr_token.into_inner();
            let instr_rule = get_first_inner(
                inner.next().unwrap(),
                "jump_with_op_imm missing instruction",
            )
            .as_rule();
            let dst = inner.next().expect("jump_with_op_imm missing destination");
            let imm = inner.next().expect("jump_with_op_imm missing immediate");
            let instr = match instr_rule {
                Rule::TAILI_instr => InstructionKind::Taili {
                    label: dst.as_str().to_string(),
                    arg: Slot::from_str(imm.as_str())?,
                },
                Rule::BNZ_instr => InstructionKind::Bnz {
                    label: dst.as_str().to_string(),
                    src: Slot::from_str(imm.as_str())?,
                },
                _ => unimplemented!("jump_with_op_imm: {:?} not implemented", instr_rule),
            };
            push_instruction(instrs, instr, has_callhint);
        }
        Rule::load_imm => {
            let mut inner = instr_token.into_inner();
            let instr_rule = get_first_inner(
                inner.next().expect("load_imm missing instruction"),
                "load_imm missing instruction",
            )
            .as_rule();
            let dst = Slot::from_str(inner.next().expect("load_imm missing destination").as_str())?;
            let imm =
                Immediate::from_str(inner.next().expect("load_imm missing immediate").as_str())?;
            match instr_rule {
                Rule::LDI_W_instr => {
                    push_instruction(instrs, InstructionKind::Ldi { dst, imm }, has_callhint);
                }
                _ => unreachable!("Unexpected load_imm instruction type"),
            }
        }
        Rule::binary_non_imm => {
            let mut inner = instr_token.into_inner();
            let instr_rule =
                get_first_inner(inner.next().unwrap(), "binary_non_imm missing instruction")
                    .as_rule();
            let dst = Slot::from_str(inner.next().expect("binary_non_imm missing dest").as_str())?;
            let src1 = Slot::from_str(inner.next().expect("binary_non_imm missing src1").as_str())?;
            let src2 = Slot::from_str(inner.next().expect("binary_non_imm missing src2").as_str())?;
            let instr = match instr_rule {
                Rule::XOR_instr => InstructionKind::Xor { dst, src1, src2 },
                Rule::ADD_instr => InstructionKind::Add { dst, src1, src2 },
                _ => unimplemented!("binary_non_imm: {:?} not implemented", instr_rule),
            };
            push_instruction(instrs, instr, has_callhint);
        }
        Rule::nullary => {
            let instr_rule = get_first_inner(
                instr_token.into_inner().next().unwrap(),
                "nullary missing instruction",
            )
            .as_rule();
            match instr_rule {
                Rule::RET_instr => push_instruction(instrs, InstructionKind::Ret, has_callhint),
                _ => unreachable!("Unexpected nullary instruction"),
            }
        }
        _ => {
            return Err(Error::UnknownInstruction(
                instr_token.as_span().as_str().to_string(),
            ));
        }
    }
    Ok(())
}

/// Entry point for parsing a program.
pub fn parse_program(input: &str) -> Result<Vec<InstructionsWithLabels>, Error> {
    let parser = AsmParser::parse(Rule::program, input);
    let mut instrs = Vec::<InstructionsWithLabels>::new();
    let mut current_framesize = None;

    let program = parser
        .map_err(|_| Error::NoStartLabelOrInstructionFound)?
        .next()
        .ok_or(Error::NoStartLabelOrInstructionFound)?
        .into_inner();

    for line in program {
        parse_line(&mut instrs, line.into_inner(), &mut current_framesize)?;
    }

    Ok(instrs)
}
