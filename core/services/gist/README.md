# Apache OpenDAL™ GitHub Gist Service

`opendal-service-gist` provides access to the files of a single GitHub Gist for
applications built with Apache OpenDAL™.

A gist is a flat set of files, so keys are gist file names and there are no
directories. The service reads files through the
[Gist API](https://docs.github.com/en/rest/gists/gists), and updating or deleting
a file is one Gist edit.

## Use through `opendal`

Applications should normally enable this service through the `opendal` facade with the
`services-gist` feature:

```shell
cargo add opendal --features services-gist
```

The service is available as `opendal::services::Gist`. Configure the
service builder, then pass it to `opendal::Operator::new`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-service-gist
```

Pass a configured service builder to `Operator::new`:

```rust
use opendal_core::{Operator, OperatorRegistry, Result};
use opendal_service_gist::{Gist, register_gist_service};

fn build_operator(builder: Gist) -> Result<Operator> {
    Operator::new(builder)
}

fn register_for_uri() {
    register_gist_service(OperatorRegistry::get());
}
```

`register_for_uri` is only needed for scheme-driven construction through
`Operator::from_uri` or `Operator::via_iter`.

Services that send HTTP requests also require an HTTP transport in
`OperationContext`. See the
[`opendal-core` composition example](https://crates.io/crates/opendal-core).

## Documentation

- [Rust API documentation](https://docs.rs/opendal-service-gist)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
