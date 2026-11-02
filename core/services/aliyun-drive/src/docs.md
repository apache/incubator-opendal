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
- [ ] presign

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Configuration

Use [`crate::AliyunDriveConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Example

### Basic Setup

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_aliyun_drive::AliyunDrive;

#[tokio::main]
async fn main() -> Result<()> {
    // Create aliyun-drive backend builder.
    let mut builder = AliyunDrive::default()
        // Set the root for aliyun-drive, all operations will happen under this root.
        //
        // NOTE: the root must be absolute path.
        .root("/path/to/dir")
        // Set the client_id. This is required.
        .client_id("client_id")
        // Set the client_secret. This is required.
        .client_secret("client_secret")
        // Set the refresh_token. This is required.
        .refresh_token("refresh_token")
        // Set the drive_type. This is required.
        //
        // Fallback to the default type if no other types found.
        .drive_type("resource");

    let op: Operator = Operator::new(builder)?;

    Ok(())
}
```
