## Capabilities

Depending on its configuration and the backing system, this service can expose:

- [ ] create_dir
- [x] stat
- [x] read
- [x] write
- [x] delete
- [x] list
- [ ] copy
- [ ] rename
- [ ] presign

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Configuration

Use [`crate::SeafileConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

Authentication and library lookup are initialized together on first use. Failed or cancelled initialization can be retried by a subsequent operation; successful initialization is shared by an operator and its clones.

## Example

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_seafile::Seafile;

#[tokio::main]
async fn main() -> Result<()> {
    // create backend builder
    let mut builder = Seafile::default()
        // set the storage bucket for OpenDAL
        .root("/")
        // set the endpoint for OpenDAL
        .endpoint("http://127.0.0.1:80")
        // set the username for OpenDAL
        .username("xxxxxxxxxx")
        // set the password name for OpenDAL
        .password("opendal")
        // set the repo_name for OpenDAL
        .repo_name("xxxxxxxxxxxxx");

    let op: Operator = Operator::new(builder)?;

    Ok(())
}
```
