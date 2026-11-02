# Apache OpenDAL™ parquet integration

[![Build Status]][actions] [![Latest Version]][crates.io] [![Crate Downloads]][crates.io] [![chat]][discord]

[build status]: https://img.shields.io/github/actions/workflow/status/apache/opendal/ci_integration_parquet.yml?branch=main
[actions]: https://github.com/apache/opendal/actions?query=branch%3Amain
[latest version]: https://img.shields.io/crates/v/parquet_opendal.svg
[crates.io]: https://crates.io/crates/parquet_opendal
[crate downloads]: https://img.shields.io/crates/d/parquet_opendal.svg
[chat]: https://img.shields.io/discord/1081052318650339399
[discord]: https://opendal.apache.org/discord

`parquet_opendal` provides [`parquet`](https://crates.io/crates/parquet) efficient IO utilities.
    
## Useful Links

- Documentation: [release](https://docs.rs/parquet_opendal/) | [dev](https://opendal.apache.org/docs/object-store-opendal/parquet_opendal/)

## Examples

Add the following dependencies to your `Cargo.toml` with correct version:

```toml
[dependencies]
parquet_opendal = "0.0.1"
opendal = { version = "0.48.0", features = ["services-s3"] }
```


```rust
use std::sync::Arc;

use arrow::array::{ArrayRef, Int64Array, RecordBatch};

use futures::StreamExt;
use opendal::{services::S3Config, Operator};
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

    /// `gap(512 * 1024)` - Sets the maximum gap size (in bytes) to merge small byte ranges
    ///   to 512 KB.
    /// `chunk(16 * 1024 * 1024)` - Sets the chunk size (in bytes) for reading data to 16 MB.
    /// `concurrent(16)` - Sets the number of concurrent fetch operations to 16.
    let reader = operator
        .reader_with(path)
        .gap(512 * 1024)
        .chunk(16 * 1024 * 1024)
        .concurrent(16)
        .await
        .unwrap();

    let content_len = operator.stat(path).await.unwrap().content_length();
    // `with_prefetch_footer_size(512 * 1024)` - Sets the prefetch footer size to 512 KB.
    let reader = AsyncReader::new(reader, content_len).with_prefetch_footer_size(512 * 1024);
    let mut stream = ParquetRecordBatchStreamBuilder::new(reader)
        .await
        .unwrap()
        .build()
        .unwrap();
    let read = stream.next().await.unwrap().unwrap();
    assert_eq!(to_write, read);
}
```

## Branding

The first and most prominent mentions must use the full form: **Apache OpenDAL™** of the name for any individual usage (webpage, handout, slides, etc.) Depending on the context and writing style, you should use the full form of the name sufficiently often to ensure that readers clearly understand the association of both the OpenDAL project and the OpenDAL software product to the ASF as the parent organization.

For more details, see the [Apache Product Name Usage Guide](https://www.apache.org/foundation/marks/guide).

## License and Trademarks

Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0

Apache OpenDAL, OpenDAL, and Apache are either registered trademarks or trademarks of the Apache Software Foundation.
