use crate::{
    emulator::{Channel, Interpreter, InterpreterChannels, InterpreterTables},
    event::Event,
};

#[derive(Debug, Clone, PartialEq)]
pub enum ShiftKind {
    Left,
    Right,
}

// Struture of an event for one of the shifts.
#[derive(Debug, Clone, PartialEq)]
pub struct SliEvent {
    pc: u16,
    fp: u16,
    timestamp: u16,
    dst: u32,
    dst_val: u32,
    src: u32,
    src_val: u32,
    shift: u32,
    kind: ShiftKind,
}

impl SliEvent {
    pub fn new(
        pc: u16,
        fp: u16,
        timestamp: u16,
        dst: u32,
        dst_val: u32,
        src: u32,
        src_val: u32,
        shift: u32,
        kind: ShiftKind,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_val,
            src,
            src_val,
            shift,
            kind,
        }
    }

    pub fn generate_event(
        interpreter: &mut Interpreter,
        dst: u32,
        src: u32,
        imm: u32,
        kind: ShiftKind,
    ) -> SliEvent {
        println!("src = {}, vrom_size = {}", src, interpreter.vrom_size());
        assert!((src as usize) < interpreter.vrom_size());
        let src_val = interpreter.vrom[src as usize];
        let new_val = if imm == 0 || imm >= 32 {
            0
        } else {
            match kind {
                ShiftKind::Left => src_val << imm,
                ShiftKind::Right => src_val >> imm,
            }
        };
        if dst as usize > interpreter.vrom_size() - 1 {
            interpreter.extend_vrom(&vec![0; dst as usize - interpreter.vrom_size() + 1]);
        }
        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;
        interpreter.vrom[dst as usize] = new_val;
        interpreter.pc = pc + 1;

        SliEvent::new(
            pc,
            interpreter.fp,
            timestamp,
            dst,
            new_val,
            src,
            src_val,
            imm,
            kind,
        )
    }
}

impl Event for SliEvent {
    fn fire(&self, channels: &mut InterpreterChannels, tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels
            .state_channel
            .push((self.pc + 1, self.fp, self.timestamp + 1));
    }
}
