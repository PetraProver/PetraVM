//! Data models for the zCrayVM proving system.
//!
//! This module contains the data structures used to represent execution traces
//! and events needed for the proving system.

use std::collections::HashMap;

use anyhow::Result;
use binius_m3::builder::B32;
use paste::paste;
use zcrayvm_assembly::{event::*, InterpreterInstruction, Opcode, ZCrayTrace};

use crate::table::*;

/// High-level representation of a zCrayVM instruction with its PC and
/// arguments.
///
/// This is a simplified representation of the instruction format used in the
/// proving system, where the arguments are stored in a more convenient form for
/// the prover.
#[derive(Debug, Clone)]
pub struct Instruction {
    /// PC value as a field element
    pub pc: B32,
    /// Opcode of the instruction
    pub opcode: Opcode,
    /// Arguments to the instruction (up to 3)
    pub args: Vec<u16>,
}

impl From<InterpreterInstruction> for Instruction {
    fn from(instr: InterpreterInstruction) -> Self {
        // Extract arguments from the interpreter instruction
        let args_array = instr.args();

        Self {
            pc: instr.field_pc,
            opcode: instr.opcode(),
            args: args_array.iter().map(|arg| arg.val()).collect(),
        }
    }
}

/// Master macro defining the [`Trace`] object, implementing the [`TableInfo`]
/// trait that lifts [`InstructionInfo`](zcrayvm_assembly::InstructionInfo) and
/// maps events to their corresponding field in the [`ZCrayTrace`], as well as
/// corresponding event accessors for the main [`Trace`].
///
/// It will also implement the mapping between an [`Opcode`] and its associated
/// [`Table`].
///
/// It takes three arguments:
/// - a list of "normal" instructions, represented by their field name in a
///   [`ZCrayTrace`]
/// - a list of "shift" instructions, gathered behind a common `shift` field in
///   a [`ZCrayTrace`]
/// - a list of "mul" instructions, gathered behind a common `signed_mul` field
///   in a [`ZCrayTrace`]
///
/// # Example
///
/// ```ignore
/// define_table_registry_and_accessors!(
/// normal = [
///     (ldi, Ldi),
///     (ret, Ret),
/// ],
/// generic_shift = [
///     (srli, Srli, SrliEvent),
/// ],
/// generic_mul = [
///     (mulsu, Mulsu, MulsuEvent),
///     (mul, Mul, MulEvent),
/// ],
/// );
/// ```
macro_rules! define_table_registry_and_accessors {
    (
        normal = [ $(($func_name:ident, $opcode_variant:ident)),* $(,)? ],
        generic_shift  = [ $(($func_name_shift:ident, $variant_shift:ident, $event_ty_shift:ty)),* $(,)? ],
        generic_mul    = [ $(($func_name_mul:ident, $variant_mul:ident, $event_ty_mul:ty)),* $(,)? ],
    ) => {
        paste! {
            /// Execution trace containing a program and all execution events.
            ///
            /// This is a wrapper around ZCrayTrace that provides a simplified interface
            /// for the proving system. It contains:
            /// 1. The program instructions in a format optimized for the prover
            /// 2. The original ZCrayTrace with all execution events and memory state
            /// 3. A list of VROM writes (address, value) pairs
            #[derive(Debug)]
            pub struct Trace {
                /// The underlying ZCrayTrace containing all execution events
                pub trace: ZCrayTrace,
                /// Program instructions in a more convenient format for the proving system
                pub program: Vec<(Instruction, u32)>,
                /// List of VROM writes (address, value, multiplicity) pairs
                pub vrom_writes: Vec<(u32, u32, u32)>,
                /// Maximum VROM address in the trace
                pub max_vrom_addr: usize,

                // Memoized caches for shifts
                $(
                    [<$func_name_shift _cache>]: once_cell::sync::OnceCell<Vec<$event_ty_shift>>,
                )*

                // Memoized caches for muls
                $(
                    [<$func_name_mul _cache>]: once_cell::sync::OnceCell<Vec<$event_ty_mul>>,
                )*
            }
        }

        paste! {
            impl Trace {
                pub fn new() -> Self {
                    Self {
                        trace: ZCrayTrace::default(),
                        program: Vec::new(),
                        vrom_writes: Vec::new(),
                        max_vrom_addr: 0,
                        $(
                            [<$func_name_shift _cache>]: once_cell::sync::OnceCell::new(),
                        )*
                        $(
                            [<$func_name_mul _cache>]: once_cell::sync::OnceCell::new(),
                        )*
                    }
                }

                $(
                    #[doc = concat!("Returns a reference to the logged `", stringify!([<$opcode_variant Event>]), "`s from the trace.")]
                    pub fn [<$func_name _events>](&self) -> &[ [<$opcode_variant Event>] ] {
                        &self.trace.$func_name
                    }
                )*

                $(
                    #[doc = concat!("Returns all `", stringify!($event_ty_shift), "`s from the shifts list in the trace.")]
                    pub fn [<$func_name_shift _events>](&self) -> &[$event_ty_shift] {
                        self.[<$func_name_shift _cache>].get_or_init(|| {
                            self.trace.shifts.iter()
                                .filter_map(|event| match event.as_any() {
                                    AnyShiftEvent::$variant_shift(e) => Some(e),
                                    _ => None,
                                })
                                .collect()
                        })
                    }
                )*

                $(
                    #[doc = concat!("Returns all `", stringify!($event_ty_mul), "`s from the signed_mul list in the trace.")]
                    pub fn [<$func_name_mul _events>](&self) -> &[$event_ty_mul] {
                        self.[<$func_name_mul _cache>].get_or_init(|| {
                            self.trace.signed_mul.iter()
                                .filter_map(|event| match event.as_any() {
                                    AnySignedMulEvent::$variant_mul(e) => Some(e),
                                    _ => None,
                                })
                                .collect()
                        })
                    }
                )*
            }
        }

        // Define accessors for regular instructions
        $(
            paste! {
                impl TableInfo for [<$opcode_variant Event>] {
                    type Table = [<$opcode_variant Table>];

                    fn accessor() -> fn(&Trace) -> &[< [<$opcode_variant Table>] as Table>::Event] {
                        Trace::[<$func_name _events>]
                    }
                }
            }
        )*

        // Define accessors for shift instructions
        $(
            paste! {
                impl TableInfo for $event_ty_shift {
                    type Table = [<$variant_shift Table>];

                    fn accessor() -> fn(&Trace) -> &[Self] {
                        Trace::[<$func_name_shift _events>]
                    }
                }
            }
        )*

        // Define accessors for signed mul instructions
        $(
            paste! {
                impl TableInfo for $event_ty_mul {
                    type Table = [<$variant_mul Table>];

                    fn accessor() -> fn(&Trace) -> &[Self] {
                        Trace::[<$func_name_mul _events>]
                    }
                }
            }
        )*


        // Build table for all instructions
        paste! {
            pub fn build_table_for_opcode(
               opcode: Opcode,
                cs: &mut binius_m3::builder::ConstraintSystem,
                channels: &$crate::channels::Channels,
            ) -> Option<Box<dyn $crate::table::FillableTable>> {
                use $crate::table::Table;
                match opcode {
                    // Process regular instructions
                    $(
                        Opcode::$opcode_variant => {
                            Some(Box::new($crate::table::TableEntry {
                                table: Box::new(<[<$opcode_variant Table>]>::new(cs, channels)),
                                get_events: <[<$opcode_variant Event>] as TableInfo>::accessor(),
                            }))
                        }
                    )*
                    // Process shift instructions
                    $(
                        Opcode::$variant_shift => {
                            Some(Box::new($crate::table::TableEntry {
                                table: Box::new(<[<$variant_shift Table>]>::new(cs, channels)),
                                get_events: <$event_ty_shift as TableInfo>::accessor(),
                            }))
                        }
                    )*
                    // Process mul instructions
                    $(
                        Opcode::$variant_mul => {
                            Some(Box::new($crate::table::TableEntry {
                                table: Box::new(<[<$variant_mul Table>]>::new(cs, channels)),
                                get_events: <$event_ty_mul as TableInfo>::accessor(),
                            }))
                        }
                    )*
                    _ => None,
                }
            }
        }
    };
}

