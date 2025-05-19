use std::collections::{HashMap, HashSet};

use binius_field::{ExtensionField, Field, PackedField};
use binius_m3::builder::{B16, B32};
use tracing::instrument;

use crate::parser::{parse_program, Error as ParserError, InstructionsWithLabels};
use crate::{
    execution::{InterpreterInstruction, G},
    memory::ProgramRom,
    opcodes::Opcode,
};

#[derive(Debug, thiserror::Error)]
pub enum AssemblerError {
    #[error("First line should be a label")]
    NoStartLabelFound,

    #[error("Multiple labels to identify a target")]
    MultipleLabelsForTarget,

    #[error("Invalid instruction: {0}")]
    InvalidInstruction(String),

    #[error("File read error: {0}")]
    FileReadError(std::io::Error),

    #[error("Failed to parse program: {0}")]
    ParseError(#[from] ParserError),

    #[error("Duplicate label: {0}")]
    DuplicateLabel(String),

    #[error("Empty label")]
    EmptyLabel,

    #[error("Function {0} has no frame size")]
    FunctionHasNoFrameSize(String),

    #[error("Function {0} not found")]
    FunctionNotFound(String),

    #[error("Label or function {0} not found")]
    LabelNotFound(String),

    #[error("Something went wrong: {0}")]
    BadError(String),
}

/// Labels hold the labels in the code, with their associated binary field PCs
/// together with its discrete logarithm as advice.
type Labels = HashMap<String, (B32, u32)>;
/// Binary field PC as the key. Values are: frame size.
pub type LabelsFrameSizes = HashMap<B32, u16>;
// Gives the field PC associated to an integer PC. Only contains the PCs that
// can be called by the PROM.
pub(crate) type PCFieldToInt = HashMap<B32, u32>;

#[derive(Clone, Debug)]
pub struct AssembledProgram {
    pub prom: ProgramRom,
    pub labels: Labels,
    pub pc_field_to_int: PCFieldToInt,
    pub frame_sizes: LabelsFrameSizes,
}

pub struct Assembler;

impl Assembler {
    pub fn from_file(file: std::path::PathBuf) -> Result<AssembledProgram, AssemblerError> {
        let file_content = std::fs::read_to_string(file).map_err(AssemblerError::FileReadError)?;
        Assembler::from_code(&file_content)
    }

    pub fn from_code(code: &str) -> Result<AssembledProgram, AssemblerError> {
        let instructions = parse_program(code)?;
        Assembler::assemble(instructions)
    }

