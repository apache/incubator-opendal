## Capabilities

Depending on its configuration and the backing system, this service can expose:

- [x] create_dir
- [x] stat
- [x] read
- [x] write
- [x] delete
- [x] list
- [x] copy
- [x] rename
- [ ] presign

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Configuration

Use [`crate::PcloudConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Example

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_pcloud::Pcloud;

#[tokio::main]
async fn main() -> Result<()> {
    // create backend builder
    let mut builder = Pcloud::default()
        // set the storage bucket for OpenDAL
        .root("/")
        // set the bucket for OpenDAL
        .endpoint("[https](https://api.pcloud.com)")
        // set the username for OpenDAL
        .username("opendal@gmail.com")
        // set the password name for OpenDAL
        .password("opendal");

    let op: Operator = Operator::new(builder)?;

    Ok(())
}
```
