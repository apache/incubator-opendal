- Proposal Name: `query_based_metadata`
- Start Date: 2022-02-22
- RFC PR: [apache/opendal#1398](https://github.com/apache/opendal/pull/1398)
- Tracking Issue: [apache/opendal#1398](https://github.com/apache/opendal/pull/1398)

# Summary

Read cached metadata based on users query.

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

RFC [Object Metadataer](./rfc_1391_object_metadataer) intends to add `ObjectMetadataer` to address this issue. But it sooner to be proved that a failure: it's hard to design a correct API.

Users have to write code like the following:

```rust
let om = o.metadata().await?;
let _ = om.content_length().await?;
let _ = om.content_md5().await?;
```

In this RFC, we will add a query based metadata.

# Guide-level explanation

After this RFC, `o.metadata()` will accept a query composed by `ObjectMetadataKey`.

To query already cached metadata:

```rust
let meta = op.object("test").metadata(None).await?;
let _ = meta.content_length();
let _ = meta.content_type();
```

To query content length and content type:

```rust
let meta = op
    .object("test")
    .metadata({
        use ObjectMetadataKey::*;
        ContentLength | ContentType
    })
    .await?;
let _ = meta.content_length();
let _ = meta.content_type();
```

To query all metadata about this object:

```rust
let meta = op
    .object("test")
    .metadata({ ObjectMetadataKey::Complete })
    .await?;
let _ = meta.content_length();
let _ = meta.content_type();
```

# Reference-level explanation

We will store bits in `ObjectMetadata` to store which fields have been set. And we can compare the bits to decide whether we need to query from storage again:

```rust
pub async fn metadata(
    &mut self,
    flags: impl Into<FlagSet<ObjectMetadataKey>>,
) -> Result<Arc<ObjectMetadata>> {
    if let Some(meta) = &self.meta {
        if meta.bit().contains(flags) || meta.bit().contains(ObjectMetadataKey::Complete) {
            return Ok(meta.clone());
        }
    }

    let meta = Arc::new(self.stat().await?);
    self.meta = Some(meta.clone());

    Ok(meta)
}
```

# Drawbacks

## Breaking changes

After this RFC, `Object::metadata()` will accept a query. And all existing users need to adapt their code for that.

# Rationale and alternatives

None

# Prior art

None

# Unresolved questions

None

# Future possibilities

None