    #[instrument(level = "debug", skip_all)]
    fn assemble(
        instructions: Vec<InstructionsWithLabels>,
    ) -> Result<AssembledProgram, AssemblerError> {
        if !matches!(
            instructions.first(),
            Some(InstructionsWithLabels::Label(_, _))
        ) {
            return Err(AssemblerError::NoStartLabelFound);
        }

        // Make sure there's one and only one label to identify a target
        if instructions
            .iter()
            .zip(instructions.iter().skip(1))
            .any(|(instr, next_instr)| {
                matches!(instr, InstructionsWithLabels::Label(_, _))
                    && matches!(next_instr, InstructionsWithLabels::Label(_, _))
            })
        {
            return Err(AssemblerError::MultipleLabelsForTarget);
        }

        // Edge case: if the last instruction is a label, just error out.
        if matches!(
            instructions.last(),
            Some(InstructionsWithLabels::Label(_, _))
        ) {
            return Err(AssemblerError::EmptyLabel);
        }

        let (labels, pc_field_to_int, frame_sizes) = get_labels(&instructions)?;
        let mut prom = ProgramRom::new();
        let mut field_pc = B32::ONE;

        for instruction in instructions.iter() {
            get_prom_inst_from_inst_with_label(&mut prom, &labels, &mut field_pc, instruction)?;
        }

        Ok(AssembledProgram {
            prom,
            labels,
            pc_field_to_int,
            frame_sizes,
        })
    }
}

// converts instructions into binary field elements
pub fn get_prom_inst_from_inst_with_label(
    prom: &mut ProgramRom,
    labels: &Labels,
    field_pc: &mut B32,
    instruction: &InstructionsWithLabels,
) -> Result<(), AssemblerError> {
    match instruction {
        InstructionsWithLabels::Label(s, _) => {
            if labels.get(s).is_none() {
                return Err(AssemblerError::BadError(format!(
                    "Label {s} not found in the HashMap of labels."
                )));
            }
        }
        InstructionsWithLabels::B32Mul { dst, src1, src2 } => {
            let instruction = [
                Opcode::B32Mul.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];

            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::B32Muli { dst, src1, imm } => {
            let instruction = [
                Opcode::B32Muli.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                imm.get_field_val(),
            ];

            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;

            let instruction = [
                Opcode::B32Muli.get_field_elt(),
                imm.get_high_field_val(),
                B16::zero(),
                B16::zero(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::B128Add { dst, src1, src2 } => {
            let instruction = [
                Opcode::B128Add.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];

            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::B128Mul { dst, src1, src2 } => {
            let instruction = [
                Opcode::B128Mul.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];

            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Mvih { dst, imm } => {
            let instruction = [
                Opcode::Mvih.get_field_elt(),
                dst.get_slot_16bfield_val(),
                dst.get_offset_field_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Mvvw { dst, src } => {
            let instruction = [
                Opcode::Mvvw.get_field_elt(),
                dst.get_slot_16bfield_val(),
                dst.get_offset_field_val(),
                src.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Mvvl { dst, src } => {
            let instruction = [
                Opcode::Mvvl.get_field_elt(),
                dst.get_slot_16bfield_val(),
                dst.get_offset_field_val(),
                src.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Taili { label, next_fp } => {
            if let Some((target, advice)) = labels.get(label) {
                let targets_16b = ExtensionField::<B16>::iter_bases(target).collect::<Vec<_>>();
                let instruction = [
                    Opcode::Taili.get_field_elt(),
                    targets_16b[0],
                    targets_16b[1],
                    next_fp.get_16bfield_val(),
                ];

                prom.push(InterpreterInstruction::new(
                    instruction,
                    *field_pc,
                    Some(*advice),
                ));
            } else {
                return Err(AssemblerError::FunctionNotFound(label.to_string()));
            }

            *field_pc *= G;
        }
        InstructionsWithLabels::Tailv { offset, next_fp } => {
            let instruction = [
                Opcode::Tailv.get_field_elt(),
                offset.get_16bfield_val(),
                next_fp.get_16bfield_val(),
                B16::zero(),
            ];

            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Calli { label, next_fp } => {
            if let Some((target, advice)) = labels.get(label) {
                let targets_16b = ExtensionField::<B16>::iter_bases(target).collect::<Vec<_>>();
                let instruction = [
                    Opcode::Calli.get_field_elt(),
                    targets_16b[0],
                    targets_16b[1],
                    next_fp.get_16bfield_val(),
                ];

                prom.push(InterpreterInstruction::new(
                    instruction,
                    *field_pc,
                    Some(*advice),
                ));
            } else {
                return Err(AssemblerError::FunctionNotFound(label.to_string()));
            }

            *field_pc *= G;
        }
        InstructionsWithLabels::Callv { offset, next_fp } => {
            let instruction = [
                Opcode::Callv.get_field_elt(),
                offset.get_16bfield_val(),
                next_fp.get_16bfield_val(),
                B16::zero(),
            ];

            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Jumpi { label } => {
            if let Some((target, advice)) = labels.get(label) {
                let targets_16b = ExtensionField::<B16>::iter_bases(target).collect::<Vec<_>>();
                let instruction = [
                    Opcode::Jumpi.get_field_elt(),
                    targets_16b[0],
                    targets_16b[1],
                    B16::zero(),
                ];

                prom.push(InterpreterInstruction::new(
                    instruction,
                    *field_pc,
                    Some(*advice),
                ));
            } else {
                return Err(AssemblerError::LabelNotFound(label.to_string()));
            }
            *field_pc *= G;
        }
        InstructionsWithLabels::Jumpv { offset } => {
            let instruction = [
                Opcode::Jumpv.get_field_elt(),
                offset.get_16bfield_val(),
                B16::zero(),
                B16::zero(),
            ];

            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Ldi { dst, imm } => {
            let instruction = [
                Opcode::Ldi.get_field_elt(),
                dst.get_16bfield_val(),
                imm.get_field_val(),
                imm.get_high_field_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Xor { dst, src1, src2 } => {
            let instruction = [
                Opcode::Xor.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Xori { dst, src, imm } => {
            let instruction = [
                Opcode::Xori.get_field_elt(),
                dst.get_16bfield_val(),
                src.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Bnz { label, src } => {
            if let Some((target, advice)) = labels.get(label) {
                let targets_16b = ExtensionField::<B16>::iter_bases(target).collect::<Vec<_>>();
                let instruction = [
                    Opcode::Bnz.get_field_elt(),
                    targets_16b[0],
                    targets_16b[1],
                    src.get_16bfield_val(),
                ];

                prom.push(InterpreterInstruction::new(
                    instruction,
                    *field_pc,
                    Some(*advice),
                ));
            } else {
                return Err(AssemblerError::LabelNotFound(label.to_string()));
            }
            *field_pc *= G;
        }
        InstructionsWithLabels::Add { dst, src1, src2 } => {
            let instruction = [
                Opcode::Add.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Addi { dst, src1, imm } => {
            let instruction = [
                Opcode::Addi.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Or { dst, src1, src2 } => {
            let instruction = [
                Opcode::Or.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Ori { dst, src1, imm } => {
            let instruction = [
                Opcode::Ori.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Sub { dst, src1, src2 } => {
            let instruction = [
                Opcode::Sub.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Sle { dst, src1, src2 } => {
            let instruction = [
                Opcode::Sle.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Slei { dst, src, imm } => {
            let instruction = [
                Opcode::Slei.get_field_elt(),
                dst.get_16bfield_val(),
                src.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Sleu { dst, src1, src2 } => {
            let instruction = [
                Opcode::Sleu.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Sleiu { dst, src, imm } => {
            let instruction = [
                Opcode::Sleiu.get_field_elt(),
                dst.get_16bfield_val(),
                src.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Slt { dst, src1, src2 } => {
            let instruction = [
                Opcode::Slt.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Slti { dst, src, imm } => {
            let instruction = [
                Opcode::Slti.get_field_elt(),
                dst.get_16bfield_val(),
                src.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Sltu { dst, src1, src2 } => {
            let instruction = [
                Opcode::Sltu.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Sltiu { dst, src, imm } => {
            let instruction = [
                Opcode::Sltiu.get_field_elt(),
                dst.get_16bfield_val(),
                src.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Sll { dst, src1, src2 } => {
            let instruction = [
                Opcode::Sll.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Srl { dst, src1, src2 } => {
            let instruction = [
                Opcode::Srl.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Sra { dst, src1, src2 } => {
            let instruction = [
                Opcode::Sra.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Andi { dst, src1, imm } => {
            let instruction = [
                Opcode::Andi.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                imm.get_field_val(),
            ];

            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::And { dst, src1, src2 } => {
            let instruction = [
                Opcode::And.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];

            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Muli { dst, src1, imm } => {
            let instruction = [
                Opcode::Muli.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Mul { dst, src1, src2 } => {
            let instruction = [
                Opcode::Mul.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Mulu { dst, src1, src2 } => {
            let instruction = [
                Opcode::Mulu.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Mulsu { dst, src1, src2 } => {
            let instruction = [
                Opcode::Mulsu.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                src2.get_16bfield_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Srli { dst, src1, imm } => {
            let instruction = [
                Opcode::Srli.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Slli { dst, src1, imm } => {
            let instruction = [
                Opcode::Slli.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Srai { dst, src1, imm } => {
            let instruction = [
                Opcode::Srai.get_field_elt(),
                dst.get_16bfield_val(),
                src1.get_16bfield_val(),
                imm.get_field_val(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
        InstructionsWithLabels::Ret => {
            let instruction = [
                Opcode::Ret.get_field_elt(),
                B16::zero(),
                B16::zero(),
                B16::zero(),
            ];
            prom.push(InterpreterInstruction::new(instruction, *field_pc, None));

            *field_pc *= G;
        }
    }
    Ok(())
}

const fn incr_pc(pc: u32) -> u32 {
    if pc == u32::MAX {
        // We skip over 0, as it is inaccessible in the multiplicative group.
        return 1;
    }
    pc + 1
}

fn get_labels(
    instructions: &[InstructionsWithLabels],
) -> Result<(Labels, PCFieldToInt, LabelsFrameSizes), AssemblerError> {
    let mut labels = HashMap::new();
    let mut pc_field_to_int = HashMap::new();
    let mut frame_sizes = HashMap::new();
    let mut field_pc = B32::ONE;
    let mut pc = 1;
    let mut functions = HashSet::new();

    let first_label = instructions.first().unwrap();
    match first_label {
        InstructionsWithLabels::Label(name, _) => {
            functions.insert(name.as_str());
        }
        _ => unreachable!(),
    }

    // Insert first label into discrete log map
    pc_field_to_int.insert(field_pc, pc);

    // Identify functions from the labels and check if they have valid frame sizes.
    for instruction in instructions {
        match instruction {
            InstructionsWithLabels::Label(s, frame_size) => {
                if labels.insert(s.clone(), (field_pc, pc)).is_some() {
                    return Err(AssemblerError::DuplicateLabel(s.clone()));
                }

                // If we have a frame size for this label, add it to our frame_sizes map
                if let Some(size) = frame_size {
                    frame_sizes.insert(field_pc, *size);
                }

                // We do not increment the PC if we found a label.
                continue;
            }
            InstructionsWithLabels::B32Muli { .. } => {
                field_pc *= G;
                pc = incr_pc(pc);
                pc_field_to_int.insert(field_pc, pc);
            }
            InstructionsWithLabels::Taili { label, .. } => {
                functions.insert(label.as_str());
            }
            InstructionsWithLabels::Calli { label, .. } => {
                functions.insert(label.as_str());
            }
            _ => {}
        }
        field_pc *= G;
        pc = incr_pc(pc);
        pc_field_to_int.insert(field_pc, pc);
    }

    for function in functions {
        let (as_pc, _) = labels
            .get(function)
            .ok_or(AssemblerError::FunctionNotFound(function.to_string()))?;
        if !frame_sizes.contains_key(as_pc) {
            return Err(AssemblerError::FunctionHasNoFrameSize(function.to_string()));
        }
    }

    Ok((labels, pc_field_to_int, frame_sizes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_with_no_frame_size() {
        let programs = [
            r#"
        #[framesize(0x10)]
            start:
                CALLI some_function, @3
                RET

            some_function:
                MVV.W @3[1], @1
                RET
            "#,
            r#"
            start:
                RET
            "#,
        ];

        for program in programs {
            let out = Assembler::from_code(program);
            assert!(matches!(
                out,
                Err(AssemblerError::FunctionHasNoFrameSize(_))
            ));
        }
    }
}
