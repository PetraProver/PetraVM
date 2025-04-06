//! Debugging module to detect unbalanced channels during program execution.

use std::{collections::HashMap, fmt::Debug, hash::Hash};

use binius_m3::builder::B32;
use tracing::{debug, trace};

#[derive(Debug, Default)]
pub struct Channel<T> {
    net_multiplicities: HashMap<T, isize>,
}

// TODO: Think on unifying types used for recurring variables (fp, pc, ...)

pub(crate) type PromChannel = Channel<(u32, u128)>; // PC, opcode, args (so 64 bits overall).
pub(crate) type VromChannel = Channel<u32>;
pub(crate) type StateChannel = Channel<(B32, u32, u32)>; // pc, *fp, timestamp

impl<T: Hash + Eq + Debug> Channel<T> {
    pub(crate) fn push(&mut self, val: T) {
        trace!("PUSH {:?}", val);
        match self.net_multiplicities.get_mut(&val) {
            Some(multiplicity) => {
                *multiplicity += 1;

                // Remove the key if the multiplicity is zero, to improve Debug behavior.
                if *multiplicity == 0 {
                    self.net_multiplicities.remove(&val);
                }
            }
            None => {
                let _ = self.net_multiplicities.insert(val, 1);
            }
        }
    }

    pub(crate) fn pull(&mut self, val: T) {
        trace!("PULL {:?}", val);
        match self.net_multiplicities.get_mut(&val) {
            Some(multiplicity) => {
                *multiplicity -= 1;

                // Remove the key if the multiplicity is zero, to improve Debug behavior.
                if *multiplicity == 0 {
                    self.net_multiplicities.remove(&val);
                }
            }
            None => {
                let _ = self.net_multiplicities.insert(val, -1);
            }
        }
    }
}

impl StateChannel {
    pub(crate) fn is_balanced(&self) -> bool {
        #[cfg(debug_assertions)]
        if !self.net_multiplicities.is_empty() {
            let mut sorted_multiplicities: Vec<_> =
                self.net_multiplicities.clone().into_iter().collect();

            // Sort by timestamp
            sorted_multiplicities.sort_by_key(|((_pc, _fp, timestamp), _)| *timestamp);

            // TODO: better debugging?
            debug!("Unbalanced State Channel:");
            let _ = sorted_multiplicities
                .iter()
                .map(|x| trace!("{:?}", x))
                .collect::<Vec<_>>();
        }
        self.net_multiplicities.is_empty()
    }
}
