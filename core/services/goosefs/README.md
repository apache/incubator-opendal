# Apache OpenDAL™ GooseFS Service

`opendal-service-goosefs` provides access to GooseFS through its native gRPC API for
applications built with Apache OpenDAL™.

## Use through `opendal`

Applications should normally enable this service through the `opendal` facade with the
`services-goosefs` feature:

```shell
cargo add opendal --features services-goosefs
```

The service is available as `opendal::services::GooseFs`. Configure the
service builder, then pass it to `opendal::Operator::new`.

Optional client caches stay on this crate, not on the `opendal` facade. Add
this crate as a direct dependency so Cargo unifies the features into the same
`opendal-service-goosefs` build:

```shell
cargo add opendal-service-goosefs --features metadata-cache,page-cache
# Linux io_uring page-cache backend:
cargo add opendal-service-goosefs --features page-cache-io-uring
```

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-service-goosefs
# Optional caches: cargo add opendal-service-goosefs --features metadata-cache,page-cache
```

Pass a configured service builder to `Operator::new`:

```rust
use opendal_core::{Operator, OperatorRegistry, Result};
use opendal_service_goosefs::{register_goosefs_service, GooseFs};

fn build_operator(builder: GooseFs) -> Result<Operator> {
    Operator::new(builder)
}

fn register_for_uri() {
    register_goosefs_service(OperatorRegistry::get());
}
```

`register_for_uri` is only needed for scheme-driven construction through
`Operator::from_uri` or `Operator::via_iter`.

Services that send HTTP requests also require an HTTP transport in
`OperationContext`. See the
[`opendal-core` composition example](https://crates.io/crates/opendal-core).

## Documentation

- [Service configuration and examples](https://opendal.apache.org/services/goosefs)
- [Rust API documentation](https://docs.rs/opendal-service-goosefs)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
