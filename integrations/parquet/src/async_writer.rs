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

use bytes::Bytes;
use parquet::arrow::async_writer::AsyncFileWriter;
use parquet::errors::{ParquetError, Result};

use futures::future::BoxFuture;
use opendal::Writer;

/// AsyncWriter implements AsyncFileWriter trait by using opendal.
///
/// ```no_run
/// use std::sync::Arc;
///
/// use arrow::array::{ArrayRef, Int64Array, RecordBatch};
///
/// use opendal::{services::S3Config, Operator};
/// use parquet::arrow::{arrow_reader::ParquetRecordBatchReaderBuilder, AsyncArrowWriter};
/// use parquet_opendal::AsyncWriter;
///
/// #[tokio::main]
/// async fn main() {
///     let mut cfg = S3Config::default();
///     cfg.access_key_id = Some("my_access_key".to_string());
///     cfg.secret_access_key = Some("my_secret_key".to_string());
///     cfg.endpoint = Some("my_endpoint".to_string());
///     cfg.region = Some("my_region".to_string());
///     cfg.bucket = "my_bucket".to_string();
///
///     // Create a new operator
///     let operator = Operator::from_config(cfg).unwrap().finish();
///     let path = "/path/to/file.parquet";
///
///     // Create an async writer
///     let writer = AsyncWriter::new(
///         operator
///             .writer_with(path)
///             .chunk(32 * 1024 * 1024)
///             .concurrent(8)
///             .await
///             .unwrap(),
///     );
///
///     let col = Arc::new(Int64Array::from_iter_values([1, 2, 3])) as ArrayRef;
///     let to_write = RecordBatch::try_from_iter([("col", col)]).unwrap();
///     let mut writer = AsyncArrowWriter::try_new(writer, to_write.schema(), None).unwrap();
///     writer.write(&to_write).await.unwrap();
///     writer.close().await.unwrap();
///
///     let buffer = operator.read(path).await.unwrap().to_bytes();
///     let mut reader = ParquetRecordBatchReaderBuilder::try_new(buffer)
///         .unwrap()
///         .build()
///         .unwrap();
///     let read = reader.next().unwrap().unwrap();
///     assert_eq!(to_write, read);
/// }
/// ```
pub struct AsyncWriter {
    inner: Writer,
}

impl AsyncWriter {
    /// Create a [`AsyncWriter`] by given [`Writer`].
    pub fn new(writer: Writer) -> Self {
        Self { inner: writer }
    }
}

impl AsyncFileWriter for AsyncWriter {
    fn write(&mut self, bs: Bytes) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            self.inner
                .write(bs)
                .await
                .map_err(|err| ParquetError::External(Box::new(err)))
        })
    }

    fn complete(&mut self) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            self.inner
                .close()
                .await
                .map(|_| ())
                .map_err(|err| ParquetError::External(Box::new(err)))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use arrow::array::{ArrayRef, Int64Array, RecordBatch};
    use opendal::{services, Operator};
    use parquet::arrow::{arrow_reader::ParquetRecordBatchReaderBuilder, AsyncArrowWriter};

    #[tokio::test]
    async fn test_basic() {
        let op = Operator::new(services::Memory::default()).unwrap().finish();
        let path = "data/test.txt";
        let mut writer = AsyncWriter::new(op.writer(path).await.unwrap());
        let bytes = Bytes::from_static(b"hello, world!");
        writer.write(bytes).await.unwrap();
        let bytes = Bytes::from_static(b"hello, OpenDAL!");
        writer.write(bytes).await.unwrap();
        writer.complete().await.unwrap();

        let bytes = op.read(path).await.unwrap().to_vec();
        assert_eq!(bytes, b"hello, world!hello, OpenDAL!");
    }

    #[tokio::test]
    async fn test_abort() {
        let op = Operator::new(services::Memory::default()).unwrap().finish();
        let path = "data/test.txt";
        let mut writer = AsyncWriter::new(op.writer(path).await.unwrap());
        let bytes = Bytes::from_static(b"hello, world!");
        writer.write(bytes).await.unwrap();
        let bytes = Bytes::from_static(b"hello, OpenDAL!");
        writer.write(bytes).await.unwrap();
        drop(writer);

        let exist = op.exists(path).await.unwrap();
        assert!(!exist);
    }

    #[tokio::test]
    async fn test_async_writer() {
        let operator = Operator::new(services::Memory::default()).unwrap().finish();
        let path = "/path/to/file.parquet";

        let writer = AsyncWriter::new(
            operator
                .writer_with(path)
                .chunk(32 * 1024 * 1024)
                .concurrent(8)
                .await
                .unwrap(),
        );

        let col = Arc::new(Int64Array::from_iter_values([1, 2, 3])) as ArrayRef;
        let to_write = RecordBatch::try_from_iter([("col", col)]).unwrap();
        let mut writer = AsyncArrowWriter::try_new(writer, to_write.schema(), None).unwrap();
        writer.write(&to_write).await.unwrap();
        writer.close().await.unwrap();

        let buffer = operator.read(path).await.unwrap().to_bytes();
        let mut reader = ParquetRecordBatchReaderBuilder::try_new(buffer)
            .unwrap()
            .build()
            .unwrap();
        let read = reader.next().unwrap().unwrap();
        assert_eq!(to_write, read);
    }
}
