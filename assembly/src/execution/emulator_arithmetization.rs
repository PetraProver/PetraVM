pub mod arithmetization {
    use binius_core::constraint_system::channel::ChannelId;
    use binius_field::{as_packed_field::PackScalar, underlier::UnderlierType, BinaryField1b};
    use binius_m3::builder::{
        ConstraintSystem, TableWitnessIndex, TableWitnessIndexSegment, WitnessIndex,
    };
    use bytemuck::Pod;

    use crate::{
        event::arithmetization::{branch::{BnzTable, BzTable}, integer_ops::AddTable, ret::RetTable},
        ZCrayTrace,
    };

    pub struct ZCrayTable {
        pub(crate) add_table: AddTable,
        pub(crate) ret_table: RetTable,
        pub(crate) bnz_table: BnzTable,
        pub(crate) bz_table: BzTable,
        pub(crate) state_channel: ChannelId,
        pub(crate) prom_channel: ChannelId,
        pub(crate) vrom_channel: ChannelId,
    }

    impl ZCrayTable {
        pub fn new(cs: &mut ConstraintSystem) -> Self {
            let state_channel = cs.add_channel("state_channel");
            let prom_channel = cs.add_channel("prom_channel");
            let vrom_channel = cs.add_channel("vrom_channel");
            Self {
                add_table: AddTable::new(cs, state_channel, vrom_channel, prom_channel),
                ret_table: RetTable::new(cs, state_channel, vrom_channel, prom_channel),
                bnz_table: BnzTable::new(cs, state_channel, vrom_channel, prom_channel),
                bz_table: BzTable::new(cs, state_channel, vrom_channel, prom_channel),
                state_channel,
                prom_channel,
                vrom_channel,
            }
        }

        pub fn populate<U: Pod + UnderlierType + PackScalar<BinaryField1b>>(
            &self,
            trace: ZCrayTrace,
            witness: &mut WitnessIndex<U>,
        ) -> Result<(), anyhow::Error> {
            println!("add: {:?}", trace.add);
            println!("ret:{:?}", trace.ret);
            println!("bnz:{:?}", trace.bnz);
            println!("bz:{:?}", trace.bz);
            witness.fill_table_sequential(&self.add_table, &trace.add)?;
            witness.fill_table_sequential(&self.ret_table, &trace.ret)?;
            witness.fill_table_sequential(&self.bnz_table, &trace.bnz)?;
            witness.fill_table_sequential(&self.bz_table, &trace.bz)?;
            Ok(())
        }
    }
}
