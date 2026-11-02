## Capabilities

Depending on its configuration and the backing system, this service can expose:

- [ ] create_dir
- [x] stat
- [x] read
- [x] write
- [ ] delete
- [ ] ~~list~~
- [ ] ~~copy~~
- [ ] ~~rename~~
- [ ] ~~presign~~

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Configuration

Use [`crate::VercelArtifactsConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Example

### Via Builder

```no_run
use anyhow::Result;
use opendal_service_vercel_artifacts::VercelArtifacts;
use opendal_core::Operator;

#[tokio::main]
async fn main() -> Result<()> {
    // create backend builder
    let mut builder = VercelArtifacts::default()
        .access_token("xxx")
        .endpoint("https://my-vercel-api.example.com")
        .team_id("team_xxx");

    let op: Operator = Operator::new(builder)?;
    Ok(())
}
```
