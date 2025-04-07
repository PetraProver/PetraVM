// use binius_field::Field;
// use binius_m3::{
//     builder::{
//         upcast_col, Col, ConstraintSystem, TableFiller, TableId,
// TableWitnessSegment, B1, B128,         B16, B32, B64,
//     },
//     gadgets::u32::U32Add,
// };
// use zcrayvm_assembly::{event::integer_ops::AddEvent, opcodes::Opcode};

// use super::cpu::{CpuColumns, CpuColumnsOptions, NextPc};
// use crate::{
//     channels::Channels,
//     types::CommonTableBounds,
//     utils::{pack_instruction_b128, pack_instruction_no_args},
// };

// const ADD_OPCODE: u16 = Opcode::Add as u16;

// /// ADD table.
// ///
// /// This table handles the ADD instruction, which returns from a function
// /// call by loading the return PC and FP from the current frame.
// ///
// /// Logic:
// /// 1. Load the current PC and FP from the state channel
// /// 2. Get the instruction from PROM channel
// /// 3. Verify this is a RET instruction
// /// 4. Load the return PC from VROM[fp+0] and return FP from VROM[fp+1]
// /// 5. Update the state with the new PC and FP values
// pub struct AddiTable {
//     id: TableId,
//     // TODO: Use the cpu gadget
//     cpu_cols: CpuColumns<{ Opcode::Add as u16 }>,
//     dst_abs: Col<B32>, // Virtual
//     dst_val_packed: Col<B32>,
//     src_abs: Col<B32>, // Virtual
//     src_val: Col<B1, 32>,
//     src_val_packed: Col<B32>,
//     imm_unpacked: Col<B1, 16>,
//     imm_unpacked_32b: Col<B1, 32>,
//     imm_upscaled: Col<B32>, // Virtual
//     u32_add: U32Add,

//     vrom_src1: Col<B64>,
//     vrom_src2: Col<B64>,
//     vrom_dst: Col<B64>,
// }

// impl AddiTable {
//     pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
//         let mut table = cs.add_table("add");

//         let Channels {
//             state_channel,
//             prom_channel,
//             vrom_channel,
//             ..
//         } = *channels;

//         let cpu_cols = CpuColumns::new(
//             &mut table,
//             state_channel,
//             prom_channel,
//             CpuColumnsOptions {
//                 next_pc: NextPc::Increment,
//                 next_fp: None,
//             },
//         );

//         let dst_abs = table.add_computed("dst", cpu_cols.fp +
// upcast_col(cpu_cols.arg0));

//         let src_abs = table.add_computed("src", cpu_cols.fp +
// upcast_col(cpu_cols.arg1));         let src_val =
// table.add_committed("src_val");         let src1_val_packed =
// table.add_packed("src1_val_packed", src_val);

//         let imm_unpacked_32b = table.add_committed("imm_unpacked");
//         let imm_packed_

//         let u32_add = U32Add::new(&mut table, src_val, src2_val,
// U32AddFlags::default());         let dst_val_packed =
// table.add_packed("dst_val_packed", u32_add.zout);

//         // Read src1
//         let vrom_src1 = pack_vrom_entry(&mut table, "vrom_src1", src1_abs,
// src1_val_packed);         table.pull(vrom_channel, [vrom_src1]);
//         // Read src2
//         let vrom_src2 = pack_vrom_entry(&mut table, "vrom_src2", src2_abs,
// src2_val_packed);         table.pull(vrom_channel, [vrom_src2]);
//         // Write dst
//         let vrom_dst = pack_vrom_entry(&mut table, "vrom_dst", dst_abs,
// dst_val_packed.into());         table.pull(vrom_channel, [vrom_dst]);

//         Self {
//             id: table.id(),
//             cpu_cols,
//             dst_abs,
//             src1_abs,
//             src1_val,
//             src2_abs,
//             src2_val,
//             src1_val_packed,
//             src2_val_packed,
//             u32_add,
//             dst_val_packed,
//             vrom_src1,
//             vrom_src2,
//             vrom_dst,
//         }
//     }
// }

// impl<U: UnderlierType> TableFiller<U> for AddTable
// where
//     U: Pod + PackScalar<B1>,
// {
//     type Event = AddEvent;

//     fn id(&self) -> TableId {
//         self.id
//     }

//     fn fill<'a>(
//         &self,
//         rows: impl Iterator<Item = &'a Self::Event> + Clone,
//         witness: &'a mut TableWitnessIndexSegment<U>,
//     ) -> Result<(), anyhow::Error> {
//         {
//             let mut dst_abs = witness.get_mut_as(self.dst_abs)?;
//             let mut src1_abs = witness.get_mut_as(self.src1_abs)?;
//             let mut src2_abs = witness.get_mut_as(self.src2_abs)?;
//             let mut src1_val = witness.get_mut_as(self.src1_val)?;
//             let mut src2_val = witness.get_mut_as(self.src2_val)?;
//             let mut vrom_src1 = witness.get_mut_as(self.vrom_src1)?;
//             let mut vrom_src2 = witness.get_mut_as(self.vrom_src2)?;
//             let mut vrom_dst = witness.get_mut_as(self.vrom_dst)?;
//             for (i, event) in rows.clone().enumerate() {
//                 dst_abs[i] = event.fp ^ (event.dst as u32);
//                 src1_abs[i] = event.fp ^ (event.src1 as u32);
//                 src2_abs[i] = event.fp ^ (event.src2 as u32);
//                 src1_val[i] = event.src1_val;
//                 src2_val[i] = event.src2_val;
//                 vrom_src1[i] = pack_vrom_entry_u64(event.src1 as u32,
// event.src1_val);                 vrom_src2[i] =
// pack_vrom_entry_u64(event.src2 as u32, event.src2_val);                 
// vrom_dst[i] = pack_vrom_entry_u64(event.dst as u32, event.dst_val);
//                 dbg!(
//                     "Add fill",
//                     src1_val[i],
//                     src2_val[i],
//                     vrom_src1[i],
//                     vrom_src2[i],
//                     vrom_dst[i],
//                 );
//                 println!(
//                     "vrom_scr1 = {:x}, vrom_src2 = {:x}, vrom_dst = {:x}",
//                     vrom_src1[i], vrom_src2[i], vrom_dst[i]
//                 );
//             }
//         }
//         let cpu_rows = rows.map(|event| CpuEvent {
//             pc: event.pc.into(),
//             next_pc: None,
//             next_fp: None,
//             fp: event.fp,
//             arg0: event.dst,
//             arg1: event.src1,
//             arg2: event.src2,
//         });
//         self.cpu_cols.populate(witness, cpu_rows)?;
//         self.u32_add.populate(witness)
//     }
// }
