#[cfg(test)]
mod test_parser {

    use binius_field::{ExtensionField, PackedField};
    use binius_m3::builder::B16;
    use pest::Parser;

    use crate::execution::G;
    use crate::opcodes::Opcode;
    use crate::parser::InstructionsWithLabels;
    use crate::parser::{parse_program, AsmParser, Rule};
    use crate::test_util::code_to_prom;
    use crate::test_util::get_binary_slot;
    use crate::Assembler;

    fn ensure_parser_succeeds(rule: Rule, asm: &str) {
        let parser = AsmParser::parse(rule, asm);
        assert!(parser.is_ok(), "assembly failed to parse: {asm}");
    }

    fn ensure_parser_fails(rule: Rule, asm: &str) {
        let parser = AsmParser::parse(rule, asm);
        assert!(parser.is_err());
    }

    #[test]
    fn test_simple_lines() {
        let ok_instrs = [
            "Somelabel:\n",
            "Somelabel: BNZ @4, Somelabel\n",
            "Somelabel:\n BNZ @4, Somelabel @4\n",
            "J  label ;; Some comment J nowhere\n",
            "J\tlabel\n",
            "RET\n\n",
            ";; Just a comment\n",
            "BNZ case_recurse, @4 ;; branch if n == 1\n",
            "MULU @4, @3, @1\n",
            "SLLI @4, @3, #1\n",
        ];
        for asm in ok_instrs {
            ensure_parser_succeeds(Rule::line, asm);
        }

        let err_instrs = [
            "J\n",
            "J \n label\n",
            "BNZ \n Somelabel @4\n",
            "Jlabel\n",
            "",
            "Random line\n",
        ];
        for asm in err_instrs {
            ensure_parser_fails(Rule::line, asm);
        }
    }

    #[test]
    fn test_simple_program() {
        let ok_programs = [
            "_start: RET",
            "_start: RET ;; Some comment",
            "_start: \n RET",
            "_start: BNZ case_recurse, @4 ;; branch if n == 1\n",
            "_start: ;; Some comment\n BNZ case_recurse, @4 ;; branch if n == 1\n",
            "\n\t_start:\n\t\t\n RET",
        ];
        for asm in ok_programs {
            ensure_parser_succeeds(Rule::program, asm);
        }

        let err_programs = [
            "",
            "RET\n\n",
            "_start: BNZ @4, case_recurse ;; branch if n == 1\n",
            "RET\n_start:",
            "RET ;; Some comment",
            "_start:",
        ];
        for asm in err_programs {
            ensure_parser_fails(Rule::program, asm);
        }
    }

    #[test]
    fn test_parsing() {
        let code = include_str!("../../../examples/fib.asm");
        let instrs = parse_program(code).unwrap();
        for instr in instrs {
            if matches!(instr, InstructionsWithLabels::Label(_, _)) {
                println!("\n{instr}");
            } else {
                println!("    {instr}");
            }
        }
    }

    #[test]
    fn test_simple_jump() {
        let code = "_start: J label\nJ @4\n";
        let instrs = parse_program(code).unwrap();
        assert!(matches!(&instrs[1], InstructionsWithLabels::Jumpi { label } if label == "label"));
        assert!(
            matches!(&instrs[2], InstructionsWithLabels::Jumpv { offset } if offset.to_string() == "@4")
        );
    }

    #[test]
    fn test_prover_flag() {
        parse_program(include_str!("../../../examples/bezout.asm")).unwrap();
    }

