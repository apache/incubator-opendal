# Apache OpenDAL™ sled Service

`opendal-service-sled` provides an embedded key-value storage backend backed by `sled`
for applications built with Apache OpenDAL™.

## Use through `opendal`

Applications should normally enable this service through the `opendal` facade with the
`services-sled` feature:

```shell
cargo add opendal --features services-sled
```

The service is available as `opendal::services::Sled`. Configure the
service builder, then pass it to `opendal::Operator::new`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-service-sled
```

Pass a configured service builder to `Operator::new`:

```rust
use opendal_core::{Operator, OperatorRegistry, Result};
use opendal_service_sled::{register_sled_service, Sled};

fn build_operator(builder: Sled) -> Result<Operator> {
    Operator::new(builder)
}

fn register_for_uri() {
    register_sled_service(OperatorRegistry::get());
}
```

`register_for_uri` is only needed for scheme-driven construction through
`Operator::from_uri` or `Operator::via_iter`.

Services that send HTTP requests also require an HTTP transport in
`OperationContext`. See the
[`opendal-core` composition example](https://crates.io/crates/opendal-core).

## Documentation

- [Service configuration and examples](https://opendal.apache.org/services/sled)
- [Rust API documentation](https://docs.rs/opendal-service-sled)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
