mod instruction_args;
mod instructions_with_labels;

use instructions_with_labels::{InstructionsWithLabels, parse_instructions};

fn main() {
    let instructions = parse_instructions(include_str!("../../examples/fib.asm")).unwrap();
    for instr in instructions {
        if matches!(instr, InstructionsWithLabels::Label(_)) {
            println!("\n{instr}");
        } else {
            println!("    {instr}");
        }
    }
}
