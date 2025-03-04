mod emulator;
mod event;
mod instruction_args;
mod instructions_with_labels;

use std::collections::HashMap;

use binius_field::{BinaryField16b, ExtensionField, Field, PackedField};
use emulator::{code_to_prom, Opcode, ValueRom, ZCrayTrace, G};
use instructions_with_labels::{
    get_frame_sizes_all_labels, get_full_prom_and_labels, parse_instructions,
};

pub(crate) fn get_binary_slot(i: u16) -> BinaryField16b {
    BinaryField16b::new(i)
}

fn main() {
    let instructions = parse_instructions(include_str!("../../examples/collatz.asm")).unwrap();

    let (prom, labels) =
        get_full_prom_and_labels(&instructions).expect("Instructions were not formatted properly.");

    let zero = BinaryField16b::zero();
    let collatz = BinaryField16b::ONE;
    let case_recurse =
        ExtensionField::<BinaryField16b>::iter_bases(&G.pow(4)).collect::<Vec<BinaryField16b>>();
    let case_odd =
        ExtensionField::<BinaryField16b>::iter_bases(&G.pow(10)).collect::<Vec<BinaryField16b>>();

    let mut labels_args = HashMap::new();
    labels_args.insert(collatz.into(), 3);
    let frame_sizes = get_frame_sizes_all_labels(&prom, labels, labels_args);
    println!("frame sizes {:?}", frame_sizes);

    let expected_prom = vec![
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
        ], //  5G: BNZ 6 case_odd 0 0
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
        ], // 9G: TAILI collatz 4 0
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
        ], //  12G: MVV.W @4[2], @7
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
        ], //  14G: TAILI collatz 4 0
    ];

    let expected_prom = code_to_prom(&expected_prom);

    assert!(
        prom.len() == expected_prom.len(),
        "Not identical number of instructions in PROM ({:?}) and expected PROM ({:?})",
        prom.len(),
        expected_prom.len()
    );

    for (key, val) in prom.iter() {
        let expected_val = expected_prom.get(&key).expect("Extra value in prom");
        assert_eq!(
            *val, *expected_val,
            "Value for key {:?} in PROM is {:?} but is {:?} in expected PROM",
            key, val, expected_val
        );
    }

    let initial_value = 3999;
    let vrom = ValueRom::new(vec![0, 0, initial_value]);
    let _ = ZCrayTrace::generate_with_vrom(prom, vrom, frame_sizes)
        .expect("Trace generation should not fail.");
}
