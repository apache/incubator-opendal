## Capabilities

Depending on its configuration and the backing system, this service can expose:

- [ ] create_dir
- [x] stat
- [x] read
- [x] write
- [x] delete
- [x] list
- [x] copy
- [ ] rename
- [x] presign

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Configuration

Use [`crate::TosConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Example

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_tos::Tos;

fn main() -> Result<()> {
    let builder = Tos::default()
        // Set the root for TOS, all operations will happen under this root.
        .root("/path/to/dir")
        // Set the bucket name. This is required.
        .bucket("test")
        // Set the endpoint.
        //
        // For example:
        // - "https://tos-cn-beijing.volces.com"
        // - "https://tos-cn-shanghai.volces.com"
        .endpoint("https://tos-cn-beijing.volces.com")
        // Set the region.
        .region("cn-beijing")
        // Set the access_key_id and secret_access_key.
        //
        // OpenDAL will try to load credentials from the environment if
        // credentials are not set explicitly.
        .access_key_id("access_key_id")
        .secret_access_key("secret_access_key");

    let _op: Operator = Operator::new(builder)?;

    Ok(())
}
```
