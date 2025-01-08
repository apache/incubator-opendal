use crate::{raw::oio, Result};

pub struct OpfsLister {}

impl oio::List for OpfsLister {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        panic!()
    }
}

impl oio::BlockingList for OpfsLister {
    fn next(&mut self) -> Result<Option<oio::Entry>> {
        panic!()
    }
}
