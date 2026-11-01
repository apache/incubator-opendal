- Proposal Name: `object_writer`
- Start Date: 2023-02-27
- RFC PR: [apache/opendal#1420](https://github.com/apache/opendal/pull/1420)
- Tracking Issue: [apache/opendal#1421](https://github.com/apache/opendal/issues/1421)

# Summary

Adding `ObjectWriter` to improve support for multipart uploads, as well as enable retry options for write operations.

# Motivation

OpenDAL works well for `read` operations:

- OpenDAL can seek over content even on services like S3.
- OpenDAL can retry read from the failing point without extra read cost.

However, OpenDAL is not good at `write`:

## Complex multipart operations

OpenDAL supports multipart operations but it's very hard to use:

```ignore
let object_multipart = o.create_multipart().await?;
let part_0 = object_multipart.write(0, content_0).await?;
...
let part_x = object_multipart.write(x, content_x).await?;
let new_object = object_multipart.complete(vec![part_0,...,part_x]).await?;
```

Users should possess the knowledge of the multipart API to effectively use it.

To exacerbate the situation, the multipart API is not standardized and only some object storage services offer support for it. Unfortunately, we cannot even provide support for it on the local file system.

## Lack of retry support

OpenDAL can't retry `write` operations because we accept an `Box<dyn AsyncRead>`. Once we pass this read into other functions, we consumed it.

```rust
async fn write(&self, path: &str, args: OpWrite, r: input::Reader) -> Result<RpWrite> {
    self.inner.write(path, args, r).await
}
```

By introducing the `ObjectWriter` feature, we anticipate resolving all the associated inquiries simultaneously.

# Guide-level explanation

`ObjectWriter` will provide the following APIs:

```rust
impl ObjectWriter {
    pub async write(&mut self, bs: Bytes) -> Result<()>;
    pub async close(&mut self) -> Result<()>;
}
```

After `ObjectWriter` has been constructed, users can use it as a normal writer:

```rust
let mut w = o.writer().await?;
w.write(bs1).await?;
w.write(bs2).await?;
w.close().await?;
```

`ObjectWriter` also implements `AsyncWrite` trait which will allow users to use `io::copy` as well:

```rust
let mut w = o.writer().await?;
let _ = io::copy_buf(r, o).await?;
```

# Reference-level explanation

OpenDAL will add a new trait called `output::Writer`:

```rust
pub trait Write: Unpin + Send + Sync {
    pub async write(&mut self, bs: Bytes) -> Result<()>;

    pub async initiate(&mut self) -> Result<()>;
    pub async append(&mut self, bs: Bytes) -> Result<()>;

    pub async close(&mut self) -> Result<()>;
}
```

- `write` is used to write full content.
- `initiate` is used to initiate a multipart writer.
- `append` is used to append more content into this writer.
- `close` is used to close and construct the final file.

And `Accessor` will change the `write` API into:

```diff
pub trait Accessor {
+    type Writer: output::Write;

-    async fn write(&self, path: &str, args: OpWrite, r: input::Reader) -> Result<RpWrite>;
+    async fn write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::Writer)>
}
```

# Drawbacks

More heavy work for service implementers.

# Rationale and alternatives

## Why accept `Bytes`?

OpenDAL's write is similar to `io::Write::write_all` which will always consume the whole input and return errors if something is wrong. By accepting `Bytes`, we can reduce the extra `Clone` between user land to OpenDAL's services/layers.

# Prior art

None.

# Unresolved questions

None.

# Future possibilities

## Vectored Write

We can add `write_vectored` support in the future:

```rust
pub trait Write: Unpin + Send + Sync {
    pub async write_vectored(&mut self, bs: &[Bytes]) -> Result<()>;
}
```

Take `s3` services as an example, we can upload different parts at the same time.

## Write From Stream

We can add `write_from` support in the future:

```rust
pub trait Write: Unpin + Send + Sync {
    pub async write_from(&mut self, r: BytesStream, size: u64) -> Result<()>;
}
```

By implementing this feature, users don't need to hold a large buffer inside memory.
