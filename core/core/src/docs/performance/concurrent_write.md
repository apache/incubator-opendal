# Concurrent Writes

OpenDAL writes parts sequentially by default. This keeps memory and request
pressure low, but one in-flight request often cannot fill the available
bandwidth on a high-latency path.

```rust
# use opendal_core::{Buffer, Operator, Result};
# async fn write_sequentially(
#     op: &Operator,
#     first: Buffer,
#     second: Buffer,
# ) -> Result<()> {
let mut writer = op.writer("large-object.bin").await?;
writer.write(first).await?;
writer.write(second).await?;
writer.close().await?;
# Ok(())
# }
```

Set `concurrent` above `1` to let services with multipart or block-based write
support upload independent parts in parallel:

```rust
# use opendal_core::{Buffer, Operator, Result};
# async fn write_concurrently(
#     op: &Operator,
#     first: Buffer,
#     second: Buffer,
# ) -> Result<()> {
let mut writer = op
    .writer_with("large-object.bin")
    .concurrent(8)
    .await?;

writer.write(first).await?;
writer.write(second).await?;
writer.close().await?;
# Ok(())
# }
```

For multipart and block writers, OpenDAL schedules part uploads through its
[`Executor`][crate::Executor] and tracks their results in a task queue. A write
call can return while earlier parts are still in flight when the queue has
capacity. [`Writer::close`][crate::Writer::close] drains the queue and performs
the final commit, so applications must always check the result from `close`.

The storage service determines the native mechanism. S3 uses multipart upload,
while Azure Blob uses block upload. Services without a concurrent write path
continue to execute sequentially even when the option is present. Check
[`Capability::write_can_multi`][crate::Capability::write_can_multi] before
tuning multipart or block writes.

## Tuning

Concurrent writes have two controls:

- `concurrent` is the maximum number of part uploads OpenDAL attempts to keep
  in flight. The default is `1`, and values below `1` are normalized to `1`.
- `chunk` is the target size of each part. Its default and valid range depend on
  the service.

### `concurrent`

`concurrent` is an upper bound, not a guaranteed level. Setting it to `8` does
not create eight active requests unless the input arrives quickly enough and
the object contains enough parts. Object size, request latency, and service
support can all reduce the observed concurrency.

Start with `2` or `4`, then try `8` while measuring the real workload. Higher
concurrency can improve bandwidth utilization, but it also increases in-flight
requests, buffered data, and pressure on service request limits. Once the
network or service is saturated, increasing it further can reduce throughput by
causing throttling and retries.

### `chunk`

OpenDAL buffers small writes until it can form a part of the configured size.
This avoids issuing one storage request for every small input buffer. The final
part may be smaller.

Larger chunks reduce request count and per-request overhead, but use more memory
per in-flight part and require more data to be retried after a failed upload.
Smaller chunks begin parallel work sooner, but increase request rate and may be
invalid for the service. For example, S3 multipart upload has a 5 MiB minimum
part size; OpenDAL raises smaller configured values to the service minimum.

Inspect [`Capability::write_multi_min_size`][crate::Capability::write_multi_min_size]
and [`Capability::write_multi_max_size`][crate::Capability::write_multi_max_size]
before overriding the service default. For large object-storage uploads, 8 MiB
is a useful starting point, followed by measurements at the expected object
sizes and concurrency.

## One-shot and adapter APIs

`write_with` can split one in-memory buffer into concurrent parts:

```rust
# use opendal_core::{Buffer, Operator, Result};
# async fn upload(op: &Operator, data: Buffer) -> Result<()> {
let _metadata = op
    .write_with("large-object.bin", data)
    .chunk(8 * 1024 * 1024)
    .concurrent(4)
    .await?;
# Ok(())
# }
```

A configured writer keeps the same chunking and concurrency behavior after it
is converted with [`Writer::into_sink`][crate::Writer::into_sink],
[`Writer::into_bytes_sink`][crate::Writer::into_bytes_sink], or
[`Writer::into_futures_async_write`][crate::Writer::into_futures_async_write].

```rust
use std::io;

use futures::SinkExt;
use opendal_core::{Buffer, Operator};

async fn upload_with_sink(op: Operator, data: Buffer) -> io::Result<()> {
    let mut sink = op
        .writer_with("large-object.bin")
        .chunk(8 * 1024 * 1024)
        .concurrent(4)
        .await?
        .into_sink();

    sink.send(data).await?;
    sink.close().await?;
    Ok(())
}
```

Measure end-to-end throughput and completion latency together with peak memory,
request count, retries, throttling, and the result from the final close.
