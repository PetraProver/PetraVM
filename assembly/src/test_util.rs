use binius_m3::builder::B16;

#[inline(always)]
pub(crate) const fn get_binary_slot(i: u16) -> B16 {
    B16::new(i)
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
        let interp_inst = InterpreterInstruction::new(instruction, pc, None, false);
        prom.push(interp_inst);
        pc *= G;
    }

    prom
}

/// Convenience macro to extract the last event logged for a given instruction
/// from the trace of a provided
/// [`EventContext`](crate::event::context::EventContext).
///
/// This will panic if no events have been pushed for the targeted instruction.
///
/// # Example
///
/// ```ignore
/// get_last_event!(ctx, signed_mul);
/// ```
macro_rules! get_last_event {
    ($ctx:ident, $trace_field:ident) => {
        $ctx.trace
            .$trace_field
            .last()
            .expect("At least one event should have been pushed.")
    };
}

// Re-export the macro for use in tests.
pub(crate) use get_last_event;

use crate::InterpreterInstruction;
