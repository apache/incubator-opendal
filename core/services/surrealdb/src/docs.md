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

Use [`crate::SurrealdbConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

-

## Example

### Via Builder

```rust
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_surrealdb::Surrealdb;

#[tokio::main]
async fn main() -> Result<()> {
    let mut builder = Surrealdb::default()
        .root("/")
        .connection_string("ws://127.0.0.1:8000")
        .username("username")
        .password("password")
        .namespace("namespace")
        .database("database")
        .table("table")
        .key_field("key")
        .value_field("value");

    let op = Operator::new(builder)?;
    Ok(())
}
```
