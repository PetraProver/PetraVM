#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::_rdtsc;

use crate::execution::InterpreterError;
use crate::Opcode;

pub const STAT_OP_COUNT: usize = 32;

/// List of all opcodes to benchmark
pub fn all_opcodes() -> &'static [Opcode] {
    &[
        Opcode::Fp,
        Opcode::Xor,
        Opcode::Xori,
        Opcode::B32Mul,
        Opcode::B32Muli,
        Opcode::B128Add,
        Opcode::B128Mul,
        Opcode::Add,
        Opcode::Addi,
        Opcode::Sub,
        Opcode::And,
        Opcode::Andi,
        Opcode::Or,
        Opcode::Ori,
        Opcode::Sll,
        Opcode::Slli,
        Opcode::Srl,
        Opcode::Srli,
        Opcode::Sra,
        Opcode::Srai,
        Opcode::Mul,
        Opcode::Muli,
        Opcode::Mulu,
        Opcode::Mulsu,
        Opcode::Slt,
        Opcode::Slti,
        Opcode::Sltu,
        Opcode::Sltiu,
        Opcode::Sle,
        Opcode::Slei,
        Opcode::Sleu,
        Opcode::Sleiu,
    ]
}

#[derive(Debug, Default, Clone, Copy)]
struct CycleStats {
    total_cycles: u64,
    count: u64,
}

impl CycleStats {
    #[cfg(target_arch = "x86_64")]
    fn record_time(&mut self, time: u64) {
        self.total_cycles += time;
        self.count += 1;
    }

    fn average_cycles(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.total_cycles as f64 / self.count as f64
        }
    }
}

#[derive(Debug)]
pub struct AllCycleStats {
    stats: [CycleStats; STAT_OP_COUNT],
}

impl AllCycleStats {
    pub(crate) fn new() -> Self {
        AllCycleStats {
            stats: [CycleStats::default(); STAT_OP_COUNT],
        }
    }

    #[cfg(target_arch = "x86_64")]
    pub(crate) fn record(
        &mut self,
        opcode: Opcode,
        f: impl FnOnce() -> Result<(), InterpreterError>,
    ) -> Result<(), InterpreterError> {
        let index = opcode as usize;
        let start = unsafe { _rdtsc() };
        let result = f();
        let end = unsafe { _rdtsc() };
        self.stats[index].record_time(end - start);
        result
    }

    #[cfg(not(target_arch = "x86_64"))]
    pub(crate) fn record(
        &mut self,
        _opcode: Opcode,
        f: impl FnOnce() -> Result<(), InterpreterError>,
    ) -> Result<(), InterpreterError> {
        f()
    }

    pub(crate) fn average_cycles(&self) -> Vec<(Opcode, f64)> {
        (0..all_opcodes().len())
            .map(|index| {
                let opcode = all_opcodes()[index];
                (opcode, self.stats[index].average_cycles())
            })
            .collect()
    }
}
