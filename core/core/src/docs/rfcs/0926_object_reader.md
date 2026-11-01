- Proposal Name: `object_reader`
- Start Date: 2022-11-13
- RFC PR: [apache/opendal#926](https://github.com/apache/opendal/pull/926)
- Tracking Issue: [apache/opendal#927](https://github.com/apache/opendal/issues/927)

# Summary

Returning reading related object meta in the reader.

# Motivation

Some services like s3 could return object meta while issuing reading requests.

In `GetObject`, we could get:

- Last-Modified
- Content-Length
- ETag
- Content-Range
- Content-Type
- Expires

We can avoid extra `HeadObject` calls by reusing that meta wisely, which could take 50ms. For example, `Content-Range` returns the content range of this read in the whole object: `<unit> <range-start>-<range-end>/<size>`. By using the content range, we can avoid `HeadObject` to get this object's total size, which means a lot for the content cache.

# Guide-level explanation

`reader` and all its related API will return `ObjectReader` instead:

```diff
- pub async fn reader(&self) -> Result<impl BytesRead> {}
+ pub async fn reader(&self) -> Result<ObjectReader> {}
```

`ObjectReader` impls `BytesRead` too, so existing code will keep working. And `ObjectReader` will provide similar APIs to `Entry`, for example:

```rust
pub async fn content_length(&self) -> Option<u64> {}
pub async fn last_modified(&self) -> Option<OffsetDateTime> {}
pub async fn etag(&self) -> Option<String> {}
```

Note:

- All fields are optional, as services like fs could not return them.
- `content_length` here is this read request's length, not the object's length.

# Reference-level explanation

We will change the API signature of `Accessor`:

```diff
- async fn read(&self, path: &str, args: OpRead) -> Result<BytesReader> {}
+ async fn read(&self, path: &str, args: OpRead) -> Result<ObjectReader> {}
```

`ObjectReader` is a wrapper of `BytesReader` and `ObjectMeta`:

```rust
pub struct ObjectReader {
    inner: BytesReader
    meta: ObjectMetadata,
}

impl ObjectReader {
    pub async fn content_length(&self) -> Option<u64> {}
    pub async fn last_modified(&self) -> Option<OffsetDateTime> {}
    pub async fn etag(&self) -> Option<String> {}
}
```

Services can decide whether or not to fill them.

# Drawbacks

None.

# Rationale and alternatives

None.

# Prior art

None.

# Unresolved questions

None.

# Future possibilities

## Add content-range support

We can add `content-range` in `ObjectMeta` so that users can fetch and use them.
