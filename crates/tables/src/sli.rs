use crate::{
    interpreter::{Channel, Interpreter, ProgramChannelInput, StateChannelInput},
    utils::Event,
};

#[derive(Debug, Clone)]
pub enum ShiftKind {
    Left,
    Right,
}

// Struture of an event for one of the shifts.
#[derive(Debug, Clone)]
pub struct SliEventStruct {
    pc: u32,
    fp: u32,
    timestamp: u32,
    dst: u32,
    dst_val: u32,
    src: u32,
    src_val: u32,
    shift: u32,
    kind: ShiftKind,
}

#[derive(Default, Debug)]
pub struct SliTrace {
    shifts_events: [Vec<SliEventStruct>; 32],
}

impl SliTrace {
    pub fn push_event(&mut self, event: SliEventStruct) {
        let shift = event.shift;
        self.shifts_events[shift as usize].push(event);
    }

    pub fn generate_event(
        interpreter: &mut Interpreter,
        dst: u32,
        src: u32,
        imm: u32,
        kind: ShiftKind,
    ) -> SliEventStruct {
        assert!((src as usize) < interpreter.vrom.len());
        let src_val = interpreter.vrom[src as usize];
        let new_val = if imm == 0 || imm >= 32 {
            0
        } else {
            match kind {
                ShiftKind::Left => src_val << imm,
                ShiftKind::Right => src_val >> imm,
            }
        };
        if dst as usize > interpreter.vrom.len() - 1 {
            interpreter
                .vrom
                .extend(&vec![0; dst as usize - interpreter.vrom.len() + 1]);
        }
        let pc = interpreter.pc;
        interpreter.vrom[dst as usize] = new_val;
        interpreter.pc += 1;
        interpreter.timestamp += 1;

        SliEventStruct::new(
            pc,
            interpreter.fp,
            interpreter.timestamp,
            dst,
            new_val,
            src,
            src_val,
            imm,
            kind,
        )
    }
}

impl SliEventStruct {
    pub fn new(
        pc: u32,
        fp: u32,
        timestamp: u32,
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
}

impl Event for SliEventStruct {
    fn fire(
        &self,
        state_channel: &mut Channel<StateChannelInput>,
        program_channel: &mut Channel<ProgramChannelInput>,
    ) {
        state_channel.pull((self.pc, self.fp, self.timestamp));
        state_channel.push((self.pc + 1, self.fp, self.timestamp + 1));
        match self.kind {
            ShiftKind::Left => program_channel.push((self.pc, 0x1b as u32)),
            ShiftKind::Right => program_channel.push((self.pc, 0x1c as u32)),
        }
    }

    // fn apply_event(&self, interpreter: &mut Interpreter) {
    //     assert!((self.src as usize) < interpreter.vrom.len());
    //     let src_val = interpreter.vrom[self.src as usize];
    //     let new_val = if self.shift == 0 || self.shift >= 32 {
    //         0
    //     } else {
    //         match self.kind {
    //             ShiftKind::Left => src_val << self.shift,
    //             ShiftKind::Right => src_val >> self.shift,
    //         }
    //     };
    //     if self.dst as usize > interpreter.vrom.len() {
    //         interpreter
    //             .vrom
    //             .extend(&vec![0; self.dst as usize - interpreter.vrom.len()]);
    //     }
    //     interpreter.vrom[self.dst as usize] = new_val;
    //     interpreter.pc += 1;
    //     interpreter.timestamp += 1;
    // }
}
