# Apache OpenDAL™ Backblaze B2 Service

`opendal-service-b2` provides access to Backblaze B2 object storage for applications
built with Apache OpenDAL™.

## Use through `opendal`

Applications should normally enable this service through the `opendal` facade with the
`services-b2` feature:

```shell
cargo add opendal --features services-b2
```

The service is available as `opendal::services::B2`. Configure the
service builder, then pass it to `opendal::Operator::new`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-service-b2
```

Pass a configured service builder to `Operator::new`:

```rust
use opendal_core::{Operator, OperatorRegistry, Result};
use opendal_service_b2::{register_b2_service, B2};

fn build_operator(builder: B2) -> Result<Operator> {
    Operator::new(builder)
}

fn register_for_uri() {
    register_b2_service(OperatorRegistry::get());
}
```

`register_for_uri` is only needed for scheme-driven construction through
`Operator::from_uri` or `Operator::via_iter`.

Services that send HTTP requests also require an HTTP transport in
`OperationContext`. See the
[`opendal-core` composition example](https://crates.io/crates/opendal-core).

## Documentation

- [Service configuration and examples](https://opendal.apache.org/services/b2)
- [Rust API documentation](https://docs.rs/opendal-service-b2)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
