- Proposal Name: `copier_api`
- Start Date: 2026-05-12
- RFC PR: [apache/opendal#7519](https://github.com/apache/opendal/pull/7519)
- Tracking Issue: [apache/opendal#7521](https://github.com/apache/opendal/issues/7521)

# Summary

Introduce a `Copier` API for long running copy operations.

`Operator::copy` will remain the simple run-to-completion API. `Operator::copier`
will create a stateful copy stream that users can drive step by step, observe
byte progress, and abort when needed.

# Motivation

OpenDAL currently exposes `copy` as a single future:

```rust
op.copy("from", "to").await?;
```

This works well for small objects and backends with native single-request copy.
It does not model large object copies well. Services have different large-copy
protocols:

- S3-compatible services use multipart copy for objects larger than the
  single-request copy limit.
- GCS uses rewrite tokens and may require multiple rewrite requests.
- Azure Blob can use block-copy style operations and commit the final block
  list.

Those protocols are stateful. They can make progress over many requests, expose
intermediate server-side state, and require explicit cleanup when aborted.

OpenDAL should support those protocols without exposing backend continuation
tokens to users. This matches the existing `Lister` design: pagination tokens
are stored in `PageContext` and are not part of the public API. Users drive the
operation by repeatedly asking for the next item.

# Guide-level explanation

Users who do not need control over the copy process keep using `copy`:

```rust
op.copy("from", "to").await?;
```

For large files or task-style workflows, users can create a `Copier`:

```rust
use futures::TryStreamExt;

let mut copier = op.copier("from", "to").await?;

let mut copied = 0usize;
while let Some(n) = copier.try_next().await? {
    copied += n;
    println!("copied {copied} bytes");
}
```

`Copier` is complete only after it returns `None`. A returned byte count means
the copy has made progress, but it is not a completion signal.

Users can abort an unfinished copy:

```rust
let mut copier = op.copier("from", "to").await?;

if let Err(err) = copier.try_next().await {
    let _ = copier.abort().await;
    return Err(err);
}
```

`abort` is needed because some backends create intermediate server-side state,
such as multipart uploads or uncommitted blocks. OpenDAL cannot perform async
cleanup from `Drop`, so explicit abort remains the user's responsibility when
they stop driving a copier before completion.

`copy_with` and `copier_with` share copy options:

```rust
let mut copier = op
    .copier_with("from", "to")
    .if_not_exists(true)
    .concurrent(8)
    .await?;

while copier.try_next().await?.is_some() {}
```

`if_not_exists` is a semantic option and must preserve the backend's atomic
destination condition. `concurrent` is an execution option for backends that can
split copy into server-side tasks. Backends that cannot copy concurrently can
ignore it.

# Reference-level explanation

## Public API

Add a public `Copier` type:

```rust
pub struct Copier {
    copier: Option<oio::Copier>,
    fut: Option<BoxedStaticFuture<(oio::Copier, Result<Option<usize>>)>>,
    errored: bool,
}
```

`Copier` implements:

```rust
impl Stream for Copier {
    type Item = Result<usize>;
}
```

The stream contract is:

- `Some(Ok(n))`: the copy operation made progress by `n` bytes.
- `Some(Err(err))`: the copy operation failed. Following stream polls return
  `None`.
- `None`: the copy operation is complete and the destination object has been
  committed.

`n` is a best-effort byte count for this step. Backends should return the real
increment when they can. Returning `0` is allowed when a backend can advance its
state without reporting a reliable byte delta.

`Copier` also exposes:

```rust
impl Copier {
    pub async fn abort(&mut self) -> Result<()>;
}
```

`abort` is only valid before the copier has completed or errored.

Add `Operator` APIs:

```rust
impl Operator {
    pub async fn copier(&self, from: &str, to: &str) -> Result<Copier>;

    pub fn copier_with(
        &self,
        from: &str,
        to: &str,
    ) -> FutureCopier<impl Future<Output = Result<Copier>>>;

    pub async fn copier_options(
        &self,
        from: &str,
        to: &str,
        opts: impl Into<options::CopyOptions>,
    ) -> Result<Copier>;
}
```

`Operator::copy` becomes a run-to-completion wrapper:

```rust
pub async fn copy(&self, from: &str, to: &str) -> Result<()> {
    let mut copier = self.copier(from, to).await?;

    loop {
        match copier.try_next().await {
            Ok(Some(_)) => continue,
            Ok(None) => return Ok(()),
            Err(err) => {
                let _ = copier.abort().await;
                return Err(err);
            }
        }
    }
}
```

## Options

`CopyOptions` will contain both semantic and execution options:

```rust
pub struct CopyOptions {
    pub if_not_exists: bool,
    pub concurrent: usize,
}
```

The raw layer splits this into operation arguments and copier execution options:

```rust
pub struct OpCopy {
    if_not_exists: bool,
}

pub struct OpCopier {
    concurrent: usize,
}
```

`if_not_exists` affects destination semantics. `concurrent` affects how many
copy tasks the copier may keep in flight.

## Raw API

Add `oio::Copy`:

```rust
pub type Copier = Box<dyn CopyDyn>;

pub trait Copy: Unpin + Send + Sync {
    fn next(&mut self) -> impl Future<Output = Result<Option<usize>>> + MaybeSend;

    fn abort(&mut self) -> impl Future<Output = Result<()>> + MaybeSend;
}
```

`next` drives the server-side copy state machine. `None` means that the final
commit has completed.

Add a dynamic version:

```rust
pub trait CopyDyn: Unpin + Send + Sync {
    fn next_dyn(&mut self) -> BoxedFuture<'_, Result<Option<usize>>>;

    fn abort_dyn(&mut self) -> BoxedFuture<'_, Result<()>>;
}
```

Extend `Access`:

```rust
trait Access {
    type Copier: oio::Copy;

    fn copy(
        &self,
        from: &str,
        to: &str,
        args: OpCopy,
        opts: OpCopier,
    ) -> impl Future<Output = Result<(RpCopy, Self::Copier)>> + MaybeSend;
}
```

This changes the raw `copy` operation from "perform copy now" to "create a copy
state machine". The public `Operator::copy` preserves the current user-facing
behavior by draining the returned copier to completion.

## Capability

Keep the existing copy capability fields:

```rust
pub copy: bool,
pub copy_with_if_not_exists: bool,
```

Do not add public multipart-copy size constraints. Copy takes an existing source
object as input, so users do not need to plan request part sizes. Backends should
choose request sizes internally based on source size and service-specific
limits.

Backends that only support single-request copy can still expose a `Copier`; it
will be a one-step copier that performs the existing copy operation.

## Helper implementations

Add `oio::OneShotCopier` for existing backends:

```rust
pub trait OneShotCopy: Send + Sync + Unpin + 'static {
    fn copy_once(&self) -> impl Future<Output = Result<Option<usize>>> + MaybeSend;
}
```

`OneShotCopier` calls `copy_once` once, records completion, and returns `None`
after the copy has completed.

Add `oio::MultipartCopier` for S3-compatible services:

```rust
pub trait MultipartCopy: Send + Sync + Unpin + 'static {
    fn initiate_copy(&self) -> impl Future<Output = Result<String>> + MaybeSend;

    fn copy_part(
        &self,
        upload_id: &str,
        part_number: usize,
        range: BytesRange,
    ) -> impl Future<Output = Result<MultipartPart>> + MaybeSend;

    fn complete_copy(
        &self,
        upload_id: &str,
        parts: &[MultipartPart],
    ) -> impl Future<Output = Result<()>> + MaybeSend;

    fn abort_copy(&self, upload_id: &str) -> impl Future<Output = Result<()>> + MaybeSend;
}
```

`MultipartCopier` owns the source identity, destination path, upload id,
completed parts, next range, and concurrent task queue. It uses `OpCopier` to
choose concurrency and uses backend-private limits to choose part size.

`next` should keep the internal task queue full up to `concurrent`, wait for one
copy task to complete, record the completed part, and return the completed byte
count. This makes `Copier` capable of concurrent server-side copy without
exposing multipart details to users.

Other backends can implement `oio::Copy` directly when their protocol does not
fit multipart copy. GCS rewrite is the main example.

## Backend behavior

### S3-compatible services

S3-compatible services should use single-request copy below the service limit
and multipart copy above it.

Large copy flow:

1. Stat the source to get content length and source identity, such as ETag or
   version id when available.
2. Initiate multipart upload for the destination.
3. Fill the internal task queue with byte-range upload-part-copy requests up to
   the configured concurrency.
4. Return `Some(part_size)` when a part has been copied.
5. Complete multipart upload after all parts have been copied.
6. Return `None` after complete succeeds.

If a part fails after retries, `next` returns the error. `Operator::copy`
attempts `abort`; users driving `Copier` manually can call `abort`.

Resume across processes is not part of this RFC. Upload id and completed part
state stay inside the copier object.

### GCS

GCS should use rewrite for large copy.

The copier owns the rewrite token internally. Each `next` call issues one
rewrite request:

- If the response reports more bytes rewritten than the previous response,
  return the delta.
- If the response advances the token but does not provide a reliable delta,
  return `Some(0)`.
- If the response is done, return `None`.

The rewrite token is not exposed to users.

GCS should not use XML multipart upload for copy. OpenDAL's current GCS
multipart writer uploads client-provided request bodies through `UploadPart`.
It does not provide a server-side upload-part-copy operation that copies a byte
range from an existing source object. Using XML multipart upload for copy would
therefore require OpenDAL to read source ranges through the client and upload
them again, which is no longer an original server-side copy.

`concurrent` is best-effort for GCS. The rewrite protocol is token driven and
must be advanced by repeatedly passing the token returned by the previous
request. A GCS copier can ignore `concurrent` unless GCS exposes a compatible
server-side range-copy protocol in the future.

### Azure Blob

Azure Blob can use either service-side async copy or block-from-url style copy.
For a controllable copier, block-from-url plus final block-list commit is a
better fit:

1. Stat the source.
2. Generate block ids internally.
3. Copy each source range into an uncommitted block.
4. Return `Some(block_size)` for each copied block.
5. Commit the block list and return `None`.

If Azure async copy is used for a backend-specific reason, polling progress can
still be hidden inside `next`.

### Local filesystems

Local filesystem copy can use a one-shot copier at first. A future
implementation can copy in chunks if we need progress for very large local
files.

## Conditions and consistency

`if_not_exists` must remain an atomic destination condition. Backends must not
emulate it with `stat(to)` followed by an unconditional copy for multipart copy.
If a backend cannot preserve the condition for its large-copy protocol, it must
return `Unsupported`.

Backends should pin source identity when possible. For multipart copy, copied
parts must come from the same logical source object. If the source object changes
during copy and the backend can detect it, `next` should return
`ConditionNotMatch` or another suitable error instead of completing a mixed
object.

## Error handling

`next` should return temporary errors as temporary, so existing retry layers can
retry them.

After `next` returns an error:

- Public `Copier` marks itself errored and future stream polls return `None`.
- `abort` remains available to clean server-side intermediate state.
- `Operator::copy` attempts `abort` before returning the original error.

`abort` failure must not hide the original `copy` error in `Operator::copy`.

# Drawbacks

- Adds a new public API and a new raw IO trait.
- Changes the internal raw `copy` contract.
- Requires every backend with native copy to return a copier, even when the
  backend only needs a one-shot implementation.
- Progress is best effort. Some services can return `Some(0)` for state
  advancement without reliable byte deltas.

# Rationale and alternatives

## Hide large copy inside `Operator::copy`

We could keep only `Operator::copy` and implement multipart copy or rewrite
internally. This is simple for users, but bad for very large copies. Users cannot
control progress, cancellation, or task scheduling.

`Copier` keeps `copy` simple while allowing advanced users to drive long copies.

## Expose backend continuation tokens

We could expose S3 upload ids, GCS rewrite tokens, or Azure block lists as
checkpoints. This would make cross-process resume possible, but it would leak
backend protocols into the public API.

OpenDAL's `Lister` does not expose backend continuation tokens. It exposes
semantic controls like `start_after` and keeps service tokens inside
`PageContext`. `Copier` should follow the same abstraction boundary.

## Add `close`

`Writer` needs `close` because users provide an open-ended stream of data.
`Copier` does not. The source range is known when the copier is created, and the
backend knows when the copy is complete. Therefore `next` can perform the final
commit and return `None`.

## Return total copied bytes instead of per-step bytes

Returning total bytes would require users to understand whether a value is a
snapshot, a delta, or a backend-reported total. Returning per-step bytes keeps
the stream item simple. Users who need accumulated progress can sum the returned
values.

# Prior art

- OpenDAL `Lister` hides backend pagination tokens inside `PageContext`.
- OpenDAL `Writer` hides multipart upload details and exposes `write`, `close`,
  and `abort`.
- AWS S3 multipart copy splits source objects into copied parts and completes a
  multipart upload.
- GCS rewrite uses a rewrite token internally until the rewrite operation is
  done.
- Azure Blob block operations copy source ranges into blocks and commit a final
  block list.

# Unresolved questions

- Should `Copier` provide an inherent async `next` method, or only implement
  `Stream<Item = Result<usize>>` like `Lister`?
- Should `CopyOptions::concurrent` be added in this RFC, or should the first
  implementation use backend defaults only?
- Should successful `copy` return `Metadata` in the future, following
  `write_returns_metadata`?

# Future possibilities

## Durable resume

This RFC does not expose durable checkpoints. A future RFC can add a separate
task-oriented API if OpenDAL wants cross-process copy resume.

That design should not expose raw backend tokens directly. It should define a
portable checkpoint envelope with source identity, destination identity, backend
scheme, and backend-private payload.

## Copy progress metadata

If users need richer progress, OpenDAL can replace the stream item with a
structured type:

```rust
pub struct CopyProgress {
    pub copied: usize,
    pub total: Option<u64>,
}
```

This RFC starts with `usize` to keep the public surface minimal.

## Directory copier

This RFC only covers file copy. Recursive directory copy can be built later by
combining `Lister`, `Deleter`, and `Copier` with explicit concurrency control.
