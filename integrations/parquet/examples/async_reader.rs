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

use arrow::array::{ArrayRef, Int64Array, RecordBatch};

use futures::StreamExt;
use opendal::{Operator, services::S3Config};
use parquet::arrow::{AsyncArrowWriter, ParquetRecordBatchStreamBuilder};
use parquet_opendal::{AsyncReader, AsyncWriter};

#[tokio::main]
async fn main() {
    let mut cfg = S3Config::default();
    cfg.access_key_id = Some("my_access_key".to_string());
    cfg.secret_access_key = Some("my_secret_key".to_string());
    cfg.endpoint = Some("my_endpoint".to_string());
    cfg.region = Some("my_region".to_string());
    cfg.bucket = "my_bucket".to_string();

    // Create a new operator
    let operator = Operator::from_config(cfg).unwrap();
    let path = "/path/to/file.parquet";

    // Create an async writer
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

    // gap: Allow the underlying reader to merge small IOs
    // when the gap between multiple IO ranges is less than the threshold.
    let reader = operator
        .reader_with(path)
        .gap(512 * 1024)
        .chunk(16 * 1024 * 1024)
        .concurrent(16)
        .await
        .unwrap();
    let content_len = operator.stat(path).await.unwrap().content_length();
    let reader = AsyncReader::new(reader, content_len).with_prefetch_footer_size(512 * 1024);
    let mut stream = ParquetRecordBatchStreamBuilder::new(reader)
        .await
        .unwrap()
        .build()
        .unwrap();
    let read = stream.next().await.unwrap().unwrap();
    assert_eq!(to_write, read);
}
