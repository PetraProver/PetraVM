//! Channel definitions for the PetraVM proving system.
//!
//! This module defines all the channels used to connect different tables
//! in the M3 arithmetic circuit.

use binius_core::constraint_system::channel::ChannelId;
use binius_m3::builder::ConstraintSystem;

/// Holds all channel IDs used in the PetraVM proving system.
#[derive(Debug, Clone)]
pub struct Channels {
    /// Channel for state transitions (PC, FP)
    /// Follows format [PC, FP]
    pub state_channel: ChannelId,

    /// Channel connecting the PROM table to instruction tables
    /// Follows format [PC, Opcode, Arg1, Arg2, Arg3]
    /// TODO: We may want this channel balanced without considering its
    /// multiplicities (a lookup table)
    pub prom_channel: ChannelId,

    /// Channel for memory operations (VROM)
    /// Follows format [Address, Value]
    pub vrom_channel: ChannelId,

    /// Channel for VROM address space (verifier pushes full address space)
    /// Follows format [Address]
    pub vrom_addr_space_channel: ChannelId,

    /// Channel for right logical shift operations
    /// Follows format [Input, ShiftAmount, Output]
    pub right_shifter_channel: ChannelId,
}

impl Channels {
    /// Create all channels needed for the proving system.
    pub fn new(cs: &mut ConstraintSystem) -> Self {
        Self {
            state_channel: cs.add_channel("state_channel"),
            prom_channel: cs.add_channel("prom_channel"),
            vrom_channel: cs.add_channel("vrom_channel"),
            vrom_addr_space_channel: cs.add_channel("vrom_addr_space_channel"),
            right_shifter_channel: cs.add_channel("right_shifter_channel"),
        }
    }
}
