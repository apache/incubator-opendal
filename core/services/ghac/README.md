# Apache OpenDAL™ GitHub Actions Cache Service

`opendal-service-ghac` provides access to GitHub Actions Cache storage for applications
built with Apache OpenDAL™.

## Use through `opendal`

Applications should normally enable this service through the `opendal` facade with the
`services-ghac` feature:

```shell
cargo add opendal --features services-ghac
```

The service is available as `opendal::services::Ghac`. Configure the
service builder, then pass it to `opendal::Operator::new`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-service-ghac
```

Pass a configured service builder to `Operator::new`:

```rust
use opendal_core::{Operator, OperatorRegistry, Result};
use opendal_service_ghac::{register_ghac_service, Ghac};

fn build_operator(builder: Ghac) -> Result<Operator> {
    Operator::new(builder)
}

fn register_for_uri() {
    register_ghac_service(OperatorRegistry::get());
}
```

`register_for_uri` is only needed for scheme-driven construction through
`Operator::from_uri` or `Operator::via_iter`.

Services that send HTTP requests also require an HTTP transport in
`OperationContext`. See the
[`opendal-core` composition example](https://crates.io/crates/opendal-core).

## Documentation

- [Service configuration and examples](https://opendal.apache.org/services/ghac)
- [Rust API documentation](https://docs.rs/opendal-service-ghac)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
