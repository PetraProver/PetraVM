use crate::{
    emulator::{Interpreter, InterpreterChannels, InterpreterTables},
    event::Event,
};

// Struture of an event for ADDI.
#[derive(Debug, Clone)]
pub(crate) struct AddiEvent {
    pc: u16,
    fp: u16,
    timestamp: u16,
    dst: u32,
    dst_val: u32,
    src: u32,
    src_val: u32,
    imm: u32,
    cout: u32,
}

impl AddiEvent {
    pub fn new(
        pc: u16,
        fp: u16,
        timestamp: u16,
        dst: u32,
        dst_val: u32,
        src: u32,
        src_val: u32,
        imm: u32,
        cout: u32,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_val,
            src,
            src_val,
            imm,
            cout,
        }
    }

    pub fn generate_event(interpreter: &mut Interpreter, dst: u32, src: u32, imm: u32) -> Self {
        let fp = interpreter.fp;
        let src_val = interpreter.vrom[src as usize + 1];
        let (dst_val, carry) = src_val.overflowing_add(imm);

        let cout = (dst_val ^ src_val ^ imm) >> 1 + (carry as u32) << 31;

        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;
        interpreter.pc += 1;

        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_val,
            src,
            src_val,
            imm,
            cout,
        }
    }
}

impl Event for AddiEvent {
    fn fire(&self, channels: &mut InterpreterChannels, tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels
            .state_channel
            .push((self.pc + 1, self.fp, self.timestamp + 1));
    }
}

// Struture of an event for ADDI.
#[derive(Debug, Clone)]
pub(crate) struct MuliEvent {
    pc: u16,
    fp: u16,
    timestamp: u16,
    dst: u32,
    dst_val: u32,
    src: u32,
    src_val: u32,
    imm: u32,
    cout: u32,
}

impl MuliEvent {
    pub fn new(
        pc: u16,
        fp: u16,
        timestamp: u16,
        dst: u32,
        dst_val: u32,
        src: u32,
        src_val: u32,
        imm: u32,
        cout: u32,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_val,
            src,
            src_val,
            imm,
            cout,
        }
    }

    pub fn generate_event(interpreter: &mut Interpreter, dst: u32, src: u32, imm: u32) -> Self {
        let fp = interpreter.fp;
        let src_val = interpreter.vrom[src as usize + 1];
        let dst_val = src_val * imm;

        // TODO: generate cout correctly.
        let cout = 0;

        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;
        interpreter.pc += 1;

        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_val,
            src,
            src_val,
            imm,
            cout,
        }
    }
}

impl Event for MuliEvent {
    fn fire(&self, channels: &mut InterpreterChannels, tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels
            .state_channel
            .push((self.pc + 1, self.fp, self.timestamp + 1));
    }
}
