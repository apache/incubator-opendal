## Capabilities

Depending on its configuration and the backing system, this service can expose:

- [x] create_dir
- [x] stat
- [x] read
- [x] write
- [x] delete
- [x] list
- [ ] copy
- [x] rename
- [ ] presign

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Configuration

Use [`crate::AzfileConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Example

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_azfile::Azfile;

#[tokio::main]
async fn main() -> Result<()> {
  // Create azfile backend builder.
  let mut builder = Azfile::default()
      // Set the root for azfile, all operations will happen under this root.
      //
      // NOTE: the root must be absolute path.
      .root("/path/to/dir")
      // Set the filesystem name, this is required.
      .share_name("test")
      // Set the endpoint, this is required.
      //
      // For examples:
      // - "https://accountname.file.core.windows.net"
      .endpoint("https://accountname.file.core.windows.net")
      // Set the account_name and account_key.
      //
      // OpenDAL will try load credential from the env.
      // If credential not set and no valid credential in env, OpenDAL will
      // send request without signing like anonymous user.
      .account_name("account_name")
      .account_key("account_key");

  // `Service` provides the low level APIs, we will use `Operator` normally.
  let op: Operator = Operator::new(builder)?;

  Ok(())
}
```
