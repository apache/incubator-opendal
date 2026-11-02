- Proposal Name: `object_native_api`
- Start Date: 2022-02-18
- RFC PR: [apache/opendal#41](https://github.com/apache/opendal/pull/41)
- Tracking Issue: [apache/opendal#35](https://github.com/apache/opendal/pull/35)

# Summary

Refactor API in object native way to make it easier to user.

# Motivation

`opendal` is not easy to use.

In our early adoption project `databend`, we can see a lot of code looks like:

```rust
let data_accessor = self.data_accessor.clone();
let path = self.path.clone();
let reader = SeekableReader::new(data_accessor, path.as_str(), stream_len);
let reader = BufReader::with_capacity(read_buffer_size as usize, reader);
Self::read_column(reader, &col_meta, data_type.clone(), arrow_type.clone()).await
```

And

```rust
op.stat(&path).run().await
```

## Conclusion

So in this proposal, I expect to address those problems. After implementing this proposal, we have a faster and easier-to-use `opendal`.

# Guide-level explanation

To operate on an object, we will use `Operator::object()` to create a new handler:

```rust
let o = op.object("path/to/file");
```

All operations that are available for `Object` for now includes:

- `metadata`: get object metadata (return an error if not exist).
- `delete`: delete an object.
- `reader`: create a new reader to read data from this object.
- `writer`: create a new writer to write data into this object.

Here is an example:

```rust
use anyhow::Result;
use futures::AsyncReadExt;

use opendal::services::fs;
use opendal::Operator;

#[tokio::main]
async fn main() -> Result<()> {
    let op = Operator::new(fs::Backend::build().root("/tmp").finish().await?);

    let o = op.object("test_file");

    // Write data info file;
    let w = o.writer();
    let n = w
        .write_bytes("Hello, World!".to_string().into_bytes())
        .await?;
    assert_eq!(n, 13);

    // Read data from file;
    let mut r = o.reader();
    let mut buf = vec![];
    let n = r.read_to_end(&mut buf).await?;
    assert_eq!(n, 13);
    assert_eq!(String::from_utf8_lossy(&buf), "Hello, World!");

    // Get file's Metadata
    let meta = o.metadata().await?;
    assert_eq!(meta.content_length(), 13);

    // Delete file.
    o.delete().await?;

    Ok(())
}
```

# Reference-level explanation

## Native Reader support

We will provide a `Reader` (which implement both `AsyncRead + AsyncSeek`) for user instead of just a `AsyncRead`. In this `Reader`, we will:

- Not maintain internal buffer: caller can decide to wrap into `BufReader`.
- Only rely on accessor's `read` and `stat` operations.

To avoid the extra cost for `stat`, we will:

- Allow user specify total_size for `Reader`.
- Lazily Send `stat` while the first time `SeekFrom::End()`

To avoid the extra cost for `poll_read`, we will:

- Keep the underlying `BoxedAsyncRead` open, so that we can reuse the same connection/fd.

With these change, we can improve the `Reader` performance both on local fs and remote storage:

- fs, before

```shell
Benchmarking fs/bench_read/64226295-b7a7-416e-94ce-666ac3ab037b:
                        time:   [16.060 ms 17.109 ms 18.124 ms]
                        thrpt:  [882.82 MiB/s 935.20 MiB/s 996.24 MiB/s]

Benchmarking fs/bench_buf_read/64226295-b7a7-416e-94ce-666ac3ab037b:
                        time:   [14.779 ms 14.857 ms 14.938 ms]
                        thrpt:  [1.0460 GiB/s 1.0517 GiB/s 1.0572 GiB/s]
```

- fs, after

```shell
Benchmarking fs/bench_read/df531bc7-54c8-43b6-b412-e4f7b9589876:
                        time:   [14.654 ms 15.452 ms 16.273 ms]
                        thrpt:  [983.20 MiB/s 1.0112 GiB/s 1.0663 GiB/s]

Benchmarking fs/bench_buf_read/df531bc7-54c8-43b6-b412-e4f7b9589876:
                        time:   [5.5589 ms 5.5825 ms 5.6076 ms]
                        thrpt:  [2.7864 GiB/s 2.7989 GiB/s 2.8108 GiB/s]
```

- s3, before

```shell
Benchmarking s3/bench_read/72025a81-a4b6-46dc-b485-8d875d23c3a5:
                        time:   [4.8315 ms 4.9331 ms 5.0403 ms]
                        thrpt:  [3.1000 GiB/s 3.1674 GiB/s 3.2340 GiB/s]

Benchmarking s3/bench_buf_read/72025a81-a4b6-46dc-b485-8d875d23c3a5:
                        time:   [16.246 ms 16.539 ms 16.833 ms]
                        thrpt:  [950.52 MiB/s 967.39 MiB/s 984.84 MiB/s]
```

- s3, after

```shell
Benchmarking s3/bench_read/6971c464-15f7-48d6-b69c-c8abc7774802:
                        time:   [4.4222 ms 4.5685 ms 4.7181 ms]
                        thrpt:  [3.3117 GiB/s 3.4202 GiB/s 3.5333 GiB/s]

Benchmarking s3/bench_buf_read/6971c464-15f7-48d6-b69c-c8abc7774802:
                        time:   [5.5598 ms 5.7174 ms 5.8691 ms]
                        thrpt:  [2.6622 GiB/s 2.7329 GiB/s 2.8103 GiB/s]
```

## Object API

Other changes are just a re-order of APIs.

- `Operator::read() -> BoxedAsyncRead` => `Object::reader() -> Reader`
- `Operator::write(r: BoxedAsyncRead, size: u64)` => `Object::writer() -> Writer`
- `Operator::stat() -> Object` => `Object::stat() -> Metadata`
- `Operator::delete()` => `Object::delete()`

# Drawbacks

None.

# Rationale and alternatives

None

# Prior art

None

# Unresolved questions

None

# Future possibilities

- Implement `AsyncWrite` for `Writer` so that we can use `Writer` easier.
- Implement `Operator::objects()` to return an object iterator.
