use crate::{
    emulator::{Channel, Interpreter, ProgramChannelInput, StateChannelInput},
    utils::Event,
};

#[derive(Debug, Clone)]
pub enum ShiftKind {
    Left,
    Right,
}

// Struture of an event for one of the shifts.
#[derive(Debug, Clone)]
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

#[derive(Default, Debug)]
pub struct SliTrace {
    shifts_events: [Vec<SliEvent>; 32],
}

impl SliTrace {
    pub fn push_event(&mut self, event: SliEvent) {
        let shift = event.shift;
        self.shifts_events[shift as usize].push(event);
    }

    pub fn generate_event(
        interpreter: &mut Interpreter,
        dst: u32,
        src: u32,
        imm: u32,
        kind: ShiftKind,
    ) -> SliEvent {
        assert!((src as usize) < interpreter.get_vrom_size());
        let src_val = interpreter.get_vrom_index(src as usize);
        let new_val = if imm == 0 || imm >= 32 {
            0
        } else {
            match kind {
                ShiftKind::Left => src_val << imm,
                ShiftKind::Right => src_val >> imm,
            }
        };
        if dst as usize > interpreter.get_vrom_size() - 1 {
            interpreter.extend_size(&vec![0; dst as usize - interpreter.get_vrom_size() + 1]);
        }
        let pc = interpreter.get_pc();
        let timestamp = interpreter.get_timestamp();
        interpreter.set_vrom_index(dst as usize, new_val);
        interpreter.set_pc(pc + 1);
        interpreter.set_timestamp(timestamp + 1);

        SliEvent::new(
            pc,
            interpreter.get_fp(),
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
}

impl Event for SliEvent {
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
}
