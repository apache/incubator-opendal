# Apache OpenDAL™ Origin Private File System Service

`opendal-service-opfs` provides access to the browser Origin Private File System for
applications built with Apache OpenDAL™.

## Use through `opendal`

Applications should normally enable this service through the `opendal` facade with the
`services-opfs` feature:

```shell
cargo add opendal --features services-opfs
```

The service is available as `opendal::services::Opfs`. Configure the
service builder, then pass it to `opendal::Operator::new`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-service-opfs
```

Pass a configured service builder to `Operator::new`:

```rust
#[cfg(target_arch = "wasm32")]
use opendal_core::{Operator, OperatorRegistry, Result};
#[cfg(target_arch = "wasm32")]
use opendal_service_opfs::{register_opfs_service, Opfs};

#[cfg(target_arch = "wasm32")]
fn build_operator(builder: Opfs) -> Result<Operator> {
    Operator::new(builder)
}

#[cfg(target_arch = "wasm32")]
fn register_for_uri() {
    register_opfs_service(OperatorRegistry::get());
}
```

`register_for_uri` is only needed for scheme-driven construction through
`Operator::from_uri` or `Operator::via_iter`.

Services that send HTTP requests also require an HTTP transport in
`OperationContext`. See the
[`opendal-core` composition example](https://crates.io/crates/opendal-core).

This service targets `wasm32` applications running in environments that expose the
browser OPFS APIs.

## Documentation

- [Service configuration and examples](https://opendal.apache.org/services/opfs)
- [Rust API documentation](https://docs.rs/opendal-service-opfs)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
