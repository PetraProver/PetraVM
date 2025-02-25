use crate::{
    emulator::{Interpreter, StateChannel},
    utils::Event,
};

// Struture of an event for ADDI.
#[derive(Debug, Clone)]
pub(crate) struct AddIEvent {
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

impl AddIEvent {
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
        let dst_val = src_val + imm;

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

impl Event for AddIEvent {
    fn fire(&self, state_channel: &mut StateChannel) {
        state_channel.pull((self.pc, self.fp, self.timestamp));
        state_channel.push((self.pc + 1, self.fp, self.timestamp + 1));
    }
}

// Struture of an event for ADDI.
#[derive(Debug, Clone)]
pub(crate) struct MulIEvent {
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

impl MulIEvent {
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

impl Event for MulIEvent {
    fn fire(&self, state_channel: &mut StateChannel) {
        state_channel.pull((self.pc, self.fp, self.timestamp));
        state_channel.push((self.pc + 1, self.fp, self.timestamp + 1));
    }
}
