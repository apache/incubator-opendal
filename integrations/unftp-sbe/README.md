# Apache OpenDAL™ unftp Integration

[![Build Status]][actions] [![Latest Version]][crates.io] [![Crate Downloads]][crates.io] [![chat]][discord]

[build status]: https://img.shields.io/github/actions/workflow/status/apache/opendal/ci_integration_unftp_sbe.yml?branch=main
[actions]: https://github.com/apache/opendal/actions?query=branch%3Amain
[latest version]: https://img.shields.io/crates/v/unftp-sbe-opendal.svg
[crates.io]: https://crates.io/crates/unftp-sbe-opendal
[crate downloads]: https://img.shields.io/crates/d/unftp-sbe-opendal.svg
[chat]: https://img.shields.io/discord/1081052318650339399
[discord]: https://opendal.apache.org/discord

`unftp-sbe-opendal` is an [unftp](https://crates.io/crates/unftp) `StorageBackend` implementation using opendal.

This crate can help you to access ANY storage services with the same FTP API.

## Useful Links

- Documentation: [release](https://docs.rs/unftp-sbe-opendal/) | [dev](https://opendal.apache.org/docs/unftp-sbe-opendal/unftp_sbe_opendal/)

## Examples

```rust
use anyhow::Result;
use opendal::Operator;
use opendal::services;
use unftp_sbe_opendal::OpendalStorage;

#[tokio::main]
async fn main() -> Result<()> {
    // Create any service desired
    let op = opendal::Operator::from_map::<services::S3>(
        [
            ("bucket".to_string(), "my_bucket".to_string()),
            ("access_key".to_string(), "my_access_key".to_string()),
            ("secret_key".to_string(), "my_secret_key".to_string()),
            ("endpoint".to_string(), "my_endpoint".to_string()),
            ("region".to_string(), "my_region".to_string()),
        ],
    )?;

    // Wrap the operator with `OpendalStorage`
    let backend = OpendalStorage::new(op);

    // Build the actual unftp server
    let server = libunftp::ServerBuilder::new(Box::new(move || backend.clone())).build()?;

    // Start the server
    server.listen("0.0.0.0:0").await?;

    Ok(())
}
```

## Branding

The first and most prominent mentions must use the full form: **Apache OpenDAL™** of the name for any individual usage (webpage, handout, slides, etc.) Depending on the context and writing style, you should use the full form of the name sufficiently often to ensure that readers clearly understand the association of both the OpenDAL project and the OpenDAL software product to the ASF as the parent organization.

For more details, see the [Apache Product Name Usage Guide](https://www.apache.org/foundation/marks/guide).

## License and Trademarks

Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0

Apache OpenDAL, OpenDAL, and Apache are either registered trademarks or trademarks of the Apache Software Foundation.
