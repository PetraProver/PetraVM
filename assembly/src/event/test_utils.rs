use std::collections::HashMap;

use binius_field::{BinaryField16b, BinaryField32b, Field};

use crate::{
    execution::{Interpreter, ZCrayTrace},
    memory::{Memory, ProgramRom, ValueRom},
};

/// Test environment for simplified instruction testing
pub struct TestEnv {
    pub interpreter: Interpreter,
    pub trace: ZCrayTrace,
    pub field_pc: BinaryField32b,
}

impl TestEnv {
    pub fn new() -> Self {
        let mut interpreter = Interpreter::new(HashMap::new(), HashMap::new());
        interpreter.timestamp = 0;
        interpreter.pc = 1;

        let memory = Memory::new(ProgramRom::new(), ValueRom::default());
        let trace = ZCrayTrace::new(memory);

        Self {
            interpreter,
            trace,
            field_pc: BinaryField32b::ONE,
        }
    }

    // Helper to set a value in VROM
    pub fn set_value(&mut self, slot: u16, value: u32) {
        self.trace
            .set_vrom_u32(self.interpreter.fp ^ slot as u32, value)
            .unwrap();
    }

    // Helper to get a value from VROM
    pub fn get_value(&self, slot: u16) -> u32 {
        self.trace
            .get_vrom_u32(self.interpreter.fp ^ slot as u32)
            .unwrap()
    }

    // Helper to get a u64 value from VROM (for multiplication results)
    pub fn get_value_u64(&self, slot: u16) -> u64 {
        self.trace
            .get_vrom_u64(self.interpreter.fp ^ slot as u32)
            .unwrap()
    }
}
