## Capabilities

Depending on its configuration and the backing system, this service can expose:

- [ ] ~~create_dir~~
- [x] stat
- [x] read
- [ ] ~~write~~
- [ ] ~~delete~~
- [ ] ~~list~~
- [ ] ~~copy~~
- [ ] ~~rename~~
- [ ] ~~presign~~

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Notes

Only `read` and `stat` are supported. We can use this service to visit any
HTTP Server like nginx, caddy.

## Configuration

Use [`crate::HttpConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Example

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_http::Http;

#[tokio::main]
async fn main() -> Result<()> {
    // create http backend builder
    let mut builder = Http::default().endpoint("127.0.0.1");

    let op: Operator = Operator::new(builder)?;
    Ok(())
}
```
