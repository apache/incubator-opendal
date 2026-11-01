# Apache OpenDAL™ Compfs Service

`opendal-service-compfs` provides local filesystem access powered by `compio` for
applications built with Apache OpenDAL™.

## Use through `opendal`

Applications should normally enable this service through the `opendal` facade with the
`services-compfs` feature:

```shell
cargo add opendal --features services-compfs
```

The service is available as `opendal::services::Compfs`. Configure the
service builder, then pass it to `opendal::Operator::new`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-service-compfs
```

Pass a configured service builder to `Operator::new`:

```rust
use opendal_core::{Operator, OperatorRegistry, Result};
use opendal_service_compfs::{register_compfs_service, Compfs};

fn build_operator(builder: Compfs) -> Result<Operator> {
    Operator::new(builder)
}

fn register_for_uri() {
    register_compfs_service(OperatorRegistry::get());
}
```

`register_for_uri` is only needed for scheme-driven construction through
`Operator::from_uri` or `Operator::via_iter`.

Services that send HTTP requests also require an HTTP transport in
`OperationContext`. See the
[`opendal-core` composition example](https://crates.io/crates/opendal-core).

This service is useful when an application wants `compio`-based filesystem I/O.

## Documentation

- [Service configuration and examples](https://opendal.apache.org/services/compfs)
- [Rust API documentation](https://docs.rs/opendal-service-compfs)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
