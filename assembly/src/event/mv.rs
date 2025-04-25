use binius_field::ExtensionField;
use binius_m3::builder::{B16, B32};

use super::context::EventContext;
use crate::{
    event::Event,
    execution::{FramePointer, InterpreterChannels, InterpreterError, ZCrayTrace},
    fire_non_jump_event,
    memory::{MemoryError, VromValueT},
    opcodes::Opcode,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MVKind {
    Mvvw,
    Mvvl,
    Mvih,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MVInfo {
    pub(crate) mv_kind: MVKind,
    pub(crate) dst: B16,
    pub(crate) offset: B16,
    pub(crate) src: B16,
    pub(crate) pc: B32,
    pub(crate) timestamp: u32,
}

/// Convenience macro to implement the [`Event`] trait for MV events.
///
/// It takes as argument the instruction and its corresponding field name in the
/// [`ZCrayTrace`] where such events are being logged.
///
/// # Example
///
/// ```ignore
/// impl_mv_event!(MVVWEvent, mvvw);
/// ```
macro_rules! impl_mv_event {
    ($event:ty, $trace_field:ident) => {
        impl Event for $event {
            fn generate(
                ctx: &mut EventContext,
                arg0: B16,
                arg1: B16,
                arg2: B16,
            ) -> Result<(), InterpreterError> {
                let opt_event = Self::generate_event(ctx, arg0, arg1, arg2)?;
                if let Some(event) = opt_event {
                    ctx.trace.$trace_field.push(event);
                }

                Ok(())
            }

            fn fire(&self, channels: &mut InterpreterChannels) {
                fire_non_jump_event!(self, channels);
            }
        }
    };
}

pub(crate) struct MVEventOutput {
    pub(crate) opcode: Opcode,
    pub(crate) field_pc: B32,    // field PC
    pub(crate) fp: FramePointer, // fp
    pub(crate) timestamp: u32,   // timestamp
    pub(crate) dst: B16,         // dst
    pub(crate) dst_addr: u32,    // dst addr
    pub(crate) src: B16,         // src
    pub(crate) offset: B16,      // offset
    pub(crate) src_val: u128,
}

impl MVEventOutput {
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        opcode: Opcode,
        field_pc: B32,    // field PC
        fp: FramePointer, // fp
        timestamp: u32,   // timestamp
        dst: B16,         // dst
        dst_addr: u32,    // dst addr
        src: B16,         // src
        offset: B16,      // offset
        src_val: u128,
    ) -> Self {
        Self {
            opcode,
            field_pc,
            fp,
            timestamp,
            dst,
            dst_addr,
            src,
            offset,
            src_val,
        }
    }

    pub(crate) fn push_mv_event(&self, trace: &mut ZCrayTrace) {
        let &MVEventOutput {
            opcode,
            field_pc,
            fp,
            timestamp,
            dst,
            dst_addr,
            src,
            offset,
            src_val,
        } = self;

        match opcode {
            Opcode::Mvvl => {
                let new_event = MvvlEvent::new(
                    field_pc,
                    fp,
                    timestamp,
                    dst.val(),
                    dst_addr,
                    src.val(),
                    src_val,
                    offset.val(),
                );
                trace.mvvl.push(new_event);
            }
            Opcode::Mvvw => {
                let new_event = MvvwEvent::new(
                    field_pc,
                    fp,
                    timestamp,
                    dst.val(),
                    dst_addr,
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
pub struct MvvwEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub dst: u16,
    pub dst_addr: u32,
    pub src: u16,
    pub src_val: u32,
    pub offset: u16,
}

// TODO: this is a 4-byte move instruction. So it needs to be updated once we
// have multi-granularity.
impl MvvwEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: B32,
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
        pc: B32,
        timestamp: u32,
        fp: FramePointer,
        dst: B16,
        offset: B16,
        src: B16,
    ) -> Result<Option<Self>, InterpreterError> {
        let dst_addr = ctx.vrom_read::<u32>(ctx.addr(dst.val()))?;
        let dst_addr_offset = dst_addr ^ offset.val() as u32;
        let src_addr = ctx.addr(src.val());
        let src_val_set = ctx.vrom_check_value_set::<u32>(src_addr)?;

        // If we already know the value to set, then we can already push an event.
        // Otherwise, we add the move to the list of MOVE events to be pushed once we
        // have access to the value.
        if src_val_set {
            let src_val = ctx.vrom_read::<u32>(src_addr)?;
            ctx.vrom_write(dst_addr_offset, src_val)?;

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
            ctx.vrom_record_access(dst_addr_offset);
            ctx.trace.insert_pending(
                dst_addr_offset,
                (
                    src_addr,
                    Opcode::Mvvw,
                    pc,
                    *fp,
                    timestamp,
                    dst,
                    dst_addr,
                    src,
                    offset,
                ),
            )?;
            Ok(None)
        }
    }

    pub(crate) fn generate_event(
        ctx: &mut EventContext,
        dst: B16,
        offset: B16,
        src: B16,
    ) -> Result<Option<Self>, InterpreterError> {
        let (_pc, field_pc, fp, timestamp) = ctx.program_state();

        let dst_addr_set = ctx.vrom_check_value_set::<u32>(ctx.addr(dst.val()))?;
        let src_val_set = ctx.vrom_check_value_set::<u32>(ctx.addr(src.val()))?;

        // If `dst_addr` is set, we check whether the value at the destination is
        // already set. If that's the case, we can set the source value.
        if dst_addr_set {
            let dst_addr = ctx.vrom_read::<u32>(ctx.addr(dst.val()))?;
            let dst_val_set = ctx.vrom_check_value_set::<u32>(dst_addr ^ offset.val() as u32)?;
            // If the destination value is set, we set the source value.
            if dst_val_set {
                let dst_val = ctx.vrom_read::<u32>(dst_addr ^ offset.val() as u32)?;
                execute_mv(ctx, ctx.addr(src.val()), dst_val)?;

                return Ok(Some(Self {
                    pc: field_pc,
                    fp,
                    timestamp,
                    dst: dst.val(),
                    dst_addr,
                    src: src.val(),
                    src_val: dst_val,
                    offset: offset.val(),
                }));
            }
        }
        // If the source value is missing or the destination address is still unknown,
        // it means we are in a MOVE that precedes a CALL, and we have to handle the
        // MOVE operation later.
        if !dst_addr_set || !src_val_set {
            delegate_move(ctx, MVKind::Mvvw, dst, offset, src, field_pc, timestamp);
            return Ok(None);
        }

        let dst_addr = ctx.vrom_read::<u32>(ctx.addr(dst.val()))?;
        let src_val = ctx.vrom_read::<u32>(ctx.addr(src.val()))?;

        execute_mv(ctx, dst_addr ^ offset.val() as u32, src_val)?;

        Ok(Some(Self {
            pc: field_pc,
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

impl_mv_event!(MvvwEvent, mvvw);

/// Event for MVV.L.
///
/// Performs a MOVE of 16-byte value between VROM addresses.
///
/// Logic:
///   1. VROM128[FP[dst] + offset] = FP128[src]
#[derive(Debug, Clone)]
pub struct MvvlEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub dst: u16,
    pub dst_addr: u32,
    pub src: u16,
    pub src_val: u128,
    pub offset: u16,
}

impl MvvlEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: B32,
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
        pc: B32,
        timestamp: u32,
        fp: FramePointer,
        dst: B16,
        offset: B16,
        src: B16,
    ) -> Result<Option<Self>, InterpreterError> {
        let dst_addr = ctx.vrom_read::<u32>(ctx.addr(dst.val()))?;
        let dst_addr_offset = dst_addr ^ offset.val() as u32;
        let src_addr = ctx.addr(src.val());
        let src_val_set = ctx.vrom_check_value_set::<u128>(src_addr)?;

        // If we already know the value to set, then we can already push an event.
        // Otherwise, we add the move to the list of MOVE events to be pushed once we
        // have access to the value.
        if src_val_set {
            let src_val = ctx.vrom_read::<u128>(src_addr)?;
            ctx.vrom_write(dst_addr_offset, src_val)?;

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
            ctx.vrom_record_access(dst_addr_offset);
            ctx.trace.insert_pending(
                dst_addr_offset,
                (
                    src_addr,
                    Opcode::Mvvl,
                    pc,
                    *fp,
                    timestamp,
                    dst,
                    dst_addr,
                    src,
                    offset,
                ),
            )?;
            Ok(None)
        }
    }

    pub(crate) fn generate_event(
        ctx: &mut EventContext,
        dst: B16,
        offset: B16,
        src: B16,
    ) -> Result<Option<Self>, InterpreterError> {
        let (_pc, field_pc, fp, timestamp) = ctx.program_state();

        let dst_addr_set = ctx.vrom_check_value_set::<u32>(ctx.addr(dst.val()))?;
        let src_val_set = ctx.vrom_check_value_set::<u128>(ctx.addr(src.val()))?;

        // If `dst_addr` is set, we check whether the value at the destination is
        // already set. If that's the case, we can set the source value.
        if dst_addr_set {
            let dst_addr = ctx.vrom_read::<u32>(ctx.addr(dst.val()))?;
            let dst_val_set = ctx.vrom_check_value_set::<u128>(dst_addr ^ offset.val() as u32)?;
            // If the destination value is set, we set the source value.
            if dst_val_set {
                let dst_val = ctx.vrom_read::<u128>(dst_addr ^ offset.val() as u32)?;
                execute_mv(ctx, ctx.addr(src.val()), dst_val)?;

                return Ok(Some(Self {
                    pc: field_pc,
                    fp,
                    timestamp,
                    dst: dst.val(),
                    dst_addr,
                    src: src.val(),
                    src_val: dst_val,
                    offset: offset.val(),
                }));
            }
        }
        // If the source value is missing or the destination address is still unknown,
        // it means we are in a MOVE that precedes a CALL, and we have to handle the
        // MOVE operation later.
        if !dst_addr_set || !src_val_set {
            delegate_move(ctx, MVKind::Mvvl, dst, offset, src, field_pc, timestamp);
            return Ok(None);
        }

        let dst_addr = ctx.vrom_read::<u32>(ctx.addr(dst.val()))?;
        let src_val = ctx.vrom_read::<u128>(ctx.addr(src.val()))?;

        execute_mv(ctx, dst_addr ^ offset.val() as u32, src_val)?;

        Ok(Some(Self {
            pc: field_pc,
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

impl_mv_event!(MvvlEvent, mvvl);

/// Event for MVI.H.
///
/// Performs a MOVE of 2-byte value from a 16-bit immediate into a VROM address,
/// zero-extending to 32-bits.
///
/// Logic:
///   1. VROM[FP[dst] + offset] = ZeroExtend(imm)
#[derive(Debug, Clone)]
pub struct MvihEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub dst: u16,
    pub dst_addr: u32,
    pub imm: u16,
    pub offset: u16,
}

// TODO: this is a 2-byte move instruction, which sets a 4 byte address to imm
// zero-extended. So it needs to be updated once we have multi-granularity.
impl MvihEvent {
    /// This method is called once the next_fp has been set by the CALL
    /// procedure.
    pub(crate) fn generate_event_from_info(
        ctx: &mut EventContext,
        pc: B32,
        timestamp: u32,
        fp: FramePointer,
        dst: B16,
        offset: B16,
        imm: B16,
    ) -> Result<Self, InterpreterError> {
        // At this point, since we are in a call procedure, `dst` corresponds to the
        // next_fp. And we know it has already been set, so we can read
        // the destination address.
        let dst_addr = ctx.vrom_read::<u32>(ctx.addr(dst.val()))?;

        ctx.vrom_write(dst_addr ^ offset.val() as u32, imm.val() as u32)?;

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

    pub(crate) fn generate_event(
        ctx: &mut EventContext,
        dst: B16,
        offset: B16,
        imm: B16,
    ) -> Result<Option<Self>, InterpreterError> {
        let (_pc, field_pc, fp, timestamp) = ctx.program_state();

        let dst_addr_set = ctx.vrom_check_value_set::<u32>(ctx.addr(dst.val()))?;

        // If the destination address is still unknown, it means we are in a MOVE that
        // precedes a CALL, and we have to handle the MOVE operation later.
        if dst_addr_set {
            let dst_addr = ctx.vrom_read::<u32>(ctx.addr(dst.val()))?;
            execute_mv(ctx, dst_addr ^ offset.val() as u32, imm.val() as u32)?;

            Ok(Some(Self {
                pc: field_pc,
                fp,
                timestamp,
                dst: dst.val(),
                dst_addr,
                imm: imm.val(),
                offset: offset.val(),
            }))
        } else {
            delegate_move(ctx, MVKind::Mvih, dst, offset, imm, field_pc, timestamp);
            Ok(None)
        }
    }
}

impl_mv_event!(MvihEvent, mvih);

/// Event for LDI (Load Immediate).
///
/// Performs a load of an immediate value into a VROM address.
///
/// Logic:
///   1. FP[dst] = imm
#[derive(Debug, Clone)]
pub struct LdiEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub dst: u16,
    pub imm: u32,
}

impl LdiEvent {
    pub(crate) fn generate_event(
        ctx: &mut EventContext,
        dst: B16,
        imm_low: B16,
        imm_high: B16,
    ) -> Result<Option<Self>, InterpreterError> {
        let (_pc, field_pc, fp, timestamp) = ctx.program_state();

        let imm =
            B32::from_bases([imm_low, imm_high]).map_err(|_| InterpreterError::InvalidInput)?;

        execute_mv(ctx, ctx.addr(dst.val()), imm.val())?;

        Ok(Some(Self {
            pc: field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            imm: imm.val(),
        }))
    }
}

impl_mv_event!(LdiEvent, ldi);

/// If the source value of a MOVE operations is missing or the destination
/// address is still unknown, it means we are in a MOVE that precedes a CALL,
/// and we have to handle the MOVE operation later.
fn delegate_move(
    ctx: &mut EventContext,
    mv_kind: MVKind,
    dst: B16,
    offset: B16,
    src: B16,
    pc: B32,
    timestamp: u32,
) {
    let new_mv_info = MVInfo {
        mv_kind,
        dst,
        offset,
        src,
        pc,
        timestamp,
    };

    // This move needs to be handled later, in the CALL.
    ctx.moves_to_apply.push(new_mv_info);
    ctx.incr_pc();
}

fn execute_mv<T: VromValueT>(
    ctx: &mut EventContext,
    dst_addr: u32,
    value: T,
) -> Result<(), MemoryError> {
    ctx.vrom_write(dst_addr, value)?;
    ctx.incr_pc();

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use binius_field::{Field, PackedField};
    use binius_m3::builder::{B16, B32};

    use crate::{
        event::mv::{MVInfo, MVKind},
        execution::{Interpreter, G},
        isa::GenericISA,
        memory::Memory,
        opcodes::Opcode,
        util::code_to_prom,
        ValueRom,
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
        // Slot 6-7: Padding for alignment
        // Slot 8-11: dst_storage2
        // Slot 12-15: src_val2: not written yet.

        let zero = B16::zero();
        let dst_addr1 = 2.into();
        let offset1 = 3.into();
        let src_addr1 = 4.into();
        let dst_addr2 = 5.into();
        let offset2 = 8.into();
        let src_addr2 = 12.into();
        // Do MVVW and MVVL with an unaccessible source value.
        let instructions = vec![
            [Opcode::Mvvw.get_field_elt(), dst_addr1, offset1, src_addr1],
            [Opcode::Mvvl.get_field_elt(), dst_addr2, offset2, src_addr2],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];

        let mut frames = HashMap::new();
        frames.insert(B32::one(), 16);

        let prom = code_to_prom(&instructions);
        let mut vrom = ValueRom::default();
        vrom.write(0, 0u32).unwrap();
        vrom.write(1, 0u32).unwrap();
        vrom.write(2, 0u32).unwrap();
        vrom.write(5, 0u32).unwrap();

        let memory = Memory::new(prom, vrom);

        let mut interpreter = Interpreter::new(Box::new(GenericISA), frames, HashMap::new());

        let _ = interpreter
            .run(memory)
            .expect("The interpreter should run smoothly.");

        // Check that `moves_to_apply` contains the two MOVE events.
        let first_move = MVInfo {
            mv_kind: MVKind::Mvvw,
            dst: dst_addr1,
            offset: offset1,
            src: src_addr1,
            pc: B32::ONE,
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

        let zero = B16::zero();
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
        frames.insert(B32::one(), 10);
        frames.insert(target, 9);

        let prom = code_to_prom(&instructions);
        let mut vrom = ValueRom::default();
        // Set FP and PC
        vrom.write(0, 0u32).unwrap();
        vrom.write(1, 0u32).unwrap();

        // Set src vals.
        let src_val1 = 1u32;
        let src_val2 = 2u128;
        vrom.write(src_addr1.val() as u32, src_val1).unwrap();
        vrom.write(src_addr2.val() as u32, src_val2).unwrap();

        // Set target
        vrom.write(call_offset.val() as u32, target.val()).unwrap();

        let memory = Memory::new(prom, vrom);

        let mut pc_field_to_int = HashMap::new();
        pc_field_to_int.insert(target, 5);
        let mut interpreter = Interpreter::new(Box::new(GenericISA), frames, pc_field_to_int);

        let traces = interpreter
            .run(memory)
            .expect("The interpreter should run smoothly.");

        assert!(traces.vrom_pending_updates().is_empty());
        assert!(interpreter.moves_to_apply.is_empty());

        let next_fp = 16;
        assert_eq!(
            traces
                .vrom()
                .read::<u32>(next_fp as u32 + offset1.val() as u32)
                .unwrap(),
            src_val1
        );
        assert_eq!(
            traces
                .vrom()
                .read::<u128>(next_fp as u32 + offset2.val() as u32)
                .unwrap(),
            src_val2
        );
        assert_eq!(
            traces
                .vrom()
                .read::<u32>(next_fp as u32 + offset3.val() as u32)
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

        let zero = B16::zero();
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
        frames.insert(B32::one(), 10);
        frames.insert(target, 9);

        let prom = code_to_prom(&instructions);
        let mut vrom = ValueRom::default();
        // Set FP and PC
        vrom.write(0, 0u32).unwrap();
        vrom.write(1, 0u32).unwrap();

        // Set target
        vrom.write(call_offset.val() as u32, target.val()).unwrap();

        // We do not set the src_addr.
        let memory = Memory::new(prom, vrom);

        let mut pc_field_to_int = HashMap::new();
        pc_field_to_int.insert(target, target_pc);
        let mut interpreter = Interpreter::new(Box::new(GenericISA), frames, pc_field_to_int);

        let traces = interpreter
            .run(memory)
            .expect("The interpreter should run smoothly.");

        let next_fp = 16;
        let mut pending_updates = HashMap::new();
        let first_move = (
            src_addr.val() as u32, // Address to set
            Opcode::Mvvw,          // Opcode
            B32::ONE,              // PC
            0u32,                  // FP
            0u32,                  // Timestamp
            next_fp_offset,        // Dst
            next_fp,               // Dst addr
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
            next_fp,               // Dst addr
            src_addr,              // Src
            offset2,               // Offset
        );

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
            traces
                .vrom()
                .read::<u32>(next_fp_offset.val() as u32)
                .unwrap(),
            next_fp
        );
        assert_eq!(
            traces.vrom().read::<u32>(storage.val() as u32).unwrap(),
            traces
                .vrom()
                .read::<u32>(next_fp_offset.val() as u32)
                .unwrap()
        );
    }

    #[test]
    fn test_mv_dst_no_src() {
        // Frame
        // Slot 0: Return PC
        // Slot 1: Return FP
        // Slot 2: Padding for alignment
        // Slot 3: Padding for alignment
        // Slot 4-7: Storage
        // Slot 8-11: Src_val_mvvl
        // Slot 12: Src_val_mvvw

        let cur_fp = B16::one();
        let zero = B16::zero();
        let storage_offsets = (4..8).map(|i| i.into()).collect::<Vec<_>>();
        let src_addr_mvvl = 8.into();
        let src_addr_mvvw = 12.into();
        let imm = 12.into();

        // Do MVVW and MVVL with an unaccessible source value.
        // The value is previously set by MVI.H. This way, the two source values can be
        // set orrectly.
        let instructions = vec![
            // Store `imm` into the storage value.
            [
                Opcode::Mvih.get_field_elt(),
                cur_fp,
                storage_offsets[0],
                imm,
            ],
            [
                Opcode::Mvih.get_field_elt(),
                cur_fp,
                storage_offsets[1],
                zero,
            ],
            [
                Opcode::Mvih.get_field_elt(),
                cur_fp,
                storage_offsets[2],
                zero,
            ],
            [
                Opcode::Mvih.get_field_elt(),
                cur_fp,
                storage_offsets[3],
                zero,
            ],
            // Set the source value for MVV.L
            [
                Opcode::Mvvl.get_field_elt(),
                cur_fp,
                storage_offsets[0],
                src_addr_mvvl,
            ],
            // Set the source value for MVV.W
            [
                Opcode::Mvvw.get_field_elt(),
                cur_fp,
                storage_offsets[0],
                src_addr_mvvw,
            ],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];

        let mut frames = HashMap::new();
        frames.insert(B32::one(), 13);

        let prom = code_to_prom(&instructions);
        let mut vrom = ValueRom::default();
        // Set FP and PC
        vrom.write(0, 0u32).unwrap();
        vrom.write(1, 0u32).unwrap();

        // We do not set `src_addr_mvvl` and `src_val_mvvw`.
        let memory = Memory::new(prom, vrom);

        let pc_field_to_int = HashMap::new();
        let mut interpreter = Interpreter::new(Box::new(GenericISA), frames, pc_field_to_int);

        let traces = interpreter
            .run(memory)
            .expect("The interpreter should run smoothly.");

        assert_eq!(
            traces
                .vrom()
                .read::<u128>(storage_offsets[0].val() as u32)
                .unwrap(),
            imm.val() as u128
        );
        assert_eq!(
            traces
                .vrom()
                .read::<u128>(src_addr_mvvl.val() as u32)
                .unwrap(),
            imm.val() as u128
        );
        assert_eq!(
            traces
                .vrom()
                .read::<u32>(src_addr_mvvw.val() as u32)
                .unwrap(),
            12
        );
    }

    #[test]
    fn test_normal_mv() {
        // Frame
        // Slot 0: Return PC
        // Slot 1: Return FP
        // Slot 2: dst_addr
        // Slot 3: Padding for alignment
        // Slot 4-7: Storage MVIH
        // Slot 8-11: Storage MVVL
        // Slot 12: Storage MVVW

        let cur_fp = B16::one();
        let zero = B16::zero();
        let dst = 2;
        let dst_val = 4;
        let storage_mvih_offsets = (4..8).map(|i| i.into()).collect::<Vec<_>>();
        let storage_mvvl = (8 ^ dst_val).into();
        let storage_mvvw = (12 ^ dst_val).into();
        let imm = 12.into();

        // Do MVVW and MVVL with an unaccessible source value.
        // The value is previously set by MVI.H. This way, the two source values can be
        // set orrectly.
        let instructions = vec![
            // Store `imm` into the storage value.
            [
                Opcode::Mvih.get_field_elt(),
                cur_fp,
                storage_mvih_offsets[0],
                imm,
            ],
            [
                Opcode::Mvih.get_field_elt(),
                cur_fp,
                storage_mvih_offsets[1],
                zero,
            ],
            [
                Opcode::Mvih.get_field_elt(),
                cur_fp,
                storage_mvih_offsets[2],
                zero,
            ],
            [
                Opcode::Mvih.get_field_elt(),
                cur_fp,
                storage_mvih_offsets[3],
                zero,
            ],
            // Set the source value for MVV.L. We use `dst` here to ensure the computation is
            // correct in `generate_event`.
            [
                Opcode::Mvvl.get_field_elt(),
                dst.into(),
                storage_mvvl,
                storage_mvih_offsets[0],
            ],
            // Set the source value for MVV.W
            [
                Opcode::Mvvw.get_field_elt(),
                dst.into(),
                storage_mvvw,
                storage_mvih_offsets[0],
            ],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];

        let mut frames = HashMap::new();
        frames.insert(B32::one(), 13);

        let prom = code_to_prom(&instructions);
        let mut vrom = ValueRom::default();
        // Set FP and PC
        vrom.write(0, 0u32).unwrap();
        vrom.write(1, 0u32).unwrap();
        // Set the destination address.
        vrom.write(dst as u32, dst_val as u32).unwrap();

        // We do not set `src_addr_mvvl` and `src_val_mvvw`.
        let memory = Memory::new(prom, vrom);

        let pc_field_to_int = HashMap::new();
        let mut interpreter = Interpreter::new(Box::new(GenericISA), frames, pc_field_to_int);

        let traces = interpreter
            .run(memory)
            .expect("The interpreter should run smoothly.");

        assert_eq!(
            traces
                .vrom()
                .read::<u128>(storage_mvih_offsets[0].val() as u32)
                .unwrap(),
            imm.val() as u128
        );
        assert_eq!(
            traces
                .vrom()
                .read::<u128>((storage_mvvl.val() ^ dst_val) as u32)
                .unwrap(),
            imm.val() as u128
        );
        assert_eq!(
            traces
                .vrom()
                .read::<u32>((storage_mvvw.val() ^ dst_val) as u32)
                .unwrap(),
            imm.val() as u32
        );
    }
}
