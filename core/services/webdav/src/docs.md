## Capabilities

Depending on its configuration and the backing system, this service can expose:

- [x] create_dir
- [x] stat
- [x] read
- [x] write
- [x] delete
- [x] list
- [x] copy
- [x] rename
- [ ] ~~presign~~

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Notes

Bazel Remote Caching and Ccache HTTP Storage is also part of this service.
Users can use `webdav` to connect those services.

## Configuration

Use [`crate::WebdavConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Example

### Via Builder

```rust,no_run
use anyhow::Result;
use opendal_service_webdav::Webdav;
use opendal_core::Operator;

#[tokio::main]
async fn main() -> Result<()> {
    // create backend builder
    let mut builder = Webdav::default()
        .endpoint("127.0.0.1")
        .username("xxx")
        .password("xxx");

    let op: Operator = Operator::new(builder)?;

    Ok(())
}
```
