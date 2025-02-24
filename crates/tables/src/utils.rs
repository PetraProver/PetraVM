use crate::{
    interpreter::{Channel, Interpreter, ProgramChannelInput, StateChannelInput},
    sli::SliTrace,
};

pub trait Event {
    fn fire(
        &self,
        state_channel: &mut Channel<StateChannelInput>,
        program_channel: &mut Channel<ProgramChannelInput>,
    );

    // fn apply_event(&self, interpreter: &mut Interpreter);
}

#[derive(Debug)]
pub struct RetEvent {
    pc: u32,
    fp: u32,
    timestamp: u32,
    fp_0_val: u32,
    fp_1_val: u32,
}

#[derive(Default, Debug)]
pub struct RetTrace {
    ret_events: Vec<RetEvent>,
}

impl RetTrace {
    pub fn push_event(&mut self, event: RetEvent) {
        self.ret_events.push(event);
    }

    pub fn generate_event(interpreter: &mut Interpreter) -> RetEvent {
        interpreter.pc = interpreter.vrom[interpreter.fp as usize];
        interpreter.pc = interpreter.vrom[interpreter.fp as usize + 1];
        interpreter.timestamp += 1;
        RetEvent::new(&interpreter)
    }
}

impl RetEvent {
    pub fn new(interpreter: &Interpreter) -> Self {
        Self {
            pc: interpreter.pc,
            fp: interpreter.fp,
            timestamp: interpreter.timestamp,
            fp_0_val: interpreter.vrom[interpreter.fp as usize],
            fp_1_val: interpreter.vrom[interpreter.fp as usize + 1],
        }
    }
}

impl Event for RetEvent {
    fn fire(
        &self,
        state_channel: &mut Channel<StateChannelInput>,
        program_channel: &mut Channel<ProgramChannelInput>,
    ) {
        state_channel.pull((self.pc, self.fp, self.timestamp));
        state_channel.push((self.fp_0_val, self.fp_1_val, self.timestamp + 1));
        program_channel.push((self.pc, 0x00 as u32));
    }

    // fn apply_event(&self, interpreter: &mut Interpreter) {
    //     interpreter.pc = interpreter.vrom[self.fp as usize];
    //     interpreter.pc = interpreter.vrom[self.fp as usize + 1];
    //     interpreter.timestamp += 1;
    // }
}
