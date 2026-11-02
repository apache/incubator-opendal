This service will visit the [DBFS API](https://docs.databricks.com/api/azure/workspace/dbfs) supported by [Databricks File System](https://docs.databricks.com/en/dbfs/index.html).

## Capabilities

Depending on its configuration and the backing system, this service can expose:

- [x] create_dir
- [x] stat
- [ ] read
- [x] write
- [x] delete
- [ ] copy
- [x] rename
- [x] list
- [ ] ~~presign~~

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Configuration

Use [`crate::DbfsConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Examples

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_dbfs::Dbfs;

#[tokio::main]
async fn main() -> Result<()> {
    let mut builder = Dbfs::default()
        // set the root for Dbfs, all operations will happen under this root
        //
        // Note:
        // if the root is not exists, the builder will automatically create the
        // root directory for you
        // if the root exists and is a directory, the builder will continue working
        // if the root exists and is a folder, the builder will fail on building backend
        .root("/path/to/dir")
        // set the endpoint of Dbfs workspace
        .endpoint("https://adb-1234567890123456.78.azuredatabricks.net")
        // set the personal access token for builder
        .token("access_token");

    let op: Operator = Operator::new(builder)?;

    Ok(())
}
```
