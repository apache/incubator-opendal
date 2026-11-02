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

Use [`crate::AzblobConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Examples

This example works on [Azurite](https://github.com/Azure/Azurite) for local developments.

### Start local blob service

```shell
docker run -p 10000:10000 mcr.microsoft.com/azure-storage/azurite
az storage container create --name test --connection-string "DefaultEndpointsProtocol=http;AccountName=devstoreaccount1;AccountKey=Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==;BlobEndpoint=http://127.0.0.1:10000/devstoreaccount1;"
```

### Init OpenDAL Operator

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_azblob::Azblob;

#[tokio::main]
async fn main() -> Result<()> {
    // Create azblob backend builder.
    let mut builder = Azblob::default()
        // Set the root for azblob, all operations will happen under this root.
        //
        // NOTE: the root must be absolute path.
        .root("/path/to/dir")
        // Set the container name, this is required.
        .container("test")
        // Set the endpoint, this is required.
        //
        // For examples:
        // - "http://127.0.0.1:10000/devstoreaccount1"
        // - "https://accountname.blob.core.windows.net"
        .endpoint("http://127.0.0.1:10000/devstoreaccount1")
        // Set the account_name and account_key.
        //
        // OpenDAL will try load credential from the env.
        // If credential not set and no valid credential in env, OpenDAL will
        // send request without signing like anonymous user.
        .account_name("devstoreaccount1")
        .account_key("Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==");

    // `Service` provides the low level APIs, we will use `Operator` normally.
    let op: Operator = Operator::new(builder)?;
    Ok(())
}
```
