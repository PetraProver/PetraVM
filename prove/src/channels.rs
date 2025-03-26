//! Channel definitions for the zCrayVM proving system.
//!
//! This module defines all the channels used to connect different tables
//! in the M3 arithmetic circuit.

use binius_core::constraint_system::channel::ChannelId;
use binius_m3::builder::ConstraintSystem;

/// Holds all channel IDs used in the zCrayVM proving system.
#[derive(Debug, Clone)]
pub struct ZkVMChannels {
    /// Channel for state transitions (PC, FP)
    pub state_channel: ChannelId,
    
    /// Channel connecting the PROM table to instruction tables
    pub prom_channel: ChannelId,
    
    /// Channel for the LDI table
    pub ldi_channel: ChannelId,
    
    /// Channel for the RET table
    pub ret_channel: ChannelId,
    
    /// Channel for VROM operations
    pub vrom_channel: ChannelId,
}

impl ZkVMChannels {
    /// Create all channels needed for the proving system.
    pub fn new(cs: &mut ConstraintSystem) -> Self {
        Self {
            state_channel: cs.add_channel("state_channel"),
            prom_channel: cs.add_channel("prom_channel"),
            ldi_channel: cs.add_channel("ldi_channel"),
            ret_channel: cs.add_channel("ret_channel"),
            vrom_channel: cs.add_channel("vrom_channel"),
        }
    }
}