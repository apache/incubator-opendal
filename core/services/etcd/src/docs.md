## Capabilities

Depending on its configuration and the backing system, this service can expose:

- [x] create_dir
- [x] stat
- [x] read
- [x] write
- [x] delete
- [x] list
- [ ] copy
- [ ] rename
- [ ] ~~presign~~

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Configuration

Use [`crate::EtcdConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

For server-authenticated TLS, configure an HTTPS endpoint and `ca_path`. The server certificate must be trusted by the supplied CA. For mutual TLS, also configure `cert_path` and `key_path`; these two paths must always be supplied together.

## Example

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_etcd::Etcd;

#[tokio::main]
async fn main() -> Result<()> {
    let mut builder = Etcd::default();

    // this will build a Operator accessing etcd which runs on http://127.0.0.1:2379
    let op: Operator = Operator::new(builder)?;
    Ok(())
}
```
