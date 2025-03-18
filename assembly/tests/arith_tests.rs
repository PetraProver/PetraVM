use std::collections::HashMap;

use binius_field::{BinaryField, BinaryField32b, Field};
use zcrayvm_assembly::{get_full_prom_and_labels, parse_program, Memory, ValueRom, ZCrayTrace};

const G: BinaryField32b = BinaryField32b::MULTIPLICATIVE_GENERATOR;

#[test]
fn test_naive_div() {
    let instructions = parse_program(include_str!("../../examples/div.asm")).unwrap();

    // Sets the call procedure hints to true for the returned PROM (where
    // instructions are given with the labels).
    let mut is_call_procedure_hints_with_labels = vec![false; instructions.len()];
    let indices_to_set_with_labels = vec![4, 5, 6, 7];
    for idx in indices_to_set_with_labels {
        is_call_procedure_hints_with_labels[idx] = true;
    }
    let (prom, _, _, frame_sizes) =
        get_full_prom_and_labels(&instructions, &is_call_procedure_hints_with_labels)
            .expect("Instructions were not formatted properly.");

    let a = rand::random();
    let b = rand::random();
    let vrom = ValueRom::new_with_init_vals(&[0, 0, a, b]);

    let mut pc = BinaryField32b::ONE;
    let mut pc_field_to_int = HashMap::new();
    for i in 0..prom.len() {
        pc_field_to_int.insert(pc, i as u32 + 1);
        pc *= G;
    }
    let memory = Memory::new(prom, vrom);
    let (trace, _) = ZCrayTrace::generate(memory, frame_sizes, pc_field_to_int)
        .expect("Trace generation should not fail.");

    assert_eq!(
        trace
            .get_vrom_u32(4)
            .expect("Return value for quotient not set."),
        a / b
    );
    assert_eq!(
        trace
            .get_vrom_u32(5)
            .expect("Return value for remainder not set."),
        a % b
    );
}

#[test]
fn test_bezout() {
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
    let (prom, _, _, frame_sizes) =
        get_full_prom_and_labels(&instructions, &is_call_procedure_hints_with_labels)
            .expect("Instructions were not formatted properly.");

    let a = 12;
    let b = 3;
    let vrom = ValueRom::new_with_init_vals(&[0, 0, a, b]);

    let mut pc = BinaryField32b::ONE;
    let mut pc_field_to_int = HashMap::new();
    for i in 0..prom.len() {
        pc_field_to_int.insert(pc, i as u32 + 1);
        pc *= G;
    }

    let memory = Memory::new(prom, vrom);
    let (trace, _) = ZCrayTrace::generate(memory, frame_sizes, pc_field_to_int)
        .expect("Trace generation should not fail.");

    // gcd
    assert_eq!(
        trace
            .get_vrom_u32(4)
            .expect("Return value for quotient not set."),
        3
    );
    // a's coefficient
    assert_eq!(
        trace
            .get_vrom_u32(5)
            .expect("Return value for remainder not set."),
        0
    );
    // b's coefficient
    assert_eq!(
        trace
            .get_vrom_u32(6)
            .expect("Return value for remainder not set."),
        1
    );
}

#[test]
fn test_non_tail_long_div() {
    let kernel_file = include_str!("../../examples/non_tail_long_div.asm");

    let instructions = parse_program(kernel_file).unwrap();
    let indices_is_call_procedure_hints = [7, 8, 9, 10];

    let mut is_call_procedure_hints_with_labels = vec![false; instructions.len()];
    for idx in indices_is_call_procedure_hints {
        is_call_procedure_hints_with_labels[idx] = true;
    }

    let (prom, _, _, frame_sizes) =
        get_full_prom_and_labels(&instructions, &is_call_procedure_hints_with_labels)
            .expect("Instructions were not formatted properly.");

    let mut pc = BinaryField32b::ONE;
    let mut pc_field_to_int = HashMap::new();
    for i in 0..prom.len() {
        pc_field_to_int.insert(pc, i as u32 + 1);
        pc *= G;
    }
    let a = 54820;
    let b = 65;

    let vrom = ValueRom::new_with_init_vals(&[0, 0, a, b]);

    let memory = Memory::new(prom, vrom);
    let (trace, _) = ZCrayTrace::generate(memory, frame_sizes, pc_field_to_int)
        .expect("Trace generation should not fail.");

    assert_eq!(
        trace
            .get_vrom_u32(4)
            .expect("Return value for quotient not set."),
        a / b
    );
    assert_eq!(
        trace
            .get_vrom_u32(5)
            .expect("Return value for remainder not set."),
        a % b
    );
}

#[test]
fn test_tail_long_div() {
    let kernel_file = include_str!("../../examples/tail_long_div.asm");

    let instructions = parse_program(kernel_file).unwrap();
    let indices_is_call_procedure_hints = [
        1, 2, 3, 4, 5, 6, 7, 8, 9, 31, 32, 33, 34, 35, 36, 37, 38, 39,
    ];

    let mut is_call_procedure_hints_with_labels = vec![false; instructions.len()];
    for idx in indices_is_call_procedure_hints {
        is_call_procedure_hints_with_labels[idx] = true;
    }

    let (prom, _, _, frame_sizes) =
        get_full_prom_and_labels(&instructions, &is_call_procedure_hints_with_labels)
            .expect("Instructions were not formatted properly.");

    let mut pc = BinaryField32b::ONE;
    let mut pc_field_to_int = HashMap::new();
    for i in 0..prom.len() {
        pc_field_to_int.insert(pc, i as u32 + 1);
        pc *= G;
    }
    let a = rand::random();
    let b = rand::random();
    let vrom = ValueRom::new_with_init_vals(&[0, 0, a, b]);

    let memory = Memory::new(prom, vrom);
    let (trace, _) = ZCrayTrace::generate(memory, frame_sizes, pc_field_to_int)
        .expect("Trace generation should not fail.");

    assert_eq!(
        trace
            .get_vrom_u32(4)
            .expect("Return value for quotient not set."),
        a / b
    );
    assert_eq!(
        trace
            .get_vrom_u32(5)
            .expect("Return value for remainder not set."),
        a % b
    );
}
