## Capabilities

Depending on its configuration and the backing system, this service can expose:

- [ ] ~~create_dir~~
- [x] stat
- [x] read
- [ ] ~~write~~
- [ ] ~~delete~~
- [x] list
- [ ] ~~copy~~
- [ ] ~~rename~~
- [ ] presign

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Configuration

Use [`crate::IpfsConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Example

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_ipfs::Ipfs;

#[tokio::main]
async fn main() -> Result<()> {
    // create backend builder
    let mut builder = Ipfs::default()
        // set the endpoint for OpenDAL
        .endpoint("https://ipfs.io")
        // set the root for OpenDAL
        .root("/ipfs/QmPpCt1aYGb9JWJRmXRUnmJtVgeFFTJGzWFYEEX7bo9zGJ");

    let op: Operator = Operator::new(builder)?;

    Ok(())
}
```
