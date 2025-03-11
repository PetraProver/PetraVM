use binius_field::{BinaryField16b, BinaryField32b};

use crate::{
    emulator::{Interpreter, InterpreterChannels, InterpreterTables, MVInfo, MVKind},
    event::Event,
    fire_non_jump_event,
    opcodes::Opcode,
    ZCrayTrace,
};

/// Convenience macro to implement the `Event` trait for MV events.
macro_rules! impl_mv_fire {
    ($event:ty) => {
        impl Event for $event {
            fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
                fire_non_jump_event!(self, channels);
            }
        }
    };
}

pub(crate) struct MVEventOutput {
    parent: u32, // parent addr
    opcode: Opcode,
    field_pc: BinaryField32b, // field PC
    fp: u32,                  // fp
    timestamp: u32,           // timestamp
    dst: BinaryField16b,      // dst
    src: BinaryField16b,      // src
    offset: BinaryField16b,   // offset
    src_val: u128,
}

impl MVEventOutput {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        parent: u32, // parent addr
        opcode: Opcode,
        field_pc: BinaryField32b, // field PC
        fp: u32,                  // fp
        timestamp: u32,           // timestamp
        dst: BinaryField16b,      // dst
        src: BinaryField16b,      // src
        offset: BinaryField16b,   // offset
        src_val: u128,
    ) -> Self {
        Self {
            parent, // parent addr
            opcode,
            field_pc,  // field PC
            fp,        // fp
            timestamp, // timestamp
            dst,       // dst
            src,       // src
            offset,    // offset
            src_val,
        }
    }

    pub(crate) fn push_mv_event(&self, trace: &mut ZCrayTrace) {
        let &MVEventOutput {
            parent,
            opcode,
            field_pc,
            fp,
            timestamp,
            dst,
            src,
            offset,
            src_val,
        } = self;

        match opcode {
            Opcode::MVVL => {
                let new_event = MVVLEvent::new(
                    field_pc,
                    fp,
                    timestamp,
                    dst.val(),
                    parent,
                    src.val(),
                    src_val,
                    offset.val(),
                );
                trace.mvvl.push(new_event);
            }
            Opcode::MVVW => {
                let new_event = MVVWEvent::new(
                    field_pc,
                    fp,
                    timestamp,
                    dst.val(),
                    parent,
                    src.val(),
                    src_val as u32,
                    offset.val(),
                );
                trace.mvvw.push(new_event);
            }
            _ => panic!(),
        }
    }
}
/// Event for MVV.W.
///
/// Performs a MOVE of 4-byte value between VROM addresses.
///
/// Logic:
///   1. VROM[FP[dst] + offset] = FP[src]
#[derive(Debug, Clone)]
pub(crate) struct MVVWEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,
    dst_addr: u32,
    src: u16,
    src_val: u32,
    offset: u16,
}

