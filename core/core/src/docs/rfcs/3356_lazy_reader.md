- Proposal Name: `lazy_reader`
- Start Date: 2023-10-22
- RFC PR: [apache/opendal#3356](https://github.com/apache/opendal/pull/3356)
- Tracking Issue: [apache/opendal#3359](https://github.com/apache/opendal/issues/3359)

# Summary

Doing read IO in a lazy way.

# Motivation

The aim is to minimize IO cost. OpenDAL sends an actual IO request to the storage when `Accessor::read()` is invoked. For storage services such as S3, this equates to an IO request. However, in practical scenarios, users typically create a reader and use `seek` to navigate to the correct position.

Take [parquet2 read_metadata](https://docs.rs/parquet2/latest/src/parquet2/read/metadata.rs.html) as an example:

```rust
/// Reads a [`FileMetaData`] from the reader, located at the end of the file.
pub fn read_metadata<R: Read + Seek>(reader: &mut R) -> Result<FileMetaData> {
    // check file is large enough to hold footer
    let file_size = stream_len(reader)?;
    if file_size < HEADER_SIZE + FOOTER_SIZE {
        return Err(Error::oos(
            "A parquet file must contain a header and footer with at least 12 bytes",
        ));
    }

    // read and cache up to DEFAULT_FOOTER_READ_SIZE bytes from the end and process the footer
    let default_end_len = min(DEFAULT_FOOTER_READ_SIZE, file_size) as usize;
    reader.seek(SeekFrom::End(-(default_end_len as i64)))?;

    ...

    deserialize_metadata(reader, max_size)
}
```

In `read_metadata`, we initiate a seek as soon as the reader is invoked. This action, when performed on non-seekable storage services such as s3, results in an immediate HTTP request and cancellation. By postponing the IO request until the first `read` call, we can significantly reduce the number of IO requests.

The expense of initiating and immediately aborting an HTTP request is significant. Here are the benchmark results, using a stat call as our baseline:

On minio server that setup locally:

```rust
service_s3_read_stat/4.00 MiB
                        time:   [315.23 µs 328.23 µs 341.42 µs]
                        
service_s3_read_abort/4.00 MiB
                        time:   [961.69 µs 980.68 µs 999.50 µs]
```

On remote storage services with high latency:

```rust
service_s3_read_stat/4.00 MiB
                        time:   [407.85 ms 409.61 ms 411.39 ms]

service_s3_read_abort/4.00 MiB
                        time:   [1.5282 s 1.5554 s 1.5828 s]

```

# Guide-level explanation

There have been no changes to the API. The only modification is that the IO request has been deferred until the first `read` call, meaning no errors will be returned when calling `op.reader()`. For instance, users won't encounter a `file not found` error when invoking `op.reader()`.

# Reference-level explanation

Most changes will happen inside `CompleteLayer`. In the past, we will call `Accessor::read()` directly in `complete_reader`:

```rust
async fn complete_reader(
    &self,
    path: &str,
    args: OpRead,
) -> Result<(RpRead, CompleteReader<A, A::Reader>)> {
    ..

    let seekable = capability.read_can_seek;
    let streamable = capability.read_can_next;

    let range = args.range();
    let (rp, r) = self.inner.read(path, args).await?;
    let content_length = rp.metadata().content_length();
    
    ...
}
```

In the future, we will postpone the `Accessor::read()` request until the first `read` call.

# Drawbacks

None

# Rationale and alternatives

None

# Prior art

None

# Unresolved questions

None

# Future possibilities

## Add `read_at` for `oio::Reader` 

After `oio::Reader` becomes zero cost, we can add `read_at` to `oio::Reader` to support read data by range.
