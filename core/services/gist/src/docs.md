## Capabilities

Depending on its configuration, this service can expose:

- [x] stat
- [x] read
- [x] write
- [x] delete
- [x] list
- [ ] create_dir
- [ ] copy
- [ ] rename
- [ ] presign

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Behavior

- A gist is a flat set of files: the key is the gist file name itself. A path
  with a directory component is rejected with [`opendal_core::ErrorKind::Unsupported`],
  `create_dir` is not supported, and listing a non-root path returns nothing.
- `write` replaces the whole content of one file with a single Gist edit. The
  Gist API transports file content as a JSON string, so content must be valid
  UTF-8. The API rejects empty content, so `write_can_empty` is `false`.
- `delete` removes one file from the gist. A gist keeps at least one file, so
  deleting the last remaining file fails with the error GitHub returns.
- Reading a file larger than 1 MiB falls back to the file's `raw_url`, because
  the Gist API truncates inline content above that size.
- Reading a public gist needs no credential. Updating files needs a token with
  the `gist` scope.

## Configuration

Use [`crate::GistConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Example

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_gist::Gist;

#[tokio::main]
async fn main() -> Result<()> {
    // create backend builder
    let mut builder = Gist::default()
        // set the access token for the GitHub API
        .token("your_access_token")
        // set the identifier of the gist to operate on
        .gist_id("aa5a315d61ae9438b18d");

    let op: Operator = Operator::new(builder)?;

    Ok(())
}
```

### Via URI

```rust,no_run
use opendal_core::Operator;
use opendal_core::OperatorRegistry;
use opendal_core::Result;
use opendal_service_gist::register_gist_service;

#[tokio::main]
async fn main() -> Result<()> {
    register_gist_service(OperatorRegistry::get());

    let op = Operator::from_uri("gist://aa5a315d61ae9438b18d")?;

    Ok(())
}
```
