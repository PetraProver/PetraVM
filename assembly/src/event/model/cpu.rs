use std::cell::RefMut;

use binius_core::{constraint_system::channel::ChannelId, oracle::ShiftVariant};
use binius_field::{as_packed_field::PackScalar, BinaryField, ExtensionField};
use binius_m3::builder::{
    upcast_col, upcast_expr, Col, ConstraintSystem, Expr, TableBuilder, TableWitnessIndexSegment,
    B1, B128, B16, B32, B64,
};
use bytemuck::Pod;

use crate::{event::arithmetization::cpu::Instruction, opcodes::Opcode};

pub(crate) struct CpuEvent {
    pub(crate) pc: u32,
    // TODO: This is only necessary for ret because when filling it can't be read from target in
    // NextPc::Target(target)
    pub(crate) next_pc: Option<u32>,
    pub(crate) fp: u32,
    pub(crate) instruction: Instruction,
}