// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

use std::sync::Arc;

use futures::select;
use futures::Future;
use futures::FutureExt;

use crate::raw::*;
use crate::*;

/// PositionWrite is used to implement [`oio::Write`] based on position write.
///
/// # Services
///
/// Services like fs support position write.
///
/// # Architecture
///
/// The architecture after adopting [`PositionWrite`]:
///
/// - Services impl `PositionWrite`
/// - `PositionWriter` impl `Write`
/// - Expose `PositionWriter` as `Accessor::Writer`
///
/// # Requirements
///
/// Services that implement `PositionWrite` must fulfill the following requirements:
///
/// - Writing data based on position: `offset`.
pub trait PositionWrite: Send + Sync + Unpin + 'static {
    /// write_all_at is used to write the data to underlying storage at the specified offset.
    fn write_all_at(
        &self,
        offset: u64,
        buf: Buffer,
    ) -> impl Future<Output = Result<()>> + MaybeSend;

    /// close is used to close the underlying file.
    fn close(&self) -> impl Future<Output = Result<Metadata>> + MaybeSend;

    /// abort is used to abort the underlying abort.
    fn abort(&self) -> impl Future<Output = Result<()>> + MaybeSend;
}

struct WriteInput<W: PositionWrite> {
    w: Arc<W>,
    executor: Executor,

    offset: u64,
    bytes: Buffer,
}

/// PositionWriter will implements [`oio::Write`] based on position write.
pub struct PositionWriter<W: PositionWrite> {
    w: Arc<W>,
    executor: Executor,

    next_offset: u64,
    cache: Option<Buffer>,
    tasks: ConcurrentTasks<WriteInput<W>, ()>,
}

#[allow(dead_code)]
impl<W: PositionWrite> PositionWriter<W> {
    /// Create a new PositionWriter.
    pub fn new(inner: W, executor: Option<Executor>, concurrent: usize) -> Self {
        let executor = executor.unwrap_or_default();

        Self {
            w: Arc::new(inner),
            executor: executor.clone(),
            next_offset: 0,
            cache: None,

            tasks: ConcurrentTasks::new(executor, concurrent, |input| {
                Box::pin(async move {
                    let fut = input.w.write_all_at(input.offset, input.bytes.clone());
                    match input.executor.timeout() {
                        None => {
                            let result = fut.await;
                            (input, result)
                        }
                        Some(timeout) => {
                            let result = select! {
                                result = fut.fuse() => {
                                    result
                                }
                                _ = timeout.fuse() => {
                                      Err(Error::new(
                                            ErrorKind::Unexpected, "write position timeout")
                                                .with_context("offset", input.offset.to_string())
                                                .set_temporary())
                                }
                            };
                            (input, result)
                        }
                    }
                })
            }),
        }
    }

    fn fill_cache(&mut self, bs: Buffer) -> usize {
        let size = bs.len();
        assert!(self.cache.is_none());
        self.cache = Some(bs);
        size
    }
}

impl<W: PositionWrite> oio::Write for PositionWriter<W> {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        if self.cache.is_none() {
            let _ = self.fill_cache(bs);
            return Ok(());
        }

        let bytes = self.cache.clone().expect("pending write must exist");
        let length = bytes.len() as u64;
        let offset = self.next_offset;

        self.tasks
            .execute(WriteInput {
                w: self.w.clone(),
                executor: self.executor.clone(),
                offset,
                bytes,
            })
            .await?;
        self.cache = None;
        self.next_offset += length;
        let _ = self.fill_cache(bs);
        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        // Make sure all tasks are finished.
        while self.tasks.next().await.transpose()?.is_some() {}

        if let Some(buffer) = self.cache.clone() {
            let offset = self.next_offset;
            self.w.write_all_at(offset, buffer).await?;
            self.cache = None;
        }
        self.w.close().await
    }

    async fn abort(&mut self) -> Result<()> {
        self.tasks.clear();
        self.cache = None;
        self.w.abort().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Mutex;
    use std::time::Duration;

    use pretty_assertions::assert_eq;
    use rand::thread_rng;
    use rand::Rng;
    use rand::RngCore;
    use tokio::time::sleep;

    use super::*;
    use crate::raw::oio::Write;

    struct TestWrite {
        length: u64,
        bytes: HashSet<u64>,
    }

    impl TestWrite {
        pub fn new() -> Arc<Mutex<Self>> {
            let v = Self {
                bytes: HashSet::new(),
                length: 0,
            };

            Arc::new(Mutex::new(v))
        }
    }

    impl PositionWrite for Arc<Mutex<TestWrite>> {
        async fn write_all_at(&self, offset: u64, buf: Buffer) -> Result<()> {
            // Add an async sleep here to enforce some pending.
            sleep(Duration::from_millis(50)).await;

            // We will have 10% percent rate for write part to fail.
            if thread_rng().gen_bool(1.0 / 10.0) {
                return Err(
                    Error::new(ErrorKind::Unexpected, "I'm a crazy monkey!").set_temporary()
                );
            }

            let mut test = self.lock().unwrap();
            let size = buf.len() as u64;
            test.length += size;

            let input = (offset..offset + size).collect::<HashSet<_>>();

            assert!(
                test.bytes.is_disjoint(&input),
                "input should not have overlap"
            );
            test.bytes.extend(input);

            Ok(())
        }

        async fn close(&self) -> Result<Metadata> {
            Ok(Metadata::default())
        }

        async fn abort(&self) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_position_writer_with_concurrent_errors() {
        let mut rng = thread_rng();

        let mut w = PositionWriter::new(TestWrite::new(), Some(Executor::new()), 200);
        let mut total_size = 0u64;

        for _ in 0..1000 {
            let size = rng.gen_range(1..1024);
            total_size += size as u64;

            let mut bs = vec![0; size];
            rng.fill_bytes(&mut bs);

            loop {
                match w.write(bs.clone().into()).await {
                    Ok(_) => break,
                    Err(e) => {
                        println!("write error: {:?}", e);
                        continue;
                    }
                }
            }
        }

        loop {
            match w.close().await {
                Ok(n) => {
                    println!("close: {:?}", n);
                    break;
                }
                Err(e) => {
                    println!("close error: {:?}", e);
                    continue;
                }
            }
        }

        let actual_bytes = w.w.lock().unwrap().bytes.clone();
        let expected_bytes: HashSet<_> = (0..total_size).collect();
        assert_eq!(actual_bytes, expected_bytes);

        let actual_size = w.w.lock().unwrap().length;
        assert_eq!(actual_size, total_size);
    }
}
