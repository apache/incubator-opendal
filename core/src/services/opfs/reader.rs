use crate::{raw::oio, Buffer, Result};

pub struct OpfsReader {}

// impl oio::Read for OpfsReader
impl oio::Read for OpfsReader {
    async fn read(&mut self) -> Result<Buffer> {
        panic!()
    }

    async fn read_all(&mut self) -> Result<Buffer> {
        panic!()
    }
}

// impl oio::BlockingRead for OpfsReader
impl oio::BlockingRead for OpfsReader {
    fn read(&mut self) -> Result<Buffer> {
        panic!()
    }
}
