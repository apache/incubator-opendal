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
- [ ] presign

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Configuration

Use [`crate::VercelBlobConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Example

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_vercel_blob::VercelBlob;

#[tokio::main]
async fn main() -> Result<()> {
    // create backend builder
    let mut builder = VercelBlob::default()
        // set the storage bucket for OpenDAL
        .root("/")
        // set the token for OpenDAL
        .token("you_token");

    let op: Operator = Operator::new(builder)?;

    Ok(())
}
```