// TODO: this is a 4-byte move instruction. So it needs to be updated once we
// have multi-granularity.
impl MVVWEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        dst: u16,
        dst_addr: u32,
        src: u16,
        src_val: u32,
        offset: u16,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_addr,
            src,
            src_val,
            offset,
        }
    }

    /// This method is called once the next_fp has been set by the CALL
    /// procedure.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn generate_event_from_info(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        pc: u32,
        timestamp: u32,
        fp: u32,
        dst: BinaryField16b,
        offset: BinaryField16b,
        src: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Option<Self> {
        let dst_addr = interpreter.vrom.get_u32(fp ^ dst.val() as u32);
        let src_addr = fp ^ src.val() as u32;
        let opt_src_val = interpreter.vrom.get_u32_move(src_addr);

        interpreter.incr_pc();

        // If we already know the value to set, then we can already push an event.
        // Otherwise, we add the move to the list of move events to be pushed once we
        // have access to the value.
        if let Some(src_val) = opt_src_val {
            interpreter
                .vrom
                .set_u32(trace, dst_addr ^ offset.val() as u32, src_val);

            Some(Self {
                pc: field_pc,
                fp,
                timestamp,
                dst: dst.val(),
                dst_addr,
                src: src.val(),
                src_val,
                offset: offset.val(),
            })
        } else {
            interpreter.vrom.insert_to_set(
                dst_addr,
                (
                    src_addr,
                    Opcode::MVVL,
                    field_pc,
                    fp,
                    timestamp,
                    dst,
                    src,
                    offset,
                ),
            );
            None
        }
    }

    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        offset: BinaryField16b,
        src: BinaryField16b,
        field_pc: BinaryField32b,
        is_call_procedure: bool,
    ) -> Option<Self> {
        let fp = interpreter.fp;
        let timestamp = interpreter.timestamp;
        let pc = interpreter.pc;

        if is_call_procedure {
            let new_mv_info = MVInfo {
                mv_kind: MVKind::Mvvw,
                dst,
                offset,
                src,
                field_pc,
                pc,
                timestamp,
            };
            // This move needs to be handled later, in the CALL.
            interpreter.moves_to_set.push(new_mv_info);
            interpreter.incr_pc();
            return None;
        }

        let dst_addr = interpreter.vrom.get_u32(fp ^ dst.val() as u32);
        let src_addr = fp ^ src.val() as u32;
        let opt_src_val = interpreter.vrom.get_u32_move(src_addr);
        assert!(
            opt_src_val.is_some(),
            "Trying to move a nonexistent value from address {:?}",
            src_addr
        );

        interpreter.incr_pc();

        let src_val = opt_src_val.unwrap();
        interpreter
            .vrom
            .set_u32(trace, dst_addr ^ offset.val() as u32, src_val);

        Some(Self {
            pc: field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            dst_addr,
            src: src.val(),
            src_val,
            offset: offset.val(),
        })
    }
}

impl_mv_fire!(MVVWEvent);

/// Event for MVV.L.
///
/// Performs a MOVE of 16-byte value between VROM addresses.
///
/// Logic:
///   1. VROM128[FP[dst] + offset] = FP128[src]
#[derive(Debug, Clone)]
pub(crate) struct MVVLEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,
    dst_addr: u32,
    src: u16,
    src_val: u128,
    offset: u16,
}

impl MVVLEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        dst: u16,
        dst_addr: u32,
        src: u16,
        src_val: u128,
        offset: u16,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_addr,
            src,
            src_val,
            offset,
        }
    }

    /// This method is called once the next_fp has been set by the CALL
    /// procedure.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn generate_event_from_info(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        pc: u32,
        timestamp: u32,
        fp: u32,
        dst: BinaryField16b,
        offset: BinaryField16b,
        src: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Option<Self> {
        let dst_addr = interpreter.vrom.get_u32(fp ^ dst.val() as u32);
        let src_addr = fp ^ src.val() as u32;
        let opt_src_val = interpreter.vrom.get_u128_move(src_addr);

        interpreter.incr_pc();

        // If we already know the value to set, then we can already push an event.
        // Otherwise, we add the move to the list of move events to be pushed once we
        // have access to the value.
        if let Some(src_val) = opt_src_val {
            interpreter
                .vrom
                .set_u128(trace, dst_addr ^ offset.val() as u32, src_val);

            Some(Self {
                pc: field_pc,
                fp,
                timestamp,
                dst: dst.val(),
                dst_addr,
                src: src.val(),
                src_val,
                offset: offset.val(),
            })
        } else {
            interpreter.vrom.insert_to_set(
                dst_addr,
                (
                    src_addr,
                    Opcode::MVVL,
                    field_pc,
                    fp,
                    timestamp,
                    dst,
                    src,
                    offset,
                ),
            );
            None
        }
    }

    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        offset: BinaryField16b,
        src: BinaryField16b,
        field_pc: BinaryField32b,
        is_call_procedure: bool,
    ) -> Option<Self> {
        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;
        let fp = interpreter.fp;

        if is_call_procedure {
            let new_mv_info = MVInfo {
                mv_kind: MVKind::Mvvl,
                dst,
                offset,
                src,
                field_pc,
                pc,
                timestamp,
            };
            // This move needs to be handled later, in the CALL.
            interpreter.moves_to_set.push(new_mv_info);
            interpreter.incr_pc();
            return None;
        }

        let dst_addr = interpreter.vrom.get_u32(fp ^ dst.val() as u32);
        let src_addr = fp ^ src.val() as u32;
        let opt_src_val = interpreter.vrom.get_u128_move(src_addr);
        assert!(
            opt_src_val.is_some(),
            "Trying to move a nonexistent value from address {:?}",
            src_addr
        );

        let src_val = opt_src_val.unwrap();

        interpreter
            .vrom
            .set_u128(trace, dst_addr ^ offset.val() as u32, src_val);

        Some(Self {
            pc: field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            dst_addr,
            src: src.val(),
            src_val,
            offset: offset.val(),
        })
    }
}

