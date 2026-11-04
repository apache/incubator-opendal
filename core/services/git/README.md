# Apache OpenDAL™ Git Service

`opendal-service-git` provides read-only access to a Git repository at a fixed
reference with optional Git LFS resolution.

The service resolves the configured reference once per operator and keeps that
immutable snapshot. It uses a temporary Git object database and does not create
or update a working tree. `HEAD` uses a shallow clone; an explicit branch, tag,
or commit uses a full clone.

## Use through `opendal`

Applications can enable this service through the `opendal` facade with the
`services-git` feature:

```shell
cargo add opendal --features services-git
```

The service is available as `opendal::services::Git`. Configure the builder,
then pass it to `opendal::Operator::new`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-service-git
```

Pass a configured builder to `Operator::new`:

```rust
use opendal_core::{Operator, OperatorRegistry, Result};
use opendal_service_git::{register_git_service, Git};

fn build_operator(builder: Git) -> Result<Operator> {
    Operator::new(builder)
}

fn register_service() {
    register_git_service(OperatorRegistry::get());
}
```

Services that use HTTP also require an HTTP transport in `OperationContext`.
The Git fetch uses the transport configured by gix; LFS batch and payload
requests use the composed OpenDAL HTTP transport.

Git fetch and object access use `tokio::task::spawn_blocking`. Run this service
within a Tokio runtime.

## Notes

- A configured reference is resolved once and is not refreshed.
- `resolve_lfs=false` returns the LFS pointer file itself.
- `stat` and `list` set a file's ETag to its Git blob object ID. When LFS
  resolution is enabled, an LFS file's ETag is its LFS sha256 object ID
  instead. Directories have no ETag.
- LFS resolution requires an HTTPS repository URL without userinfo, a query,
  or a fragment. Use the username and password options for authentication.
- LFS payloads are streamed from the composed HTTP transport.
- Repository credentials are not sent to cross-origin LFS download actions.
- Submodule entries return `Unsupported`.
- No language binding is enabled for this service by this crate alone.

## Documentation

- [Service configuration and examples](https://opendal.apache.org/services/git)
- [Rust API documentation](https://docs.rs/opendal-service-git)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
