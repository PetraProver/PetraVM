use binius_m3::builder::B16;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use strum::EnumCount;
use strum_macros::{Display, EnumCount, IntoStaticStr, VariantArray};

use crate::event::*;

/// Represents the set of instructions supported by the PetraVM.
#[derive(
    Debug,
    Display,
    Clone,
    Copy,
    Hash,
    Default,
    EnumCount,
    TryFromPrimitive,
    IntoPrimitive,
    PartialEq,
    Eq,
    VariantArray,
    IntoStaticStr,
)]
#[repr(u16)]
#[allow(clippy::upper_case_acronyms)]
// TODO: Adjust opcode discriminants once settled on their values.
// Consider Deref to account for aliases?
pub enum Opcode {
    #[default]
    Invalid = 0x00,

    // Integer instructions
    Xori,
    Xor,
    Andi,
    Srli,
    Slli,
    Srai,
    Addi,
    Add,
    Muli,
    Mulu,
    Mulsu,
    Mul,
    B32Mul,
    B32Muli,
    B128Add,
    B128Mul,
    And,
    Or,
    Ori,
    Sub,
    Sll,
    Srl,
    Sra,

    // Move instructions
    Mvvw,
    Mvih,
    Ldi,
    Mvvl,

    // Jump instructions
    Jumpi,
    Jumpv,
    Taili,
    Tailv,
    Calli,
    Callv,
    Ret,

    // Comparison instructions
    Sle,
    Slei,
    Sleu,
    Sleiu,
    Slt,
    Slti,
    Sltu,
    Sltiu,

    // Allocation instructions (prover-only)
    Alloci,
    Allocv,

    // Register instructions
    Fp,

    // Memory Access (RAM) instructions
    // TODO: optional ISA extension for future implementation
    // Not needed for recursion program or first version of PetraVM
    // Design note: Considering 32-bit word-sized memory instead of byte-addressed memory
    // LW,
    // SW,
    // LB,
    // LBU,
    // LH,
    // LHU,
    // SB,
    // SH,

    // Branch instructions
    Bnz,
    /// Bz is only declared to allow for proper mapping with the associated
    /// table. This is an *invalid* instruction and should never be reached.
    /// [`BzEvent`] should only be generated through the execution of
    /// [`Opcode::Bnz`] when no branching occurs.
    Bz = 0xffff,
}

impl Opcode {
    pub const OP_COUNT: usize = Self::COUNT - 1;
    pub const fn get_field_elt(&self) -> B16 {
        B16::new(*self as u16)
    }

    /// Returns the number of arguments expected by the given opcode.
    pub const fn num_args(&self) -> usize {
        match self {
            Opcode::Fp => 2,      // dst, imm
            Opcode::Bnz => 3,     // target_low, target_high, cond
            Opcode::Bz => 0,      // non-existing instruction
            Opcode::Jumpi => 2,   // target_low, target_high
            Opcode::Jumpv => 1,   // offset
            Opcode::Xori => 3,    // dst, src, imm
            Opcode::Xor => 3,     // dst, src1, src2
            Opcode::Ret => 0,     //
            Opcode::Slli => 3,    // dst, src, imm
            Opcode::Srli => 3,    // dst, src, imm
            Opcode::Srai => 3,    // dst, src, imm
            Opcode::Sll => 3,     // dst, src1, src2
            Opcode::Srl => 3,     // dst, src1, src2
            Opcode::Sra => 3,     // dst, src1, src2
            Opcode::Tailv => 2,   // offset, next_fp
            Opcode::Taili => 3,   // target_low, target_high, next_fp
            Opcode::Calli => 3,   // target_low, target_high, next_fp
            Opcode::Callv => 2,   // offset, next_fp
            Opcode::And => 3,     // dst, src1, src2
            Opcode::Andi => 3,    // dst, src, imm
            Opcode::Sub => 3,     // dst, src1, src2
            Opcode::Sle => 3,     // dst, src1, src2
            Opcode::Slei => 3,    // dst, src, imm
            Opcode::Sleu => 3,    // dst, src1, src2
            Opcode::Sleiu => 3,   // dst, src, imm
            Opcode::Slt => 3,     // dst, src1, src2
            Opcode::Slti => 3,    // dst, src, imm
            Opcode::Sltu => 3,    // dst, src1, src2
            Opcode::Sltiu => 3,   // dst, src, imm
            Opcode::Or => 3,      // dst, src1, src2
            Opcode::Ori => 3,     // dst, src, imm
            Opcode::Muli => 3,    // dst, src, imm
            Opcode::Mulu => 3,    // dst, src1, src2
            Opcode::Mul => 3,     // dst, src1, src2
            Opcode::Mulsu => 3,   // dst, src1, src2
            Opcode::B32Mul => 3,  // dst, src1, src2
            Opcode::B32Muli => 3, // dst, src, imm
            Opcode::B128Add => 3, // dst, src1, src2
            Opcode::B128Mul => 3, // dst, src1, src2
            Opcode::Add => 3,     // dst, src1, src2
            Opcode::Addi => 3,    // dst, src, imm
            Opcode::Mvvw => 3,    // dst, offset, src
            Opcode::Mvvl => 3,    // dst, offset, src
            Opcode::Mvih => 3,    // dst, offset, imm
            Opcode::Ldi => 3,     // dst, imm_low, imm_high
            Opcode::Alloci => 2,  // dst, imm
            Opcode::Allocv => 2,  // dst, src
            Opcode::Invalid => 0, // invalid
        }
    }

