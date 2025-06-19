use binius_m3::builder::{
    upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B32,
};
use petravm_asm::{JumpiEvent, JumpvEvent, Opcode};

use crate::gadgets::state::{NextPc, StateColumns, StateColumnsOptions, StateGadget};
use crate::utils::pull_vrom_channel;
use crate::{channels::Channels, table::Table, types::ProverPackedField};

/// Table for JUMPI instruction.
///
/// Implements the unconditional jump to an immediate address.
/// Logic: PC = target
pub struct JumpiTable {
    id: TableId,
    state_cols: StateColumns<{ Opcode::Jumpi as u16 }>,
}

impl Table for JumpiTable {
    type Event = JumpiEvent;

    fn name(&self) -> &'static str {
        "JumpiTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("jumpi");

        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions {
                next_pc: NextPc::Immediate,
                next_fp: None,
            },
        );

        Self {
            id: table.id(),
            state_cols,
        }
    }
}

impl TableFiller<ProverPackedField> for JumpiTable {
    type Event = JumpiEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        let state_rows = rows.map(|event| StateGadget {
            pc: event.pc.val(),
            next_pc: Some(event.target.val()),
            fp: *event.fp,
            arg0: event.target.val() as u16,
            arg1: (event.target.val() >> 16) as u16,
            arg2: 0, // Unused for jumpi
        });
        self.state_cols.populate(witness, state_rows)?;
        Ok(())
    }
}

/// Table for JUMPV instruction.
///
/// Implements the unconditional jump to an address stored in VROM.
/// Logic: PC = FP[offset]
pub struct JumpvTable {
    id: TableId,
    state_cols: StateColumns<{ Opcode::Jumpv as u16 }>,
    offset_addr: Col<B32>, // Virtual
    target_val: Col<B32>,
}

impl Table for JumpvTable {
    type Event = JumpvEvent;

    fn name(&self) -> &'static str {
        "JumpvTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("jumpv");

        let target_val = table.add_committed("target_val");

        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions {
                next_pc: NextPc::Target(target_val),
                next_fp: None,
            },
        );

        let offset_addr =
            table.add_computed("offset_addr", state_cols.fp + upcast_col(state_cols.arg0));

        // Read target_val from VROM
        pull_vrom_channel(&mut table, channels.vrom_channel, [offset_addr, target_val]);

        Self {
            id: table.id(),
            state_cols,
            offset_addr,
            target_val,
        }
    }
}

impl TableFiller<ProverPackedField> for JumpvTable {
    type Event = JumpvEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            let mut offset_addr = witness.get_scalars_mut(self.offset_addr)?;
            let mut target_val = witness.get_scalars_mut(self.target_val)?;
            for (i, event) in rows.clone().enumerate() {
                offset_addr[i] = B32::new(event.fp.addr(event.offset));
                target_val[i] = B32::new(event.target);
            }
        }
        let state_rows = rows.map(|event| StateGadget {
            pc: event.pc.val(),
            next_pc: Some(event.target),
            fp: *event.fp,
            arg0: event.offset,
            arg1: 0, // Unused for jumpv
            arg2: 0, // Unused for jumpv
        });
        self.state_cols.populate(witness, state_rows)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use binius_field::BinaryField;
    use petravm_asm::isa::GenericISA;

    use super::*;
    use crate::model::Trace;
    use crate::prover::Prover;
    use crate::test_utils::generate_trace;

    pub(crate) const G: B32 = B32::MULTIPLICATIVE_GENERATOR;

    /// Creates an execution trace for a simple program that uses the J (Jump)
    /// instruction in both its variants: jump to label and jump to address in
    /// VROM.
    fn generate_j_instruction_trace() -> Result<Trace> {
        let pc_val = (G * G * G).val();
        // Create an assembly program that tests both J instruction variants
        let asm_code = format!(
            "#[framesize(0x10)]\n\
        _start:\n\
            LDI.W @3, #{pc_val}\n\
            J @3\n\
            ;; Code that should be skipped\n\
            LDI.W @2, #998\n\
            LDI.W @4, #999\n\
            J jump_target\n\
            ;; Code that should be skipped\n\
            LDI.W @2, #1000\n\
        jump_target:\n\
            LDI.W @2, #0  ;; Success\n\
            RET\n",
        );

        // Add VROM writes with appropriate access counts
        let vrom_writes = vec![
            (3, pc_val, 2), // Jump target
            (0, 0, 1),      // Return PC
            (1, 0, 1),      // Return FP
            (2, 0, 1),      // Success Result
            (4, 999, 1),    // LDI.W @4, #999
        ];
        let isa = Box::new(GenericISA);
        generate_trace(asm_code, None, Some(vrom_writes), isa).map(|(trace, _)| trace)
    }

    #[test]
    fn test_jump_tables() -> Result<()> {
        let trace = generate_j_instruction_trace()?;
        trace.validate()?;
        assert_eq!(trace.jumpi_events().len(), 1);
        assert_eq!(trace.jumpv_events().len(), 1);
        Prover::new(Box::new(GenericISA)).validate_witness(&trace)
    }
}
