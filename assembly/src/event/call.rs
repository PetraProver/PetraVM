use binius_field::{BinaryField16b, BinaryField32b};

use crate::{
    emulator::{Interpreter, InterpreterChannels, InterpreterError, InterpreterTables},
    event::Event,
    ZCrayTrace,
};

/// Event for TAILI.
///
/// Performs a tail function call to the target address given by an immediate.
///
/// Logic:
///   1. [FP[next_fp] + 0] = FP[0] (return address)
///   2. [FP[next_fp] + 1] = FP[1] (old frame pointer)
///   3. FP = FP[next_fp]
///   4. PC = target
#[derive(Debug, Clone)]
pub(crate) struct TailiEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    target: u32,
    next_fp: u16,
    next_fp_val: u32,
    return_addr: u32,
    old_fp_val: u16,
}

impl TailiEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        target: u32,
        next_fp: u16,
        next_fp_val: u32,
        return_addr: u32,
        old_fp_val: u16,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            target,
            next_fp,
            next_fp_val,
            return_addr,
            old_fp_val,
        }
    }

    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        target: BinaryField32b,
        next_fp: BinaryField16b,
        next_fp_val: u32,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let return_addr = interpreter.vrom.get_u32(interpreter.fp)?;
        let old_fp_val = interpreter.vrom.get_u32(interpreter.fp ^ 1)?;
        interpreter
            .vrom
            .set_u32(trace, interpreter.fp ^ next_fp.val() as u32, next_fp_val)?;

        interpreter.handles_call_moves(trace)?;

        let pc = interpreter.pc;
        let fp = interpreter.fp;
        let timestamp = interpreter.timestamp;

        interpreter.fp = next_fp_val;
        interpreter.jump_to(target);

        interpreter.vrom.set_u32(trace, next_fp_val, return_addr)?;
        interpreter
            .vrom
            .set_u32(trace, next_fp_val + 1, old_fp_val)?;

        Ok(Self {
            pc: field_pc,
            fp,
            timestamp,
            target: target.val(),
            next_fp: next_fp.val(),
            next_fp_val,
            return_addr,
            old_fp_val: old_fp_val as u16,
        })
    }
}

impl Event for TailiEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels.state_channel.push((
            BinaryField32b::new(self.target),
            self.next_fp_val,
            self.timestamp + 1,
        ));
    }
}

/// Event for TAILV.
///
/// Performs a tail function call to the indirect target address read from VROM.
///
/// Logic:
///   1. [FP[next_fp] + 0] = FP[0] (return address)
///   2. [FP[next_fp] + 1] = FP[1] (old frame pointer)
///   3. FP = FP[next_fp]
///   4. PC = FP[offset]
#[derive(Debug, Clone)]
pub(crate) struct TailVEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    offset: u16,
    next_fp: u16,
    next_fp_val: u32,
    return_addr: u32,
    old_fp_val: u16,
    target: u32,
}

impl TailVEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        offset: u16,
        next_fp: u16,
        next_fp_val: u32,
        return_addr: u32,
        old_fp_val: u16,
        target: u32,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            offset,
            next_fp,
            next_fp_val,
            return_addr,
            old_fp_val,
            target,
        }
    }

    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        offset: BinaryField16b,
        next_fp: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let return_addr = interpreter.vrom.get_u32(interpreter.fp)?;
        let old_fp_val = interpreter.vrom.get_u32(interpreter.fp ^ 4)?;

        let next_fp_addr = interpreter.fp ^ offset.val() as u32;
        let target = interpreter.vrom.get_u32(next_fp_addr)?;

        // We allocate a frame for the call.
        let next_fp_val = interpreter.allocate_new_frame(target.into())?;
        interpreter.vrom.set_u32(trace, next_fp_addr, next_fp_val)?;

        // Once we have the next_fp, we knpw the destination address for the moves in
        // the call procedures. We can then generate events for some moves and correctly
        // delegate the other moves.
        interpreter.handles_call_moves(trace);

        let pc = interpreter.pc;
        let fp = interpreter.fp;
        let timestamp = interpreter.timestamp;

        interpreter.fp = next_fp_val;
        interpreter.jump_to(BinaryField32b::new(interpreter.fp ^ offset.val() as u32));

        interpreter.vrom.set_u32(trace, next_fp_val, return_addr)?;
        interpreter
            .vrom
            .set_u32(trace, next_fp_val + 1, old_fp_val)?;

        Ok(Self {
            pc: field_pc,
            fp,
            timestamp,
            offset: offset.val(),
            next_fp: next_fp.val(),
            next_fp_val,
            return_addr,
            old_fp_val: old_fp_val as u16,
            target,
        })
    }
}

impl Event for TailVEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels.state_channel.push((
            BinaryField32b::new(self.offset as u32),
            self.next_fp_val,
            self.timestamp + 1,
        ));
    }
}
