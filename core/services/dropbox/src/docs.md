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

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Configuration

Use [`crate::DropboxConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

### Credentials related

#### Just provide Access Token (Temporary)

- `access_token`: set the access_token for this backend.
  Please notice its expiration.

#### Or provide Client ID and Client Secret and refresh token (Long Term)

If you want to let OpenDAL to refresh the access token automatically,
please provide the following fields:

- `refresh_token`: set the refresh_token for dropbox api
- `client_id`: set the client_id for dropbox api
- `client_secret`: set the client_secret for dropbox api

OpenDAL is a library, it cannot do the first step of OAuth2 for you.
You need to get authorization code from user by calling Dropbox's authorize url
and exchange it for refresh token.

Please refer to [Dropbox OAuth2 Guide](https://www.dropbox.com/developers/reference/oauth-guide)
for more information.

## Example

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_dropbox::Dropbox;

#[tokio::main]
async fn main() -> Result<()> {
    let mut builder = Dropbox::default()
        .root("/opendal")
        .access_token("<token>");

    let op: Operator = Operator::new(builder)?;
    Ok(())
}
```
