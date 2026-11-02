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

**Note**: As for [Known Limitations - FoundationDB](https://apple.github.io/foundationdb/known-limitations), keys cannot exceed 10,000 bytes in size, and values cannot exceed 100,000 bytes in size. Errors will be raised by OpenDAL if these limits are exceeded.

## Configuration

Use [`crate::FoundationdbConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Example

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_foundationdb::Foundationdb;

#[tokio::main]
async fn main() -> Result<()> {
    let mut builder = Foundationdb::default()
        .config_path("/etc/foundationdb/foundationdb.conf");

    let op: Operator = Operator::new(builder)?;
    Ok(())
}
```
