use binius_field::{BinaryField, BinaryField16b, BinaryField32b, Field, PackedField};
use thiserror::Error;

#[derive(Debug, Clone, Copy)]
pub struct Slot(u32);

#[derive(Debug, Clone, Copy)]
pub struct SlotWithOffset(u32, u16);

#[derive(Debug, Clone, Copy)]
pub struct Immediate(u16);

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
    pub(crate) fn get_16bfield_val(&self) -> BinaryField16b {
        BinaryField16b::new(self.0 as u16)
    }

    pub(crate) fn get_high_16bfield_val(&self) -> BinaryField16b {
        BinaryField16b::new((self.0 >> 16) as u16)
    }

    pub(crate) fn get_32bfield_val(&self) -> BinaryField32b {
        BinaryField32b::new(self.0)
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
    pub(crate) fn get_slot_16bfield_val(&self) -> BinaryField16b {
        BinaryField16b::new(self.0 as u16)
    }

    pub(crate) fn get_slot_32bfield_val(&self) -> BinaryField32b {
        BinaryField32b::new(self.0)
    }

    pub(crate) fn get_offset_field_val(&self) -> BinaryField16b {
        BinaryField16b::new(self.1)
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
        let is_field = s.chars().last().unwrap() == 'G';
        let s = s.trim_start_matches('#').trim_end_matches("G");
        let int_val = i16::from_str(s).map_err(|_| BadArgumentError::Immediate(s.to_string()))?;
        if is_field {
            let v = BinaryField32b::MULTIPLICATIVE_GENERATOR.pow(int_val.abs() as u64);
            if int_val < 0 {
                Ok(Immediate(
                    v.invert().expect("We already ensured v is not 0.").val() as u16,
                ))
            } else {
                Ok(Immediate(v.val() as u16))
            }
        } else {
            Ok(Immediate(int_val as u16))
        }
    }
}

impl Immediate {
    pub(crate) fn get_field_val(&self) -> BinaryField16b {
        BinaryField16b::new(self.0 as u16)
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
}
