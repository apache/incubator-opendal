use crate::{raw::oio, Buffer, Result};

pub struct OpfsWriter {}

impl oio::Write for OpfsWriter {
    async fn abort(&mut self) -> Result<()> {
        panic!()
    }

    async fn close(&mut self) -> Result<()> {
        panic!()
    }

    async fn write(&mut self, bs: Buffer) -> Result<()> {
        panic!()
    }
}

impl oio::BlockingWrite for OpfsWriter {
    fn close(&mut self) -> Result<()> {
        panic!()
    }

    fn write(&mut self, bs: Buffer) -> Result<()> {
        panic!()
    }
}
