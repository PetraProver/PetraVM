use petravm_asm::{isa::GenericISA, Assembler, Memory, PetraTrace, ValueRom};

#[test]
fn test_naive_div() {
    let compiled_program = Assembler::from_code(include_str!("../../examples/div.asm")).unwrap();

    let a = rand::random();
    let b = rand::random();
    let vrom = ValueRom::new_with_init_vals(&[0, 0, a, b]);

    let memory = Memory::new(compiled_program.prom, vrom);
    let (trace, _) = PetraTrace::generate(
        Box::new(GenericISA),
        memory,
        compiled_program.frame_sizes,
        compiled_program.pc_field_to_index_pc,
    )
    .expect("Trace generation should not fail.");

    assert_eq!(
        trace
            .vrom()
            .read::<u32>(4)
            .expect("Return value for quotient not set."),
        a / b
    );
    assert_eq!(
        trace
            .vrom()
            .read::<u32>(5)
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
    let full_kernel = kernel_files.join("\n");

    let compiled_program = Assembler::from_code(&full_kernel).unwrap();

    let a = 274; // -2 in 32-bit unsigned representation
    let b = 685;
    let vrom = ValueRom::new_with_init_vals(&[0, 0, a, b]);

    let memory = Memory::new(compiled_program.prom, vrom);
    let (trace, _) = PetraTrace::generate(
        Box::new(GenericISA),
        memory,
        compiled_program.frame_sizes,
        compiled_program.pc_field_to_index_pc,
    )
    .expect("Trace generation should not fail.");

    // gcd
    assert_eq!(
        trace
            .vrom()
            .read::<u32>(4)
            .expect("Return value for gcd not set."),
        137
    );
    // a's coefficient
    assert_eq!(
        trace
            .vrom()
            .read::<u32>(5)
            .expect("Return value for a coefficient not set."),
        (-2i32) as u32
    );
    // b's coefficient
    assert_eq!(
        trace
            .vrom()
            .read::<u32>(6)
            .expect("Return value for b coefficient not set."),
        1
    );
}

#[test]
fn test_non_tail_long_div() {
    let compiled_program =
        Assembler::from_code(include_str!("../../examples/non_tail_long_div.asm")).unwrap();

    let a = 54820;
    let b = 65;

    let vrom = ValueRom::new_with_init_vals(&[0, 0, a, b]);

    let memory = Memory::new(compiled_program.prom, vrom);
    let (trace, _) = PetraTrace::generate(
        Box::new(GenericISA),
        memory,
        compiled_program.frame_sizes,
        compiled_program.pc_field_to_index_pc,
    )
    .expect("Trace generation should not fail.");

    assert_eq!(
        trace
            .vrom()
            .read::<u32>(4)
            .expect("Return value for quotient not set."),
        a / b
    );
    assert_eq!(
        trace
            .vrom()
            .read::<u32>(5)
            .expect("Return value for remainder not set."),
        a % b
    );
}

#[test]
fn test_tail_long_div() {
    let compiled_program =
        Assembler::from_code(include_str!("../../examples/tail_long_div.asm")).unwrap();

    let a = rand::random();
    let b = rand::random();
    let vrom = ValueRom::new_with_init_vals(&[0, 0, a, b]);

    let memory = Memory::new(compiled_program.prom, vrom);
    let (trace, _) = PetraTrace::generate(
        Box::new(GenericISA),
        memory,
        compiled_program.frame_sizes,
        compiled_program.pc_field_to_index_pc,
    )
    .expect("Trace generation should not fail.");

    assert_eq!(
        trace
            .vrom()
            .read::<u32>(4)
            .expect("Return value for quotient not set."),
        a / b
    );
    assert_eq!(
        trace
            .vrom()
            .read::<u32>(5)
            .expect("Return value for remainder not set."),
        a % b
    );
}
