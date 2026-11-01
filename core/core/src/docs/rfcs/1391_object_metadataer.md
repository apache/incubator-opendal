- Proposal Name: `object_metadataer`
- Start Date: 2023-02-21
- RFC PR: [apache/opendal#1391](https://github.com/apache/opendal/pull/1391)
- Tracking Issue: [apache/opendal#1393](https://github.com/apache/opendal/issues/1393)

# Summary

Add object metadataer to avoid unneeded extra metadata call.

# Motivation

OpenDAL has native metadata cache for now:

```rust
let _ = o.metadata().await?;
// This call doesn't need to send a request.
let _ = o.metadata().await?;
```

Also, OpenDAL can reuse metadata from `list` or `scan`:

```rust
let mut ds = o.scan().await?;
while let Some(de) = ds.try_next().await? {
    // This call doesn't need to send a request (if we are lucky enough).
    let _ = de.metadata().await?;
}
```

By reusing metadata from `list` or `scan` we can reduce the extra `stat` call for each object. In our real use cases, we can reduce the total time to calculate the total length inside a dir with 6k files from 4 minutes to 2 seconds.

However, metadata can only be cached as a whole. If services could return more metadata in `stat` than in `list`, we wouldn't be able to mark the metadata as cacheable. If services add more metadata, we could inadvertently introduce the performance degradation.

This RFC aims to address this problem by hiding `ObjectMetadata` and adding `ObjectMetadataer` instead. All object metadata values will be cached separately and all user calls to object metadata will go to the cache.

# Guide-level explanation

This RFC will add `ObjectMetadataer` and `BlockingObjectMetadataer` for users:

Users call to `o.metadata()` will return `ObjectMetadataer` instead:

```rust
let om: ObjectMetadataer = o.metadata().await?;
```

And users can query more metadata over it:

```rust
let content_length = om.content_length().await;
let etag = om.etag().await;
```

During the whole lifetime of the corresponding `Object` or `ObjectMetadataer`, we make sure that at most one `stat` call is sent. After this change, users will never get an `ObjectMetadata` anymore.

# Reference-level explanation

We will introduce a bitmap to store the state of all object metadata fields separately. Everytime users call `key` on metadata, we will check as following:

- If `bitmap` is set, return directly.
- If `bitmap` is not set, but is complete, return directly.
- If both `bitmap` is not set and not `complete`, call `stat` to get the meta.

`Object` will return `ObjectMetadataer` instead of `ObjectMetadata`:

```diff
- pub async fn metadata(&self) -> Result<ObjectMetadata> {}
+ pub async fn metadata(&self) -> Result<ObjectMetadataer> {}
```

And `ObjectMetadataer` will provide the following API:

```rust
impl ObjectMetadataer {
    pub async fn mode(&self) -> Result<ObjectMode>;
    pub async fn content_length(&self) -> Result<u64>;
    pub async fn content_md5(&self) -> Result<Option<String>>;
    pub async fn last_modified(&self) -> Result<Option<OffsetDateTime>>;
    pub async fn etag(&self) -> Result<Option<String>>;
}

impl BlockingObjectMetadataer {
    pub fn mode(&self) -> Result<ObjectMode>
    pub fn content_length(&self) -> Result<u64>;
    pub fn content_md5(&self) -> Result<Option<String>>;
    pub fn last_modified(&self) -> Result<Option<OffsetDateTime>>;
    pub fn etag(&self) -> Result<Option<String>>;
}
```

# Drawbacks

## Breaking changes

This RFC will introduce breaking changes for `Object::metadata`. And users can't do `serde::Serialize` or `serde::Deserialize` on object metadata any more. All metadata related API calls will be removed from `Object`.

# Rationale and alternatives

None.

# Prior art

None.

# Unresolved questions

None.

# Future possibilities

None.
