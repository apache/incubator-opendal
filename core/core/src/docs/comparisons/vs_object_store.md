# OpenDAL vs object_store

> NOTE: This document is written by OpenDAL's maintainers and not reviewed by
> object_store's maintainers. So it could not be very objective.

## About object_store

[object_store](https://crates.io/crates/object_store) is

> A focused, easy to use, idiomatic, high performance, `async` object store library interacting with object stores.

It was initially developed for [InfluxDB IOx](https://github.com/influxdata/influxdb_iox/) and later split out and donated to [Apache Arrow](https://arrow.apache.org/).

## Similarities

### Language

Yes, of course. Both `opendal` and `object_store` are developed in [Rust](https://www.rust-lang.org/), a language empowering everyone to build reliable and efficient software.

### License

Both `opendal` and `object_store` are licensed under [Apache 2.0](https://www.apache.org/licenses/LICENSE-2.0).

### Owner

`object_store` is a part of `Apache Arrow` which means it's hosted and maintained by the [Apache Software Foundation](https://www.apache.org/).

`opendal` is now hosted by the [Apache Software Foundation](https://www.apache.org/) also.

### Domain

Both `opendal` and `object_store` can be used to access data stored on object storage services. The primary users of those projects are both cloud-native databases too:

- `opendal` is mainly used by:
  - [databend](https://github.com/datafuselabs/databend): A modern Elasticity and Performance cloud data warehouse
  - [GreptimeDB](https://github.com/GreptimeTeam/greptimedb): An open-source, cloud-native, distributed time-series database.
  - [mozilla/sccache](https://github.com/mozilla/sccache/): sccache is ccache with cloud storage
  - [risingwave](https://github.com/risingwavelabs/risingwave): A Distributed SQL Database for Stream Processing
  - [Vector](https://github.com/vectordotdev/vector): A high-performance observability data pipeline.
- `object_store` is mainly used by:
  - [datafusion](https://github.com/apache/arrow-datafusion): Apache Arrow DataFusion SQL Query Engine
  - [Influxdb IOx](https://github.com/influxdata/influxdb_iox/): The new core of InfluxDB is written in Rust on top of Apache Arrow.

## Differences

### Vision

`opendal` is Open Data Access Layer that accesses data freely, painlessly, and efficiently. `object_store` is more focused on async object store support.

You will see the different visions lead to very different routes.

### Design

`object_store` exposed a trait called [`ObjectStore`](https://docs.rs/object_store/latest/object_store/trait.ObjectStore.html) to users.

Users need to build a `dyn ObjectStore` and operate on it directly:

```rust
let object_store: Arc<dyn ObjectStore> = Arc::new(get_object_store());
let path: Path = "data/file01.parquet".try_into()?;
let stream = object_store
    .get(&path)
    .await?
    .into_stream();
```

`opendal` has a similar trait called [`Access`][crate::raw::Access]

But `opendal` don't expose this trait to end users directly. Instead, `opendal` expose a new struct called [`Operator`][crate::Operator] and builds public API on it.

```rust
let op: Operator = Operator::from_env(Scheme::S3)?;
let r = op.reader("data/file01.parquet").await?;
```

### Interception

Both `object_store` and `opendal` provide a mechanism to intercept operations.

`object_store` called `Adapters`:

```rust
let object_store = ThrottledStore::new(get_object_store(), ThrottleConfig::default())
```

`opendal` called [`Layer`](crate::raw::Layer):

```rust
let op = op.layer(TracingLayer).layer(MetricsLayer);
```

At the time of writing:

object_store (`v0.5.0`) supports:

- ThrottleStore: Rate Throttling
- LimitStore: Concurrent Request Limit

opendal supports:

- ImmutableIndexLayer: immutable in-memory index.
- LoggingLayer: logging.
- MetadataCacheLayer: metadata cache.
- ContentCacheLayer: content data cache.
- MetricsLayer: metrics
- RetryLayer: retry
- SubdirLayer: Allow switch directory without changing original operator.
- TracingLayer: tracing

### Services

`opendal` and `object_store` have different visions, so they have other services support:

| service | opendal         | object_store                            |
|---------|-----------------|-----------------------------------------|
| azblob  | Y               | Y                                       |
| fs      | Y               | Y                                       |
| ftp     | Y               | N                                       |
| gcs     | Y               | Y                                       |
| hdfs    | Y               | Y *(via [datafusion-objectstore-hdfs])* |
| http    | Y *(read only)* | N                                       |
| ipfs    | Y *(read only)* | N                                       |
| ipmfs   | Y               | N                                       |
| memory  | Y               | Y                                       |
| obs     | Y               | N                                       |
| s3      | Y               | Y                                       |

opendal has an idea called [`Capability`][crate::Capability], so it's services may have different capability sets. For example, opendal's `http` and `ipfs` are read only.

### Features

`opendal` and `object_store` have different visions, so they have different feature sets:

| opendal   | object_store         | notes                                        |
|-----------|----------------------|----------------------------------------------|
| metadata  | -                    | get some metadata from underlying storage    |
| create    | put                  | -                                            |
| read      | get                  | -                                            |
| read      | get_range            | -                                            |
| -         | get_ranges           | opendal doesn't support read multiple ranges |
| write     | put                  | -                                            |
| stat      | head                 | -                                            |
| delete    | delete               | -                                            |
| -         | list                 | opendal doesn't support list with prefix     |
| list      | list_with_delimiter  | -                                            |
| -         | copy                 | -                                            |
| -         | copy_if_not_exists   | -                                            |
| rename    | rename               | -                                            |
| rename_with(if_not_exists) | rename_if_not_exists | -                            |
| presign   | -                    | get a presign URL of object                  |
| multipart | multipart            | both support, but API is different           |
| blocking  | -                    | opendal supports blocking API                |

## Demo show

The most straightforward complete demo how to read a file from s3:

`opendal`

```rust
let mut builder = S3::default();
builder.bucket("example");
builder.access_key_id("access_key_id");
builder.secret_access_key("secret_access_key");

let store = Operator::new(builder)?;
let r = store.reader("data.parquet").await?;
```

`object_store`

```rust
let mut builder = AmazonS3Builder::new()
.with_bucket_name("example")
.with_access_key_id("access_key_id")
.with_secret_access_key("secret_access_key");

let store = Arc::new(builder.build()?);
let path: Path = "data.parquet".try_into().unwrap();
let stream = store.get(&path).await()?.into_stream();
```

[datafusion-objectstore-hdfs]: https://github.com/datafusion-contrib/datafusion-objectstore-hdfs/
