## Capabilities

Depending on its configuration and the backing system, this service can expose:

- [ ] create_dir
- [x] stat
- [x] read
- [x] write
- [x] delete
- [x] list
- [x] copy
- [ ] ~~rename~~
- [ ] ~~presign~~

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Configuration

Use [`crate::SwiftConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Examples

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_swift::Swift;

#[tokio::main]
async fn main() -> Result<()> {
    // Create Swift backend builder
    let mut builder = Swift::default()
        // Set the root for swift, all operations will happen under this root
        .root("/path/to/dir")
        // set the endpoint of Swift backend
        .endpoint("https://openstack-controller.example.com:8080/v1/account")
        // set the container name of Swift workspace
        .container("container")
        // set the auth token for builder
        .token("token");

    let op: Operator = Operator::new(builder)?;

    Ok(())
}
```