    #[test]
    fn test_all_instructions() {
        let lines = [
            "#[framesize(0x1a)] label:",
            "label:",
            "XOR @4, @3, @2",
            "B32_ADD @4, @3, @2",
            "B32_MUL @4, @3, @2",
            "B32_MULI @3, @2, #1",
            "B128_ADD @4, @3, @2",
            "B128_MUL @4, @3, @2",
            "ADD @3, @2, @1",
            "SUB @3, @2, @1",
            "SLE @3, @2, @1",
            "SLEI @3, @2, #1",
            "SLEU @3, @2, @1",
            "SLEIU @3, @2, #1",
            "SLT @3, @2, @1",
            "SLTU @3, @2, @1",
            "AND @3, @2, @1",
            "OR @3, @2, @1",
            "SLL @3, @2, @1",
            "SRL @3, @2, @1",
            "SRA @3, @2, @1",
            "MUL @3, @2, @1",
            "MULU @3, @2, @1",
            "MULSU @3, @2, @1",
            "XORI @3, @2, #1",
            "B32_ADDI @3, @2, #1",
            "ADDI @3, @2, #1",
            "SLTI @3, @2, #1",
            "SLTIU @3, @2, #1",
            "ANDI @3, @2, #1",
            "ORI @3, @2, #1",
            "SLLI @3, @2, #1",
            "SRLI @3, @2, #1",
            "SRAI @3, @2, #1",
            "MULI @3, @2, #1",
            "LW @3, @2, #1",
            "SW @3, @2, #1",
            "LB @3, @2, #1",
            "LBU @3, @2, #1",
            "LH @3, @2, #1",
            "LHU @3, @2, #1",
            "SB @3, @2, #1",
            "SH @3, @2, #1",
            "MVV.W @3[4], @2",
            "MVV.L @3[4], @2",
            "MVI.H @3[4], #2",
            "LDI.W @3, #2",
            "RET",
            "J label",
            "J @4",
            "CALLI label, @4",
            "TAILI label, @4",
            "BNZ label, @4",
            "CALLV @5, @3",
            "TAILV @5, @3",
        ];
        for line in lines {
            ensure_parser_succeeds(Rule::line, line);
        }
    }