    /// Returns true if the opcode cannot be prover-only.
    pub const fn is_verifier_only(&self) -> bool {
        matches!(
            self,
            Opcode::Bnz
                | Opcode::Bz
                | Opcode::Jumpi
                | Opcode::Jumpv
                | Opcode::Taili
                | Opcode::Tailv
                | Opcode::Calli
                | Opcode::Callv
                | Opcode::Ret
        )
    }
}

/// Trait implemented by each [`Event`] type.
pub trait InstructionInfo {
    /// The unique opcode associated with this instruction.
    fn opcode() -> Opcode;
}

macro_rules! impl_instruction_info {
    ( $( ($event_ty:ty, $opcode:path) ),* $(,)? ) => {
        $(
            impl InstructionInfo for $event_ty {
                fn opcode() -> Opcode {
                    $opcode
                }
            }
        )*
    };
}

impl_instruction_info!(
    (AddEvent, Opcode::Add),
    (AddiEvent, Opcode::Addi),
    (AndEvent, Opcode::And),
    (AndiEvent, Opcode::Andi),
    (BnzEvent, Opcode::Bnz),
    // `BzEvent` is actually triggered through the `Bnz` instruction
    (BzEvent, Opcode::Bz),
    (B32MulEvent, Opcode::B32Mul),
    (B32MuliEvent, Opcode::B32Muli),
    (B128AddEvent, Opcode::B128Add),
    (B128MulEvent, Opcode::B128Mul),
    (CalliEvent, Opcode::Calli),
    (CallvEvent, Opcode::Callv),
    (FpEvent, Opcode::Fp),
    (JumpiEvent, Opcode::Jumpi),
    (JumpvEvent, Opcode::Jumpv),
    (LdiEvent, Opcode::Ldi),
    (MulEvent, Opcode::Mul),
    (MuliEvent, Opcode::Muli),
    (MuluEvent, Opcode::Mulu),
    (MulsuEvent, Opcode::Mulsu),
    (MvihEvent, Opcode::Mvih),
    (MvvlEvent, Opcode::Mvvl),
    (MvvwEvent, Opcode::Mvvw),
    (OrEvent, Opcode::Or),
    (OriEvent, Opcode::Ori),
    (RetEvent, Opcode::Ret),
    (SleEvent, Opcode::Sle),
    (SleiEvent, Opcode::Slei),
    (SleuEvent, Opcode::Sleu),
    (SleiuEvent, Opcode::Sleiu),
    (SllEvent, Opcode::Sll),
    (SlliEvent, Opcode::Slli),
    (SltEvent, Opcode::Slt),
    (SltiEvent, Opcode::Slti),
    (SltuEvent, Opcode::Sltu),
    (SltiuEvent, Opcode::Sltiu),
    (SraEvent, Opcode::Sra),
    (SraiEvent, Opcode::Srai),
    (SrlEvent, Opcode::Srl),
    (SrliEvent, Opcode::Srli),
    (SubEvent, Opcode::Sub),
    (TailiEvent, Opcode::Taili),
    (TailvEvent, Opcode::Tailv),
    (XorEvent, Opcode::Xor),
    (XoriEvent, Opcode::Xori),
    (AllociEvent, Opcode::Alloci),
    (AllocvEvent, Opcode::Allocv),
);
