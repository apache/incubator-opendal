- Proposal Name: `foyer_integration`
- Start Date: 2025-07-07
- RFC PR: [apache/opendal#6370](https://github.com/apache/opendal/pull/6370)
- Tracking Issue: [apache/opendal#6372](https://github.com/apache/opendal/issues/6372)

# Summary

Integrate [*foyer*](https://github.com/foyer-rs/foyer) hybrid support into OpenDAL for performance boost and cost reduction.

# Motivation

Object storage is the most commonly used option by OpenDAL users. In cloud-based Object Storage services like AWS S3 / GCS, the distribution of request latency is often one to several orders of magnitude higher than local disks or memory, and these services are often billed based on the number of requests. 

Applications based on these cloud object storage services often need to introduce caching to optimize storage performance while reducing request overhead. *Foyer* provides a mixed caching capability of memory and disk, offering a better balance between performance and cost, thus becoming a dependency for many systems based on cloud object storage along with OpenDAL. e.g. RisingWave, SlateDB, etc.

However, regardless of which cache component is introduced, users need to operate additional cache-related APIs apart from operating OpenDAL. If *foyer* can be integrated as an optional component into OpenDAL, it can provide users with a more friendly, convenient, and transparent interaction method.

By introducing *foyer* integration, the users will be benefited in to following aspects:

- Performance boost and cost reduction by caching with both memory and disk.
- A completely transparent implementation, using the same operation APIs as before.

[RFC#6297](https://github.com/apache/opendal/pull/6297) has mentioned a general cache layer design, and *foyer* can be integrated into OpenDAL as a general cache in this way. However, this may not fully leverage *foyer*'s capabilities:

- *Foyer* support automatic cache refilling on cache miss. The behavior differs based on the reason of cache miss and the statistics of the entry (e.g. entry not in cache, disk operation throttled, age of entry, etc). All of the abilities are supported by a non-standard API `fetch()`, which other cache libraries don't have.
- *Foyer* support requests deduplication on the same key. *Foyer* ensures that for concurrent access to the same key, only one request will actually access the disk cache or remote storage, while other requests will wait for this request to return and directly reuse the result, in order to minimize overhead as much as possible.

These capabilities overlap with some of the functionalities provided by a general cache Layer, while others are orthogonal. An independent *foyer* integration (e.g. `FoyerLayer`) can fully leverage Foyer's capabilities. At the same time, this will not affect future integration with Foyer and other cache libraries through the general cache layer.

# Guide-level explanation

## 1. Enable feature

```toml
opendal = { version = "*", features = ["layers-foyer"] }
```

## 2. Build foyer instance

```rust
let cache = HybridCacheBuilder::new()
    .memory(10)
    .with_shards(1)
    .storage(Engine::Large(LargeEngineOptions::new()))
    .with_device_options(
        DirectFsDeviceOptions::new(dir.path())
            .with_capacity(16 * MiB as usize)
            .with_file_size(1 * MiB as usize),
    )
    .with_recover_mode(RecoverMode::None)
    .build()
    .await
    .unwrap();
```

## 3. Build OpenDAL operator with foyer layer

```rust
let op = Operator::new(Dashmap::default())
    .unwrap()
    .layer(FoyerLayer::new(cache.clone()))
    .finish();
```

## 4. Perform operations as you used to

```rust
op.write("obj-1").await.unwrap();

assert_eq!(op.list("/").await.unwrap().len(), 1);

op.read("obj-1").await.unwrap();

op.delete("obj-1").await.unwrap();

assert!(op.list("/").await.unwrap().is_empty());
```

# Reference-level explanation

As mentioned in the previous section, this RFC aims to integrate *foyer* to fully leverage its capabilities, rather than designing a generic cache layer. Therefore, a transparent integration can be achieved through a `FoyerLayer`.

`FoyerLayer` holds both the reference of both internal accessor, and a *foyer* instance. For operations supported by *foyer* and compatible in behavior, the `FoyerLayer` will use *foyer* to handle requests, accessing the internal accessor as needed. For operations that *foyer* cannot support, it will automatically fallback to using the internal accessor implementation.

Here are the details of operations that involve *foyer* operation:

- `read`: Read from *foyer* hybrid cache, if the hybrid cache misses, fallback to internal accessor `read` operation.
    - For range get, *foyer* caches and fetches the whole object and returns the requested object range. (In future versions, it may be possible to support user configuration for whether to cache the entire object or only the objects covered by the range.)
- `write`: Insert hybrid cache on internal accessor `write` operation success.
- `delete`: Delete object from *foyer* hybrid cache regardless of internal accessor `delete` operation success.

# Drawbacks

Since we cannot perceive whether other users have updated the data in the underlying storage system, introducing a cache in this case may lead to data inconsistency. Therefore, the integration of Foyer is more suitable for object storage systems that do not support updating objects.

# Rationale and alternatives

[RFC#6297](https://github.com/apache/opendal/pull/6297) has mentioned a general cache layer design, but cannot fully leverage *foyer*'s capabilities. However, the two are not in conflict.  At the same time, because #6297 has not yet been finalized, I prefer to implement a layer specifically for the *foyer* first. This does not affect the future implementation of a general cache layer and can also help quickly identify potential user needs and issues.

# Prior art

*Foyer* has already been applied in systems like RisingWave, ChromaDB, and SlateDB. We can learn from this experience. Notably, both RisingWave and SlateDB support using OpenDAL as the data access layer. This RFC will provide a smoother experience for users with similar needs.

# Unresolved questions

None

# Future possibilities

- Based on the experience of implementing the *foyer* layer, a more general cache layer can be developed.
- Adjust the API of *foyer* to align with the usage of OpenDAL, enhancing compatibility between the two.
