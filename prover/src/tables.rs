//! Tables for the zCrayVM M3 circuit.
//!
//! This module contains the definitions of all the arithmetic tables needed
//! to represent the zCrayVM execution in the M3 arithmetization system.

use binius_field::as_packed_field::PackScalar;
use binius_m3::builder::{Col, ConstraintSystem, TableFiller, TableId, B128, B16, B32};
use binius_m3::builder::{TableWitnessSegment, B1};
use bytemuck::Pod;

// Re-export instruction-specific tables
pub use crate::opcodes::{LdiTable, RetTable};
use crate::{
    channels::Channels,
    model::Instruction,
    utils::{pack_prom_entry, pack_prom_entry_b128},
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
    pub pc: Col<B32, 1>,
    /// Opcode column
    pub opcode: Col<B16, 1>,
    /// Argument 1 column
    pub arg1: Col<B16, 1>,
    /// Argument 2 column
    pub arg2: Col<B16, 1>,
    /// Argument 3 column
    pub arg3: Col<B16, 1>,
    /// Packed instruction for PROM channel
    pub prom_entry: Col<B128, 1>,
}

impl PromTable {
    /// Create a new PROM table with the given constraint system and channels.
    ///
    /// # Arguments
    /// * `cs` - Constraint system to add the table to
    /// * `channels` - Channel IDs for communication with other tables
    pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("prom");

        // Add columns for PC and instruction components
        let pc = table.add_committed("pc");
        let opcode = table.add_committed("opcode");
        let arg1 = table.add_committed("arg1");
        let arg2 = table.add_committed("arg2");
        let arg3 = table.add_committed("arg3");

        // Pack the values for the PROM channel
        let prom_entry = pack_prom_entry(&mut table, "prom_entry", pc, opcode, [arg1, arg2, arg3]);

        // Push to the PROM channel
        table.push(channels.prom_channel, [prom_entry]);

        Self {
            id: table.id(),
            pc,
            opcode,
            arg1,
            arg2,
            arg3,
            prom_entry,
        }
    }
}

impl<U> TableFiller<U> for PromTable
where
    U: Pod + PackScalar<B1>,
{
    type Event = Instruction;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event>,
        witness: &'a mut TableWitnessSegment<U>,
    ) -> anyhow::Result<()> {
        let mut pc_col = witness.get_mut_as(self.pc)?;
        let mut opcode_col = witness.get_mut_as(self.opcode)?;
        let mut arg1_col = witness.get_mut_as(self.arg1)?;
        let mut arg2_col = witness.get_mut_as(self.arg2)?;
        let mut arg3_col = witness.get_mut_as(self.arg3)?;
        let mut prom_entry_col = witness.get_mut_as(self.prom_entry)?;

        for (i, instr) in rows.enumerate() {
            pc_col[i] = instr.pc;
            opcode_col[i] = instr.opcode as u16;

            // Fill arguments, using 0 if the argument doesn't exist
            arg1_col[i] = instr.args.first().copied().unwrap_or(0);
            arg2_col[i] = instr.args.get(1).copied().unwrap_or(0);
            arg3_col[i] = instr.args.get(2).copied().unwrap_or(0);

            prom_entry_col[i] = pack_prom_entry_b128(
                pc_col[i].val(),
                opcode_col[i],
                arg1_col[i],
                arg2_col[i],
                arg3_col[i],
            );
        }

        Ok(())
    }
}

/// VROM (Value ROM) table for writing memory values.
///
/// This table handles the case where we want to write a value to an address.
/// It pulls an address from the address space channel and pushes the
/// address+value to the VROM channel.
///
/// Format: [Address, Value] packed into B64
pub struct VromWriteTable {
    /// Table ID
    pub id: TableId,
    /// Address column (from address space channel)
    pub addr: Col<B32, 1>,
    /// Value column (from VROM channel)
    pub value: Col<B32, 1>,
}

impl VromWriteTable {
    /// Create a new VROM write table with the given constraint system and
    /// channels.
    ///
    /// # Arguments
    /// * `cs` - Constraint system to add the table to
    /// * `channels` - Channel IDs for communication with other tables
    pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("vrom_write");

        // Add columns for address and value
        let addr = table.add_committed("addr");
        let value = table.add_committed("value");

        // Pull from VROM address space channel (verifier pushes full address space)
        table.pull(channels.vrom_addr_space_channel, [addr]);

        // Push to VROM channel (address+value)
        table.push(channels.vrom_channel, [addr, value]);

        Self {
            id: table.id(),
            addr,
            value,
        }
    }
}

impl<U> TableFiller<U> for VromWriteTable
where
    U: Pod + PackScalar<B32>,
{
    type Event = (u32, u32);

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event>,
        witness: &'a mut TableWitnessSegment<U>,
    ) -> anyhow::Result<()> {
        let mut addr_col = witness.get_mut_as(self.addr)?;
        let mut value_col = witness.get_mut_as(self.value)?;

        // Fill in values from events
        for (i, (addr, value)) in rows.enumerate() {
            addr_col[i] = *addr;
            value_col[i] = *value;
        }

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
    pub addr: Col<B32, 1>,
}

impl VromSkipTable {
    /// Create a new VROM skip table with the given constraint system and
    /// channels.
    ///
    /// # Arguments
    /// * `cs` - Constraint system to add the table to
    /// * `channels` - Channel IDs for communication with other tables
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

impl<U> TableFiller<U> for VromSkipTable
where
    U: Pod + PackScalar<B32>,
{
    type Event = u32;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event>,
        witness: &'a mut TableWitnessSegment<U>,
    ) -> anyhow::Result<()> {
        let mut addr_col = witness.get_mut_as(self.addr)?;

        // Fill in addresses from events
        for (i, addr) in rows.enumerate() {
            addr_col[i] = *addr;
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
    pub addr: Col<B32, 1>,
}

impl VromAddrSpaceTable {
    /// Create a new VROM Address Space table with the given constraint system
    /// and channels.
    ///
    /// # Arguments
    /// * `cs` - Constraint system to add the table to
    /// * `channels` - Channel IDs for communication with other tables
    pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("vrom_addr_space");

        // Add column for address
        let addr = table.add_committed("addr");

        // Push to VROM address space channel
        table.push(channels.vrom_addr_space_channel, [addr]);

        Self {
            id: table.id(),
            addr,
        }
    }
}

impl<U> TableFiller<U> for VromAddrSpaceTable
where
    U: Pod + PackScalar<B32>,
{
    type Event = u32;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event>,
        witness: &'a mut TableWitnessSegment<U>,
    ) -> anyhow::Result<()> {
        let mut addr_col = witness.get_mut_as(self.addr)?;

        // Fill the addresses from the provided rows
        for (i, &addr) in rows.enumerate() {
            addr_col[i] = addr;
        }

        Ok(())
    }
}