impl_mv_fire!(MVVLEvent);

/// Event for MVI.H.
///
/// Performs a MOVE of 2-byte value from a 16-bit immediate into a VROM address,
/// zero-extending to 32-bits.
///
/// Logic:
///   1. VROM[FP[dst] + offset] = ZeroExtend(imm)
#[derive(Debug, Clone)]
pub(crate) struct MVIHEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,
    dst_addr: u32,
    imm: u16,
    offset: u16,
}

// TODO: this is a 2-byte move instruction, which sets a 4 byte address to imm
// zero-extended. So it needs to be updated once we have multi-granularity.
impl MVIHEvent {
    pub const fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        dst: u16,
        dst_addr: u32,
        imm: u16,
        offset: u16,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_addr,
            imm,
            offset,
        }
    }

    /// This method is called once the next_fp has been set by the CALL
    /// procedure.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn generate_event_from_info(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        pc: u32,
        timestamp: u32,
        fp: u32,
        dst: BinaryField16b,
        offset: BinaryField16b,
        imm: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Self {
        // At this point, since we are in a call procedure, `dst` corresponds to the
        // next_fp. And we know it has already been set, so we can read
        // the destination address.
        let dst_addr = interpreter.vrom.get_u32(fp ^ dst.val() as u32);

        interpreter
            .vrom
            .set_u32(trace, dst_addr ^ offset.val() as u32, imm.val() as u32);
        interpreter.incr_pc();

        Self {
            pc: field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            dst_addr,
            imm: imm.val(),
            offset: offset.val(),
        }
    }

    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        offset: BinaryField16b,
        imm: BinaryField16b,
        field_pc: BinaryField32b,
        is_call_procedure: bool,
    ) -> Option<Self> {
        let fp = interpreter.fp;
        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;

        if is_call_procedure {
            let new_mv_info = MVInfo {
                mv_kind: MVKind::Mvih,
                dst,
                offset,
                src: imm,
                field_pc,
                pc,
                timestamp,
            };
            // This move needs to be handled later, in the CALL.
            interpreter.moves_to_set.push(new_mv_info);
            interpreter.incr_pc();
            return None;
        }
        let dst_addr = interpreter.vrom.get_u32(fp ^ dst.val() as u32);

        interpreter
            .vrom
            .set_u32(trace, dst_addr ^ offset.val() as u32, imm.val() as u32);
        interpreter.incr_pc();

        Some(Self {
            pc: field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            dst_addr,
            imm: imm.val(),
            offset: offset.val(),
        })
    }
}

impl_mv_fire!(MVIHEvent);

// Event for LDI.
#[derive(Debug, Clone)]
pub(crate) struct LDIEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,
    imm: u32,
}

impl LDIEvent {
    pub const fn new(pc: BinaryField32b, fp: u32, timestamp: u32, dst: u16, imm: u32) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            imm,
        }
    }

    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        imm: BinaryField32b,
        field_pc: BinaryField32b,
    ) -> Self {
        let fp = interpreter.fp;
        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;

        interpreter
            .vrom
            .set_u32(trace, fp ^ dst.val() as u32, imm.val());
        interpreter.incr_pc();

        Self {
            pc: field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            imm: imm.val(),
        }
    }
}

impl_mv_fire!(LDIEvent);
