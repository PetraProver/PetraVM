use binius_field::{BinaryField, Field, PackedField};
use binius_m3::builder::{B16, B32};
use thiserror::Error;

#[derive(Debug, Clone, Copy)]
pub struct Slot(u32);

#[derive(Debug, Clone, Copy)]
pub struct SlotWithOffset(u32, u16);

#[derive(Debug, Clone, Copy)]
pub struct Immediate(u32);

#[derive(Debug, Clone, Copy)]
pub struct ExcCode(u8);

impl std::fmt::Display for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}", self.0)
    }
}

impl std::str::FromStr for Slot {
    type Err = BadArgumentError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        u32::from_str(s.trim_start_matches('@'))
            .map(Self)
            .map_err(|_| BadArgumentError::Slot(s.to_string()))
    }
}

impl Slot {
    pub(crate) const fn get_16bfield_val(self) -> B16 {
        B16::new(self.0 as u16)
    }
}

impl std::fmt::Display for SlotWithOffset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}[{}]", self.0, self.1)
    }
}

impl std::str::FromStr for SlotWithOffset {
    type Err = BadArgumentError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (slot, offset) = s
            .split_once('[')
            .ok_or(BadArgumentError::SlotOffset(s.to_string()))?;
        let slot = Slot::from_str(slot)?;
        let offset = u16::from_str(offset.trim_end_matches(']'))
            .map_err(|_| BadArgumentError::SlotOffset(s.to_string()))?;
        Ok(Self(slot.0, offset))
    }
}

impl SlotWithOffset {
    pub(crate) const fn get_slot_16bfield_val(self) -> B16 {
        B16::new(self.0 as u16)
    }

    pub(crate) const fn get_offset_field_val(self) -> B16 {
        B16::new(self.1)
    }
}

impl std::fmt::Display for Immediate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}G", self.0)
    }
}

impl std::str::FromStr for Immediate {
    type Err = BadArgumentError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let is_field = s.ends_with('G');
        let s = s.trim_start_matches('#').trim_end_matches("G");

        let int_val =
            i64::from_str(s).map_err(|_| BadArgumentError::Immediate(s.to_string()))? as i32;
        if is_field {
            let v = B32::MULTIPLICATIVE_GENERATOR.pow(int_val.unsigned_abs() as u64);
            if int_val < 0 {
                Ok(Immediate(
                    v.invert().expect("We already ensured v is not 0.").val(),
                ))
            } else {
                Ok(Immediate(v.val()))
            }
        } else {
            Ok(Immediate(int_val as u32))
        }
    }
}

impl Immediate {
    pub(crate) const fn get_field_val(self) -> B16 {
        B16::new(self.0 as u16)
    }

    pub(crate) const fn get_high_field_val(self) -> B16 {
        B16::new((self.0 >> 16) as u16)
    }
}

impl std::fmt::Display for ExcCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}G", self.0)
    }
}

impl std::str::FromStr for ExcCode {
    type Err = BadArgumentError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim_start_matches('#');

        let int_val = u8::from_str(s).map_err(|_| BadArgumentError::Immediate(s.to_string()))?;
        Ok(ExcCode(int_val))
    }
}

impl ExcCode {
    pub(crate) const fn get_field_val(self) -> B16 {
        B16::new(self.0 as u16)
    }
}

#[derive(Error, Debug)]
pub enum BadArgumentError {
    #[error("Bad slot argument: {0}")]
    Slot(String),

    #[error("Bad slot offset argument: {0}")]
    SlotOffset(String),

    #[error("Bad immediate argument: {0}")]
    Immediate(String),

    #[error("Bad exception code: {0}")]
    ExcCode(String),

    #[error("Bad frame size argument: {0}")]
    FrameSize(String),
}
