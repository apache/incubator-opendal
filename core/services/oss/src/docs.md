## Capabilities

Depending on its configuration and the backing system, this service can expose:

- [ ] create_dir
- [x] stat
- [x] read
- [x] write
- [x] delete
- [x] list
- [x] copy
- [ ] rename
- [x] presign

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Configuration

Use [`crate::OssConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

### Credential resolution

OSS tries explicit access keys, environment credentials, OIDC assume-role
credentials, and ECS instance RAM role credentials, in that order. The first
provider that returns credentials is used. In ACK RRSA deployments, OIDC
credentials therefore take precedence over the node's ECS metadata credentials.
If a provider returns no credentials or fails, the chain tries the next provider.
An OSS permission error does not trigger a switch to another credential provider.

When `role_arn` is set in the service configuration, these providers supply the
base credentials for the additional AK-based AssumeRole request.

## Example

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_oss::Oss;

#[tokio::main]
async fn main() -> Result<()> {
    // Create OSS backend builder.
    let mut builder = Oss::default()
        // Set the root for oss, all operations will happen under this root.
        //
        // NOTE: the root must be absolute path.
        .root("/path/to/dir")
        // Set the bucket name, this is required.
        .bucket("test")
        // Set the endpoint.
        //
        // For example:
        // - "https://oss-ap-northeast-1.aliyuncs.com"
        // - "https://oss-hangzhou.aliyuncs.com"
        .endpoint("https://oss-cn-beijing.aliyuncs.com")
        // Set the access_key_id and access_key_secret.
        //
        // OpenDAL will try load credential from the env.
        // If credential not set and no valid credential in env, OpenDAL will
        // send request without signing like anonymous user.
        .access_key_id("access_key_id")
        .access_key_secret("access_key_secret");

    let op: Operator = Operator::new(builder)?;

    Ok(())
}
```
