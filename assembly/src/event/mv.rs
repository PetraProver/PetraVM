use binius_m3::builder::{B16, B32};

use super::context::EventContext;
use crate::{
    event::Event,
    execution::{FramePointer, InterpreterChannels, InterpreterError},
    macros::fire_non_jump_event,
    memory::{MemoryError, VromValueT},
};

/// Convenience macro to implement the [`Event`] trait for MV events.
///
/// It takes as argument the instruction and its corresponding field name in the
/// [`PetraTrace`] where such events are being logged.
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

    pub(crate) fn generate_event(
        ctx: &mut EventContext,
        dst: B16,
        offset: B16,
        src: B16,
    ) -> Result<Option<Self>, InterpreterError> {
        let (_pc, field_pc, fp, timestamp) = ctx.program_state();

        let src_val_set = ctx.vrom_check_value_set::<u32>(ctx.addr(src.val()))?;
        let dst_addr = ctx.vrom_read::<u32>(ctx.addr(dst.val()))?;

        if src_val_set {
            let src_val = ctx.vrom_read::<u32>(ctx.addr(src.val()))?;
            execute_mv(ctx, dst_addr ^ offset.val() as u32, src_val)?;
            if ctx.prover_only {
                Ok(None)
            } else {
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
        } else {
            // If the destination value is set, we set the source value.
            let dst_val = ctx.vrom_read::<u32>(dst_addr ^ offset.val() as u32)?;
            execute_mv(ctx, ctx.addr(src.val()), dst_val)?;
            if ctx.prover_only {
                Ok(None)
            } else {
                Ok(Some(Self {
                    pc: field_pc,
                    fp,
                    timestamp,
                    dst: dst.val(),
                    dst_addr,
                    src: src.val(),
                    src_val: dst_val,
                    offset: offset.val(),
                }))
            }
        }
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

    pub(crate) fn generate_event(
        ctx: &mut EventContext,
        dst: B16,
        offset: B16,
        src: B16,
    ) -> Result<Option<Self>, InterpreterError> {
        let (_pc, field_pc, fp, timestamp) = ctx.program_state();

        let src_val_set = ctx.vrom_check_value_set::<u128>(ctx.addr(src.val()))?;
        let dst_addr = ctx.vrom_read::<u32>(ctx.addr(dst.val()))?;

        if src_val_set {
            let src_val = ctx.vrom_read::<u128>(ctx.addr(src.val()))?;

            execute_mv(ctx, dst_addr ^ offset.val() as u32, src_val)?;

            if ctx.prover_only {
                Ok(None)
            } else {
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
        } else {
            // If the destination value is set, we set the source value.
            let dst_val = ctx.vrom_read::<u128>(dst_addr ^ offset.val() as u32)?;

            execute_mv(ctx, ctx.addr(src.val()), dst_val)?;

            if ctx.prover_only {
                Ok(None)
            } else {
                Ok(Some(Self {
                    pc: field_pc,
                    fp,
                    timestamp,
                    dst: dst.val(),
                    dst_addr,
                    src: src.val(),
                    src_val: dst_val,
                    offset: offset.val(),
                }))
            }
        }
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

impl MvihEvent {
    pub(crate) fn generate_event(
        ctx: &mut EventContext,
        dst: B16,
        offset: B16,
        imm: B16,
    ) -> Result<Option<Self>, InterpreterError> {
        let (_pc, field_pc, fp, timestamp) = ctx.program_state();

        let dst_addr = ctx.vrom_read::<u32>(ctx.addr(dst.val()))?;

        execute_mv(ctx, dst_addr ^ offset.val() as u32, imm.val() as u32)?;

        if ctx.prover_only {
            Ok(None)
        } else {
            Ok(Some(Self {
                pc: field_pc,
                fp,
                timestamp,
                dst: dst.val(),
                dst_addr,
                imm: imm.val(),
                offset: offset.val(),
            }))
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
        let imm = B32::new(imm_low.val() as u32 + ((imm_high.val() as u32) << 16));

        execute_mv(ctx, ctx.addr(dst.val()), imm.val())?;

        if ctx.prover_only {
            Ok(None)
        } else {
            let (_pc, field_pc, fp, timestamp) = ctx.program_state();

            Ok(Some(Self {
                pc: field_pc,
                fp,
                timestamp,
                dst: dst.val(),
                imm: imm.val(),
            }))
        }
    }
}

impl_mv_event!(LdiEvent, ldi);

fn execute_mv<T: VromValueT>(
    ctx: &mut EventContext,
    dst_addr: u32,
    value: T,
) -> Result<(), MemoryError> {
    ctx.vrom_write(dst_addr, value)?;
    ctx.incr_counters();

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use binius_field::PackedField;
    use binius_m3::builder::{B16, B32};

    use crate::{
        execution::{Interpreter, G},
        isa::GenericISA,
        memory::Memory,
        opcodes::Opcode,
        test_util::code_to_prom_no_prover_only,
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
        let src1_val = 20u32;
        let src2_val = 30u32;
        // Do MVVW and MVVL with an unaccessible source value.
        let instructions = vec![
            [Opcode::Mvvw.get_field_elt(), dst_addr1, offset1, src_addr1],
            [Opcode::Mvvl.get_field_elt(), dst_addr2, offset2, src_addr2],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];

        let mut frames = HashMap::new();
        frames.insert(B32::one(), 16);

        let prom = code_to_prom_no_prover_only(&instructions);
        let mut vrom = ValueRom::default();
        vrom.write(0, 0u32, false).unwrap();
        vrom.write(1, 0u32, false).unwrap();
        vrom.write(2, 0u32, false).unwrap();
        vrom.write(3, src1_val, false).unwrap();
        vrom.write(5, 0u32, false).unwrap();
        vrom.write(8, src2_val, false).unwrap();
        vrom.write(9, 0u32, false).unwrap();
        vrom.write(10, 0u32, false).unwrap();
        vrom.write(11, 0u32, false).unwrap();

        let memory = Memory::new(prom, vrom);

        let mut interpreter = Interpreter::new(Box::new(GenericISA), frames, HashMap::new());

        let trace = interpreter
            .run(memory)
            .expect("The interpreter should run smoothly.");

        assert_eq!(
            trace.vrom().read::<u32>(src_addr1.val() as u32).unwrap(),
            src1_val
        );
        assert_eq!(
            trace.vrom().read::<u128>(src_addr2.val() as u32).unwrap(),
            src2_val as u128
        );
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
        let next_fp = 16u32;

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

        let prom = code_to_prom_no_prover_only(&instructions);
        let mut vrom = ValueRom::default();
        // Set FP and PC
        vrom.write(0, 0u32, false).unwrap();
        vrom.write(1, 0u32, false).unwrap();

        // Set next frame pointer.
        vrom.write(next_fp_offset.val() as u32, next_fp, false)
            .unwrap();
        // Set src vals.
        let src_val1 = 1u32;
        let src_val2 = 2u128;
        vrom.write(src_addr1.val() as u32, src_val1, false).unwrap();
        vrom.write(src_addr2.val() as u32, src_val2, false).unwrap();

        // Set target
        vrom.write(call_offset.val() as u32, target.val(), false)
            .unwrap();

        let memory = Memory::new(prom, vrom);

        let mut pc_field_to_index_pc = HashMap::new();
        pc_field_to_index_pc.insert(target, (4, 5));
        let mut interpreter = Interpreter::new(Box::new(GenericISA), frames, pc_field_to_index_pc);

        let traces = interpreter
            .run(memory)
            .expect("The interpreter should run smoothly.");

        assert_eq!(
            traces
                .vrom()
                .read::<u32>(next_fp + offset1.val() as u32)
                .unwrap(),
            src_val1
        );
        assert_eq!(
            traces
                .vrom()
                .read::<u128>(next_fp + offset2.val() as u32)
                .unwrap(),
            src_val2
        );
        assert_eq!(
            traces
                .vrom()
                .read::<u32>(next_fp + offset3.val() as u32)
                .unwrap(),
            imm.val() as u32
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

        let prom = code_to_prom_no_prover_only(&instructions);
        let mut vrom = ValueRom::default();
        // Set FP and PC
        vrom.write(0, 0u32, false).unwrap();
        vrom.write(1, 0u32, false).unwrap();

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

        let prom = code_to_prom_no_prover_only(&instructions);
        let mut vrom = ValueRom::default();
        // Set FP and PC
        vrom.write(0, 0u32, false).unwrap();
        vrom.write(1, 0u32, false).unwrap();
        // Set the destination address.
        vrom.write(dst as u32, dst_val as u32, false).unwrap();

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
