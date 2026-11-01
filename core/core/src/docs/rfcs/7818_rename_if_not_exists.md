- Proposal Name: `rename_if_not_exists`
- Start Date: 2026-06-24
- RFC PR: [apache/opendal#7818](https://github.com/apache/opendal/pull/7818)

# Summary

Extend rename with an `if_not_exists` option:

```rust
op.rename_with("staging/file", "published/file")
    .if_not_exists(true)
    .await?;
```

The existing `rename` API keeps its overwrite semantics. When
`if_not_exists` is enabled, rename succeeds only if the destination does not
exist. A destination conflict returns `ConditionNotMatch` without modifying the
source or destination.

# Motivation

OpenDAL defines `rename` as an overwrite operation. Some applications also need
an atomic publish primitive: move a completed staging file into place only when
no other writer has already published that destination.

`stat` followed by `rename` are two atomic actions.
A user might want to move a file to destination in an atomic action.

An example is that when user is trying to:

```
stat_result = stat(destination)
if !stat_result {
  rename(source, destination)
}
```

Another writer could create destination file during the critical section.

A service configuration flag is also unsuitable because it
would make the meaning of the same `rename` call depend on backend construction
rather than an explicit call-site condition.

We will follow how OpenDAL models `write` and `copy` with options:

```rust
op.write_with("path", content)
    .if_not_exists(true)
    .await?;

op.copy_with("source", "target")
    .if_not_exists(true)
    .await?;
```

Rename should follow the same public API and error model.

# Guide-level explanation

Current behavior: use `rename` when users want to overwrite a destination file when the destination file exist:

```rust
use opendal::{Operator, Result};

async fn replace(op: Operator) -> Result<()> {
    op.rename("staging/file", "published/file").await?;
    Ok(())
}
```

New API: use `rename_with(...).if_not_exists(true)` when an existing destination must be
preserved when the destination file exist:

```rust
use opendal::{ErrorKind, Operator, Result};

async fn publish(op: Operator) -> Result<()> {
    match op
        .rename_with("staging/file", "published/file")
        .if_not_exists(true)
        .await
    {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::ConditionNotMatch => Err(err),
        Err(err) => Err(err),
    }
}
```

The conditional operation has the following outcomes:

- If the destination does not exist, the source is renamed to the destination.
- If the destination exists, the operation returns `ConditionNotMatch` and
  leaves both paths unchanged.
- If the service cannot enforce the destination condition atomically, the
  operation returns `Unsupported`.
- If source and destination are the same path, the operation returns
  `IsSameFile`, matching normal rename.

Users can inspect `Capability::rename_with_if_not_exists` before enabling the
option.

Blocking users configure the same condition through `RenameOptions`:

```rust
use opendal::blocking;
use opendal::options::RenameOptions;
use opendal::Result;

fn publish(op: blocking::Operator) -> Result<()> {
    let mut options = RenameOptions::default();
    options.if_not_exists = true;
    op.rename_options("staging/file", "published/file", options)?;
    Ok(())
}
```

# Reference-level explanation

## Public API

Add `RenameOptions`:

```rust
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct RenameOptions {
    pub if_not_exists: bool,
}
```

The asynchronous operator exposes:

```rust
impl Operator {
    pub async fn rename(&self, from: &str, to: &str) -> Result<()>;

    pub fn rename_with(
        &self,
        from: &str,
        to: &str,
    ) -> FutureRename<impl Future<Output = Result<()>>>;

    pub async fn rename_options(
        &self,
        from: &str,
        to: &str,
        options: impl Into<RenameOptions>,
    ) -> Result<()>;
}
```

`rename` delegates to `rename_options` with default options. `FutureRename`
provides:

```rust
impl<F: Future<Output = Result<()>>> FutureRename<F> {
    pub fn if_not_exists(self, value: bool) -> Self;
}
```

The blocking operator follows existing blocking options APIs:

```rust
impl blocking::Operator {
    pub fn rename_options(
        &self,
        from: &str,
        to: &str,
        options: RenameOptions,
    ) -> Result<()>;
}
```

No standalone `rename_if_not_exists` method is added. The options API matches
write and copy and leaves room for future composable rename conditions.

The rename API follows the copy API at every layer:

| Layer | Copy | Rename |
| --- | --- | --- |
| Default operation | `copy` | `rename` |
| Fluent options | `copy_with(...).if_not_exists(true)` | `rename_with(...).if_not_exists(true)` |
| Options struct | `CopyOptions` | `RenameOptions` |
| Explicit options call | `copy_options` | `rename_options` |
| Raw arguments | `OpCopy::if_not_exists()` | `OpRename::if_not_exists()` |
| Capability | `copy_with_if_not_exists` | `rename_with_if_not_exists` |

This RFC does not add a public `copy_if_not_exists`-style standalone method,
because copy itself exposes the condition through its options APIs.

## Service API

`RenameOptions` converts into the raw `OpRename`:

```rust
pub struct OpRename {
    if_not_exists: bool,
}
```

The `Service::rename` signature does not change. Services inspect
`OpRename::if_not_exists()` to select their native overwrite or no-overwrite
operation.

Add a capability field:

```rust
pub struct Capability {
    pub rename: bool,
    pub rename_with_if_not_exists: bool,
}
```

`rename_with_if_not_exists` is meaningful only when `rename` is also supported.
The correctness check returns `Unsupported` before dispatch when the option is
enabled but the service does not advertise the capability.

This follows the existing pattern for write and copy, where
`RenameOptions` is always constructable and the runtime check enforces
the underlying service support. See
`correctness_check.rs` for the equivalent copy and write checks.

## Error semantics

`if_not_exists` is a destination precondition, so a destination conflict maps
to `ConditionNotMatch`. This is consistent with existing write and copy
behavior.

Backend errors caused by the native no-replace condition must be translated to
`ConditionNotMatch`, even when the backend or operating system reports a native
error such as `AlreadyExists`.

Other errors retain their normal meanings. For example, a missing source
returns `NotFound`, a directory passed where a file is required returns the
corresponding directory error, and an unsupported condition returns
`Unsupported`.

## Atomicity requirement

A service must advertise `rename_with_if_not_exists` only when the
destination-does-not-exist check and the rename itself are performed by a
single native operation. Performing a separate stat check followed by an
overwriting rename has a time-of-check to time-of-use race and does not
satisfy the capability.

When the destination already exists (the `if_not_exists` condition fails):

- The source remains at its original path.
- The destination remains unchanged.

When the rename succeeds:

- The source no longer exists at its original path.
- The destination contains the source data.

This RFC does not require rename to be crash-atomic beyond the guarantees of the
underlying service. It requires the no-replace decision to be atomic with
respect to competing destination creation.

This RFC does not require `rename` (overwrite) to be atomic. A service may
implement overwrite rename as a multi-step delete-then-rename and still
advertise `rename: true` because overwrite has no external condition to
enforce. The atomicity gate is specific to `rename_with_if_not_exists`,
where the destination existence check and the move must not race with
another writer.

A backend whose overwrite rename is implemented as delete-then-rename
may still support `rename_with_if_not_exists` when its native move
atomically rejects an existing destination: it simply skips the
pre-delete when `if_not_exists` is set. The HDFS analysis below
illustrates this pattern.

## Service analysis

### Audit scope

At the time of this RFC, 19 services advertise `Capability::rename`. The table
below audits all of them, plus S3 as the representative object store without a
native rename operation.

The "assessment" column describes whether the current backend primitive could
support `rename_with_if_not_exists`. It does not enable the capability. A
backend must still demonstrate that the destination check and move are one
atomic service operation and map destination conflicts to
`ConditionNotMatch`.

| Service | Current rename implementation | Assessment for `if_not_exists` |
| --- | --- | --- |
| `aliyun_drive` | Deletes the destination, moves by file ID, then updates the name | Not suitable as implemented; overwrite is a multi-request workflow |
| `azdls` | Sends one Data Lake Storage rename request | Strong candidate: the API supports `If-None-Match: *` on the destination |
| `azfile` | Sends one Azure Files rename request with `x-ms-file-rename-replace-if-exists: true` | Strong candidate: set the native replace header to `false` |
| `compfs` | Calls `compio::fs::rename` | Same portability constraints as `fs` |
| `dbfs` | Sends one DBFS move request | Candidate, but atomicity must be verified; DBFS rejects an existing destination |
| `dropbox` | Sends one `files/move_v2` request | Candidate, but destination-conflict and atomicity guarantees must be verified |
| `fs` | Calls `tokio::fs::rename` | No portable no-replace primitive in the current implementation |
| `gdrive` | Trashes the destination, then patches source metadata | Not suitable; overwrite is a multi-request workflow and Drive permits duplicate names |
| `goosefs` | Stats and deletes the destination, then calls the native rename RPC | Candidate if the native conflict is atomic; the current overwrite path is multi-step |
| `hdfs_native` | Mutates the destination, then calls the client rename API with an overwrite flag | Strong candidate through the native `overwrite = false` path, after removing destination pre-mutation |
| `hdfs` | Deletes the destination for normal rename, then calls native no-overwrite rename | Supported by this RFC's initial implementation |
| `koofr` | Removes the destination, then sends one move request | Candidate only if the move endpoint atomically rejects conflicts; current overwrite is multi-step |
| `monoiofs` | Calls `monoio::fs::rename` | Same portability constraints as `fs` |
| `onedrive` | Sends a PATCH move request with conflict behavior set to `replace` | Candidate through conflict behavior `fail`, subject to OneDrive variant compatibility |
| `pcloud` | Sends one `renamefile` or `renamefolder` request | Requires service-specific conflict and atomicity verification |
| `sftp` | Calls the SFTP client's rename operation | Requires protocol/server-specific verification; overwrite behavior is not portable across servers |
| `upyun` | Sends one move request using `x-upyun-move-source` | Requires service-specific conflict and atomicity verification |
| `webdav` | Sends one `MOVE` request with `Overwrite: T` | Strong candidate: RFC 4918 defines `Overwrite: F` and a failed precondition |
| `yandex_disk` | Sends one move request with `overwrite=true` | Candidate through `overwrite=false`, subject to atomicity and error-mapping verification |
| `s3` | Does not advertise rename; supports copy and delete separately | Not suitable: conditional copy followed by delete is not an atomic rename |

This audit also reveals that several existing backends implement OpenDAL's
overwrite contract by deleting the destination before a native move. That
behavior is inherently multi-step, but it does not prevent the same backend
from supporting conditional rename when its native move atomically rejects an
existing destination. The conditional capability must be based on the native
operation, not on a `stat` or delete preflight.

### HDFS

HDFS exposes rename options through `Options.Rename`. libhdfs `hdfsRename`
invokes rename with `Options.Rename.NONE`, which rejects an existing
destination. The HDFS service can therefore advertise
`rename_with_if_not_exists` and use that native operation directly.

Normal OpenDAL rename continues to provide overwrite behavior. Where the
libhdfs operation does not overwrite, the HDFS service removes an existing file
destination before invoking native rename.

The HDFS implementation must translate both an existing destination discovered
before the call and a native conflict reported by the final rename into
`ConditionNotMatch`. The final native operation remains authoritative so a
concurrent creator cannot be overwritten.

### FS

The FS service currently uses `tokio::fs::rename`. Normal rename behavior and
destination replacement differ across operating systems, and this API does not
offer a portable no-replace option.

The FS service must not advertise `rename_with_if_not_exists` with a
`metadata`-then-`rename` implementation. A future implementation may use native
primitives such as Linux `renameat2(RENAME_NOREPLACE)` and equivalent supported
operations on other platforms, with a clearly defined portability policy.

Any existing cross-platform differences in default FS overwrite behavior are
outside this RFC and should be handled separately.

`compfs` and `monoiofs` wrap the same operating-system rename model through
different runtimes, so they have the same portability constraint. SFTP also
requires caution because the effective rename behavior depends on the protocol
operation and server implementation.

### Native conditional move services

Several services expose a destination policy in the same server-side move
request:

- Azure Data Lake Storage accepts `If-None-Match: *` for rename.
- Azure Files exposes `x-ms-file-rename-replace-if-exists`.
- WebDAV defines the `Overwrite` header for `MOVE`; `F` requires the server to
  reject an existing destination.
- Yandex Disk exposes an `overwrite` query parameter.
- OneDrive exposes conflict behavior, although support differs between service
  variants and must be verified before advertising the capability.

These services are candidates for follow-up implementations. They are not
enabled by this RFC's initial HDFS change. Each follow-up must include behavior
coverage and service-specific error translation.

### Multi-step overwrite services

`aliyun_drive`, `gdrive`, `goosefs`, and `koofr` currently delete or trash an
existing destination before moving the source. `hdfs`, `hdfs_native`, and some
other filesystem-like services also perform destination preparation to retain
OpenDAL's default overwrite contract.

A multi-step overwrite implementation does not itself satisfy conditional
rename. A backend may advertise `rename_with_if_not_exists` only if it can skip
the destination mutation and invoke a native move that atomically rejects a
conflict. Where that native guarantee is absent or undocumented, the capability
must remain disabled.

### S3 and object stores without rename

S3 does not expose a native rename operation, and the S3 service does not
advertise `rename`. S3 supports conditional copy, but conditional copy followed
by delete is not an atomic rename:

- The copy may succeed while deleting the source fails.
- Other clients can observe both source and destination between operations.
- Retrying after partial completion has different semantics from native rename.

Therefore, `copy_with(...).if_not_exists(true)` plus delete must not be used to
advertise either rename capability. Other object stores with the same operation
model, including services whose `rename` implementation is an unsupported
placeholder, follow the same rule.

### Other services

`dbfs`, `dropbox`, `pcloud`, and `upyun` issue server-side move requests but the
current OpenDAL implementations do not expose enough evidence to claim the
required atomic destination condition. They remain follow-up candidates pending
verification against service guarantees and real backend behavior.

Services can adopt `rename_with_if_not_exists` independently after that
verification. A service-specific implementation may translate native error
codes, but it must preserve the common OpenDAL result and path-state contract.

# Drawbacks

The public options and capability surfaces gain new fields and a new
`FutureRename` alias.

Only a subset of rename-capable services can initially support the option.
Callers that operate across arbitrary services must inspect capability or handle
`Unsupported`.

The common error kind hides backend-specific conflict errors. This is
intentional for portable conditional-operation handling but removes some native
detail from the primary error kind.

# Rationale and alternatives

## Add standalone `rename_if_not_exists`

A separate method is easy to discover but diverges from OpenDAL's write and
copy APIs. Rename conditions belong in `RenameOptions`, and `rename_with`
provides the established fluent API.

## Change default rename behavior

Changing `rename` to reject an existing destination would break the existing
overwrite contract and callers that rely on replacement.

## Add an HDFS configuration flag

A backend configuration flag would make the meaning of a standard operation
depend on service construction. The destination condition is a per-call
requirement and should be explicit at the call site.

## Simulate with stat then rename

This cannot enforce the condition under concurrent destination creation and
must not be exposed as the native capability.

## Simulate with conditional copy then delete

This can be useful in application-specific workflows, but it is not an atomic
rename and has partial-success states. OpenDAL should not expose it under the
same contract.

# Prior art

OpenDAL already provides `if_not_exists` options for write and copy, with
`ConditionNotMatch` representing a failed destination condition.

HDFS provides `Options.Rename.NONE` and `Options.Rename.OVERWRITE`. Linux
provides `renameat2` with `RENAME_NOREPLACE`. Rust's `object_store` crate
exposes a conditional rename operation as a distinct method; OpenDAL adopts the
same semantic capability while retaining its established options-based API.

# Unresolved questions

- Which additional filesystem platforms have a suitable native no-replace
  primitive that OpenDAL can support without weakening the contract?
- Should OpenDAL eventually provide a separate, explicitly non-atomic
  copy-then-delete move helper for object stores?

Neither question blocks the HDFS implementation or the common API.

# Future possibilities

Additional rename conditions can be added to `RenameOptions` if services expose
portable native support.

The FS service can advertise `rename_with_if_not_exists` on platforms where a
native implementation is available and the project defines the desired
cross-platform capability behavior.

Applications that accept non-atomic move semantics can build a separate helper
from copy and delete without changing the guarantees of rename.
