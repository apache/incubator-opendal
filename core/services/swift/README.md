# Apache OpenDAL™ OpenStack Swift Service

`opendal-service-swift` provides access to OpenStack Swift object storage for
applications built with Apache OpenDAL™.

## Use through `opendal`

Applications should normally enable this service through the `opendal` facade with the
`services-swift` feature:

```shell
cargo add opendal --features services-swift
```

The service is available as `opendal::services::Swift`. Configure the
service builder, then pass it to `opendal::Operator::new`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-service-swift
```

Pass a configured service builder to `Operator::new`:

```rust
use opendal_core::{Operator, OperatorRegistry, Result};
use opendal_service_swift::{register_swift_service, Swift};

fn build_operator(builder: Swift) -> Result<Operator> {
    Operator::new(builder)
}

fn register_for_uri() {
    register_swift_service(OperatorRegistry::get());
}
```

`register_for_uri` is only needed for scheme-driven construction through
`Operator::from_uri` or `Operator::via_iter`.

Services that send HTTP requests also require an HTTP transport in
`OperationContext`. See the
[`opendal-core` composition example](https://crates.io/crates/opendal-core).

## Documentation

- [Service configuration and examples](https://opendal.apache.org/services/swift)
- [Rust API documentation](https://docs.rs/opendal-service-swift)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
