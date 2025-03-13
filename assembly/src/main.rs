// TODO: Remove these once stable enough
#![allow(unused)]
#![allow(dead_code)]

// TODO: Add doc

mod emulator;
mod event;
mod instruction_args;
mod instructions_with_labels;
mod opcodes;
mod parser;
mod vrom_allocator;

use std::collections::HashMap;

use binius_field::{BinaryField16b, BinaryField32b, ExtensionField, Field, PackedField};
use emulator::{Instruction, InterpreterInstruction, ProgramRom, ValueRom, ZCrayTrace, G};
use instructions_with_labels::{get_frame_sizes_all_labels, get_full_prom_and_labels};
use opcodes::Opcode;
use parser::parse_program;

#[inline(always)]
pub(crate) const fn get_binary_slot(i: u16) -> BinaryField16b {
    BinaryField16b::new(i)
}

pub(crate) fn code_to_prom(
    code: &[Instruction],
    is_calling_procedure_hints: &[bool],
) -> ProgramRom {
    let mut prom = ProgramRom::new();
    let mut pc = BinaryField32b::ONE; // we start at PC = 1G.
    for (i, &instruction) in code.iter().enumerate() {
        let interp_inst =
            InterpreterInstruction::new(instruction, pc, is_calling_procedure_hints[i]);
        prom.push(interp_inst);
        pc *= G;
    }

    prom
}
fn main() {
    let kernel_files = [
        include_str!("../../examples/bezout.asm"),
        include_str!("../../examples/div.asm"),
    ];
    let instructions = kernel_files
        .into_iter()
        .flat_map(|file| parse_program(file).unwrap())
        .collect::<Vec<_>>();

    // Sets the call procedure hints to true for the returned PROM (where
    // instructions are given with the labels).
    let mut is_call_procedure_hints_with_labels = vec![false; instructions.len()];
    let indices_to_set_with_labels = vec![
        7, 8, 9, 10, 12, 13, 14, 15, 16, // Bezout
        25, 26, 27, 28, // Div
    ];
    for idx in indices_to_set_with_labels {
        is_call_procedure_hints_with_labels[idx] = true;
    }
    let (prom, labels, pc_field_to_int) =
        get_full_prom_and_labels(&instructions, &is_call_procedure_hints_with_labels)
            .expect("Instructions were not formatted properly.");

    let frame_sizes = get_frame_sizes_all_labels(&prom, labels, &pc_field_to_int);
    println!("frame sizes {:?}", frame_sizes);

    let a = 12;
    let b = 3;
    let mut vrom = ValueRom::new_from_vec_u32(vec![0, 0, a, b]);

    let mut pc = BinaryField32b::ONE;
    let mut pc_field_to_int = HashMap::new();
    for i in 0..prom.len() {
        pc_field_to_int.insert(pc, i as u32 + 1);
        pc *= G;
    }
    let (trace, _) = ZCrayTrace::generate_with_vrom(prom, vrom, frame_sizes, pc_field_to_int)
        .expect("Trace generation should not fail.");

    // gcd
    assert_eq!(
        trace
            .vrom
            .get_u32(16)
            .expect("Return value for quotient not set."),
        3
    );
    // a's coefficient
    assert_eq!(
        trace
            .vrom
            .get_u32(20)
            .expect("Return value for remainder not set."),
        0
    );
    // b's coefficient
    assert_eq!(
        trace
            .vrom
            .get_u32(24)
            .expect("Return value for remainder not set."),
        1
    );
}
