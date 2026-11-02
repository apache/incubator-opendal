This service provides read-only access to a Git repository at a fixed
reference, with optional Git LFS resolution.

The service resolves the configured reference lazily on the first operation and
keeps that immutable snapshot for the lifetime of the operator. It uses a
temporary Git object database and does not create or update a working tree.
`HEAD` uses a shallow clone; an explicit branch, tag, or commit uses a full
clone.

## Capabilities

This service can be used to:

- [ ] create_dir
- [x] stat
- [x] read
- [ ] write
- [ ] delete
- [x] list
- [ ] copy
- [ ] rename
- [ ] ~~presign~~

## Configuration

- `repository`: Git repository URL or local path. This is required.
- `reference`: Branch, tag, commit hash, or `HEAD`. The default is `HEAD`.
- `root`: Root path within the repository. The default is `/`.
- `username`: Username used for the Git fetch and same-origin LFS requests.
- `password`: Password or token used for the Git fetch and same-origin LFS
  requests.
- `resolve_lfs`: Resolve Git LFS pointers. The default is `true`.

## Notes

- A configured reference is resolved once. Later remote updates are not
  observed by the same operator; construct a new operator to resolve again.
- `resolve_lfs=false` returns the pointer file itself.
- LFS payloads are streamed from the composed HTTP transport. The service does
  not buffer a complete LFS object before returning the reader.
- Repository credentials are sent to the Git remote and to same-origin LFS
  requests. They are not sent to a cross-origin LFS download action. An
  action-provided `Authorization` header takes precedence.
- LFS download action state is retained only by the reader that resolved it.
  Reuse is capped at 60 seconds and at any advertised expiry minus a small
  margin. A rejected cached action is refreshed once.
- Submodule entries are not resolved. Accessing one returns `Unsupported`
  instead of panicking.
- `list` returns the immediate children of a directory. Recursive listing is
  not advertised by this service.

## Example

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_git::Git;

#[tokio::main]
async fn main() -> Result<()> {
    let builder = Git::default()
        .repository("https://huggingface.co/hf-internal-testing/tiny-random-bert")
        .reference("main")
        .root("/");

    let op: Operator = Operator::new(builder)?;
    let _ = op.stat("pytorch_model.bin").await?;

    Ok(())
}
```