impl Default for Trace {
    fn default() -> Self {
        Self::new()
    }
}

impl Trace {
    /// Creates a Trace from an existing ZCrayTrace.
    ///
    /// This is useful when you have a trace from the interpreter and want
    /// to convert it to the proving format.
    ///
    /// Note: This creates an empty program vector. You'll need to populate
    /// the program instructions separately using add_instructions().
    ///
    /// TODO: Refactor this approach to directly obtain the zkVMTrace from
    /// program emulation rather than requiring separate population of
    /// program instructions.
    pub fn from_zcray_trace(program: Vec<InterpreterInstruction>, trace: ZCrayTrace) -> Self {
        // Add the program instructions to the trace
        let mut zkvm_trace = Self::new();
        zkvm_trace.add_instructions(program, &trace.instruction_counter);
        zkvm_trace.trace = trace;
        zkvm_trace
    }

    /// Add multiple interpreter instructions to the program.
    ///
    /// Instructions are added in descending order of their execution count.
    ///
    /// # Arguments
    /// * `instructions` - An iterator of InterpreterInstructions to add
    pub fn add_instructions<I>(&mut self, instructions: I, instruction_counter: &HashMap<B32, u32>)
    where
        I: IntoIterator<Item = InterpreterInstruction>,
    {
        // Collect all instructions with their counts
        let mut instructions_with_counts: Vec<_> = instructions
            .into_iter()
            .map(|instr| {
                let count = instruction_counter.get(&instr.field_pc).unwrap_or(&0);
                (instr.into(), *count)
            })
            .collect();

        // Sort by count in descending order
        instructions_with_counts.sort_by(|(_, count_a), (_, count_b)| count_b.cmp(count_a));

        // Add instructions in sorted order
        self.program = instructions_with_counts;
    }

