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

use std::pin::Pin;
use std::sync::Arc;
use std::task::ready;
use std::task::Context;
use std::task::Poll;

use async_trait::async_trait;
use bytes::Bytes;
use futures::Future;
use futures::FutureExt;
use futures::StreamExt;

use crate::raw::*;
use crate::*;

pub trait MultipartWrite: Send + Sync + Unpin + 'static {
    /// write_once is used to write the data to underlying storage at once.
    ///
    /// MultipartWriter will call this API when:
    ///
    /// - All the data has been written to the buffer and we can perform the upload at once.
    fn write_once(&self, size: u64, body: AsyncBody) -> impl Future<Output = Result<()>> + Send;

    /// initiate_part will call start a multipart upload and return the upload id.
    ///
    /// MultipartWriter will call this when:
    ///
    /// - the total size of data is unknown.
    /// - the total size of data is known, but the size of current write
    /// is less then the total size.
    fn initiate_part(&self) -> impl Future<Output = Result<String>> + Send;

    /// write_part will write a part of the data and returns the result
    /// [`MultipartPart`].
    ///
    /// MultipartWriter will call this API and stores the result in
    /// order.
    ///
    /// - part_number is the index of the part, starting from 0.
    fn write_part(
        &self,
        upload_id: &str,
        part_number: usize,
        size: u64,
        body: AsyncBody,
    ) -> impl Future<Output = Result<MultipartPart>> + Send;

    /// complete_part will complete the multipart upload to build the final
    /// file.
    fn complete_part(
        &self,
        upload_id: &str,
        parts: &[MultipartPart],
    ) -> impl Future<Output = Result<()>> + Send;

    /// abort_part will cancel the multipart upload and purge all data.
    fn abort_part(&self, upload_id: &str) -> impl Future<Output = Result<()>> + Send;
}

/// The result of [`MultipartWrite::write_part`].
///
/// services implement should convert MultipartPart to their own represents.
///
/// - `part_number` is the index of the part, starting from 0.
/// - `etag` is the `ETag` of the part.
#[derive(Clone)]
pub struct MultipartPart {
    /// The number of the part, starting from 0.
    pub part_number: usize,
    /// The etag of the part.
    pub etag: String,
}

/// WritePartResult is the result returned by [`WritePartFuture`].
///
/// The error part will carries input `(part_number, bytes, err)` so caller can retry them.
type WritePartResult = std::result::Result<MultipartPart, (usize, oio::ChunkedBytes, Error)>;

struct WritePartFuture(BoxedStaticFuture<WritePartResult>);

/// # Safety
///
/// wasm32 is a special target that we only have one event-loop for this WritePartFuture.
unsafe impl Send for WritePartFuture {}

/// # Safety
///
/// We will only take `&mut Self` reference for WritePartFuture.
unsafe impl Sync for WritePartFuture {}

impl Future for WritePartFuture {
    type Output = WritePartResult;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().0.poll_unpin(cx)
    }
}

impl WritePartFuture {
    pub fn new<W: MultipartWrite>(
        w: Arc<W>,
        upload_id: Arc<String>,
        part_number: usize,
        bytes: oio::ChunkedBytes,
    ) -> Self {
        let fut = async move {
            w.write_part(
                &upload_id,
                part_number,
                bytes.len() as u64,
                AsyncBody::ChunkedBytes(bytes.clone()),
            )
            .await
            .map_err(|err| (part_number, bytes, err))
        };

        WritePartFuture(Box::pin(fut))
    }
}

/// MultipartWriter will implements [`Write`] based on multipart
/// uploads.
pub struct MultipartWriter<W: MultipartWrite> {
    w: Arc<W>,

    upload_id: Option<Arc<String>>,
    parts: Vec<MultipartPart>,
    cache: Option<oio::ChunkedBytes>,
    futures: ConcurrentFutures<WritePartFuture>,
    next_part_number: usize,
}

/// # Safety
///
/// wasm32 is a special target that we only have one event-loop for this state.

impl<W: MultipartWrite> MultipartWriter<W> {
    /// Create a new MultipartWriter.
    pub fn new(inner: W, concurrent: usize) -> Self {
        Self {
            w: Arc::new(inner),
            upload_id: None,
            parts: Vec::new(),
            cache: None,
            futures: ConcurrentFutures::new(1.max(concurrent)),
            next_part_number: 0,
        }
    }

    fn fill_cache(&mut self, bs: Bytes) -> usize {
        let size = bs.len();
        let bs = oio::ChunkedBytes::from_vec(vec![bs]);
        assert!(self.cache.is_none());
        self.cache = Some(bs);
        size
    }
}

