## Capabilities

Depending on its configuration and the backing system, this service can expose:

- [x] create_dir
- [x] stat
- [x] read
- [x] write
- [x] delete
- [ ] list
- [x] copy
- [x] rename
- [ ] ~~presign~~

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Configuration

Use [`crate::MonoiofsConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Example

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_monoiofs::Monoiofs;

#[tokio::main]
async fn main() -> Result<()> {
    // Create monoiofs backend builder.
    let mut builder = Monoiofs::default()
        // Set the root for monoiofs, all operations will happen under this root.
        //
        // NOTE: the root must be absolute path.
        .root("/tmp");

    let op: Operator = Operator::new(builder)?;

    Ok(())
}
```
