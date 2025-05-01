//! Memory tables for the zCrayVM M3 circuit.
//!
//! This module contains the definitions of all the memory tables needed
//! to represent the zCrayVM execution in the M3 arithmetization system.

use binius_field::Field;
use binius_m3::builder::{Col, ConstraintSystem, TableFiller, TableId, B128, B16, B32};
use binius_m3::builder::{StructuredDynSize, TableWitnessSegment};
use binius_m3::gadgets::lookup::LookupProducer;
use binius_m3::gadgets::structured::fill_incrementing_b32;

use crate::prover::{PROM_MULTIPLICITY_BITS, VROM_MULTIPLICITY_BITS};
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

        let lookup_producer = LookupProducer::new(
            &mut table,
            channels.prom_channel,
            &[instruction],
            PROM_MULTIPLICITY_BITS,
        );

        Self {
            id: table.id(),
            pc,
            opcode,
            arg1,
            arg2,
            arg3,
            instruction,
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
        self.lookup_producer
            .populate(witness, rows.map(|(_, multiplicity)| *multiplicity))?;

        Ok(())
    }
}

/// VROM (Value ROM) table for writing memory values.
///
/// This table handles the case where we want to write a value to an address.
/// It pulls an address from the address space channel and pushes the
/// address+value to the VROM channel.
///
/// Format: [Address, Value]
pub struct VromWriteTable {
    /// Table ID
    pub id: TableId,
    /// Address column (from address space channel)
    pub addr: Col<B32>,
    /// Value column (from VROM channel)
    pub value: Col<B32>,
    /// To support multiple lookups, we need to create a lookup producer
    pub lookup_producer: LookupProducer,
}

impl VromWriteTable {
    /// Create a new VROM write table with the given constraint system and
    /// channels.
    ///
    /// # Arguments
    /// * `cs` - [`ConstraintSystem`] to add the table to
    /// * `channels` - [`Channels`] IDs for communication with other tables
    pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("vrom_write");
        table.require_power_of_two_size();

        // Add columns for address and value
        let addr = table.add_committed("addr");
        let value = table.add_committed("value");

        // Pull from VROM address space channel (verifier pushes full address space)
        table.pull(channels.vrom_addr_space_channel, [addr]);

        let lookup_producer = LookupProducer::new(
            &mut table,
            channels.vrom_channel,
            &[addr, value],
            VROM_MULTIPLICITY_BITS,
        );

        Self {
            id: table.id(),
            addr,
            value,
            lookup_producer,
        }
    }
}

impl TableFiller<ProverPackedField> for VromWriteTable {
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
            // Fill the address and value columns
            let mut addr_col = witness.get_scalars_mut(self.addr)?;
            let mut value_col = witness.get_scalars_mut(self.value)?;

            // Fill in values from events
            for (i, (addr, value, _)) in rows.clone().enumerate() {
                addr_col[i] = B32::new(*addr);
                value_col[i] = B32::new(*value);
            }
        }

        // Populate lookup producer with multiplicity iterator
        self.lookup_producer
            .populate(witness, rows.map(|(_, _, multiplicity)| *multiplicity))?;

        Ok(())
    }
}

/// VROM (Value ROM) table for skipping addresses.
///
/// This table handles the case where we don't want to write a value to an
/// address. It pulls an address from the address space channel but doesn't push
/// anything to the VROM channel.
///
/// Format: [Address]
pub struct VromSkipTable {
    /// Table ID
    pub id: TableId,
    /// Address column (from address space channel)
    pub addr: Col<B32>,
}

impl VromSkipTable {
    /// Create a new VROM skip table with the given constraint system and
    /// channels.
    ///
    /// # Arguments
    /// * `cs` - [`ConstraintSystem`] to add the table to
    /// * `channels` - [`Channels`] IDs for communication with other tables
    pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("vrom_skip");

        // Add column for address
        let addr = table.add_committed("addr");

        // Pull from VROM address space channel (verifier pushes full address space)
        table.pull(channels.vrom_addr_space_channel, [addr]);

        Self {
            id: table.id(),
            addr,
        }
    }
}

impl TableFiller<ProverPackedField> for VromSkipTable {
    type Event = u32;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event>,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        let mut addr_col = witness.get_scalars_mut(self.addr)?;

        // Fill in addresses from events
        for (i, addr) in rows.enumerate() {
            addr_col[i] = B32::new(*addr);
        }

        Ok(())
    }
}

/// VROM Address Space table that pushes all possible addresses into the
/// vrom_addr_space_channel.
///
/// This table is used by the verifier to push the full address space into the
/// vrom_addr_space_channel. Each address must be pulled exactly once by either
/// VromWriteTable or VromSkipTable.
///
/// Format: [Address]
pub struct VromAddrSpaceTable {
    /// Table ID
    pub id: TableId,
    /// Address column
    pub addr: Col<B32>,
}

impl VromAddrSpaceTable {
    /// Create a new VROM Address Space table with the given constraint system
    /// and channels.
    ///
    /// # Arguments
    /// * `cs` - [`ConstraintSystem`] to add the table to
    /// * `channels` - [`Channels`] IDs for communication with other tables
    pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("vrom_addr_space");
        table.require_power_of_two_size();

        // Add column for address
        let addr = table.add_structured::<B32>("addr", StructuredDynSize::Incrementing);

        // Push to VROM address space channel
        table.push(channels.vrom_addr_space_channel, [addr]);

        Self {
            id: table.id(),
            addr,
        }
    }
}

impl TableFiller<ProverPackedField> for VromAddrSpaceTable {
    type Event = u32;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        _rows: impl Iterator<Item = &'a Self::Event>,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        fill_incrementing_b32(witness, self.addr)?;

        Ok(())
    }
}
