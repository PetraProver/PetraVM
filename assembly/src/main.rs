mod emulator;
mod event;
mod instruction_args;
mod instructions_with_labels;

use binius_field::{BinaryField16b, Field, PackedField};
use emulator::Opcode;
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
    let prom = prom;

    let frame_sizes = get_frame_sizes_all_labels(&prom, labels);
    println!("frame sizes {:?}", frame_sizes);

    let zero = BinaryField16b::zero();
    let collatz = BinaryField16b::ONE;
    let case_recurse = BinaryField16b::new(5);
    let case_odd = BinaryField16b::new(11);
    let expected_prom = vec![
        // collatz:
        [
            Opcode::Xori.get_field_elt(),
            get_binary_slot(5),
            get_binary_slot(2),
            get_binary_slot(1),
        ], //  1: XORI 5 2 1
        [
            Opcode::Bnz.get_field_elt(),
            get_binary_slot(5),
            zero,
            case_recurse,
        ], //  2: BNZ 5 case_recurse
        // case_return:
        [
            Opcode::Xori.get_field_elt(),
            get_binary_slot(3),
            get_binary_slot(2),
            zero,
        ], //  3: XORI 3 2 zero
        [Opcode::Ret.get_field_elt(), zero, zero, zero], //  4: RET
        // case_recurse:
        [
            Opcode::Andi.get_field_elt(),
            get_binary_slot(6),
            get_binary_slot(2),
            get_binary_slot(1),
        ], //  5: ANDI 6 2 1
        [
            Opcode::Bnz.get_field_elt(),
            get_binary_slot(6),
            zero,
            case_odd,
        ], //  6: BNZ 6 case_odd 0 0
        // case_even:
        [
            Opcode::Srli.get_field_elt(),
            get_binary_slot(8),
            get_binary_slot(2),
            get_binary_slot(1),
        ], //  7: SRLI 8 2 1
        [
            Opcode::MVVW.get_field_elt(),
            get_binary_slot(4),
            get_binary_slot(2),
            get_binary_slot(8),
        ], //  8: MVV.W @4[2], @8
        [
            Opcode::MVVW.get_field_elt(),
            get_binary_slot(4),
            get_binary_slot(3),
            get_binary_slot(3),
        ], //  9: MVV.W @4[3], @3
        [
            Opcode::Taili.get_field_elt(),
            zero,
            collatz,
            get_binary_slot(4),
        ], // 10: TAILI collatz 4 0
        // case_odd:
        [
            Opcode::Muli.get_field_elt(),
            get_binary_slot(7),
            get_binary_slot(2),
            get_binary_slot(3),
        ], //  11: MULI 7 2 3
        [
            Opcode::Addi.get_field_elt(),
            get_binary_slot(8),
            get_binary_slot(7),
            get_binary_slot(1),
        ], //  12: ADDI 8 7 1
        [
            Opcode::MVVW.get_field_elt(),
            get_binary_slot(4),
            get_binary_slot(2),
            get_binary_slot(8),
        ], //  13: MVV.W @4[2], @7
        [
            Opcode::MVVW.get_field_elt(),
            get_binary_slot(4),
            get_binary_slot(3),
            get_binary_slot(3),
        ], //  14: MVV.W @4[3], @3
        [
            Opcode::Taili.get_field_elt(),
            zero,
            collatz,
            get_binary_slot(4),
        ], //  15: TAILI collatz 4 0
    ];

    assert!(prom == expected_prom);
}