impl<W> oio::Write for MultipartWriter<W>
where
    W: MultipartWrite,
{
    async fn write(&mut self, bs: Bytes) -> Result<usize> {
        let upload_id = match self.upload_id.clone() {
            Some(v) => v,
            None => {
                // Fill cache with the first write.
                if self.cache.is_none() {
                    let size = self.fill_cache(bs);
                    return Ok(size);
                }

                let upload_id = self.w.initiate_part().await?;
                let upload_id = Arc::new(upload_id);
                self.upload_id = Some(upload_id.clone());
                upload_id
            }
        };

        loop {
            if self.futures.has_remaining() {
                let cache = self.cache.take().expect("pending write must exist");
                let part_number = self.next_part_number;
                self.next_part_number += 1;

                self.futures.push_back(WritePartFuture::new(
                    self.w.clone(),
                    upload_id.clone(),
                    part_number,
                    cache,
                ));
                let size = self.fill_cache(bs);
                return Ok(size);
            }

            if let Some(part) = self.futures.next().await {
                match part {
                    Ok(part) => {
                        self.parts.push(part);
                        continue;
                    }
                    Err((part_number, bytes, err)) => {
                        self.futures.push_front(WritePartFuture::new(
                            self.w.clone(),
                            upload_id.clone(),
                            part_number,
                            bytes,
                        ));
                        return Err(err);
                    }
                }
            }
        }
    }

    async fn close(&mut self) -> Result<()> {
        let upload_id = match self.upload_id.clone() {
            Some(v) => v,
            None => {
                let (size, body) = match self.cache.clone() {
                    Some(cache) => (cache.len(), AsyncBody::ChunkedBytes(cache)),
                    None => (0, AsyncBody::Empty),
                };
                // Call write_once if there is no upload_id.
                self.w.write_once(size as u64, body).await?;
                self.cache = None;
                return Ok(());
            }
        };

        loop {
            // futures queue is empty and cache is consumed, we can complete the upload.
            if self.futures.is_empty() && self.cache.is_none() {
                return self.w.complete_part(&upload_id, &self.parts).await;
            }

            if self.futures.has_remaining() {
                // This must be the final task.
                if let Some(cache) = self.cache.take() {
                    let part_number = self.next_part_number;
                    self.next_part_number += 1;

                    self.futures.push_back(WritePartFuture::new(
                        self.w.clone(),
                        upload_id.clone(),
                        part_number,
                        cache,
                    ));
                }
            }

            if let Some(part) = self.futures.next().await {
                match part {
                    Ok(part) => {
                        self.parts.push(part);
                        continue;
                    }
                    Err((part_number, bytes, err)) => {
                        self.futures.push_front(WritePartFuture::new(
                            self.w.clone(),
                            upload_id.clone(),
                            part_number,
                            bytes,
                        ));
                        return Err(err);
                    }
                }
            }
        }
    }

    async fn abort(&mut self) -> Result<()> {
        let Some(upload_id) = self.upload_id.clone() else {
            return Ok(());
        };

        self.futures.clear();
        self.w.abort_part(&upload_id).await?;
        self.cache = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use pretty_assertions::assert_eq;
    use rand::thread_rng;
    use rand::Rng;
    use rand::RngCore;

    use super::*;
    use crate::raw::oio::Write;

    struct TestWrite {
        upload_id: String,
        part_numbers: Vec<usize>,
        length: u64,
    }

    impl TestWrite {
        pub fn new() -> Arc<Mutex<Self>> {
            let v = Self {
                upload_id: uuid::Uuid::new_v4().to_string(),
                part_numbers: Vec::new(),
                length: 0,
            };

            Arc::new(Mutex::new(v))
        }
    }

    impl MultipartWrite for Arc<Mutex<TestWrite>> {
        async fn write_once(&self, size: u64, _: AsyncBody) -> Result<()> {
            self.lock().unwrap().length += size;
            Ok(())
        }

        async fn initiate_part(&self) -> Result<String> {
            let upload_id = self.lock().unwrap().upload_id.clone();
            Ok(upload_id)
        }

        async fn write_part(
            &self,
            upload_id: &str,
            part_number: usize,
            size: u64,
            _: AsyncBody,
        ) -> Result<MultipartPart> {
            let mut test = self.lock().unwrap();
            assert_eq!(upload_id, test.upload_id);

            // We will have 50% percent rate for write part to fail.
            if thread_rng().gen_bool(5.0 / 10.0) {
                return Err(Error::new(ErrorKind::Unexpected, "I'm a crazy monkey!"));
            }

            test.part_numbers.push(part_number);
            test.length += size;

            Ok(MultipartPart {
                part_number,
                etag: "etag".to_string(),
            })
        }

        async fn complete_part(&self, upload_id: &str, parts: &[MultipartPart]) -> Result<()> {
            let test = self.lock().unwrap();
            assert_eq!(upload_id, test.upload_id);
            assert_eq!(parts.len(), test.part_numbers.len());

            Ok(())
        }

        async fn abort_part(&self, upload_id: &str) -> Result<()> {
            let test = self.lock().unwrap();
            assert_eq!(upload_id, test.upload_id);

            Ok(())
        }
    }

    #[tokio::test]
    async fn test_multipart_upload_writer_with_concurrent_errors() {
        let mut rng = thread_rng();

        let mut w = MultipartWriter::new(TestWrite::new(), 8);
        let mut total_size = 0u64;

        for _ in 0..1000 {
            let size = rng.gen_range(1..1024);
            total_size += size as u64;

            let mut bs = vec![0; size];
            rng.fill_bytes(&mut bs);

            loop {
                match w.write(Bytes::copy_from_slice(&bs)).await {
                    Ok(_) => break,
                    Err(_) => continue,
                }
            }
        }

        loop {
            match w.close().await {
                Ok(_) => break,
                Err(_) => continue,
            }
        }

        let actual_parts: Vec<_> = w.parts.into_iter().map(|v| v.part_number).collect();
        let expected_parts: Vec<_> = (0..1000).collect();
        assert_eq!(actual_parts, expected_parts);

        let actual_size = w.w.lock().unwrap().length;
        assert_eq!(actual_size, total_size);
    }
}
