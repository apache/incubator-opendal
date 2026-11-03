- Proposal Name: `conflict_error_kind`
- Start Date: 2026-08-25
- RFC PR: [apache/opendal#8145](https://github.com/apache/opendal/pull/8145)
- Tracking Issue: [apache/opendal#0000](https://github.com/apache/opendal/issues/0000)

# Summary

Add `ErrorKind::Conflict` for operations that cannot commit against a
resource's current or transitional state. Narrow `ConditionNotMatch` to mean
that a condition supplied for the OpenDAL operation evaluated to false.

Error kind and retry status remain independent. A conflict is temporary only
when OpenDAL can safely replay the same operation at the layer that receives
the error. Conflicts that require refreshing state or restarting a multipart
session remain permanent for `RetryLayer`.

# Motivation

Conditional failure and operation conflict require different recovery. A
failed condition tells the caller that its expected resource state is no
longer current. A conflict tells the caller that the service could not commit
the request against its current state, but does not establish that the
caller's condition evaluated to false.

HTTP defines `412 Precondition Failed` for conditions that evaluate to false
and `409 Conflict` for requests that conflict with the current resource state.
Object storage services preserve this distinction while assigning different
recovery rules. For example, Amazon S3 returns `412` when an `If-Match` value
does not match, but can return `409 ConditionalRequestConflict` when a
concurrent operation interrupts a conditional write. Google Cloud Storage
uses `412 conditionNotMet` for failed generation preconditions and `409
conflict` for concurrent changes that prevent a commit. Azure Blob Storage
uses `304` or `412` for failed conditional headers, while its `409` errors
cover states such as pending copies, leases, immutability policies, and blob
type mismatches.

Status-only mappings erase these semantics. They can also select an invalid
retry scope: S3 allows a `PutObject` request to be retried after
`ConditionalRequestConflict`, but requires a new multipart upload and new
parts after the same error from `CompleteMultipartUpload`.

# Guide-level explanation

OpenDAL reports three related resource-state outcomes:

- `ConditionNotMatch` means a condition explicitly attached to the operation
  evaluated to false. This includes ETag and time conditions, `if_not_exists`,
  conditional `304` responses, and native no-replace errors translated for an
  OpenDAL conditional operation.
- `Conflict` means a valid operation could not commit because of the current or
  transitional resource state. It does not imply that retrying the same call
  is safe.
- `AlreadyExists` means an unconditional create-style operation requires a new
  resource but its target already exists.

Concurrency does not determine the error kind by itself. If two conditional
creates race and the service serializes the loser by evaluating
`if_not_exists` as false, OpenDAL returns `ConditionNotMatch`. If the service
cannot commit because another operation is in progress, OpenDAL returns
`Conflict`.

Callers handle the outcomes separately:

```rust,ignore
match result {
    Ok(value) => use_value(value),
    Err(err) if err.kind() == ErrorKind::ConditionNotMatch => {
        refresh_expected_state().await?;
    }
    Err(err) if err.kind() == ErrorKind::Conflict => {
        resolve_or_restart_operation(err).await?;
    }
    Err(err) => return Err(err),
}
```

`Error::is_temporary()` continues to control `RetryLayer`. Callers must not
infer retryability from `ErrorKind::Conflict` alone.

# Reference-level explanation

## Public error contract

`ErrorKind` gains one variant:

```rust,ignore
/// The operation conflicts with the current or transitional state of the
/// resource.
///
/// This error kind does not indicate whether retrying the same operation is
/// safe. Inspect the error's retry status for that decision.
Conflict,
```

`ConditionNotMatch` has the following normative meaning:

> A condition supplied through the OpenDAL operation evaluated to false at
> the authoritative service or native operation boundary.

Observation operations such as stat and read preserve `NotFound` when the
authoritative service reports that the target is missing. Mutation operations
interpret target absence according to the documented condition predicate. For
example, a conditional delete returns `ConditionNotMatch` when a positive
match requires a live target, while a negative match succeeds as an idempotent
no-op.

The condition can originate from a public option such as `if_match`,
`if_none_match`, or `if_not_exists`. A backend may translate a native
`AlreadyExists` result to `ConditionNotMatch` when that result authoritatively
enforces an explicit OpenDAL condition. Conditions internal to a service
implementation do not qualify unless they implement a documented OpenDAL
operation contract.

## Service error mapping

Services classify errors by native error code and operation context before
using an HTTP status fallback. The response status alone is insufficient when
a provider assigns multiple meanings to the same status.

Initial mappings follow these rules:

| Service response | OpenDAL kind | Retry status |
| --- | --- | --- |
| HTTP `304`, or `412` caused by an OpenDAL condition | `ConditionNotMatch` | Permanent |
| S3 `PreconditionFailed` | `ConditionNotMatch` | Permanent |
| S3 `ConditionalRequestConflict` or `OperationAborted` | `Conflict` | Operation-dependent |
| GCS `conditionNotMet` | `ConditionNotMatch` | Permanent |
| GCS `conflict` | `Conflict` | Operation-dependent |
| Azure `ConditionNotMet`, `SourceConditionNotMet`, or `TargetConditionNotMet` | `ConditionNotMatch` | Permanent |
| Azure state-conflict error codes | `Conflict` or a more specific existing kind | Error-code-dependent |

Provider responses that use `412` for policy, lifecycle, or resource-type
requirements retain a more suitable kind instead of becoming
`ConditionNotMatch`. An Azure `BlobAlreadyExists` response becomes
`ConditionNotMatch` when it enforces an explicit `if_not_exists` operation and
`AlreadyExists` for an unconditional create-style operation.

Unknown `409` and `412` responses use `Unexpected` unless the protocol defines
a portable fallback. The original service error code and response remain in
the error message or context.

## Retry contract

`set_temporary()` means the receiving OpenDAL layer can safely replay the same
operation method with its retained state. It does not mean that a broader
workflow could eventually succeed.

S3 `PutObject` with `ConditionalRequestConflict` is `Conflict` and temporary
because the complete request body can be replayed. S3
`CompleteMultipartUpload` with the same error is `Conflict` but is not
temporary for `Writer::close`: recovery requires `CreateMultipartUpload` and
re-uploading every part, which the close call cannot reconstruct. A future
writer implementation may mark it temporary only if it owns enough data and
state to perform that complete recovery internally.

# Compatibility and migration

The Rust `ErrorKind` enum is non-exhaustive. Bindings add `Conflict` without
renumbering existing error values. Existing conditional operations continue
to return `ConditionNotMatch` when their conditions evaluate to false.

Applications that currently interpret every `ConditionNotMatch` as a lost
conditional update become more accurate: native operation conflicts move to
`Conflict`. Applications that relied on automatic retries for those responses
retain retries only where replaying the same operation is valid.

The existing `ConditionNotMatch` name remains unchanged to avoid a cross-
binding rename. Its documentation adopts the narrower contract. A future
breaking release may rename it to the grammatically clearer
`ConditionNotMet` without changing semantics.

# Drawbacks

Every binding must expose another public error code. Service parsers also need
native-code-aware mappings and, in some cases, operation context to select
retry status. Backends with incomplete error documentation will continue to
return `Unexpected` until their behavior is verified.

Some responses previously grouped under `ConditionNotMatch` change kind. This
is an observable behavior correction for callers that branch on errors.

# Rationale and alternatives

Retry status cannot replace `Conflict`: it describes whether the current layer
can replay an operation, not why the operation failed. The same conflict kind
can require immediate replay, state refresh, session restart, or an external
configuration change.

Mapping every HTTP `409` to `Conflict` would preserve HTTP terminology but
would discard more precise provider errors such as `AlreadyExists` and would
misclassify provider-specific uses of `409`. Native error codes remain the
authoritative classification input.

Replacing `ConditionNotMatch` with `Conflict` would merge a stable
compare-and-swap outcome with failures whose recovery is service- and
operation-dependent. Returning a typed conditional outcome instead of an error
would require parallel result types across read, stat, write, copy, delete,
and rename APIs without addressing operation conflicts.

# Prior art

[RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html#section-15.5.10)
separates resource-state conflicts from failed request preconditions. The
[Amazon S3 conditional write
contract](https://docs.aws.amazon.com/AmazonS3/latest/userguide/conditional-writes.html),
[Google Cloud Storage precondition
contract](https://cloud.google.com/storage/docs/request-preconditions), and
[Azure Blob conditional header
contract](https://learn.microsoft.com/rest/api/storageservices/specifying-conditional-headers-for-blob-service-operations)
apply the same distinction while defining service-specific retry behavior.
