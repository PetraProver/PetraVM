mod emulator;
mod event;
mod instruction_args;
mod instructions_with_labels;

use emulator::Opcode;
use instructions_with_labels::{
    get_frame_sizes_all_labels, get_full_prom_and_labels, parse_instructions,
};

fn main() {
    let instructions = parse_instructions(include_str!("../../examples/collatz.asm")).unwrap();

    let (prom, labels) =
        get_full_prom_and_labels(&instructions).expect("Instructions were not formatted properly.");
    let prom = prom;

    let frame_sizes = get_frame_sizes_all_labels(&prom, labels);
    println!("frame sizes for collatz {:?}", frame_sizes);

    let collatz = 1;
    let case_recurse = 5;
    let case_odd = 11;
    let expected_prom = vec![
        // collatz:
        [Opcode::Xori as u16, 4, 2, 1],           //  1: XORI 4 2 1
        [Opcode::Bnz.into(), 4, case_recurse, 0], //  2: BNZ 4 case_recurse
        // case_return:
        [Opcode::Xori.into(), 3, 2, 0], //  3: XORI 3 2 0
        [Opcode::Ret.into(), 0, 0, 0],  //  4: RET
        // case_recurse:
        [Opcode::Andi.into(), 5, 2, 1],       //  5: ANDI 5 2 1
        [Opcode::Bnz.into(), 5, case_odd, 0], //  6: BNZ 5 case_odd 0 0
        // case_even:
        [Opcode::Srli.into(), 7, 2, 1],        //  7: SRLI 7 2 1
        [Opcode::MVVW.into(), 8, 2, 7],        //  8: MVV.W @8[2], @7
        [Opcode::MVVW.into(), 8, 3, 3],        //  9: MVV.W @8[3], @3
        [Opcode::Taili.into(), collatz, 8, 0], // 10: TAILI collatz 8 0
        // case_odd:
        [Opcode::Muli.into(), 6, 2, 3],        //  11: MULI 6 2 3
        [Opcode::Addi.into(), 7, 6, 1],        //  12: ADDI 7 6 1
        [Opcode::MVVW.into(), 8, 2, 7],        //  13: MVV.W @8[2], @7
        [Opcode::MVVW.into(), 8, 3, 3],        //  14: MVV.W @8[3], @3
        [Opcode::Taili.into(), collatz, 8, 0], //  15: TAILI collatz 8 0
    ];

    assert!(prom == expected_prom);
}
