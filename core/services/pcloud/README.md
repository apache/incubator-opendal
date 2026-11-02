# Apache OpenDAL™ pCloud Service

`opendal-service-pcloud` provides access to pCloud storage for applications built with
Apache OpenDAL™.

## Use through `opendal`

Applications should normally enable this service through the `opendal` facade with the
`services-pcloud` feature:

```shell
cargo add opendal --features services-pcloud
```

The service is available as `opendal::services::Pcloud`. Configure the
service builder, then pass it to `opendal::Operator::new`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-service-pcloud
```

Pass a configured service builder to `Operator::new`:

```rust
use opendal_core::{Operator, OperatorRegistry, Result};
use opendal_service_pcloud::{register_pcloud_service, Pcloud};

fn build_operator(builder: Pcloud) -> Result<Operator> {
    Operator::new(builder)
}

fn register_for_uri() {
    register_pcloud_service(OperatorRegistry::get());
}
```

`register_for_uri` is only needed for scheme-driven construction through
`Operator::from_uri` or `Operator::via_iter`.

Services that send HTTP requests also require an HTTP transport in
`OperationContext`. See the
[`opendal-core` composition example](https://crates.io/crates/opendal-core).

## Documentation

- [Service configuration and examples](https://opendal.apache.org/services/pcloud)
- [Rust API documentation](https://docs.rs/opendal-service-pcloud)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
