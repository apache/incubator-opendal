## Capabilities

This service can be used to:

- [x] stat
- [x] read
- [x] write
- [x] append
- [x] create_dir
- [x] delete
- [x] copy
- [ ] rename
- [x] list
- [x] presign
- [ ] blocking

## Configuration

- `root`: Set the work dir for backend.
- `bucket`: Set the container name for backend.
- `endpoint`: Set the endpoint for backend.
- `region`: Set the region for backend.
- `access_key_id`: Set the access_key_id for backend.
- `secret_access_key`: Set the secret_access_key for backend.
- `session_token`: Set the session_token for backend.
- `default_storage_class`: Set the default storage_class for backend.
- `server_side_encryption`: Set the server_side_encryption for backend.
- `server_side_encryption_aws_kms_key_id`: Set the server_side_encryption_aws_kms_key_id for backend.
- `server_side_encryption_customer_algorithm`: Set the server_side_encryption_customer_algorithm for backend.
- `server_side_encryption_customer_key`: Set the server_side_encryption_customer_key for backend.
- `server_side_encryption_customer_key_md5`: Set the server_side_encryption_customer_key_md5 for backend.
- `disable_config_load`: Disable aws config load from env.
- `enable_virtual_host_style`: Enable virtual host style.
- `disable_write_with_if_match`: Disable write with if match.

Refer to [`S3Builder`]'s public API docs for more information.

## Temporary security credentials

OpenDAL now provides support for S3 temporary security credentials in IAM.

The way to take advantage of this feature is to build your S3 backend with `Builder::session_token`.

But OpenDAL will not refresh the temporary security credentials, please keep in mind to refresh those credentials in time.

## Server Side Encryption

OpenDAL provides full support of S3 Server Side Encryption(SSE) features.

The easiest way to configure them is to use helper functions like

- SSE-KMS: `server_side_encryption_with_aws_managed_kms_key`
- SSE-KMS: `server_side_encryption_with_customer_managed_kms_key`
- SSE-S3: `server_side_encryption_with_s3_key`
- SSE-C: `server_side_encryption_with_customer_key`

If those functions don't fulfill need, low-level options are also provided:

- Use service managed kms key
    - `server_side_encryption="aws:kms"`
- Use customer provided kms key
    - `server_side_encryption="aws:kms"`
    - `server_side_encryption_aws_kms_key_id="your-kms-key"`
- Use S3 managed key
    - `server_side_encryption="AES256"`
- Use customer key
    - `server_side_encryption_customer_algorithm="AES256"`
    - `server_side_encryption_customer_key="base64-of-your-aes256-key"`
    - `server_side_encryption_customer_key_md5="base64-of-your-aes256-key-md5"`

After SSE have been configured, all requests send by this backed will attach those headers.

Reference: [Protecting data using server-side encryption](https://docs.aws.amazon.com/AmazonS3/latest/userguide/serv-side-encryption.html)

## Example

## Via Builder

### Basic Setup

```rust,no_run
use std::sync::Arc;

use anyhow::Result;
use opendal::services::S3;
use opendal::Operator;

#[tokio::main]
async fn main() -> Result<()> {
    // Create s3 backend builder.
    let mut builder = S3::default()
      // Set the root for s3, all operations will happen under this root.
      //
      // NOTE: the root must be absolute path.
      .root("/path/to/dir")
      // Set the bucket name. This is required.
      .bucket("test")
      // Set the region. This is required for some services, if you don't care about it, for example Minio service, just set it to "auto", it will be ignored.
      .region("us-east-1")
      // Set the endpoint.
      //
      // For examples:
      // - "https://s3.amazonaws.com"
      // - "http://127.0.0.1:9000"
      // - "https://oss-ap-northeast-1.aliyuncs.com"
      // - "https://cos.ap-seoul.myqcloud.com"
      //
      // Default to "https://s3.amazonaws.com"
      .endpoint("https://s3.amazonaws.com")
      // Set the access_key_id and secret_access_key.
      //
      // OpenDAL will try load credential from the env.
      // If credential not set and no valid credential in env, OpenDAL will
      // send request without signing like anonymous user.
      .access_key_id("access_key_id")
      .secret_access_key("secret_access_key");

    let op: Operator = Operator::new(builder)?.finish();

    Ok(())
}

```

### S3 with SSE-C

```rust,no_run
use anyhow::Result;
use log::info;
use opendal::services::S3;
use opendal::Operator;

#[tokio::main]
async fn main() -> Result<()> {
    let mut builder = S3::default()
      .root("/path/to/dir")
      .bucket("test")
      .region("us-east-1")
      .endpoint("https://s3.amazonaws.com")
      .access_key_id("access_key_id")
      .secret_access_key("secret_access_key")
      // Enable SSE-C
      .server_side_encryption_with_customer_key("AES256", "customer_key".as_bytes());

    let op = Operator::new(builder)?.finish();
    info!("operator: {:?}", op);

    // Writing your testing code here.

    Ok(())
}
```

### S3 with SSE-KMS and aws managed kms key

```rust,no_run
use anyhow::Result;
use log::info;
use opendal::services::S3;
use opendal::Operator;

#[tokio::main]
async fn main() -> Result<()> {
    let mut builder = S3::default()
      // Setup builders
      .root("/path/to/dir")
      .bucket("test")
      .region("us-east-1")
      .endpoint("https://s3.amazonaws.com")
      .access_key_id("access_key_id")
      .secret_access_key("secret_access_key")
      // Enable SSE-KMS with aws managed kms key
      .server_side_encryption_with_aws_managed_kms_key();

    let op = Operator::new(builder)?.finish();
    info!("operator: {:?}", op);

    // Writing your testing code here.

    Ok(())
}
```

### S3 with SSE-KMS and customer managed kms key

```rust,no_run
use anyhow::Result;
use log::info;
use opendal::services::S3;
use opendal::Operator;

#[tokio::main]
async fn main() -> Result<()> {
    let mut builder = S3::default()
      // Setup builders
      .root("/path/to/dir")
      .bucket("test")
      .region("us-east-1")
      .endpoint("https://s3.amazonaws.com")
      .access_key_id("access_key_id")
      .secret_access_key("secret_access_key")
      // Enable SSE-KMS with customer managed kms key
      .server_side_encryption_with_customer_managed_kms_key("aws_kms_key_id");

    let op = Operator::new(builder)?.finish();
    info!("operator: {:?}", op);

    // Writing your testing code here.

    Ok(())
}
```

### S3 with SSE-S3

```rust,no_run
use anyhow::Result;
use log::info;
use opendal::services::S3;
use opendal::Operator;

#[tokio::main]
async fn main() -> Result<()> {
    let mut builder = S3::default()
      // Setup builders
      .root("/path/to/dir")
      .bucket("test")
      .region("us-east-1")
      .endpoint("https://s3.amazonaws.com")
      .access_key_id("access_key_id")
      .secret_access_key("secret_access_key")
      // Enable SSE-S3
      .server_side_encryption_with_s3_key();

    let op = Operator::new(builder)?.finish();
    info!("operator: {:?}", op);

    // Writing your testing code here.

    Ok(())
}
```

