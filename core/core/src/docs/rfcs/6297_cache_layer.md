- Proposal Name: `cache_layer`
- Start Date: 2025-06-16
- RFC PR: [apache/opendal#6297](https://github.com/apache/opendal/pull/6297)
- Tracking Issue: [apache/opendal#7107](https://github.com/apache/opendal/issues/7107)

# Summary

This RFC proposes a Cache Layer for OpenDAL that composes:

- a **source** `Operator` (the backend you actually want to read/write), and
- a **cache** `Operator` (a faster backend, for example Memory/Moka/Redis), and
- a user-extensible **`CachePolicy`** that decides _whether to use cache_, _whether to fill_, and _how to shape cache objects_ (e.g. whole-object vs chunked).

The layer itself stays thin: policy controls cache behavior, while concrete cache backends control eviction/TTL/limits.

# Motivation

Storage access performance varies greatly across different storage services. Remote object stores like S3 or GCS have much higher latency than local storage or in-memory caches. In many applications, particularly those with read-heavy workloads or repeated access to the same data, caching can significantly improve performance.

Without a built-in cache layer, users have to implement common patterns repeatedly:

1. Look up in cache
2. On miss, read from source
3. Decide whether/when/how to populate cache
4. Handle best-effort consistency (e.g., invalidation on writes/deletes) themselves

A Cache Layer inside OpenDAL can:

- Provide a unified and composable caching abstraction.
- Keep cache decisions user-controlled via policy (including bypass/fill/chunking).
- Reuse existing OpenDAL services as cache backends.

# Guide-level explanation

## Mental model

`CacheLayer` wraps a source `Operator` with a cache `Operator`. On each operation (read/stat/write/delete), the layer consults a `CachePolicy` to decide:

- whether to **bypass** cache entirely for this request, or **use** cache.
- if cache is used and a miss happens, whether to **fill** cache.
- whether to cache as a **whole object** or in **chunks** (range-aware / 1:M mapping between source object and cache entries).

Cache eviction/TTL is handled by the chosen cache backend itself.

## Basic usage

```rust
use opendal::{
  layers::{CacheLayer, WholeCachePolicy},
  services::Memory,
  Operator,
};

#[tokio::main]
async fn main() -> opendal::Result<()> {
    let cache = Operator::new(Memory::default())?.finish();

    let policy = WholeCachePolicy::new()
        .fill_on_read_miss(true)
        .invalidate_on_write(true)
        .invalidate_on_delete(true);

    let op = Operator::new(/* source service builder */)?
        .layer(CacheLayer::new(cache, policy))
        .finish();

    let _ = op.read("path/to/file").await?;
    Ok(())
}
```

This example highlights the intended responsibilities:

- `CacheLayer` is glue code that composes `source + cache + policy`.
- `CachePolicy` decides behavior; the cache backend decides eviction/TTL.

## Route-based caching

`CacheLayer` does not select which paths should be cached. Path-based dispatch is handled by
`RouteLayer`, which is available behind the `layers-route` feature.

Users can build independent operator stacks for different path groups, each with its own cache
policy, and then combine them with `RouteLayer`:

```rust
use opendal::{
  layers::{CacheLayer, ChunkedCachePolicy, RouteLayer, WholeCachePolicy},
  services::Memory,
  Operator,
};

fn build_routed_operator() -> opendal::Result<Operator> {
    let default_op = Operator::new(/* source service builder */)?.finish();

    let json_cache = Operator::new(Memory::default())?.finish();
    let json_op = Operator::new(/* source service builder */)?
        .layer(CacheLayer::new(json_cache, WholeCachePolicy::new()))
        .finish();

    let parquet_cache = Operator::new(Memory::default())?.finish();
    let parquet_op = Operator::new(/* source service builder */)?
        .layer(CacheLayer::new(
            parquet_cache,
            ChunkedCachePolicy::new(8 * 1024 * 1024),
        ))
        .finish();

    let op = default_op.layer(
        RouteLayer::builder()
            .route("**/*.json", json_op)
            .route("**/*.parquet", parquet_op)
            .build()?,
    );

    Ok(op)
}
```

This keeps routing and caching separated:

- `RouteLayer` decides which operator stack handles a path.
- `CacheLayer` decides how a selected stack uses its cache.

# Reference-level explanation

## Public API surface

To keep the public API small and consistent with the rest of OpenDAL, the cache layer should accept `Operator` as inputs (both source and cache). Internally it can obtain the underlying accessor/dispatcher as needed.

This RFC proposes:

- `CacheLayer::new(cache: Operator, policy: impl CachePolicy) -> CacheLayer`
- `CachePolicy` trait: defines evaluation and shaping behavior.
- Minimal supporting request/decision types (`Cache*Request`, `CacheReadDecision`, `CacheWriteDecision`, `CacheDeleteDecision`, `CacheLayout`).

Notably, this design does **not** introduce a parallel “cache backend trait” separate from OpenDAL; the cache backend is just an `Operator`.

### CachePolicy

`CachePolicy` decides what to do for each operation. To keep decisions aligned with OpenDAL
semantics, the policy receives a typed request that borrows the corresponding `Op*` options.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheLayout {
    Whole,
    Chunked { size: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheReadDecision {
    /// Do not consult cache and do not fill it for this request.
    Bypass,
    /// Use cache; optionally define layout and whether to fill on miss.
    Use { layout: CacheLayout, fill: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheWriteDecision {
    /// Do not consult cache and do not fill/invalidate it for this request.
    Bypass,
    /// Best-effort invalidation after a successful write.
    Invalidate { layout: CacheLayout },
    /// Write-through caching after a successful write.
    WriteThrough { layout: CacheLayout },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheDeleteDecision {
    /// Do not consult cache and do not invalidate it for this request.
    Bypass,
    /// Best-effort invalidation after a successful delete.
    Invalidate { layout: CacheLayout },
}

pub struct CacheReadRequest<'a> {
    /// Path as seen by OpenDAL.
    pub path: &'a str,
    /// Read options from OpenDAL.
    pub op: &'a OpRead,
}

pub struct CacheStatRequest<'a> {
    /// Path as seen by OpenDAL.
    pub path: &'a str,
    /// Stat options from OpenDAL.
    pub op: &'a OpStat,
}

pub struct CacheWriteRequest<'a> {
    /// Path as seen by OpenDAL.
    pub path: &'a str,
    /// Write options from OpenDAL.
    pub op: &'a OpWrite,
}

pub struct CacheDeleteRequest<'a> {
    /// Path as seen by OpenDAL.
    pub path: &'a str,
    /// Delete options from OpenDAL.
    pub op: &'a OpDelete,
}

pub trait CachePolicy: Send + Sync + 'static {
    fn on_read(&self, req: CacheReadRequest<'_>) -> CacheReadDecision;
    fn on_stat(&self, req: CacheStatRequest<'_>) -> CacheReadDecision;
    fn on_write(&self, req: CacheWriteRequest<'_>) -> CacheWriteDecision;
    fn on_delete(&self, req: CacheDeleteRequest<'_>) -> CacheDeleteDecision;
}
```

OpenDAL may provide some basic policies:

- `WholeCachePolicy`: whole-object caching for reads; optional fill; best-effort invalidation on write/delete.
- `ChunkedCachePolicy`: range-aware chunk caching (1:M mapping from source object to cache keys).
- `MetadataOnlyCachePolicy`: cache only metadata (stat) for selected paths.

Users can implement `CachePolicy` to match their own access patterns (e.g. parquet/iceberg range scans, content immutability assumptions, etc).

## Cache consistency model (best effort)

This layer provides **best-effort** consistency between cache and source:

- On `read`: may serve from cache when allowed by policy; on miss may fill depending on policy.
- On `write`: the layer should **invalidate** relevant cache entries after a successful write (best effort). If the write is cached (write-through), then cache will contain the new content. If write-through is disabled by policy, invalidation prevents serving stale data.
- On `delete`: the layer should **invalidate** relevant cache entries after a successful delete (best effort), to avoid serving deleted content from cache.

For operations that return stateful objects, success is defined by OpenDAL's operation lifecycle.
For example, `delete` returns a `Deleter` that may batch entries and commit them during `close()`;
cache invalidation must happen after the underlying delete has been committed successfully, not when
the path is merely queued.

Cache failures (e.g. Redis unavailable) should not fail the source operation by default; they only reduce caching effectiveness.

## Cache key shaping

A cache entry key is not required to be identical to the source path:

- Whole-object caching can use `{path}` (optionally with a namespace prefix).
- Chunked caching can use `{path}@{offset}-{len}` (or an equivalent scheme).

The shaping is driven by `CachePolicy` (via `CacheLayout` and request fields like range).

`CacheLayer` should use a namespace that cannot collide with normal user paths stored in the cache
operator. Version and range information should be encoded into keys when the underlying operation
uses versions or chunked layout.

## Out of scope

- Caching results of `list`.
- Providing strong consistency guarantees across external writers.
- Mandating specific eviction/TTL/size policies (left to cache backend).

# Drawbacks

- Cache correctness depends on user policy choices (bypass/fill/chunking/invalidation).
- Best-effort invalidation does not handle external mutations to the source content.
- Chunked caching introduces additional complexity in key mapping and invalidation.

# Rationale and alternatives

## Why CachePolicy instead of boolean options?

Embedding booleans makes the cache layer grow “policy” over time (ranges, chunking, metadata-only, bypass rules).
A `CachePolicy` keeps the layer thin and makes behavior user-extensible without expanding the layer’s public API for every new scenario.

## Why accept Operator instead of a new CacheService trait?

Using `Operator` as the cache backend avoids creating a parallel storage abstraction surface and keeps cache backends consistent with existing OpenDAL services. Internally, the layer can convert `Operator` to the underlying accessor.

# Prior art

- Database and filesystem cache layers commonly separate “storage backend” from “policy/strategy”.
- Range/chunk caches are common for analytical formats (parquet/iceberg) to avoid whole-object read amplification.

# Unresolved questions

1. Cache key namespace strategy: should we standardize a prefix (e.g. `__opendal_cache__/`) to prevent collisions?
2. Range reads with chunked caching: how should partial hits and range alignment behave?
3. Metadata caching boundary: which metadata fields should be trusted/required when serving from cache?

# Future possibilities

- Tiered caching with different policies per layer (L1/L2) and optional promotion strategies.
- Built-in metrics (hit/miss, fill latency, invalidation failures).
- More advanced directives (e.g. “metadata-only”, “write-back”) when there are clear use cases.
- Configurable error handling strategy (best-effort vs strict mode for cache failures).
- Version-aware cache keys for backends that support object versioning.
