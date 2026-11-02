# Apache OpenDAL™ Monoio Filesystem Service

`opendal-service-monoiofs` provides local filesystem access powered by `monoio` for
applications built with Apache OpenDAL™.

## Use through `opendal`

Applications should normally enable this service through the `opendal` facade with the
`services-monoiofs` feature:

```shell
cargo add opendal --features services-monoiofs
```

The service is available as `opendal::services::Monoiofs`. Configure the
service builder, then pass it to `opendal::Operator::new`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-service-monoiofs
```

Pass a configured service builder to `Operator::new`:

```rust
use opendal_core::{Operator, OperatorRegistry, Result};
use opendal_service_monoiofs::{register_monoiofs_service, Monoiofs};

fn build_operator(builder: Monoiofs) -> Result<Operator> {
    Operator::new(builder)
}

fn register_for_uri() {
    register_monoiofs_service(OperatorRegistry::get());
}
```

`register_for_uri` is only needed for scheme-driven construction through
`Operator::from_uri` or `Operator::via_iter`.

Services that send HTTP requests also require an HTTP transport in
`OperationContext`. See the
[`opendal-core` composition example](https://crates.io/crates/opendal-core).

This service is useful when an application wants `monoio`-based filesystem I/O.

## Documentation

- [Service configuration and examples](https://opendal.apache.org/services/monoiofs)
- [Rust API documentation](https://docs.rs/opendal-service-monoiofs)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
