- Proposal Name: `range_based_read`
- Start Date: 2024-03-20
- RFC PR: [apache/opendal#4382](https://github.com/apache/opendal/pull/4382)
- Tracking Issue: [apache/opendal#4383](https://github.com/apache/opendal/issues/4383)

# Summary

Convert `oio::Read` into a stateless, range-based reading pattern.

# Motivation

The current `oio::Read` API is stateful:

```rust
pub trait Read: Unpin + Send + Sync {
    fn read(&mut self, limit: usize) -> impl Future<Output = Result<Bytes>> + Send;
    fn seek(&mut self, pos: io::SeekFrom) -> impl Future<Output = Result<u64>> + Send;
}
```

Users use `read` to retrieve data from storage and can use `seek` to navigate to specific positions. OpenDAL manages the underlying state. This design is good for users from `std::io::Read`, `futures::AsyncRead` and `tokio::io::AsyncRead`.

OpenDAL also provides `range` option at the `Operator` level for users to read a specific range of data. The most common usage will be like:

```rust
let r: Reader = op.reader_with(path).range(1024..2048).await?;
```

However, after observing our users, we found that:

- `AsyncSeek` in `Reader` is prone to misuse.
- `Reader` does not support concurrent reading.
- `Reader` can't adopt Completion-based IO

## Misuse of `AsyncSeek`

When designing `Reader`, I expected users to check the `read_can_seek` capability to determine if the underlying storage services natively support `seek`. However, many users are unaware of this and directly use `seek`, leading to suboptimal performance.

For example, `s3` storage does not support `seek` natively. When users call `seek`, opendal will drop current reader and sending a new request. This behavior is hidden from users and can lead to unexpected performance issues like [What's going on in my parquet stream](https://github.com/apache/opendal/issues/3725).

## Lack of concurrent reading

`oio::Read` complicates supporting concurrent reading. Users must implement a feature similar to merge IO, as discussed in [support merge io read api by settings](https://github.com/apache/opendal/issues/3675).

There is no way for opendal to support this feature.

## Can't adopt Completion-based IO

Completion-based IO requires take the buffer's owner ship. But API that take `&mut [u8]` can't do that.

# Guide-level explanation

So I propose to convert `Reader` into a stateless, range-based reading pattern.

We will remove the following `impl` from `Reader`:

- `futures::AsyncRead`
- `futures::AsyncSeek`
- `futures::Stream`
- `tokio::AsyncRead`
- `tokio::AsyncSeek`

We will add the following new APIs to `Reader`:

```rust
impl Reader {
    /// Read data from the storage at the specified offset.
     pub async fn read(&self, buf: &mut impl BufMut, offset: u64, limit: usize) -> Result<usize>;

    /// Read data from the storage at the specified range.
    pub async fn read_range(
        &self,
        buf: &mut impl BufMut,
        range: impl RangeBounds<u64>,
    ) -> Result<usize>;

    /// Read all data from the storage into given buf.
    pub async fn read_to_end(&self, buf: &mut impl BufMut) -> Result<usize>;

    /// Copy data from the storage into given writer.
    pub async fn copy(&mut self, write_into: &mut impl futures::AsyncWrite) -> Result<u64>;

    /// Sink date from the storage into given sink.
    pub async fn sink<S, T>(&mut self, sink_from: &mut S) -> Result<u64>
    where
        S: futures::Sink<T, Error = Error>,
        T: Into<Bytes>,
}
```

Apart from `Reader`'s own API, we will also provide convert to existing IO APIs like:

```rust
impl Reader {
    /// Convert Reader into `futures::AsyncRead`  
    pub fn into_futures_io_async_read(self, range: Range<u64>) -> FuturesIoAsyncReader;
    
    /// Convert Reader into `futures::Stream`
    pub fn into_futures_bytes_stream(self, range: Range<u64>) -> FuturesBytesStream;
}
```

After this change, users will be able to use `Reader` to read data from storage in a stateless, range-based pattern. Users can also convert `Reader` into `futures::AsyncRead`, `futures::AsyncSeek` and `futures::Stream` as needed.

# Reference-level explanation

The new raw API will be:

```rust
pub trait Read: Unpin + Send + Sync {
    fn read_at(
        &self,
        offset: u64,
        limit: usize,
    ) -> impl Future<Output = Result<oio::Buffer>> + Send;
}
```

The API is similar to [`ReadAt`](https://doc.rust-lang.org/std/fs/struct.File.html#method.read_at), but with following changes:

```diff
- async fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize>
+ async fn read_at(&self, offset: u64, limit: usize) -> Result<oio::Buffer>
```

- opendal chooses to use `oio::Buffer` instead of `&mut [u8]` to avoid lifetime issues.
- opendal chooses to return `oio::Buffer` to let services itself manage the buffer.

For example, http based storage services like `s3` is a stream that generating data on the fly.

# Drawbacks

## Breaking changes to `Reader`

This change will break the existing `Reader` API. Users will need to update their code to use the new `Reader` API.

Users wishing to migrate to the new range-based API will need to update their code. Those who simply want to use `futures::AsyncRead` can instead utilize `Reader::into_futures_read`.
  
# Rationale and alternatives

None.

# Prior art

## `object_store`'s API design  

Current API design inspired from `object_store`'s `ObjectStore` a lot:
    
```rust
#[async_trait]
pub trait ObjectStore: std::fmt::Display + Send + Sync + Debug + 'static {
    /// Return the bytes that are stored at the specified location.
    async fn get(&self, location: &Path) -> Result<GetResult> {
        self.get_opts(location, GetOptions::default()).await
    }

    /// Perform a get request with options
    async fn get_opts(&self, location: &Path, options: GetOptions) -> Result<GetResult>;

    /// Return the bytes that are stored at the specified location
    /// in the given byte range.
    ///
    /// See [`GetRange::Bounded`] for more details on how `range` gets interpreted
    async fn get_range(&self, location: &Path, range: Range<usize>) -> Result<Bytes> {
        let options = GetOptions {
            range: Some(range.into()),
            ..Default::default()
        };
        self.get_opts(location, options).await?.bytes().await
    }
    
    /// Return the bytes that are stored at the specified location
    /// in the given byte ranges
    async fn get_ranges(&self, location: &Path, ranges: &[Range<usize>]) -> Result<Vec<Bytes>> {
        coalesce_ranges(
            ranges,
            |range| self.get_range(location, range),
            OBJECT_STORE_COALESCE_DEFAULT,
        )
        .await
    }
}
```

We can add support that similar to `get_ranges` in the future.
  
OpenDAL opts to return a `Reader` rather than directly implementing `read` to allow for optimization with storage services like `fs` to reduce the extra `open` syscall. 

# Unresolved questions

## Buffer

After switching to range-based reading, we can no longer keep a buffer within the reader. As of writing this proposal, users should use `into_async_buf_read` instead.

# Future possibilities

## Read Ranges

We can implement `read_ranges` support in the future. This will allow users to read multiple ranges of data in less requests.

## Native `read_at` for fs and hdfs

We can reduce unnecessary `open` and `seek` syscalls by using the `read_at` API across different platforms.

## Auto Range Read

We can implement [Auto ranged read support](https://github.com/apache/opendal/issues/1105) like AWS S3 Crt Client. For examples, split the range into multiple ranges and read them concurrently. 

Services can define the preferred io size as default, and users can override it. For example, s3 can use `8 MiB` as preferred io size, while fs can use `4 KiB` instead.

## Completion-based IO

`oio::Read` is designed with Completion-based IO in mind. We can add IOCP/io_uring support in the future.
