use binius_field::{BinaryField16b, BinaryField32b, ExtensionField, Field, PackedField};
use std::collections::HashMap;
use zcrayvm_assembly::{
    code_to_prom, parse_program, get_full_prom_and_labels,
    Memory, Opcode, ProgramRom, ValueRom, ZCrayTrace,
};

// Utility function from the original main.rs
fn get_binary_slot(i: u16) -> BinaryField16b {
    BinaryField16b::new(i)
}

#[test]
fn test_collatz_program() {
    // Step 1: Parse the Collatz program
    let instructions = parse_program(include_str!("../../examples/collatz.asm")).unwrap();

    // Set call procedure hints for the program with labels
    let mut is_call_procedure_hints_with_labels = vec![false; instructions.len()];
    let indices_to_set_with_labels = vec![9, 10, 11, 15, 16, 17];
    for idx in indices_to_set_with_labels {
        is_call_procedure_hints_with_labels[idx] = true;
    }
    
    // Generate the PROM and associated data
    let (prom, labels, pc_field_to_int) = get_full_prom_and_labels(
        &instructions, 
        &is_call_procedure_hints_with_labels
    ).expect("Instructions were not formatted properly.");

    // Step 2: Verify that the parsing produced the expected PROM structure
    let zero = BinaryField16b::zero();
    let collatz = BinaryField16b::ONE;
    let g = BinaryField32b::MULTIPLICATIVE_GENERATOR;
    let case_recurse = ExtensionField::<BinaryField16b>::iter_bases(&g.pow(4))
        .collect::<Vec<BinaryField16b>>();
    let case_odd = ExtensionField::<BinaryField16b>::iter_bases(&g.pow(10))
        .collect::<Vec<BinaryField16b>>();

    // Define the expected PROM
    let expected_instructions = vec![
        // collatz:
        [
            Opcode::Xori.get_field_elt(),
            get_binary_slot(5),
            get_binary_slot(2),
            get_binary_slot(1),
        ], //  0G: XORI 5 2 1
        [
            Opcode::Bnz.get_field_elt(),
            get_binary_slot(5),
            case_recurse[0],
            case_recurse[1],
        ], //  1G: BNZ 5 case_recurse
        // case_return:
        [
            Opcode::Xori.get_field_elt(),
            get_binary_slot(3),
            get_binary_slot(2),
            zero,
        ], //  2G: XORI 3 2 zero
        [Opcode::Ret.get_field_elt(), zero, zero, zero], //  3G: RET
        // case_recurse:
        [
            Opcode::Andi.get_field_elt(),
            get_binary_slot(6),
            get_binary_slot(2),
            get_binary_slot(1),
        ], // 4G: ANDI 6 2 1
        [
            Opcode::Bnz.get_field_elt(),
            get_binary_slot(6),
            case_odd[0],
            case_odd[1],
        ], //  5G: BNZ 6 case_odd
        // case_even:
        [
            Opcode::Srli.get_field_elt(),
            get_binary_slot(8),
            get_binary_slot(2),
            get_binary_slot(1),
        ], //  6G: SRLI 8 2 1
        [
            Opcode::MVVW.get_field_elt(),
            get_binary_slot(4),
            get_binary_slot(2),
            get_binary_slot(8),
        ], //  7G: MVV.W @4[2], @8
        [
            Opcode::MVVW.get_field_elt(),
            get_binary_slot(4),
            get_binary_slot(3),
            get_binary_slot(3),
        ], //  8G: MVV.W @4[3], @3
        [
            Opcode::Taili.get_field_elt(),
            collatz,
            zero,
            get_binary_slot(4),
        ], // 9G: TAILI collatz 4
        // case_odd:
        [
            Opcode::Muli.get_field_elt(),
            get_binary_slot(7),
            get_binary_slot(2),
            get_binary_slot(3),
        ], //  10G: MULI 7 2 3
        [
            Opcode::Addi.get_field_elt(),
            get_binary_slot(8),
            get_binary_slot(7),
            get_binary_slot(1),
        ], //  11G: ADDI 8 7 1
        [
            Opcode::MVVW.get_field_elt(),
            get_binary_slot(4),
            get_binary_slot(2),
            get_binary_slot(8),
        ], //  12G: MVV.W @4[2], @8
        [
            Opcode::MVVW.get_field_elt(),
            get_binary_slot(4),
            get_binary_slot(3),
            get_binary_slot(3),
        ], //  13G: MVV.W @4[3], @3
        [
            Opcode::Taili.get_field_elt(),
            collatz,
            zero,
            get_binary_slot(4),
        ], //  14G: TAILI collatz 4
    ];

    // Set call procedure hints for the expected PROM
    let mut is_call_procedure_hints = vec![false; expected_instructions.len()];
    let indices_to_set = vec![7, 8, 9, 12, 13, 14];
    for idx in indices_to_set {
        is_call_procedure_hints[idx] = true;
    }
    let expected_prom = code_to_prom(&expected_instructions, &is_call_procedure_hints);

    // Validate that the parsed PROM matches the expected PROM
    assert_eq!(
        prom.len(), 
        expected_prom.len(),
        "Not identical number of instructions in PROM ({:?}) and expected PROM ({:?})",
        prom.len(),
        expected_prom.len()
    );

    for (i, inst) in prom.iter().enumerate() {
        let expected_inst = &expected_prom[i];
        assert_eq!(
            *inst, 
            *expected_inst,
            "Value for index {:?} in PROM is {:?} but is {:?} in expected PROM",
            i, inst, expected_inst
        );
    }

    // Step 3: Test the execution of the Collatz program
    let initial_values = [5, 27, 3999];
    for &initial_value in &initial_values {
        // Set up frame sizes for the program
        let mut frame_sizes = HashMap::new();
        frame_sizes.insert(BinaryField32b::ONE, 9);
        
        // Initialize the VROM with the initial value
        let vrom = ValueRom::new_with_init_vals(&[0, 0, initial_value]);
        let memory = Memory::new(prom.clone(), vrom);

        // Execute the program and generate the trace
        let (trace, boundary_values) = ZCrayTrace::generate(
            memory, 
            frame_sizes, 
            pc_field_to_int.clone()
        ).expect("Trace generation should not fail.");

        // Validate the trace
        trace.validate(boundary_values);

        // Calculate the expected sequence of even and odd values
        let (expected_evens, expected_odds) = collatz_orbits(initial_value);

        // Verify the trace has the correct number of even and odd cases
        assert_eq!(
            trace.shift.len(), 
            expected_evens.len(),
            "Generated an incorrect number of even cases for initial value {}", 
            initial_value
        );
        
        assert_eq!(
            trace.muli.len(), 
            expected_odds.len(),
            "Generated an incorrect number of odd cases for initial value {}", 
            initial_value
        );

        // Verify the trace has the correct values at each step
        for (i, &even) in expected_evens.iter().enumerate() {
            assert_eq!(
                trace.shift[i].src_val, 
                even,
                "Incorrect input to an even case for initial value {}", 
                initial_value
            );
        }

        for (i, &odd) in expected_odds.iter().enumerate() {
            assert_eq!(
                trace.muli[i].src_val, 
                odd,
                "Incorrect input to an odd case for initial value {}", 
                initial_value
            );
        }

        // Verify the final result is 1
        assert_eq!(
            trace.get_vrom_u32(3).unwrap(), 
            1,
            "Final result should be 1 for initial value {}", 
            initial_value
        );
    }
}

// Helper to calculate the expected Collatz sequence
fn collatz_orbits(initial_val: u32) -> (Vec<u32>, Vec<u32>) {
    let mut cur_value = initial_val;
    let mut evens = vec![];
    let mut odds = vec![];
    while cur_value != 1 {
        if cur_value % 2 == 0 {
            evens.push(cur_value);
            cur_value /= 2;
        } else {
            odds.push(cur_value);
            cur_value = 3 * cur_value + 1;
        }
    }
    (evens, odds)
}
