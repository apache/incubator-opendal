# Apache OpenDAL™ Core

`opendal-core` contains the storage abstraction, operator APIs, extension
traits, and shared infrastructure used by Apache OpenDAL™.

Most applications should depend on the [`opendal`](https://crates.io/crates/opendal)
facade. The facade re-exports the core APIs and wires optional service, layer,
executor, and HTTP transport crates behind feature flags.

Depend on `opendal-core` directly when implementing an OpenDAL service or layer,
building a minimal integration from the split crates, or managing service
registration and HTTP transport explicitly.

## Quick start

The in-memory service is always available:

```shell
cargo add opendal-core
```

```rust
use opendal_core::services;
use opendal_core::Operator;
use opendal_core::Result;

fn build_operator() -> Result<Operator> {
    Operator::new(services::Memory::default())
}
```

Storage services and optional layers live in separate
`opendal-service-*` and `opendal-layer-*` crates. Applications using the facade
can enable them through `services-*` and `layers-*` features instead.

## Compose split crates

Applications that depend on `opendal-core` directly compose storage and runtime
behavior explicitly:

- A service crate exports a builder that implements `opendal_core::Builder`.
  Pass the configured builder to `Operator::new`.
- A layer crate exports a type that implements `opendal_core::raw::Layer`. Pass
  the configured layer to `Operator::layer`.
- An HTTP transport crate exports an `opendal_core::HttpTransport`
  implementation. Wrap it in `HttpTransporter` and place it in the operator's
  `OperationContext`.

For example, the following dependencies compose S3, retry, and the reqwest HTTP
transport without the `opendal` facade:

```shell
cargo add opendal-core opendal-service-s3 opendal-layer-retry
cargo add opendal-http-transport-reqwest
```

```rust,ignore
use opendal_core::HttpTransporter;
use opendal_core::OperationContext;
use opendal_core::Operator;
use opendal_core::Result;
use opendal_http_transport_reqwest::ReqwestTransport;
use opendal_layer_retry::RetryLayer;
use opendal_service_s3::S3;

fn build_operator(builder: S3) -> Result<Operator> {
    let context = OperationContext::new()
        .with_http_transport(HttpTransporter::new(ReqwestTransport::default()));

    Ok(Operator::new(builder)?
        .with_context(context)
        .layer(RetryLayer::default()))
}
```

Configure `builder` for the target S3-compatible service before passing it to
`build_operator`.

`Operator::new` uses the builder directly and does not require service
registration. Call the service crate's `register_*_service` function with
`OperatorRegistry::get()` only when using scheme-driven construction through
`Operator::from_uri` or `Operator::via_iter`.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-core)
- [Apache OpenDAL Rust guide](https://opendal.apache.org/docs/core)
- [Services and configuration](https://opendal.apache.org/services)
- [Apache OpenDAL project](https://opendal.apache.org/)

## License

Licensed under the Apache License, Version 2.0.
