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

use std::cmp::{max, min};
use std::ops::Range;
use std::sync::Arc;

use futures::FutureExt;
use futures::future::BoxFuture;
use opendal::Reader;
use parquet::arrow::arrow_reader::ArrowReaderOptions;
use parquet::arrow::async_reader::AsyncFileReader;
use parquet::errors::{ParquetError, Result as ParquetResult};
use parquet::file::FOOTER_SIZE;
use parquet::file::metadata::ParquetMetaData;
use parquet::file::metadata::ParquetMetaDataReader;

const PREFETCH_FOOTER_SIZE: usize = 512 * 1024;

/// AsyncReader implements AsyncFileReader trait by using opendal.
///
/// ```no_run
/// use std::sync::Arc;
/// use arrow::array::{ArrayRef, Int64Array, RecordBatch};
///
/// use futures::StreamExt;
/// use opendal::{services::S3Config, Operator};
/// use parquet::arrow::{AsyncArrowWriter, ParquetRecordBatchStreamBuilder};
/// use parquet_opendal::{AsyncReader, AsyncWriter};
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
///     let operator = Operator::from_config(cfg).unwrap();
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
///     // gap: Allow the underlying reader to merge small IOs
///     // when the gap between multiple IO ranges is less than the threshold.
///     let reader = operator
///         .reader_with(path)
///         .gap(512 * 1024)
///         .chunk(16 * 1024 * 1024)
///         .concurrent(16)
///         .await
///         .unwrap();
///     let content_len = operator.stat(path).await.unwrap().content_length();
///     let reader = AsyncReader::new(reader, content_len).with_prefetch_footer_size(512 * 1024);
///     let mut stream = ParquetRecordBatchStreamBuilder::new(reader)
///         .await
///         .unwrap()
///         .build()
///         .unwrap();
///     let read = stream.next().await.unwrap().unwrap();
///     assert_eq!(to_write, read);
/// }
/// ```
pub struct AsyncReader {
    inner: Reader,
    content_length: u64,
    // The prefetch size for fetching file footer.
    prefetch_footer_size: usize,
}

fn set_prefetch_footer_size(footer_size: usize, content_size: u64) -> usize {
    let footer_size = max(footer_size, FOOTER_SIZE);
    min(footer_size as u64, content_size) as usize
}

impl AsyncReader {
    /// Create a [`AsyncReader`] by given [`Reader`].
    pub fn new(reader: Reader, content_length: u64) -> Self {
        Self {
            inner: reader,
            content_length,
            prefetch_footer_size: set_prefetch_footer_size(PREFETCH_FOOTER_SIZE, content_length),
        }
    }

    /// Set prefetch size for fetching file footer.
    pub fn with_prefetch_footer_size(mut self, footer_size: usize) -> Self {
        self.prefetch_footer_size = set_prefetch_footer_size(footer_size, self.content_length);
        self
    }
}

