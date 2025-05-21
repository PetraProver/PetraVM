use std::fmt;

use crate::execution::PetraTrace;

/// An event representing a right logical shift operation for gadget purposes.
/// Unlike opcode events, this is not fired directly but is collected to
/// generate proof gadgets.
#[derive(Clone, PartialEq)]
pub struct RightLogicShiftGadgetEvent {
    /// The input value to be shifted
    pub input: u32,
    /// The shift amount (only lower 5 bits are used)
    pub shift_amount: u32,
    /// The result after shifting
    pub output: u32,
}

impl RightLogicShiftGadgetEvent {
    /// Creates a new RightLogicShiftGadgetEvent
    pub fn new(input: u32, shift_amount: u32, output: u32) -> Self {
        Self {
            input,
            shift_amount,
            output,
        }
    }
}

impl fmt::Debug for RightLogicShiftGadgetEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RightLogicShiftGadgetEvent {{ input: 0x{:08x}, shift_amount: {}, output: 0x{:08x} }}",
            self.input, self.shift_amount, self.output
        )
    }
}

/// Extension trait for PetraTrace to add right shift events
pub trait RightLogicShiftExtension {
    /// Adds a new right logic shift gadget event to the trace
    fn add_right_shift_event(&mut self, input: u32, shift_amount: u32, output: u32);
}

impl RightLogicShiftExtension for PetraTrace {
    fn add_right_shift_event(&mut self, input: u32, shift_amount: u32, output: u32) {
        self.right_logic_shift_gadget
            .push(RightLogicShiftGadgetEvent::new(input, shift_amount, output));
    }
}
