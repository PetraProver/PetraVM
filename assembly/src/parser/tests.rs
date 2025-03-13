#[cfg(test)]
mod test_parser {
    use pest::Parser;

    use crate::parser::InstructionsWithLabels;
    use crate::parser::{parse_line, parse_program, AsmParser, Rule};

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
            if matches!(instr, InstructionsWithLabels::Label(_)) {
                println!("\n{instr}");
            } else {
                println!("    {instr}");
            }
        }
    }

    #[test]
    fn test_all_instructions() {
        let lines = [
            "label:",
            "XOR @4, @3, @2",
            "B32_ADD @4, @3, @2",
            "B32_MUL @4, @3, @2",
            "B128_ADD @4, @3, @2",
            "B128_MUL @4, @3, @2",
            "ADD @3, @2, @1",
            "SUB @3, @2, @1",
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
            "B32_MULI @3, @2, #1",
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
}
