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

## Configuration

Use [`crate::SftpConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

For security reasons, it doesn't support password login. Use SSH key-based authentication (e.g., configure your public key on the server via `ssh-copy-id` and provide the private key here).

## Example

### Via Builder

```rust,no_run
use anyhow::Result;
use opendal_service_sftp::Sftp;
use opendal_core::Operator;

#[tokio::main]
async fn main() -> Result<()> {
    let mut builder = Sftp::default()
        .endpoint("127.0.0.1")
        .user("test")
        .key("test_key");

    let op: Operator = Operator::new(builder)?;
    Ok(())
}
```
