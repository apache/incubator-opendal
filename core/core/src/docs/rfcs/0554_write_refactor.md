- Proposal Name: `write_refactor`
- Start Date: 2022-08-22
- RFC PR: [apache/opendal#554](https://github.com/apache/opendal/pull/554)
- Tracking Issue: [apache/opendal#555](https://github.com/apache/opendal/issues/555)

# Summary

Refactor `write` operation to accept a `BytesReader` instead.

# Motivation

To simulate the similar operation like POSIX fs, OpenDAL returns `BytesWriter` for users to write, flush and close:

```rust
pub trait Accessor {
    async fn write(&self, args: &OpWrite) -> Result<BytesWriter> {}
}
```

`Operator` builds the high level APIs upon this:

```rust
impl Object {
    pub async fn write(&self, bs: impl AsRef<[u8]>) -> Result<()> {}
    
    pub async fn writer(&self, size: u64) -> Result<impl BytesWrite> {}
}
```

However, we are meeting the following problems:

- Performance: HTTP body channel is mush slower than read from Reader directly.
- Complicity: Service implementer have to deal with APIs like `new_http_channel`.
- Extensibility: Current design can't be extended to multipart APIs.

# Guide-level explanation

Underlying `write` implementations will be replaced by:

```rust
pub trait Accessor {
    async fn write(&self, args: &OpWrite, r: BytesReader) -> Result<u64> {}
}
```

Existing API will have no changes, and we will add a new API:

```rust
impl Object {
    pub async fn write_from(&self, size: u64, r: impl BytesRead) -> Result<u64> {}
}
```

# Reference-level explanation

`Accessor`'s `write` API will be changed to accept a `BytesReader`:

```rust
pub trait Accessor {
    async fn write(&self, args: &OpWrite, r: BytesReader) -> Result<u64> {}
}
```

We will provide `Operator::writer` based on this new API instead.

[RFC-0438: Multipart](./0438-multipart.md) will also be updated to:

```rust
pub trait Accessor {
    async fn write_multipart(&self, args: &OpWriteMultipart, r: BytesReader) -> Result<u64> {}
}
```

In this way, we don't need to introduce a `PartWriter`.

# Drawbacks

## Layer API breakage

This change will introduce break changes to layers.

# Rationale and alternatives

None.

# Prior art

- [RFC-0191: Async Streaming IO](./0191-async-streaming-io.md)

# Unresolved questions

None.

# Future possibilities

None.
