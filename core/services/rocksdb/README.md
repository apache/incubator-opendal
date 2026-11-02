# Apache OpenDAL™ RocksDB Service

`opendal-service-rocksdb` provides an embedded key-value storage backend backed by
RocksDB for applications built with Apache OpenDAL™.

## Use through `opendal`

Applications should normally enable this service through the `opendal` facade with the
`services-rocksdb` feature:

```shell
cargo add opendal --features services-rocksdb
```

The service is available as `opendal::services::Rocksdb`. Configure the
service builder, then pass it to `opendal::Operator::new`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-service-rocksdb
```

Pass a configured service builder to `Operator::new`:

```rust
use opendal_core::{Operator, OperatorRegistry, Result};
use opendal_service_rocksdb::{register_rocksdb_service, Rocksdb};

fn build_operator(builder: Rocksdb) -> Result<Operator> {
    Operator::new(builder)
}

fn register_for_uri() {
    register_rocksdb_service(OperatorRegistry::get());
}
```

`register_for_uri` is only needed for scheme-driven construction through
`Operator::from_uri` or `Operator::via_iter`.

Services that send HTTP requests also require an HTTP transport in
`OperationContext`. See the
[`opendal-core` composition example](https://crates.io/crates/opendal-core).

## Documentation

- [Service configuration and examples](https://opendal.apache.org/services/rocksdb)
- [Rust API documentation](https://docs.rs/opendal-service-rocksdb)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