    #[test]
    fn test_parsing_collatz() {
        let collatz_prom_index = 5;
        let collatz_advice = 5;
        let collatz = ExtensionField::<B16>::iter_bases(&G.pow((collatz_advice - 1) as u64))
            .collect::<Vec<B16>>();
        let case_recurse_prom_index = 9;
        let case_recurse_advice = 9;
        let case_recurse =
            ExtensionField::<B16>::iter_bases(&G.pow((case_recurse_advice - 1) as u64))
                .collect::<Vec<B16>>();
        let case_odd_prom_index = 16;
        let case_odd_advice = 15;
        let case_odd = ExtensionField::<B16>::iter_bases(&G.pow((case_odd_advice - 1) as u64))
            .collect::<Vec<B16>>();

        let compiled_program =
            Assembler::from_code(include_str!("../../../examples/collatz.asm")).unwrap();

        let zero = B16::zero();

        let expected_prom = [
            // collatz_main:
            [
                Opcode::Fp.get_field_elt(),
                get_binary_slot(3),
                4.into(),
                zero,
            ], // 0G: FP @3, #4
            [
                Opcode::Alloci.get_field_elt(),
                get_binary_slot(5),
                10.into(),
                zero,
            ], // ALLOCI! @2, @1, #0
            [
                Opcode::Mvvw.get_field_elt(),
                get_binary_slot(5),
                get_binary_slot(2),
                get_binary_slot(2),
            ], // 1G: MVV.W @5[2], @2
            [
                Opcode::Mvvw.get_field_elt(),
                get_binary_slot(5),
                get_binary_slot(3),
                get_binary_slot(3),
            ], // 2G: MVV.W @5[3], @3
            [
                Opcode::Taili.get_field_elt(),
                collatz[0],
                collatz[1],
                get_binary_slot(5),
            ], //  3G: TAILI collatz, @5
            // collatz:
            [
                Opcode::Xori.get_field_elt(),
                get_binary_slot(5),
                get_binary_slot(2),
                get_binary_slot(1),
            ], //  4G: XORI @5, @2, #1
            [
                Opcode::Bnz.get_field_elt(),
                case_recurse[0],
                case_recurse[1],
                get_binary_slot(5),
            ], //  5G: BNZ case_recurse, @5
            // case_return:
            [
                Opcode::Mvvw.get_field_elt(),
                get_binary_slot(3),
                zero,
                get_binary_slot(2),
            ], //  6G: XORI @3, @2, #0
            [Opcode::Ret.get_field_elt(), zero, zero, zero], //  7G: RET
            // case_recurse:
            [
                Opcode::Andi.get_field_elt(),
                get_binary_slot(6),
                get_binary_slot(2),
                get_binary_slot(1),
            ], // 8G: ANDI @6, @2, #1
            [
                Opcode::Alloci.get_field_elt(),
                get_binary_slot(4),
                10.into(),
                zero,
            ], // ALLOCI! @4, #10
            [
                Opcode::Bnz.get_field_elt(),
                case_odd[0],
                case_odd[1],
                get_binary_slot(6),
            ], //  9G: BNZ case_odd, @6
            // case_even:
            [
                Opcode::Srli.get_field_elt(),
                get_binary_slot(7),
                get_binary_slot(2),
                get_binary_slot(1),
            ], //  10G: SRLI @7, @2, #1
            [
                Opcode::Mvvw.get_field_elt(),
                get_binary_slot(4),
                get_binary_slot(2),
                get_binary_slot(7),
            ], //  11G: MVV.W @4[2], @7
            [
                Opcode::Mvvw.get_field_elt(),
                get_binary_slot(4),
                get_binary_slot(3),
                get_binary_slot(3),
            ], //  12G: MVV.W @4[3], @3
            [
                Opcode::Taili.get_field_elt(),
                collatz[0],
                collatz[1],
                get_binary_slot(4),
            ], // 13G: TAILI collatz, @4
            // case_odd:
            [
                Opcode::Muli.get_field_elt(),
                get_binary_slot(8),
                get_binary_slot(2),
                get_binary_slot(3),
            ], //  14G: MULI @8, @2, #3
            [
                Opcode::Addi.get_field_elt(),
                get_binary_slot(7),
                get_binary_slot(8),
                get_binary_slot(1),
            ], //  15G: ADDI @7, @8, #1
            [
                Opcode::Mvvw.get_field_elt(),
                get_binary_slot(4),
                get_binary_slot(2),
                get_binary_slot(7),
            ], //  16G: MVV.W @4[2], @7
            [
                Opcode::Mvvw.get_field_elt(),
                get_binary_slot(4),
                get_binary_slot(3),
                get_binary_slot(3),
            ], //  17G: MVV.W @4[3], @3
            [
                Opcode::Taili.get_field_elt(),
                collatz[0],
                collatz[1],
                get_binary_slot(4),
            ], //  18G: TAILI collatz, @4
        ];

        // Add `prover_only` flags to the instructions.
        let expected_prom_prover_only = expected_prom
            .iter()
            .map(|inst| {
                if inst[0].val() == Opcode::Alloci.get_field_elt().val() {
                    (*inst, true) // Alloci is the only prover-only instruction
                                  // in this program
                } else {
                    (*inst, false)
                }
            })
            .collect::<Vec<_>>();

        let mut expected_prom = code_to_prom(&expected_prom_prover_only);

        // Set the expected advice for the first TAILI
        expected_prom[4].advice = Some((collatz_prom_index, collatz_advice));
        // Set the expected advice for BNZ
        expected_prom[6].advice = Some((case_recurse_prom_index, case_recurse_advice));
        // Set the expected advice for the second BNZ
        expected_prom[11].advice = Some((case_odd_prom_index, case_odd_advice));
        // Set the expected advice for the second TAILI
        expected_prom[15].advice = Some((collatz_prom_index, collatz_advice));
        // Set the expected advice for the third TAILI
        expected_prom[20].advice = Some((collatz_prom_index, collatz_advice));

        assert!(
            compiled_program.prom.len() == expected_prom.len(),
            "Not identical number of instructions in PROM ({:?}) and expected PROM ({:?})",
            compiled_program.prom.len(),
            expected_prom.len()
        );

        for (i, inst) in compiled_program.prom.iter().enumerate() {
            let expected_inst = &expected_prom[i];
            assert_eq!(
                *inst, *expected_inst,
                "Value for index {i:?} in PROM is {inst:?} but is {expected_inst:?} in expected PROM"
            );
        }
    }
}
