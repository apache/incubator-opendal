As known as `abfs`, `azdls` or `azdls`.

This service will visit the [ABFS](https://learn.microsoft.com/en-us/azure/storage/blobs/data-lake-storage-abfs-driver) URI supported by [Azure Data Lake Storage Gen2](https://learn.microsoft.com/en-us/azure/storage/blobs/data-lake-storage-introduction).

## Notes

`azdls` is different from `azfile` service which used to visit [Azure File Storage](https://azure.microsoft.com/en-us/services/storage/files/).

## Capabilities

Depending on its configuration and the backing system, this service can expose:

- [x] create_dir
- [x] stat
- [x] read
- [x] write
- [x] delete
- [x] list
- [ ] copy
- [x] rename
- [ ] presign

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Configuration

Use [`crate::AzdlsConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Example

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_azdls::Azdls;

#[tokio::main]
async fn main() -> Result<()> {
    // Create azdls backend builder.
    let mut builder = Azdls::default()
        // Set the root for azdls, all operations will happen under this root.
        //
        // NOTE: the root must be absolute path.
        .root("/path/to/dir")
        // Set the filesystem name, this is required.
        .filesystem("test")
        // Set the endpoint, this is required.
        //
        // For examples:
        // - "https://accountname.dfs.core.windows.net"
        .endpoint("https://accountname.dfs.core.windows.net")
        // Set the account_name and account_key.
        //
        // OpenDAL will try load credential from the env.
        // If credential not set and no valid credential in env, OpenDAL will
        // send request without signing like anonymous user.
        .account_name("account_name")
        .account_key("account_key");

    // `Service` provides the low level APIs, we will use `Operator` normally.
    let op: Operator = Operator::new(builder)?;

    Ok(())
}
```
