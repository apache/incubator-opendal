## Capabilities

This service can be used to:

- [x] stat
- [x] read
- [ ] write
- [ ] append
- [ ] create_dir
- [ ] delete
- [ ] copy
- [ ] rename
- [x] list
- [ ] scan
- [ ] presign
- [ ] blocking

# Configuration

This service will look into the OCI default configuration at `~/.oci/config`,
please follow [OCI Docs](https://docs.oracle.com/en-us/iaas/Content/API/Concepts/sdkconfig.htm#SDK_and_CLI_Configuration_File) to setup your configuration file.

- `root`: Set the work dir for backend.
- `bucket`: Set the container name for backend.

Refer to [`OciOsBuilder`]'s public API docs for more information,
and [Oracle IaaS API docs](https://docs.oracle.com/en-us/iaas/api/) for API Detail.

# Example

## Via Builder

```rust
use std::sync::Arc;

use anyhow::Result;
use opendal::services::OciOs;
use opendal::Operator;

#[tokio::main]
async fn main() -> Result<()> {
    // Create OciOs backend builder.
    let mut builder = OciOs::default();
    // Set the root for oci os, all operations will happen under this root.
    //
    // NOTE: the root must be absolute path.
    builder.root("/path/to/dir");
    // Set the bucket name, this is required.
    builder.bucket("test");
    // Set the host.
    //
    // For example:
    // - "compat.objectstorage.us-phoenix-1.oraclecloud.com"
    builder.host("compat.objectstorage.us-phoenix-1.oraclecloud.com");
    // Set the namespace
    builder.namespace("namespace");
    // Set the access_key_id and access_key_secret.
    //
    // OpenDAL will try load credential from the env.
    // If credential not set and no valid credential in env, OpenDAL will
    // send request without signing like anonymous user.
    builder.access_key_id("access_key_id");
    builder.access_key_secret("access_key_secret");

    let op: Operator = Operator::new(builder)?.finish();

    Ok(())
}
```
