## Capabilities

Depending on its configuration and the backing system, this service can expose:

- [ ] create_dir
- [x] stat
- [x] read
- [x] write
- [ ] delete
- [ ] list
- [ ] copy
- [ ] rename
- [ ] presign

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Notes

This service is mainly provided by GitHub actions.

Refer to [Caching dependencies to speed up workflows](https://docs.github.com/en/actions/using-workflows/caching-dependencies-to-speed-up-workflows) for more information.

To make this service work as expected, please make sure to either call `endpoint` and `token` to
configure the URL and credentials, or that the following environment has been setup correctly:

- `ACTIONS_CACHE_URL`
- `ACTIONS_RUNTIME_TOKEN`

They can be exposed by following action:

```yaml
- name: Configure Cache Env
  uses: actions/github-script@v6
  with:
    script: |
      core.exportVariable('ACTIONS_CACHE_URL', process.env.ACTIONS_CACHE_URL || '');
      core.exportVariable('ACTIONS_RUNTIME_TOKEN', process.env.ACTIONS_RUNTIME_TOKEN || '');
```

To make `delete` work as expected, `GITHUB_TOKEN` should also be set via:

```yaml
env:
  GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

## Limitations

Unlike other services, ghac doesn't support create empty files.
We provide a `enable_create_simulation()` to support this operation but may result unexpected side effects.

Also, `ghac` is a cache service which means the data store inside could
be automatically evicted at any time.

## Configuration

Use [`crate::GhacConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Example

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_ghac::Ghac;

#[tokio::main]
async fn main() -> Result<()> {
    // Create ghac backend builder.
    let mut builder = Ghac::default()
        // Set the root for ghac, all operations will happen under this root.
        //
        // NOTE: the root must be absolute path.
        .root("/path/to/dir");

    let op: Operator = Operator::new(builder)?;

    Ok(())
}
```
