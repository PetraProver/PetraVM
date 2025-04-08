use binius_m3::builder::B16;
use tracing_subscriber::EnvFilter;

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
pub(crate) fn code_to_prom(code: &[crate::Instruction]) -> crate::ProgramRom {
    use binius_field::Field;
    use binius_m3::builder::B32;

    use crate::execution::G;

    let mut prom = crate::ProgramRom::new();
    // TODO: type-gate field_pc and use some `incr()` method to abstract away `+1` /
    // `*G`.
    let mut pc = B32::ONE; // we start at PC = 1G.
    for &instruction in code.iter() {
        let interp_inst = crate::InterpreterInstruction::new(instruction, pc);
        prom.push(interp_inst);
        pc *= G;
    }

    prom
}

/// Convenience macro to extract the last event logged for a given instruction
/// from the trace of a provided [`EventContext`].
///
/// This will panic if no events have been pushed for the targeted instruction.
///
/// # Example
///
/// ```ignore
/// get_last_event!(ctx, signed_mul);
/// ```
#[macro_export]
macro_rules! get_last_event {
    ($ctx:ident, $trace_field:ident) => {
        $ctx.trace
            .$trace_field
            .last()
            .expect("At least one event should have been pushed.")
    };
}
