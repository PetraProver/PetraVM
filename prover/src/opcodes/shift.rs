use binius_core::oracle::ShiftVariant;
use binius_field::underlier::U1;
use binius_m3::builder::{
    upcast_col, Col, ConstraintSystem, Expr, TableFiller, TableId, TableWitnessSegment, B1, B16,
    B32,
};
use zcrayvm_assembly::{Opcode, SrliEvent};

use crate::{
    channels::Channels,
    gadgets::cpu::{CpuColumns, CpuColumnsOptions, CpuGadget},
    types::ProverPackedField,
};

/// Maximum number of bits of the shift amount, i.e. 0 < shift_ammount < 1 <<
/// SHIFT_MAX_BITS - 1 = 31 where dst_val = src_val >> shift_amount or dst_val =
/// src_val << shift_amount
const MAX_SHIFT_BITS: usize = 5;

/// Table for the SRLI (Shift Right Logical Immediate) instruction. It
/// constraints the values src_val  to be equal to dst_val << shift_amount. The
/// shift amount is given as an immediate. In addition to the standard CPU
/// columns and src, dst columns, it also includes `MAX_SHIFT_BITS` partial
/// shift columns to constraint intermediate results of the shift operation.
pub struct SrliTable {
    id: TableId,
    cpu_cols: CpuColumns<{ Opcode::Srli as u16 }>,
    /// Partial shift columns containing intermediate results of the shift.
    partial_shift: [Col<B1, 32>; MAX_SHIFT_BITS],
    dst_abs: Col<B32>, // Virtual
    dst_val: Col<B32>, // Virtual
    src_abs: Col<B32>, // Virtual
    src_val: Col<B32>, // Virtual
    /// Packed partial shift columns containing intermediate results of the
    /// shift.
    shifted: Vec<Col<B1, 32>>, // Virtual
    /// Binary decomposition of the shifted amount.
    imm_bit: Vec<Col<B1>>, // Virtual
}

impl SrliTable {
    pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("srli");
        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions::default(),
        );

        let src_val_unpacked: Col<B1, 32> = table.add_committed("src_val_unpacked");
        let src_val: Col<B32> = table.add_packed("src_val", src_val_unpacked);
        let dst_abs = table.add_computed("dst_abs", cpu_cols.fp + upcast_col(cpu_cols.arg0));
        let src_abs = table.add_computed("src_abs", cpu_cols.fp + upcast_col(cpu_cols.arg1));

        let partial_shift =
            core::array::from_fn(|i| table.add_committed(format!("partial_shift_{i}")));
        let mut shifted = Vec::with_capacity(MAX_SHIFT_BITS);
        let mut imm_bit = Vec::with_capacity(MAX_SHIFT_BITS);

        // Note that even though the immediate is 16 bits, we only need to
        // shift by 5 bits.
        let imm = cpu_cols.arg2_unpacked;
        let mut current_shift = src_val_unpacked;
        for i in 0..MAX_SHIFT_BITS {
            imm_bit.push(table.add_selected(format!("imm_bit_{i}"), imm, i));
            shifted.push(table.add_shifted(
                "shifted",
                current_shift,
                5,
                1 << i,
                ShiftVariant::LogicalRight,
            ));
            let partial_shift_packed: Col<B32> =
                table.add_packed(format!("partial_shift_packed_{i}"), partial_shift[i]);
            let shifted_packed: Expr<B32, 1> = table
                .add_packed(format!("shifted_packed_{i}"), shifted[i])
                .into();
            let current_shift_packed: Col<B32> =
                table.add_packed(format!("current_shift_packed_{i}"), current_shift);
            // table.assert_zero(
            //     format!("correct_partial_shift_{i}"),
            //     partial_shift_packed
            //         - (shifted_packed * upcast_col(imm_bit)
            //             + current_shift_packed * (upcast_expr(imm_bit.into()) + B32::ONE)
            //             ),
            // );
            current_shift = partial_shift[i];
        }
        let dst_val = table.add_packed("dst_val", partial_shift[MAX_SHIFT_BITS - 1]);

        table.pull(channels.vrom_channel, [dst_abs, dst_val]);
        table.pull(channels.vrom_channel, [src_abs, src_val]);

        Self {
            id: table.id(),
            cpu_cols,
            dst_abs,
            dst_val,
            src_abs,
            src_val,
            partial_shift,
            shifted,
            imm_bit,
        }
    }
}
impl TableFiller<ProverPackedField> for SrliTable {
    type Event = SrliEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            let mut src_val = witness.get_mut_as(self.src_val)?;
            let mut dst_abs = witness.get_mut_as(self.dst_abs)?;
            let mut src_abs = witness.get_mut_as(self.src_abs)?;
            // TODO: Propagate the error
            let mut partial_shift: [_; MAX_SHIFT_BITS] =
                core::array::from_fn(|i| witness.get_mut_as(self.partial_shift[i]).unwrap());
            let mut shifted: [_; MAX_SHIFT_BITS] =
                core::array::from_fn(|i| witness.get_mut_as(self.shifted[i]).unwrap());
            let mut imm_bit: [_; MAX_SHIFT_BITS] =
                core::array::from_fn(|i| witness.get_mut_as(self.imm_bit[i]).unwrap());

            for (i, event) in rows.clone().enumerate() {
                println!("shift ammunt = {:b}", event.shift_amount);
                src_val[i] = event.src_val;
                dst_abs[i] = event.fp.addr(event.dst as u32);
                src_abs[i] = event.fp.addr(event.src as u32);
                let mut current_shift = event.src_val;
                for j in 0..MAX_SHIFT_BITS {
                    imm_bit[j][i] = (((event.shift_amount >> j) & 1) == 1) as u16;
                    shifted[j][i] = current_shift >> (1 << j);
                    println!(
                        "bit[{j}][{i}] = {:?}, current_shift = {current_shift}",
                        imm_bit[j][i]
                    );
                    // println!(
                    //     "(bit * (current_shift >> (1 << j)) + (1-bit) * current_shift) = {}",
                    //     ((imm_bit[j][i] as u32) * (current_shift >> (1 << j))
                    //         + (1 - (imm_bit[j][i] as u32)) * current_shift)
                    // );
                    if imm_bit[j][i] == 1 {
                        current_shift = shifted[j][i];
                    }
                    partial_shift[j][i] = current_shift;
                    // println!("partial_shift[{i}][{j}] = {}",
                    // partial_shift[j][i]);
                }
            }
        }
        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.val(),
            next_pc: None,
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.src,
            arg2: event.shift_amount as u16,
            ..Default::default()
        });
        self.cpu_cols.populate(witness, cpu_rows)?;
        Ok(())
    }
}
