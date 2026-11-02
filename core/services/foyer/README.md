# Apache OpenDAL™ Foyer Service

`opendal-service-foyer` provides a volatile storage backend backed by the Foyer
hybrid cache for applications built with Apache OpenDAL™. Foyer can evict data
when the cache is full, so do not use this service for persistent storage.

## Use through `opendal`

Applications should normally enable this service through the `opendal` facade with the
`services-foyer` feature:

```shell
cargo add opendal --features services-foyer
```

The service is available as `opendal::services::Foyer`. Configure the
service builder, then pass it to `opendal::Operator::new`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-service-foyer
```

Pass a configured service builder to `Operator::new`:

```rust
use opendal_core::{Operator, OperatorRegistry, Result};
use opendal_service_foyer::{register_foyer_service, Foyer};

fn build_operator(builder: Foyer) -> Result<Operator> {
    Operator::new(builder)
}

fn register_for_uri() {
    register_foyer_service(OperatorRegistry::get());
}
```

`register_for_uri` is only needed for scheme-driven construction through
`Operator::from_uri` or `Operator::via_iter`.

Services that send HTTP requests also require an HTTP transport in
`OperationContext`. See the
[`opendal-core` composition example](https://crates.io/crates/opendal-core).

## Documentation

- [Service configuration and examples](https://opendal.apache.org/services/foyer)
- [Rust API documentation](https://docs.rs/opendal-service-foyer)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
