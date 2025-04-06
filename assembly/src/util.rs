use binius_field::Field;
use binius_m3::builder::{B16, B32};
use tracing_subscriber::EnvFilter;

use crate::execution::G;
use crate::execution::{Instruction, InterpreterInstruction};
use crate::memory::ProgramRom;

/// Initializes the global tracing subscriber.
///
/// The default `Level` is `INFO`. It can be overriden with `RUSTFLAGS`.
pub fn init_logger() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

#[inline(always)]
pub(crate) const fn get_binary_slot(i: u16) -> B16 {
    B16::new(i)
}

/// Helper method to obtain the n-th Fibonacci number.
pub(crate) fn fibonacci(n: usize) -> u32 {
    let mut cur_fibs = [0, 1];
    for _ in 0..n {
        let s = cur_fibs[0] + cur_fibs[1];
        cur_fibs[0] = cur_fibs[1];
        cur_fibs[1] = s;
    }
    cur_fibs[0]
}

/// Helper method to obtain the Collatz orbits.
pub(crate) fn collatz_orbits(initial_val: u32) -> (Vec<u32>, Vec<u32>) {
    let mut cur_value = initial_val;
    let mut evens = vec![];
    let mut odds = vec![];
    while cur_value != 1 {
        if cur_value % 2 == 0 {
            evens.push(cur_value);
            cur_value /= 2;
        } else {
            odds.push(cur_value);
            cur_value = 3 * cur_value + 1;
        }
    }
    (evens, odds)
}

#[cfg(test)]
/// Helper method to convert Instructions to a program ROM.
pub(crate) fn code_to_prom(code: &[Instruction]) -> ProgramRom {
    use binius_m3::builder::B32;

    let mut prom = ProgramRom::new();
    // TODO: type-gate field_pc and use some `incr()` method to abstract away `+1` /
    // `*G`.
    let mut pc = B32::ONE; // we start at PC = 1G.
    for (i, &instruction) in code.iter().enumerate() {
        let interp_inst = InterpreterInstruction::new(instruction, pc);
        prom.push(interp_inst);
        pc *= G;
    }

    prom
}