impl AsyncFileReader for AsyncReader {
    fn get_bytes(&mut self, range: Range<u64>) -> BoxFuture<'_, ParquetResult<bytes::Bytes>> {
        async move {
            Ok(self
                .inner
                .read(range)
                .await
                .map_err(|err| ParquetError::External(Box::new(err)))?
                .to_bytes())
        }
        .boxed()
    }

    fn get_byte_ranges(
        &mut self,
        ranges: Vec<Range<u64>>,
    ) -> BoxFuture<'_, ParquetResult<Vec<bytes::Bytes>>> {
        async move {
            Ok(self
                .inner
                .fetch(ranges)
                .await
                .map_err(|err| ParquetError::External(Box::new(err)))?
                .into_iter()
                .map(|buf| buf.to_bytes())
                .collect::<Vec<_>>())
        }
        .boxed()
    }

    fn get_metadata<'a>(
        &'a mut self,
        _options: Option<&'a ArrowReaderOptions>,
    ) -> BoxFuture<'a, ParquetResult<std::sync::Arc<ParquetMetaData>>> {
        let content_length = self.content_length;
        let prefetch_footer_size = self.prefetch_footer_size;
        async move {
            let reader =
                ParquetMetaDataReader::new().with_prefetch_hint(Some(prefetch_footer_size));
            let meta = reader.load_and_finish(self, content_length).await?;

            Ok(Arc::new(meta))
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use opendal::{Operator, services};
    use rand::{RngExt, distr::Alphanumeric};

    use crate::{AsyncReader, AsyncWriter, async_reader::PREFETCH_FOOTER_SIZE};
    use std::sync::Arc;

    use arrow::array::{ArrayRef, Int64Array, RecordBatch};
    use parquet::{
        arrow::{AsyncArrowWriter, ParquetRecordBatchStreamBuilder},
        file::metadata::KeyValue,
        file::properties::WriterProperties,
    };

    #[tokio::test]
    async fn test_async_reader_with_prefetch_footer_size() {
        let operator = Operator::new(services::Memory::default()).unwrap();
        let path = "/path/to/file.parquet";

        let reader = AsyncReader::new(operator.reader(path).await.unwrap(), 1024);
        assert_eq!(reader.prefetch_footer_size, 1024);
        assert_eq!(reader.content_length, 1024);

        let reader = AsyncReader::new(operator.reader(path).await.unwrap(), 1024 * 1024);
        assert_eq!(reader.prefetch_footer_size, PREFETCH_FOOTER_SIZE);
        assert_eq!(reader.content_length, 1024 * 1024);

        let reader = AsyncReader::new(operator.reader(path).await.unwrap(), 1024 * 1024)
            .with_prefetch_footer_size(2048 * 1024);
        assert_eq!(reader.prefetch_footer_size, 1024 * 1024);
        assert_eq!(reader.content_length, 1024 * 1024);

        let reader = AsyncReader::new(operator.reader(path).await.unwrap(), 1024 * 1024)
            .with_prefetch_footer_size(1);
        assert_eq!(reader.prefetch_footer_size, 8);
        assert_eq!(reader.content_length, 1024 * 1024);
    }

    fn gen_fixed_string(size: usize) -> String {
        rand::rng()
            .sample_iter(&Alphanumeric)
            .take(size)
            .map(char::from)
            .collect()
    }

    #[tokio::test]
    async fn test_async_reader() {
        let operator = Operator::new(services::Memory::default()).unwrap();
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

        let reader = operator.reader(path).await.unwrap();
        let content_len = operator.stat(path).await.unwrap().content_length();
        let reader = AsyncReader::new(reader, content_len);
        let mut stream = ParquetRecordBatchStreamBuilder::new(reader)
            .await
            .unwrap()
            .build()
            .unwrap();
        let read = stream.next().await.unwrap().unwrap();
        assert_eq!(to_write, read);
    }

    struct TestCase {
        metadata_size: usize,
        prefetch: Option<usize>,
    }

    #[tokio::test]
    async fn test_async_reader_with_large_metadata() {
        for case in [
            TestCase {
                metadata_size: 256 * 1024,
                prefetch: None,
            },
            TestCase {
                metadata_size: 1024 * 1024,
                prefetch: None,
            },
            TestCase {
                metadata_size: 256 * 1024,
                prefetch: Some(4),
            },
            TestCase {
                metadata_size: 1024 * 1024,
                prefetch: Some(4),
            },
        ] {
            let operator = Operator::new(services::Memory::default()).unwrap();
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

            let mut writer = AsyncArrowWriter::try_new(
                writer,
                to_write.schema(),
                Some(
                    WriterProperties::builder()
                        .set_key_value_metadata(Some(vec![KeyValue {
                            key: "__metadata".to_string(),
                            value: Some(gen_fixed_string(case.metadata_size)),
                        }]))
                        .build(),
                ),
            )
            .unwrap();
            writer.write(&to_write).await.unwrap();
            writer.close().await.unwrap();

            let reader = operator.reader(path).await.unwrap();
            let content_len = operator.stat(path).await.unwrap().content_length();
            let mut reader = AsyncReader::new(reader, content_len);
            if let Some(footer_size) = case.prefetch {
                reader = reader.with_prefetch_footer_size(footer_size);
            }
            let mut stream = ParquetRecordBatchStreamBuilder::new(reader)
                .await
                .unwrap()
                .build()
                .unwrap();
            let read = stream.next().await.unwrap().unwrap();
            assert_eq!(to_write, read);
        }
    }
}
