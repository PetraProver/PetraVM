use binius_field::underlier::UnderlierType;
use num_traits::{ops::overflowing::OverflowingAdd, FromPrimitive, PrimInt};

use crate::event::context::EventContext;

/// Generic gadget for addition over the integers.
#[derive(Debug, Clone)]
pub struct AddGadget<T: Copy + PrimInt + FromPrimitive + OverflowingAdd> {
    pub timestamp: u32,
    pub output: T,
    pub input1: T,
    pub input2: T,
    pub cout: T,
}

impl<T: Copy + PrimInt + FromPrimitive + OverflowingAdd + UnderlierType> AddGadget<T> {
    pub(crate) fn generate_gadget(ctx: &mut EventContext, input1: T, input2: T) -> Self {
        let (output, carry) = input1.overflowing_add(&input2);

        // cin's i-th bit stores the carry which was added to the sum's i-th bit.
        let cin = output ^ input1 ^ input2;
        // cout's i-th bit stores the carry for input1[i] + input2[i].
        let cout = (cin >> 1)
            + (T::from(carry as usize).expect("It should be possible to get T from usize.")
                << (T::BITS - 1));

        // Check cout.
        assert!(((input1 ^ cin) & (input2 ^ cin)) ^ cin == cout);

        let timestamp = ctx.timestamp;

        Self {
            timestamp,
            output,
            input1,
            input2,
            cout,
        }
    }
}

pub(crate) type Add32Gadget = AddGadget<u32>;
pub(crate) type Add64Gadget = AddGadget<u64>;
