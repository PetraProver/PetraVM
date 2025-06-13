pub mod common;

use binius_field::BinaryField;
use binius_field::PackedField;
use binius_m3::builder::B32;
use petravm_asm::{isa::GenericISA, Assembler, Memory, PetraTrace, ValueRom};

#[test]
fn test_exception_integration() {
    // Set initial value
    let a = 100;
    let b = 0; // This causes an exception.

    let kernel_files = [
        include_str!("../../examples/exception.asm"),
        include_str!("../../examples/div.asm"),
    ];
    let full_kernel = kernel_files.join("\n");

    let compiled_program = Assembler::from_code(&full_kernel).unwrap();

    let vrom = ValueRom::new_with_init_vals(&[0, 0, a, b]);

    let memory = Memory::new(compiled_program.prom, vrom);
    let (trace, boundary_values) = PetraTrace::generate(
        Box::new(GenericISA),
        memory,
        compiled_program.frame_sizes,
        compiled_program.pc_field_to_index_pc,
    )
    .expect("Trace generation should not fail.");

    let exception_fp = boundary_values.final_fp;

    // Validate the trace
    trace.validate(boundary_values);

    // Use the multiplicative generator G for calculations
    const G: B32 = B32::MULTIPLICATIVE_GENERATOR;

    // TRAP PC
    assert_eq!(
        trace
            .vrom()
            .read::<u32>(*exception_fp)
            .expect("TRAP PC is not set."),
        G.pow(8).val()
    );
    // TRAP FP
    assert_eq!(
        trace
            .vrom()
            .read::<u32>(exception_fp.addr(1u32))
            .expect("TRAP FP is not set."),
        8
    );
    // Exception code
    assert_eq!(
        trace
            .vrom()
            .read::<u32>(exception_fp.addr(2u32))
            .expect("Exception code is not set."),
        3
    );
}
