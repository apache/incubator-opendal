This service will visit the [Lakefs API](https://Lakefs.co/docs/Lakefs_hub/package_reference/hf_api) to access the Lakefs File System.
Currently, we only support the `model` and `dataset` types of repositories, and operations are limited to reading and listing/stating.

Lakefs doesn't host official HTTP API docs. Detailed HTTP request API information can be found on the [`Lakefs_hub` Source Code](https://github.com/Lakefs/Lakefs_hub).

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
- [ ] ~~presign~~

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Configuration

Use [`crate::LakefsConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Examples

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_lakefs::Lakefs;

#[tokio::main]
async fn main() -> Result<()> {
    // Create Lakefs backend builder
    let mut builder = Lakefs::default()
        // set the type of Lakefs endpoint
        .endpoint("https://whole-llama-mh6mux.us-east-1.lakefscloud.io")
        // set the id of Lakefs repository
        .repository("sample-repo")
        // set the branch of Lakefs repository
        .branch("main")
        // set the username for accessing the repository
        .username("xxx")
        // set the password for accessing the repository
        .password("xxx");

    let op: Operator = Operator::new(builder)?;

    let stat = op.stat("README.md").await?;
    println!("{:?}", stat);
    Ok(())
}
```
