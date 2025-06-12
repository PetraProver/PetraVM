#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::_rdtsc;

use strum::EnumCount;

#[cfg(not(target_arch = "x86_64"))]
use crate::execution::InterpreterError;
use crate::Opcode;

#[derive(Debug, Default, Clone, Copy)]
struct CycleStats {
    total_cycles: u64,
    count: u64,
}

impl CycleStats {
    #[cfg(target_arch = "x86_64")]
    fn record_time(&mut self, time: usize) {
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
    stats: [CycleStats; Opcode::COUNT as usize],
}

impl AllCycleStats {
    pub(crate) fn new() -> Self {
        AllCycleStats {
            stats: [CycleStats::default(); Opcode::COUNT as usize],
        }
    }

    #[cfg(target_arch = "x86_64")]
    pub(crate) fn record(
        &mut self,
        opcode: Opcode,
        f: impl FnOnce() -> Result<(), InterpreterError>,
    ) -> Result<(), InterpreterError> {
        let start = unsafe { _rdtsc() };
        let result = f();
        let end = unsafe { _rdtsc() };
        self.states[index].record_time(end - start);
        self.states[index].count += 1;
        result
    }

    #[cfg(not(target_arch = "x86_64"))]
    pub(crate) fn record(
        &mut self,
        _opcode: Opcode,
        _f: impl FnOnce() -> Result<(), InterpreterError>,
    ) -> Result<(), InterpreterError> {
        Ok(())
    }

    pub(crate) fn average_cycles(&self) {
        for index in 0..Opcode::COUNT as usize {
            if self.stats[index].count == 0 {
                continue;
            }
            let opcode = Opcode::try_from(index as u16).expect("Invalid opcode index");
            println!(
                "Opcode: {:?}, Average Cycles: {:.2}",
                opcode,
                self.stats[index].average_cycles()
            );
        }
    }
}
