use thiserror::Error;

#[derive(Debug, Clone, Copy)]
pub struct Slot(u16);

#[derive(Debug, Clone, Copy)]
pub struct SlotWithOffset(u16, u16);

#[derive(Debug, Clone, Copy)]
pub struct Immediate(i16);

impl std::fmt::Display for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}", self.0)
    }
}

impl std::str::FromStr for Slot {
    type Err = BadArgumentError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        u16::from_str(s.trim_start_matches('@'))
            .map(Self)
            .map_err(|_| BadArgumentError::Slot(s.to_string()))
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

impl std::fmt::Display for Immediate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}G", self.0)
    }
}

impl std::str::FromStr for Immediate {
    type Err = BadArgumentError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim_start_matches('#').trim_end_matches("G");
        i16::from_str(s)
            .map(Self)
            .map_err(|_| BadArgumentError::Immediate(s.to_string()))
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