    /// Add a VROM write event.
    ///
    /// # Arguments
    /// * `addr` - The address to write to
    /// * `value` - The value to write
    /// * `multiplicity` - The multiplicity of pulls of this VROM write
    pub fn add_vrom_write(&mut self, addr: u32, value: u32, multiplicity: u32) {
        self.vrom_writes.push((addr, value, multiplicity));
    }

    /// Ensures the trace has enough data for proving.
    ///
    /// This will verify that:
    /// 1. The program has at least one instruction
    /// 2. The trace has at least one RET event
    /// 3. The trace has at least one VROM write
    ///
    /// # Returns
    /// * Ok(()) if the trace is valid, or an error with a description of what's
    ///   missing
    pub fn validate(&self) -> Result<()> {
        if self.program.is_empty() {
            return Err(anyhow::anyhow!(
                "Trace must contain at least one instruction"
            ));
        }

        if self.ret_events().is_empty() {
            return Err(anyhow::anyhow!("Trace must contain at least one RET event"));
        }

        if self.vrom_writes.is_empty() {
            return Err(anyhow::anyhow!(
                "Trace must contain at least one VROM write"
            ));
        }

        Ok(())
    }
}

define_table_registry_and_accessors!(
    normal = [
        (ldi, Ldi),
        (ret, Ret),
        (bz, Bz),
        (bnz, Bnz),
        (b32_mul, B32Mul),
        (b32_muli, B32Muli),
        (b128_add, B128Add),
        (b128_mul, B128Mul),
        (andi, Andi),
        (xori, Xori),
        (add, Add),
        (taili, Taili),
        (tailv, Tailv),
        (calli, Calli),
        (callv, Callv),
        (mvvw, Mvvw),
        (mvih, Mvih),
        (and, And),
        (xor, Xor),
        (or, Or),
        (ori, Ori),
        (jumpi, Jumpi),
        (jumpv, Jumpv),
    ],
    generic_shift = [
        (srli, Srli, SrliEvent),
        // TODO
        // (slli, Slli, SlliEvent),
        // (srai, Srai, SraiEvent),
        // (sll, Sll, SllEvent),
        // (srl, Srl, SrlEvent),
        // (sra, Sra, SraEvent),
    ],
    generic_mul = [
        // TODO
        // (mulsu, Mulsu, MulsuEvent),
        // (mul, Mul, MuluEvent),
    ],
);
