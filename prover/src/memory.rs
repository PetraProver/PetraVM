//! Memory tables for the PetraVM M3 circuit.
//!
//! This module contains the definitions of all the memory tables needed
//! to represent the PetraVM execution in the M3 arithmetization system.

use binius_field::Field;
use binius_m3::builder::{Col, ConstraintSystem, TableFiller, TableId, B128, B16, B32};
use binius_m3::builder::{StructuredDynSize, TableWitnessSegment};
#[cfg(not(all(feature = "disable_prom_channel", feature = "disable_vrom_channel")))]
use binius_m3::gadgets::lookup::LookupProducer;
use binius_m3::gadgets::structured::fill_incrementing_b32;

#[cfg(not(feature = "disable_prom_channel"))]
use crate::prover::PROM_MULTIPLICITY_BITS;
#[cfg(not(feature = "disable_vrom_channel"))]
use crate::prover::VROM_MULTIPLICITY_BITS;
use crate::{
    channels::Channels,
    model::Instruction,
    types::ProverPackedField,
    utils::{pack_instruction, pack_instruction_b128},
};

/// PROM (Program ROM) table for storing program instructions.
///
/// This table stores all the instructions in the program and makes them
/// available to the instruction-specific tables.
///
/// Format: [PC, Opcode, Arg1, Arg2, Arg3] packed into B128
pub struct PromTable {
    /// Table ID
    pub id: TableId,
    /// PC column
    pub pc: Col<B32>,
    /// Opcode column
    pub opcode: Col<B16>,
    /// Argument 1 column
    pub arg1: Col<B16>,
    /// Argument 2 column
    pub arg2: Col<B16>,
    /// Argument 3 column
    pub arg3: Col<B16>,
    /// Packed instruction for PROM channel
    pub instruction: Col<B128>,
    /// To support multiple lookups, we need to create a lookup producer
    #[cfg(not(feature = "disable_prom_channel"))]
    pub lookup_producer: LookupProducer,
}

impl PromTable {
    /// Create a new PROM table with the given constraint system and channels.
    ///
    /// # Arguments
    /// * `cs` - [`ConstraintSystem`] to add the table to
    /// * `channels` - [`Channels`] IDs for communication with other tables
    pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("prom");
        table.require_power_of_two_size();

        // Add columns for PC and instruction components
        let pc = table.add_committed("pc");
        let opcode = table.add_committed("opcode");
        let arg1 = table.add_committed("arg1");
        let arg2 = table.add_committed("arg2");
        let arg3 = table.add_committed("arg3");

        // Pack the values for the PROM channel
        let instruction =
            pack_instruction(&mut table, "instruction", pc, opcode, [arg1, arg2, arg3]);

        #[cfg(not(feature = "disable_prom_channel"))]
        let lookup_producer = LookupProducer::new(
            &mut table,
            channels.prom_channel,
            &[instruction],
            PROM_MULTIPLICITY_BITS,
        );
        let _ = channels;

        Self {
            id: table.id(),
            pc,
            opcode,
            arg1,
            arg2,
            arg3,
            instruction,
            #[cfg(not(feature = "disable_prom_channel"))]
            lookup_producer,
        }
    }
}

impl TableFiller<ProverPackedField> for PromTable {
    type Event = (Instruction, u32);

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            let mut pc_col = witness.get_scalars_mut(self.pc)?;
            let mut opcode_col = witness.get_scalars_mut(self.opcode)?;
            let mut arg1_col = witness.get_scalars_mut(self.arg1)?;
            let mut arg2_col = witness.get_scalars_mut(self.arg2)?;
            let mut arg3_col = witness.get_scalars_mut(self.arg3)?;
            let mut instruction_col = witness.get_scalars_mut(self.instruction)?;

            for (i, (instr, _)) in rows.clone().enumerate() {
                pc_col[i] = B32::new(instr.pc.val());
                opcode_col[i] = B16::new(instr.opcode as u16);

                // Fill arguments, using ZERO if the argument doesn't exist
                arg1_col[i] = instr.args.first().map_or(B16::ZERO, |&arg| B16::new(arg));
                arg2_col[i] = instr.args.get(1).map_or(B16::ZERO, |&arg| B16::new(arg));
                arg3_col[i] = instr.args.get(2).map_or(B16::ZERO, |&arg| B16::new(arg));

                instruction_col[i] = pack_instruction_b128(
                    pc_col[i],
                    opcode_col[i],
                    arg1_col[i],
                    arg2_col[i],
                    arg3_col[i],
                );
            }
        }

        // Populate lookup producer with multiplicity iterator
        #[cfg(not(feature = "disable_prom_channel"))]
        self.lookup_producer
            .populate(witness, rows.map(|(_, multiplicity)| *multiplicity))?;

        Ok(())
    }
}

/// VROM (Value ROM) table for memory values.
///
/// This table contains all [Address, Value] couples of the VROM.
/// The uniqueness of addresses is guaranteed by the VROM address space column.
///
/// Format: [Address, Value]
pub struct VromTable {
    /// Table ID
    pub id: TableId,
    /// Sorted address space
    pub addr_space: Col<B32>,
    /// Address column (sorted by multiplicity)
    pub addr: Col<B32>,
    /// Value column (from VROM channel)
    pub value: Col<B32>,
    /// To support multiple lookups, we need to create a lookup producer
    #[cfg(not(feature = "disable_vrom_channel"))]
    pub lookup_producer: LookupProducer,
}

impl VromTable {
    /// Create a new VROM table with the given constraint system and
    /// channels.
    ///
    /// # Arguments
    /// * `cs` - [`ConstraintSystem`] to add the table to
    /// * `channels` - [`Channels`] IDs for communication with other tables
    pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("vrom");
        table.require_power_of_two_size();

        // Add address space column to ensure addresses are unique
        let addr_space = table.add_structured::<B32>(
            "addr_space",
            StructuredDynSize::Incrementing { max_size_log: 32 },
        );

        // Add columns for address and value
        let addr = table.add_committed("addr");
        let value = table.add_committed("value");

        // Assess that address space column and address columns are permuted
        table.push(channels.vrom_addr_space_channel, [addr_space]);
        table.pull(channels.vrom_addr_space_channel, [addr]);

        #[cfg(not(feature = "disable_vrom_channel"))]
        let lookup_producer = LookupProducer::new(
            &mut table,
            channels.vrom_channel,
            &[addr, value],
            VROM_MULTIPLICITY_BITS,
        );

        Self {
            id: table.id(),
            addr_space,
            addr,
            value,
            #[cfg(not(feature = "disable_vrom_channel"))]
            lookup_producer,
        }
    }
}

impl TableFiller<ProverPackedField> for VromTable {
    type Event = (u32, u32, u32);

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            // Fill the address and value columns from events
            let mut addr_col = witness.get_scalars_mut(self.addr)?;
            let mut value_col = witness.get_scalars_mut(self.value)?;

            // Fill in values from events
            for (i, (addr, value, _)) in rows.clone().enumerate() {
                addr_col[i] = B32::new(*addr);
                value_col[i] = B32::new(*value);
            }
        }

        // Fill address space column
        fill_incrementing_b32(witness, self.addr_space)?;

        // Populate lookup producer with multiplicity iterator
        #[cfg(not(feature = "disable_vrom_channel"))]
        self.lookup_producer
            .populate(witness, rows.map(|(_, _, multiplicity)| *multiplicity))?;

        Ok(())
    }
}
