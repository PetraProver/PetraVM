use binius_field::{BinaryField16b, BinaryField32b, ExtensionField};

use super::context::EventContext;
use crate::{
    event::Event,
    execution::{
        FramePointer, Interpreter, InterpreterChannels, InterpreterError, InterpreterTables,
        ZCrayTrace,
    },
    fire_non_jump_event,
    memory::MemoryError,
    opcodes::Opcode,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MVKind {
    Mvvw,
    Mvvl,
    Mvih,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MVInfo {
    pub(crate) mv_kind: MVKind,
    pub(crate) dst: BinaryField16b,
    pub(crate) offset: BinaryField16b,
    pub(crate) src: BinaryField16b,
    pub(crate) pc: BinaryField32b,
    pub(crate) timestamp: u32,
}

/// Convenience macro to implement the `Event` trait for MV events.
macro_rules! impl_mv_event {
    ($event:ty, $trace_field:ident) => {
        impl Event for $event {
            fn generate(
                ctx: &mut EventContext,
                arg0: BinaryField16b,
                arg1: BinaryField16b,
                arg2: BinaryField16b,
            ) -> Result<(), InterpreterError> {
                let opt_event = Self::generate_event(ctx, arg0, arg1, arg2)?;
                if let Some(event) = opt_event {
                    ctx.trace.$trace_field.push(event);
                }

                Ok(())
            }

            fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
                fire_non_jump_event!(self, channels);
            }
        }
    };
}

pub(crate) struct MVEventOutput {
    pub(crate) parent: u32, // parent addr
    pub(crate) opcode: Opcode,
    pub(crate) field_pc: BinaryField32b, // field PC
    pub(crate) fp: FramePointer,         // fp
    pub(crate) timestamp: u32,           // timestamp
    pub(crate) dst: BinaryField16b,      // dst
    pub(crate) src: BinaryField16b,      // src
    pub(crate) offset: BinaryField16b,   // offset
    pub(crate) src_val: u128,
}

impl MVEventOutput {
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        parent: u32, // parent addr
        opcode: Opcode,
        field_pc: BinaryField32b, // field PC
        fp: FramePointer,         // fp
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
            Opcode::Mvvl => {
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
            Opcode::Mvvw => {
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
            o => panic!("Events for {:?} should already have been generated.", o),
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
    fp: FramePointer,
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
        fp: FramePointer,
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
    pub(crate) fn generate_event_from_info(
        ctx: &mut EventContext,
        pc: BinaryField32b,
        timestamp: u32,
        fp: FramePointer,
        dst: BinaryField16b,
        offset: BinaryField16b,
        src: BinaryField16b,
    ) -> Result<Option<Self>, InterpreterError> {
        let dst_addr = ctx.load_vrom_u32(ctx.addr(dst.val()))?;
        let src_addr = ctx.addr(src.val());
        let opt_src_val = ctx.load_vrom_opt_u32(ctx.addr(src.val()))?;

        // If we already know the value to set, then we can already push an event.
        // Otherwise, we add the move to the list of MOVE events to be pushed once we
        // have access to the value.
        if let Some(src_val) = opt_src_val {
            ctx.store_vrom_u32(dst_addr ^ offset.val() as u32, src_val)?;

            Ok(Some(Self {
                pc,
                fp,
                timestamp,
                dst: dst.val(),
                dst_addr,
                src: src.val(),
                src_val,
                offset: offset.val(),
            }))
        } else {
            // `src_val` is not yet known, which is means it's a return value from the
            // function called. So we insert `dst_addr ^ offset` to the addresses to track
            // in `pending_updates`. As soon as it is set in the called function, we can
            // also set the value at `src_addr` and generate the MOVE event.
            ctx.trace.insert_pending(
                dst_addr ^ offset.val() as u32,
                (src_addr, Opcode::Mvvw, pc, *fp, timestamp, dst, src, offset),
            );
            Ok(None)
        }
    }

    pub fn generate_event(
        ctx: &mut EventContext,
        dst: BinaryField16b,
        offset: BinaryField16b,
        src: BinaryField16b,
    ) -> Result<Option<Self>, InterpreterError> {
        let fp = ctx.fp;
        let timestamp = ctx.timestamp;
        let pc = ctx.pc;

        let opt_dst_addr = ctx.load_vrom_opt_u32(ctx.addr(dst.val()))?;
        let opt_src_val = ctx.load_vrom_opt_u32(ctx.addr(src.val()))?;

        // If the source value is missing or the destination address is still unknown,
        // it means we are in a MOVE that precedes a CALL, and we have to handle the
        // MOVE operation later.
        if opt_dst_addr.is_none() || opt_src_val.is_none() {
            let new_mv_info = MVInfo {
                mv_kind: MVKind::Mvvw,
                dst,
                offset,
                src,
                pc: ctx.field_pc,
                timestamp,
            };
            // This move needs to be handled later, in the CALL.
            ctx.moves_to_apply.push(new_mv_info);
            ctx.incr_pc();
            return Ok(None);
        }

        let dst_addr = opt_dst_addr.expect("We checked previously that dst_addr is some");
        let src_val = opt_src_val.expect("We checked previously that src_val is some");

        ctx.incr_pc();

        ctx.store_vrom_u32(ctx.addr(offset.val()), src_val)?;

        Ok(Some(Self {
            pc: ctx.field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            dst_addr,
            src: src.val(),
            src_val,
            offset: offset.val(),
        }))
    }
}

impl_mv_event!(MVVWEvent, mvvw);

/// Event for MVV.L.
///
/// Performs a MOVE of 16-byte value between VROM addresses.
///
/// Logic:
///   1. VROM128[FP[dst] + offset] = FP128[src]
#[derive(Debug, Clone)]
pub(crate) struct MVVLEvent {
    pc: BinaryField32b,
    fp: FramePointer,
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
        fp: FramePointer,
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
    pub(crate) fn generate_event_from_info(
        ctx: &mut EventContext,
        pc: BinaryField32b,
        timestamp: u32,
        fp: FramePointer,
        dst: BinaryField16b,
        offset: BinaryField16b,
        src: BinaryField16b,
    ) -> Result<Option<Self>, InterpreterError> {
        let dst_addr = ctx.load_vrom_u32(ctx.addr(dst.val()))?;
        let src_addr = ctx.addr(src.val());
        let opt_src_val = ctx.load_vrom_opt_u128(ctx.addr(src.val()))?;

        // If we already know the value to set, then we can already push an event.
        // Otherwise, we add the move to the list of MOVE events to be pushed once we
        // have access to the value.
        if let Some(src_val) = opt_src_val {
            ctx.store_vrom_u128(dst_addr ^ offset.val() as u32, src_val)?;

            Ok(Some(Self {
                pc,
                fp,
                timestamp,
                dst: dst.val(),
                dst_addr,
                src: src.val(),
                src_val,
                offset: offset.val(),
            }))
        } else {
            // `src_val` is not yet known, which is means it's a return value from the
            // function called. So we insert `dst_addr ^ offset` to the addresses to track
            // in `pending_updates`. As soon as it is set in the called function, we can
            // also set the value at `src_addr` and generate the MOVE event.
            ctx.trace.insert_pending(
                dst_addr ^ offset.val() as u32,
                (src_addr, Opcode::Mvvl, pc, *fp, timestamp, dst, src, offset),
            );
            Ok(None)
        }
    }

    pub fn generate_event(
        ctx: &mut EventContext,
        dst: BinaryField16b,
        offset: BinaryField16b,
        src: BinaryField16b,
    ) -> Result<Option<Self>, InterpreterError> {
        let pc = ctx.pc;
        let timestamp = ctx.timestamp;
        let fp = ctx.fp;

        let opt_dst_addr = ctx.load_vrom_opt_u32(ctx.addr(dst.val()))?;
        let opt_src_val = ctx.load_vrom_opt_u128(ctx.addr(src.val()))?;

        // If the source value is missing or the destination address is still unknown,
        // it means we are in a MOVE that precedes a CALL, and we have to handle the
        // MOVE operation later.
        if opt_dst_addr.is_none() || opt_src_val.is_none() {
            let new_mv_info = MVInfo {
                mv_kind: MVKind::Mvvl,
                dst,
                offset,
                src,
                pc: ctx.field_pc,
                timestamp,
            };
            // This move needs to be handled later, in the CALL.
            ctx.moves_to_apply.push(new_mv_info);
            ctx.incr_pc();
            return Ok(None);
        }

        let dst_addr = opt_dst_addr.expect("We checked previously that dst_addr is some");
        let src_val = opt_src_val.expect("We checked previously that src_val is some");

        ctx.store_vrom_u128(ctx.addr(offset.val()), src_val)?;

        Ok(Some(Self {
            pc: ctx.field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            dst_addr,
            src: src.val(),
            src_val,
            offset: offset.val(),
        }))
    }
}

impl_mv_event!(MVVLEvent, mvvl);

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
    fp: FramePointer,
    timestamp: u32,
    dst: u16,
    dst_addr: u32,
    imm: u16,
    offset: u16,
}

// TODO: this is a 2-byte move instruction, which sets a 4 byte address to imm
// zero-extended. So it needs to be updated once we have multi-granularity.
impl MVIHEvent {
    /// This method is called once the next_fp has been set by the CALL
    /// procedure.
    pub(crate) fn generate_event_from_info(
        ctx: &mut EventContext,
        pc: BinaryField32b,
        timestamp: u32,
        fp: FramePointer,
        dst: BinaryField16b,
        offset: BinaryField16b,
        imm: BinaryField16b,
    ) -> Result<Self, InterpreterError> {
        // At this point, since we are in a call procedure, `dst` corresponds to the
        // next_fp. And we know it has already been set, so we can read
        // the destination address.
        let dst_addr = ctx.load_vrom_u32(ctx.addr(dst.val()))?;

        ctx.store_vrom_u32(dst_addr ^ offset.val() as u32, imm.val() as u32)?;

        Ok(Self {
            pc,
            fp,
            timestamp,
            dst: dst.val(),
            dst_addr,
            imm: imm.val(),
            offset: offset.val(),
        })
    }

    pub fn generate_event(
        ctx: &mut EventContext,
        dst: BinaryField16b,
        offset: BinaryField16b,
        imm: BinaryField16b,
    ) -> Result<Option<Self>, InterpreterError> {
        let fp = ctx.fp;
        let pc = ctx.pc;
        let timestamp = ctx.timestamp;

        let opt_dst_addr = ctx.load_vrom_opt_u32(ctx.addr(dst.val()))?;

        // If the destination address is still unknown, it means we are in a MOVE that
        // precedes a CALL, and we have to handle the MOVE operation later.
        if let Some(dst_addr) = opt_dst_addr {
            ctx.store_vrom_u32(ctx.addr(offset.val()), imm.val() as u32)?;
            ctx.incr_pc();

            Ok(Some(Self {
                pc: ctx.field_pc,
                fp,
                timestamp,
                dst: dst.val(),
                dst_addr,
                imm: imm.val(),
                offset: offset.val(),
            }))
        } else {
            let new_mv_info = MVInfo {
                mv_kind: MVKind::Mvih,
                dst,
                offset,
                src: imm,
                pc: ctx.field_pc,
                timestamp,
            };
            // This move needs to be handled later, in the CALL.
            ctx.moves_to_apply.push(new_mv_info);
            ctx.incr_pc();
            Ok(None)
        }
    }
}

impl_mv_event!(MVIHEvent, mvih);

// Event for LDI.
#[derive(Debug, Clone)]
pub(crate) struct LDIEvent {
    pc: BinaryField32b,
    fp: FramePointer,
    timestamp: u32,
    dst: u16,
    imm: u32,
}

impl LDIEvent {
    pub fn generate_event(
        ctx: &mut EventContext,
        dst: BinaryField16b,
        imm_low: BinaryField16b,
        imm_high: BinaryField16b,
    ) -> Result<Option<Self>, InterpreterError> {
        let fp = ctx.fp;
        let pc = ctx.pc;
        let timestamp = ctx.timestamp;

        let imm = BinaryField32b::from_bases([imm_low, imm_high])
            .map_err(|_| InterpreterError::InvalidInput)?;

        ctx.store_vrom_u32(ctx.addr(dst.val()), imm.val())?;
        ctx.incr_pc();

        Ok(Some(Self {
            pc: ctx.field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            imm: imm.val(),
        }))
    }
}

impl_mv_event!(LDIEvent, ldi);

mod tests {
    use std::collections::HashMap;

    use binius_field::{BinaryField16b, BinaryField32b, Field, PackedField};

    use crate::{
        event::mv::{MVInfo, MVKind},
        execution::{Interpreter, G},
        memory::{Memory, VromPendingUpdates, VromUpdate},
        opcodes::Opcode,
        util::code_to_prom,
        ValueRom, ZCrayTrace,
    };

    #[test]
    fn test_mv_no_src() {
        // Frame
        // Slot 0: Return PC
        // Slot 1: Return FP
        // Slot 2: dst_addr1 = 0
        // Slot 3: dst_storage1
        // Slot 4: src_val1: not written yet.
        // Slot 5: dst_addr2 = 0
        // Slot 6: dst_storage2
        // Slot 7: padding for alignment.
        // Slot 8: src_val2: not written yet.

        let zero = BinaryField16b::zero();
        let dst_addr1 = 2.into();
        let offset1 = 3.into();
        let src_addr1 = 4.into();
        let dst_addr2 = 5.into();
        let offset2 = 6.into();
        let src_addr2 = 8.into();
        // Do MVVW and MVVL with an unaccessible source value.
        let instructions = vec![
            [Opcode::Mvvw.get_field_elt(), dst_addr1, offset1, src_addr1],
            [Opcode::Mvvl.get_field_elt(), dst_addr2, offset2, src_addr2],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];

        let mut frames = HashMap::new();
        frames.insert(BinaryField32b::one(), 9);

        let prom = code_to_prom(&instructions);
        let mut vrom = ValueRom::default();
        vrom.set_u32(0, 0).unwrap();
        vrom.set_u32(1, 0).unwrap();
        vrom.set_u32(2, 0).unwrap();
        vrom.set_u32(5, 0).unwrap();

        let memory = Memory::new(prom, vrom);

        let mut interpreter = Interpreter::new(frames, HashMap::new());

        let _ = interpreter
            .run(memory)
            .expect("The interpreter should run smoothly.");

        // Check that `moves_to_apply` contains the two MOVE events.
        let first_move = MVInfo {
            mv_kind: MVKind::Mvvw,
            dst: dst_addr1,
            offset: offset1,
            src: src_addr1,
            pc: BinaryField32b::ONE,
            timestamp: 0,
        };
        let second_move = MVInfo {
            mv_kind: MVKind::Mvvl,
            dst: dst_addr2,
            offset: offset2,
            src: src_addr2,
            pc: G,
            timestamp: 0, // Only RAM operations increase the timestamp
        };

        assert_eq!(interpreter.moves_to_apply, vec![first_move, second_move]);
    }

    #[test]
    fn test_mv_no_dst() {
        // Frame
        // Slot 0: Return PC
        // Slot 1: Return FP
        // Slot 2: src_val1
        // Slot 3: padding for alignment.
        // Slot 4-7: src_val2
        // Slot 8: target
        // Slot 9: next_fp

        let zero = BinaryField16b::zero();
        let offset1 = 2.into();
        let src_addr1 = 2.into();
        let offset2 = 4.into();
        let src_addr2 = 4.into();
        let offset3 = 8.into();
        let imm = 12.into();

        let call_offset = 8.into();
        let next_fp_offset = 9.into();

        let target = G.pow(4);

        // Do MVVW and MVVL with an unaccessible source value.
        let instructions = vec![
            [
                Opcode::Mvvw.get_field_elt(),
                next_fp_offset,
                offset1,
                src_addr1,
            ],
            [
                Opcode::Mvvl.get_field_elt(),
                next_fp_offset,
                offset2,
                src_addr2,
            ],
            [Opcode::Mvih.get_field_elt(), next_fp_offset, offset3, imm],
            [
                Opcode::Tailv.get_field_elt(),
                call_offset,
                next_fp_offset,
                0.into(),
            ],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];

        let mut frames = HashMap::new();
        frames.insert(BinaryField32b::one(), 10);
        frames.insert(target, 9);

        let prom = code_to_prom(&instructions);
        let mut vrom = ValueRom::default();
        // Set FP and PC
        vrom.set_u32(0, 0).unwrap();
        vrom.set_u32(1, 0).unwrap();

        // Set src vals.
        let src_val1 = 1;
        let src_val2 = 2;
        vrom.set_u32(src_addr1.val() as u32, src_val1).unwrap();
        vrom.set_u128(src_addr2.val() as u32, src_val2).unwrap();

        // Set target
        vrom.set_u32(call_offset.val() as u32, target.val())
            .unwrap();

        let memory = Memory::new(prom, vrom);

        let mut pc_field_to_int = HashMap::new();
        pc_field_to_int.insert(target, 5);
        let mut interpreter = Interpreter::new(frames, pc_field_to_int);

        let traces = interpreter
            .run(memory)
            .expect("The interpreter should run smoothly.");

        assert!(traces.vrom_pending_updates().is_empty());
        assert!(interpreter.moves_to_apply.is_empty());

        let next_fp = 16;
        assert_eq!(
            traces
                .get_vrom_u32(next_fp as u32 + offset1.val() as u32)
                .unwrap(),
            src_val1
        );
        assert_eq!(
            traces
                .get_vrom_u128(next_fp as u32 + offset2.val() as u32)
                .unwrap(),
            src_val2
        );
        assert_eq!(
            traces
                .get_vrom_u32(next_fp as u32 + offset3.val() as u32)
                .unwrap(),
            imm.val() as u32
        );
    }

    #[test]
    fn test_mv_no_dst_no_src() {
        // Frame
        // Slot 0: Return PC
        // Slot 1: Return FP
        // Slot 2: Storage
        // Slot 3: Padding for alignment
        // Slot 4-7: Src_val
        // Slot 8: Target
        // Slot 9: Next_fp

        let zero = BinaryField16b::zero();
        let offset1 = 2.into();
        let offset2 = 4.into();
        let src_addr = 4.into();
        let offset3 = 8.into();
        let storage = 2.into();
        let imm = 12.into();

        let call_offset = 8.into();
        let next_fp_offset = 9.into();

        // In TAILV, we jump to the PC for RET.
        let target_pc = 6u32;
        let target = G.pow(target_pc as u64 - 1);

        // Do MVVW and MVVL with an unaccessible source value.
        let instructions = vec![
            [
                Opcode::Mvvw.get_field_elt(),
                next_fp_offset,
                offset1,
                src_addr,
            ],
            [
                Opcode::Mvvl.get_field_elt(),
                next_fp_offset,
                offset2,
                src_addr,
            ],
            // The following MOVE operation should be executed, since TAILV sets `next_fp`
            // correctly.
            [
                Opcode::Mvvw.get_field_elt(),
                0.into(),
                storage,
                next_fp_offset,
            ],
            [Opcode::Mvih.get_field_elt(), next_fp_offset, offset3, imm],
            [
                Opcode::Tailv.get_field_elt(),
                call_offset,
                next_fp_offset,
                0.into(),
            ],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];

        let mut frames = HashMap::new();
        frames.insert(BinaryField32b::one(), 10);
        frames.insert(target, 9);

        let prom = code_to_prom(&instructions);
        let mut vrom = ValueRom::default();
        // Set FP and PC
        vrom.set_u32(0, 0).unwrap();
        vrom.set_u32(1, 0).unwrap();

        // Set target
        vrom.set_u32(call_offset.val() as u32, target.val())
            .unwrap();

        // We do not set the src_addr.
        let memory = Memory::new(prom, vrom);

        let mut pc_field_to_int = HashMap::new();
        pc_field_to_int.insert(target, target_pc);
        let mut interpreter = Interpreter::new(frames, pc_field_to_int);

        let traces = interpreter
            .run(memory)
            .expect("The interpreter should run smoothly.");

        let mut pending_updates = HashMap::new();
        let first_move = (
            src_addr.val() as u32, // Address to set
            Opcode::Mvvw,          // Opcode
            BinaryField32b::ONE,   // PC
            0u32,                  // FP
            0u32,                  // Timestamp
            next_fp_offset,        // Dst
            src_addr,              // Src
            offset1,               // Offset
        );
        let second_move = (
            src_addr.val() as u32, // Address to set
            Opcode::Mvvl,          // Opcode
            G,                     // PC
            0u32,                  // FP
            0u32,                  // Timestamp (Only RAM operations increase it)
            next_fp_offset,        // Dst
            src_addr,              // Src
            offset2,               // Offset
        );

        let next_fp = 16;
        pending_updates.insert(next_fp + offset1.val() as u32, vec![first_move]);
        pending_updates.insert(next_fp + offset2.val() as u32, vec![second_move]);

        assert_eq!(traces.vrom_pending_updates().len(), pending_updates.len(), "The expected pending updates are of length {} but the actual pending updates are of length {}", traces.vrom_pending_updates().len(), pending_updates.len());
        for (k, pending_update) in traces.vrom_pending_updates() {
            let expected_update = pending_updates.get(k).unwrap_or_else(|| {
                panic!("Missing expected update {:?} at addr {}", pending_update, k)
            });
            assert_eq!(
                *expected_update, *pending_update,
                "expected update {:?}, but got {:?}",
                *expected_update, *pending_update
            );
        }
        // Check that `next_fp` has been set and the third MOVE operation was carried
        // out correctly,
        assert_eq!(
            traces.get_vrom_u32(next_fp_offset.val() as u32).unwrap(),
            next_fp
        );
        assert_eq!(
            traces.get_vrom_u32(storage.val() as u32).unwrap(),
            traces.get_vrom_u32(next_fp_offset.val() as u32).unwrap()
        );
    }
}
