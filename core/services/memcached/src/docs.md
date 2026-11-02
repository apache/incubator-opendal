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

Use [`crate::MemcachedConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Example

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_memcached::Memcached;

#[tokio::main]
async fn main() -> Result<()> {
    // create memcached backend builder
    let mut builder = Memcached::default()
        .endpoint("tcp://127.0.0.1:11211");
        // if you enable authentication, set username and password for authentication
        // builder.username("admin")
        // builder.password("password");

    let op: Operator = Operator::new(builder)?;
    Ok(())
}
```
