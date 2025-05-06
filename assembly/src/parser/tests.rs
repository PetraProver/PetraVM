#[cfg(test)]
mod test_parser {

    use binius_field::{ExtensionField, Field, PackedField};
    use binius_m3::builder::B16;
    use pest::Parser;

    use crate::execution::G;
    use crate::opcodes::Opcode;
    use crate::parser::InstructionsWithLabels;
    use crate::parser::{parse_program, AsmParser, Rule};
    use crate::util::code_to_prom;
    use crate::util::get_binary_slot;
    use crate::Assembler;

    fn ensure_parser_succeeds(rule: Rule, asm: &str) {
        let parser = AsmParser::parse(rule, asm);
        assert!(parser.is_ok(), "assembly failed to parse: {}", asm);
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
        let collatz = B16::ONE;
        let case_recurse = ExtensionField::<B16>::iter_bases(&G.pow(4)).collect::<Vec<B16>>();
        let case_odd = ExtensionField::<B16>::iter_bases(&G.pow(10)).collect::<Vec<B16>>();

        let compiled_program =
            Assembler::from_code(include_str!("../../../examples/collatz.asm")).unwrap();

        let zero = B16::zero();

        let expected_prom = vec![
            // collatz:
            [
                Opcode::Xori.get_field_elt(),
                get_binary_slot(5),
                get_binary_slot(2),
                get_binary_slot(1),
            ], //  0G: XORI @5, @2, #1
            [
                Opcode::Bnz.get_field_elt(),
                case_recurse[0],
                case_recurse[1],
                get_binary_slot(5),
            ], //  1G: BNZ case_recurse, @5
            // case_return:
            [
                Opcode::Xori.get_field_elt(),
                get_binary_slot(3),
                get_binary_slot(2),
                zero,
            ], //  2G: XORI @3, @2, #0
            [Opcode::Ret.get_field_elt(), zero, zero, zero], //  3G: RET
            // case_recurse:
            [
                Opcode::Andi.get_field_elt(),
                get_binary_slot(6),
                get_binary_slot(2),
                get_binary_slot(1),
            ], // 4G: ANDI @6, @2, #1
            [
                Opcode::Bnz.get_field_elt(),
                case_odd[0],
                case_odd[1],
                get_binary_slot(6),
            ], //  5G: BNZ case_odd, @6
            // case_even:
            [
                Opcode::Srli.get_field_elt(),
                get_binary_slot(7),
                get_binary_slot(2),
                get_binary_slot(1),
            ], //  6G: SRLI @7, @2, #1
            [
                Opcode::Mvvw.get_field_elt(),
                get_binary_slot(4),
                get_binary_slot(2),
                get_binary_slot(7),
            ], //  7G: MVV.W @4[2], @7
            [
                Opcode::Mvvw.get_field_elt(),
                get_binary_slot(4),
                get_binary_slot(3),
                get_binary_slot(3),
            ], //  8G: MVV.W @4[3], @3
            [
                Opcode::Taili.get_field_elt(),
                collatz,
                zero,
                get_binary_slot(4),
            ], // 9G: TAILI collatz, @4
            // case_odd:
            [
                Opcode::Muli.get_field_elt(),
                get_binary_slot(8),
                get_binary_slot(2),
                get_binary_slot(3),
            ], //  10G: MULI @8, @2, #3
            [
                Opcode::Addi.get_field_elt(),
                get_binary_slot(7),
                get_binary_slot(8),
                get_binary_slot(1),
            ], //  11G: ADDI @7, @8, #1
            [
                Opcode::Mvvw.get_field_elt(),
                get_binary_slot(4),
                get_binary_slot(2),
                get_binary_slot(7),
            ], //  12G: MVV.W @4[2], @7
            [
                Opcode::Mvvw.get_field_elt(),
                get_binary_slot(4),
                get_binary_slot(3),
                get_binary_slot(3),
            ], //  13G: MVV.W @4[3], @3
            [
                Opcode::Taili.get_field_elt(),
                collatz,
                zero,
                get_binary_slot(4),
            ], //  14G: TAILI collatz, @4
        ];

        let expected_prom = code_to_prom(&expected_prom);

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
                "Value for index {:?} in PROM is {:?} but is {:?} in expected PROM",
                i, inst, expected_inst
            );
        }
    }
}
