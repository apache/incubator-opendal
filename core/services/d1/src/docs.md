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

Use [`crate::D1Config`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Example

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_service_d1::D1;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = D1::default()
        .token("token")
        .account_id("account_id")
        .database_id("database_id")
        .table("table")
        .key_field("key_field")
        .value_field("value_field");

    let op = Operator::new(builder)?;
    Ok(())
}
```
