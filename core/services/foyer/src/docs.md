# Foyer Service

[foyer](https://github.com/foyer-rs/foyer) is a high-performance hybrid cache library
that supports both in-memory and on-disk caching.

This service provides foyer as a volatile KV storage backend. Data stored may be evicted
when the cache is full.

## Capabilities

Depending on its configuration and the backing system, this service can expose:

- [x] stat
- [x] read
- [x] write
- [x] delete
- [ ] list (not supported)
- [ ] blocking (not supported)

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Configuration

Use [`crate::FoyerConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

Foyer service can be configured in two ways:

1. **Pre-configured cache**: Pass a fully configured `HybridCache` instance for maximum control
2. **Auto-configured cache**: Use builder methods to configure memory and disk caching

### Auto-configured Cache Options

When using the builder API without providing a pre-built cache, the following options are available:

- `memory`: Memory cache capacity (default: 1 GiB)
- `disk_path`: Directory path for disk cache (enables hybrid caching)
- `disk_capacity`: Total disk cache capacity
- `disk_file_size`: Individual cache file size (default: 1 MiB)
- `recover_mode`: Recovery mode on startup - "none" (default), "quiet", or "strict"
- `shards`: Number of cache shards for concurrency (default: 1)

## Example

### Via Pre-configured Cache

```rust,no_run
use opendal::Operator;
use opendal_service_foyer::Foyer;
use opendal_service_foyer::FoyerKey;
use opendal_service_foyer::FoyerValue;
use foyer::{BlockEngineConfig, DeviceBuilder, FsDeviceBuilder, HybridCacheBuilder};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a foyer HybridCache with full control
    let device = FsDeviceBuilder::new("/tmp/foyer")
        .with_capacity(1024 * 1024 * 1024)
        .build()?;
    let cache = HybridCacheBuilder::new()
        .memory(64 * 1024 * 1024) // 64MB memory cache
        .storage()
        .with_engine_config(BlockEngineConfig::new(device))
        .build()
        .await?;

    // Create operator
    let op = Operator::new(Foyer::new().cache(cache))?;

    // Use it like any other OpenDAL operator
    op.write("key", "value").await?;
    let value = op.read("key").await?;

    Ok(())
}
```

### Via Auto-configuration (Hybrid Cache)

```rust,no_run
use opendal::Operator;
use opendal_service_foyer::Foyer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create operator with hybrid cache (memory + disk)
    let op = Operator::new(
        Foyer::new()
            .memory(64 * 1024 * 1024)           // 64MB memory
            .disk_path("/tmp/foyer_cache")      // Enable disk cache
            .disk_capacity(1024 * 1024 * 1024)  // 1GB disk
            .disk_file_size(1024 * 1024)        // 1MB per file
            .recover_mode("quiet")              // Recover on restart
            .shards(4)                          // 4 shards for concurrency
    )?
    ;

    op.write("key", "value").await?;
    let value = op.read("key").await?;

    Ok(())
}
```

### Via URI

```rust,no_run
use opendal::Operator;
use opendal::OperatorRegistry;
use opendal_service_foyer::register_foyer_service;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    register_foyer_service(OperatorRegistry::get());

    // Memory-only cache via URI
    let op = Operator::from_uri("foyer:///?memory=67108864")?;

    // Hybrid cache via URI
    let op = Operator::from_uri(
        "foyer:///cache?memory=67108864&disk_path=/tmp/foyer&disk_capacity=1073741824"
    )?;

    Ok(())
}
```

## Notes

- **Data Volatility**: Foyer is a cache, not persistent storage. Data may be
  evicted at any time when the cache reaches its capacity limit.
- **Hybrid Caching**: When `disk_path` is configured, cold data automatically
  moves from memory to disk as the memory cache fills up.
- **Recovery Modes**:
  - `"none"`: Don't restore data from disk on restart (volatile cache)
  - `"quiet"`: Restore data and skip any corrupted entries
  - `"strict"`: Restore data and fail on any corruption
- **No List Support**: Foyer does not support efficient key iteration, so the
  `list` operation is not available.
- **Async Only**: Foyer is async-only, blocking operations are not supported.
