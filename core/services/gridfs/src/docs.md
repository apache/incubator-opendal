## Capabilities

Depending on its configuration and the backing system, this service can expose:

- [ ] create_dir
- [x] stat
- [x] read
- [x] write
- [x] delete
- [ ] list
- [ ] copy
- [ ] rename
- [ ] ~~presign~~

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Configuration

Use [`crate::GridfsConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Example

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_gridfs::Gridfs;

#[tokio::main]
async fn main() -> Result<()> {
    let mut builder = Gridfs::default()
        .root("/")
        .connection_string("mongodb://myUser:myPassword@localhost:27017/myAuthDB")
        .database("your_database")
        .bucket("your_bucket")
        // The chunk size in bytes used to break the user file into chunks.
        .chunk_size(255);

    let op = Operator::new(builder)?;
    Ok(())
}
```
