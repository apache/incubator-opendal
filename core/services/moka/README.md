# Apache OpenDAL™ Moka Service

`opendal-service-moka` provides an in-process cache backend backed by `moka` for
applications built with Apache OpenDAL™.

## Use through `opendal`

Applications should normally enable this service through the `opendal` facade with the
`services-moka` feature:

```shell
cargo add opendal --features services-moka
```

The service is available as `opendal::services::Moka`. Configure the
service builder, then pass it to `opendal::Operator::new`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-service-moka
```

Pass a configured service builder to `Operator::new`:

```rust
use opendal_core::{Operator, OperatorRegistry, Result};
use opendal_service_moka::{register_moka_service, Moka};

fn build_operator(builder: Moka) -> Result<Operator> {
    Operator::new(builder)
}

fn register_for_uri() {
    register_moka_service(OperatorRegistry::get());
}
```

`register_for_uri` is only needed for scheme-driven construction through
`Operator::from_uri` or `Operator::via_iter`.

Services that send HTTP requests also require an HTTP transport in
`OperationContext`. See the
[`opendal-core` composition example](https://crates.io/crates/opendal-core).

## Documentation

- [Service configuration and examples](https://opendal.apache.org/services/moka)
- [Rust API documentation](https://docs.rs/opendal-service-moka)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
