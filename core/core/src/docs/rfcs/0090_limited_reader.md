- Proposal Name: `limited_reader`
- Start Date: 2022-03-02
- RFC PR: [apache/opendal#0090](https://github.com/apache/opendal/pull/0090)
- Tracking Issue: [apache/opendal#0090](https://github.com/apache/opendal/issues/0090)

# Summary

Native support for the limited reader.

# Motivation

In proposal [object-native-api](./0041-object-native-api.md) we introduced `Reader`, in which we will send request like:

```rust
let op = OpRead {
    path: self.path.to_string(),
    offset: Some(self.current_offset()),
    size: None,
};
```

In this implementation, we depend on the HTTP client to drop the request when we stop reading. However, we always read too much extra data, which decreases our reading performance.

Here is a benchmark around reading the whole file and only reading half:

```txt
s3/read/1c741003-40ef-43a9-b23f-b6a32ed7c4c6
                        time:   [7.2697 ms 7.3521 ms 7.4378 ms]
                        thrpt:  [2.1008 GiB/s 2.1252 GiB/s 2.1493 GiB/s]
s3/read_half/1c741003-40ef-43a9-b23f-b6a32ed7c4c6
                        time:   [7.0645 ms 7.1524 ms 7.2473 ms]
                        thrpt:  [1.0780 GiB/s 1.0923 GiB/s 1.1059 GiB/s]
```

So our current behavior is buggy, and we need more clear API to address that.

# Guide-level explanation

We will remove `Reader::total_size()` from public API instead of adding the following APIs for `Object`:

```rust
pub fn reader(&self) -> Reader {}
pub fn range_reader(&self, offset: u64, size: u64) -> Reader {}
pub fn offset_reader(&self, offset: u64) -> Reader {}
pub fn limited_reader(&self, size: u64) -> Reader {}
```

- `reader`: returns a new reader who can read the whole file.
- `range_reader`: returns a ranged reader which read `[offset, offset+size)`.
- `offset_reader`: returns a reader from offset `[offset:]`
- `limited_reader`: returns a limited reader `[:size]`

Take `parquet`'s actual logic as an example. We can rewrite:

```rust
async fn _read_single_column_async<'b, R, F>(
    factory: F,
    meta: &ColumnChunkMetaData,
) -> Result<(&ColumnChunkMetaData, Vec<u8>)>
where
    R: AsyncRead + AsyncSeek + Send + Unpin,
    F: Fn() -> BoxFuture<'b, std::io::Result<R>>,
{
    let mut reader = factory().await?;
    let (start, len) = meta.byte_range();
    reader.seek(std::io::SeekFrom::Start(start)).await?;
    let mut chunk = vec![0; len as usize];
    reader.read_exact(&mut chunk).await?;
    Result::Ok((meta, chunk))
}
```

into

```rust
async fn _read_single_column_async<'b, R, F>(
    factory: F,
    meta: &ColumnChunkMetaData,
) -> Result<(&ColumnChunkMetaData, Vec<u8>)>
where
    R: AsyncRead + AsyncSeek + Send + Unpin,
    F: Fn(usize, usize) -> BoxFuture<'b, std::io::Result<R>>,
{
    let (start, len) = meta.byte_range();
    let mut reader = factory(start, len).await?;
    let mut chunk = vec![0; len as usize];
    reader.read_exact(&mut chunk).await?;
    Result::Ok((meta, chunk))
}
```

So that:

- No extra data will be read.
- No extra `seek`/`stat` operation is needed.

# Reference-level explanation

Inside `Reader`, we will correctly maintain `offset`, `size`, and `pos`.

- If `offset` is `None`, we will use `0` instead.
- If `size` is `None`, we will use `meta.content_length() - self.offset.unwrap_or_default()` instead.

We will calculate `Reader` current offset and size easily:

```rust
fn current_offset(&self) -> u64 {
    self.offset.unwrap_or_default() + self.pos
}

fn current_size(&self) -> Option<u64> {
    self.size.map(|v| v - self.pos)
}
```

Instead of constantly requesting the entire object content, we will set the size:

```rust
let op = OpRead {
    path: self.path.to_string(),
    offset: Some(self.current_offset()),
    size: self.current_size(),
};
```

After this change, we will have a similar throughput for `read_all` and `read_half`:

```txt
s3/read/6dd40f8d-7455-451e-b510-3b7ac23e0468
                        time:   [4.9554 ms 5.0888 ms 5.2282 ms]
                        thrpt:  [2.9886 GiB/s 3.0704 GiB/s 3.1532 GiB/s]
s3/read_half/6dd40f8d-7455-451e-b510-3b7ac23e0468
                        time:   [3.1868 ms 3.2494 ms 3.3052 ms]
                        thrpt:  [2.3637 GiB/s 2.4043 GiB/s 2.4515 GiB/s]
```

# Drawbacks

None

# Rationale and alternatives

None

# Prior art

None

# Unresolved questions

None

# Future possibilities

- Refactor the parquet reading logic to make the most use of `range_reader`.
