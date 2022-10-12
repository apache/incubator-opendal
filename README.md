# OpenDAL &emsp; [![Build Status]][actions] [![Latest Version]][crates.io] [![Crate Downloads]][crates.io]

[Build Status]: https://img.shields.io/github/workflow/status/datafuselabs/opendal/CI/main
[actions]: https://github.com/datafuselabs/opendal/actions?query=branch%3Amain
[Latest Version]: https://img.shields.io/crates/v/opendal.svg
[crates.io]: https://crates.io/crates/opendal
[Crate Downloads]: https://img.shields.io/crates/d/opendal.svg

**Open** **D**ata **A**ccess **L**ayer: Access data freely, painlessly, and efficiently

---

You may be looking for:

- [Documentation](https://opendal.databend.rs)
- [API Reference](https://opendal.databend.rs/opendal/)
- [Release notes](https://github.com/datafuselabs/opendal/releases)

## Services

- [azblob](https://opendal.databend.rs/opendal/services/azblob/index.html): [Azure Storage Blob](https://azure.microsoft.com/en-us/services/storage/blobs/) services.
- [fs](https://opendal.databend.rs/opendal/services/fs/index.html): POSIX alike file system.
- [ftp](https://opendal.databend.rs/opendal/services/ftp/index.html): FTP and FTPS support.
- [gcs](https://opendal.databend.rs/opendal/services/gcs/index.html): [Google Cloud Storage](https://cloud.google.com/storage) Service.
- [hdfs](https://opendal.databend.rs/opendal/services/hdfs/index.html): [Hadoop Distributed File System](https://hadoop.apache.org/docs/r3.3.4/hadoop-project-dist/hadoop-hdfs/HdfsDesign.html)(HDFS).
- [http](https://opendal.databend.rs/opendal/services/http/index.html): HTTP read-only services.
- [ipfs](https://opendal.databend.rs/opendal/services/ipfs/index.html): [InterPlanetary File System](https://ipfs.tech/) HTTP Gateway support.
- [ipmfs](https://opendal.databend.rs/opendal/services/ipmfs/index.html): [InterPlanetary File System](https://ipfs.tech/) MFS API support.
- [memory](https://opendal.databend.rs/opendal/services/memory/index.html): In memory backend.
- [moka](https://opendal.databend.rs/opendal/services/moka/index.html): [moka](https://github.com/moka-rs/moka) backend support.
- [obs](https://opendal.databend.rs/opendal/services/obs/index.html): [Huawei Cloud Object Storage](https://www.huaweicloud.com/intl/en-us/product/obs.html) Service (OBS).
- [oss](https://opendal.databend.rs/opendal/services/oss/index.html): [Aliyun Object Storage Service](https://www.aliyun.com/product/oss) (OSS).
- [redis](https://opendal.databend.rs/opendal/services/redis/index.html): [Redis](https://redis.io/) services support.
- [s3](https://opendal.databend.rs/opendal/services/s3/index.html): [AWS S3](https://aws.amazon.com/s3/) alike services.

## Features

Access data **freely**

- Access different storage services in the same way
- Behavior tests for all services

Access data **painlessly**

- **100%** documents covered
- Powerful [`Layers`](https://opendal.databend.rs/opendal/layers/index.html)
- Automatic [retry](https://opendal.databend.rs/opendal/layers/struct.RetryLayer.html) support
- Full observability support: [logging](https://opendal.databend.rs/opendal/layers/struct.LoggingLayer.html), [tracing](https://opendal.databend.rs/opendal/layers/struct.TracingLayer.html), [metrics](https://opendal.databend.rs/opendal/layers/struct.MetricsLayer.html).
- Native decompress support
- Native service-side encryption support

Access data **efficiently**

- Zero cost: mapping to underlying API calls directly
- [Auto metadata reuse](https://opendal.databend.rs/rfcs/0561-list-metadata-reuse.html): avoid extra `metadata` calls

## Quickstart

```rust
use anyhow::Result;
use futures::StreamExt;
use futures::TryStreamExt;
use opendal::ObjectEntry;
use opendal::ObjectStreamer;
use opendal::Object;
use opendal::ObjectMetadata;
use opendal::ObjectMode;
use opendal::Operator;
use opendal::Scheme;

#[tokio::main]
async fn main() -> Result<()> {
    // Init Operator
    let op = Operator::from_env(Scheme::Fs)?;

    // Create object handler.
    let o = op.object("test_file");

    // Write data info object;
    o.write("Hello, World!").await?;

    // Read data from object;
    let bs = o.read().await?;

    // Read range from object;
    let bs = o.range_read(1..=11).await?;

    // Get object's path
    let name = o.name();
    let path = o.path();

    // Fetch more meta about object.
    let meta = o.metadata().await?;
    let mode = meta.mode();
    let length = meta.content_length();
    let content_md5 = meta.content_md5();
    let etag = meta.etag();

    // Delete object.
    o.delete().await?;

    // List dir object.
    let o = op.object("test_dir/");
    let mut os = o.list().await?;
    while let Some(entry) = os.try_next().await? {
        let path = entry.path();
        let mode = entry.mode();
    }

    Ok(())
}
```

More examples could be found at [Documentation](https://opendal.databend.rs).

## Projects

- [databend](https://github.com/datafuselabs/databend/): A modern Elasticity and Performance cloud data warehouse.

## Contributing

Check out the [CONTRIBUTING.md](./CONTRIBUTING.md) guide for more details on getting started with contributing to this project.

## Getting help

Submit [issues](https://github.com/datafuselabs/opendal/issues/new/choose) for bug report or asking questions in [discussion](https://github.com/datafuselabs/opendal/discussions/new?category=q-a).

#### License

<sup>
Licensed under <a href="./LICENSE">Apache License, Version 2.0</a>.
</sup>
