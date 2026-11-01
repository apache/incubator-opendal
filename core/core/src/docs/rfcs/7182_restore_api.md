- Proposal Name: (`restore_api`)
- Start Date: 2025-02-04
- RFC PR: [apache/opendal#7182](https://github.com/apache/opendal/pull/7182)
- Tracking Issue: [apache/opendal#4321](https://github.com/apache/opendal/issues/4321)

# Summary

Add a first-class `restore` operation that makes a recoverable object version
current again. Implement the operation for versioned S3 buckets first.

# Motivation

Storage services retain deleted data through different mechanisms. S3 places a
delete marker above existing object versions, while other services can expose a
separate soft-delete lifecycle. OpenDAL users currently need service-specific
recovery code even when each service can restore the object.

A dedicated operation gives applications one recovery contract and lets each
service implement that contract with its native mechanism. The operation also
keeps restore-specific capabilities and conditions independent from general
copy or delete capabilities.

# Guide-level explanation

Use [`crate::Operator::restore`] to restore the latest recoverable state for a
path:

```rust
use opendal_core::{Operator, Result};

async fn restore(op: Operator) -> Result<()> {
    op.restore("path/to/file").await?;
    Ok(())
}
```

The operation has these path-level semantics:

- It succeeds when the path is already live.
- It restores the service's latest recoverable deletion state.
- It returns `ErrorKind::NotFound` when neither a live object nor a recoverable
  deletion state exists.
- It returns `ErrorKind::Unsupported` when the service does not implement
  restoration.

Use [`crate::Operator::restore_with`] to promote a specific historical version:

```rust
use opendal_core::{Operator, Result};

async fn restore_version(op: Operator, version: &str) -> Result<()> {
    op.restore_with("path/to/file")
        .version(version)
        .await?;
    Ok(())
}
```

Restoring a version creates a new current version from the selected historical
version. By default, it can replace the current value at the path.

Add `if_not_exists(true)` when a recovery workflow must not overwrite an object
that another writer recreated after the version was selected:

```rust
use opendal_core::{Operator, Result};

async fn restore_version_if_absent(op: Operator, version: &str) -> Result<()> {
    op.restore_with("path/to/file")
        .version(version)
        .if_not_exists(true)
        .await?;
    Ok(())
}
```

`if_not_exists` requires an explicit version. The operation returns
`ErrorKind::ConfigInvalid` when the option is set without one and
`ErrorKind::ConditionNotMatch` when the path already exists.

Applications must inspect the effective capabilities before relying on optional
restore behavior:

```rust,ignore
let capability = op.info().full_capability();

if capability.restore_with_version {
    op.restore_with("path/to/file")
        .version("version-id")
        .await?;
}
```

# Reference-level explanation

## Public API

The async operator provides:

```rust,ignore
impl Operator {
    pub async fn restore(&self, path: &str) -> Result<()>;

    pub fn restore_with(
        &self,
        path: &str,
    ) -> FutureRestore<impl Future<Output = Result<()>>>;

    pub async fn restore_options(
        &self,
        path: &str,
        opts: impl Into<RestoreOptions>,
    ) -> Result<()>;
}
```

`FutureRestore` exposes `version` and `if_not_exists` setters. The blocking
operator provides `restore` and `restore_options` with the same contract.

`RestoreOptions` contains:

```rust
pub struct RestoreOptions {
    pub version: Option<String>,
    pub if_not_exists: bool,
}
```

OpenDAL normalizes the path and rejects directory paths before dispatching the
operation.

## Raw operation

The raw API adds `Operation::Restore`, `OpRestore`, and `RpRestore`.
`Service::restore` receives the path and `OpRestore`. Services that do not
support restoration return `ErrorKind::Unsupported`.

`ServiceDyn::restore_dyn` is a required method, matching the other dynamic
service operations. All layers must either implement restore-specific behavior
or forward the call to the inner service.

## Capabilities

`Capability` exposes three restore flags:

```rust
pub struct Capability {
    pub restore: bool,
    pub restore_with_version: bool,
    pub restore_with_if_not_exists: bool,
}
```

- `restore` indicates support for the base operation.
- `restore_with_version` indicates support for selecting a historical version.
- `restore_with_if_not_exists` indicates support for conditional version
  promotion.

The correctness and capability-check layers reject unsupported restore options
before dispatch. Capability override and simulation layers can independently
disable each flag.

## S3 implementation

S3 restoration requires bucket versioning.

For `restore(path)`, the S3 service requests the first entry from
`ListObjectVersions` for the exact object key:

1. If the current entry is a live version, the operation succeeds without a
   write.
2. If the current entry is a delete marker, the service deletes that marker by
   version ID and succeeds.
3. If neither entry exists for the exact key, the operation returns
   `ErrorKind::NotFound`.

Each service call removes at most one current delete marker. It does not scan or
delete older marker versions. This keeps one restore call mapped to one S3
delete-marker operation and leaves version-history policy to the caller. S3 can
[stack delete markers](https://docs.aws.amazon.com/AmazonS3/latest/userguide/ManagingDelMarkers.html),
so callers can issue another restore operation when they intend to remove the
next marker.

For `restore_with(path).version(version)`, the S3 service issues a server-side
copy from that version to the same key. The copy creates a new current version.
When `if_not_exists` is set, the service applies the destination
`If-None-Match: *` condition to the copy request.

S3 Express directory buckets do not support object versioning, so they expose
all restore capabilities as `false`.

## Error and retry behavior

Restore errors carry `Operation::Restore` context. Observability layers record
restore as a distinct operation. Retry and timeout layers apply their existing
operation policies to restore calls.

# Drawbacks

- A path-only restore maps to different native mechanisms across services.
- Services need explicit restore capability declarations even when their
  implementation reuses copy or delete requests.
- S3 path-only restore performs a list request before deleting a marker.
- One call removes only the current S3 delete marker; applications that manage
  older marker history must do so explicitly.

# Rationale and alternatives

## Extend copy with a source version

Versioned copy can promote a historical S3 version, but it does not express
path-level recovery when the caller has no version ID. It also makes recovery
capabilities depend on the broader copy contract. A first-class restore
operation supports both path-level and version-selected workflows.

## Add a separate undelete operation

`undelete` matches the terminology of some soft-delete APIs but does not match
S3's versioning model. `restore` describes the user outcome without exposing the
provider's retention mechanism.

## Remove every S3 delete marker

Scanning and removing historical markers makes one restore call perform an
unbounded number of irreversible version-history changes. The chosen contract
operates on the current marker once per call.

# Future possibilities

- Implement restore for services with native soft-delete APIs.
- Implement version-selected restore for other versioned services.
- Add batch restore after individual service contracts are established.
