#[cfg(test)]
mod test_parser {
    use pest::Parser;

    use crate::parser::InstructionsWithLabels;
    use crate::parser::{parse_line, parse_program, AsmParser, Rule};

    fn ensure_parser_succeeds(rule: Rule, asm: &str) {
        let parser = AsmParser::parse(rule, asm);
        assert!(parser.is_ok());
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
}
