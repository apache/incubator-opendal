- Proposal Name: `compose_api`
- Start Date: 2026-08-28
- RFC PR: [apache/opendal#8188](https://github.com/apache/opendal/pull/8188)
- Tracking Issue: [apache/opendal#8167](https://github.com/apache/opendal/issues/8167)

# Summary

Add `Operator::compose` and `Composer` to create one object from an ordered
sequence of existing whole objects. Services advertise support through
`Capability::compose` and implement the operation with their own composition
mechanism.

`Composer` accepts inputs incrementally and commits the destination from
`close`. Its raw implementation owns scheduling, backpressure, and completion
state under the `concurrent` limit in `OpCompose`. Provider request limits and
physical part sizes remain internal, and composition never modifies its input
objects.

# Motivation

Applications commonly join uploaded parts, compaction outputs, log segments,
and generated fragments into one object. Reading every source through the
client and uploading the bytes again adds bandwidth, memory use, and latency
when the storage service can assemble the destination internally.

`Writer::copy_from` supports ranges and interleaving copied data with
client-provided bytes. Compose communicates one narrower intent: concatenate an
ordered sequence of complete existing objects. This intent maps to GCS
`ComposeObject`, S3 multipart
`UploadPartCopy`, and Azure Blob `Put Block From URL` followed by
`Put Block List`. Those mechanisms use different request limits and physical
work units, so OpenDAL needs one semantic operation with service-owned
planning.

# Guide-level explanation

Compose complete objects in input order:

```rust
let metadata = op
    .compose(["parts/0", "parts/1", "parts/2"], "result")
    .await?;
```

The result contains the same bytes as reading each input through the same
`Operator` and writing those complete byte sequences to the destination in the
same order. Inputs and the destination belong to the same operator namespace.
Duplicate inputs are allowed. The input sequence must be non-empty, and the
destination must not also be an input.

Composition preserves every input. It does not expose a delete-sources option.
It accepts whole objects only; ranges and client-provided bytes remain part of
`Writer::copy_from` and `Writer::write`.

## Input identity

Path strings cover the common case. `ComposeInput` adds identity requirements
without introducing a source-specific API:

```rust
use opendal::ComposeInput;

let inputs = [
    ComposeInput::new("parts/0").with_version(version_0),
    ComposeInput::new("parts/1").with_if_match(etag_1),
];

let metadata = op
    .compose_with(inputs, "result")
    .if_not_exists(true)
    .content_type("application/octet-stream")
    .concurrent(8)
    .await?;
```

`version` selects one source revision. `if_match` requires the selected source
to have the supplied ETag. `if_not_changed` accepts `Metadata` and uses its
version when version identity is available, otherwise its ETag. Explicit
identity requirements must agree with `if_not_changed`.

Source metadata does not implicitly become destination metadata. Destination
metadata and preconditions come from `ComposeOptions` and follow the matching
`WriteOptions` contracts.

## Incremental composition

`Composer` accepts one input at a time:

```rust
let mut composer = op
    .composer_with("result")
    .concurrent(8)
    .await?;

composer.compose("parts/0").await?;
composer
    .compose(ComposeInput::new("parts/1").with_version(version_1))
    .await?;

let metadata = composer.close().await?;
```

`compose` may schedule backend work before returning. A successful return means
the input was accepted in sequence; pending work may still be running. When the
concurrency window is full, `compose` waits for progress and provides
backpressure. `close` seals the sequence, drains pending work, commits the
destination, and returns its metadata. `Operator::compose` performs these steps
for an iterator. Closing a composer that accepted no inputs returns
`ErrorKind::ConfigInvalid` without creating the destination.

`concurrent` defaults to `1` and is a best-effort upper bound on independent
backend tasks owned by the composer. A backend that uses one atomic request may
ignore values greater than `1`. Provider limits such as GCS's per-request
source count do not constrain the public input count; the service builds legal
request trees and chooses legal physical ranges and part sizes. Compose has no
`chunk` option.

An execution error makes the composer terminal. Dropping a composer,
cancelling an operation, or receiving an error may leave multipart uploads,
uncommitted blocks, or intermediate objects. OpenDAL does not expose `abort` or
guarantee cleanup of this provider-side state.

# Reference-level explanation

## Public API

```rust
#[non_exhaustive]
pub struct ComposeInput {
    pub path: String,
    pub version: Option<String>,
    pub if_match: Option<String>,
    pub if_not_changed: Option<Metadata>,
}

pub trait IntoComposeInput: Send + Sync + Unpin {
    fn into_compose_input(self) -> ComposeInput;
}

impl Operator {
    pub async fn compose<I, D>(&self, inputs: I, to: &str) -> Result<Metadata>
    where
        I: IntoIterator<Item = D>,
        D: IntoComposeInput;

    pub fn compose_with<I, D>(
        &self,
        inputs: I,
        to: &str,
    ) -> FutureCompose<impl Future<Output = Result<Metadata>>>
    where
        I: IntoIterator<Item = D>,
        D: IntoComposeInput;

    pub async fn composer(&self, to: &str) -> Result<Composer>;

    pub fn composer_with(
        &self,
        to: &str,
    ) -> FutureComposer<impl Future<Output = Result<Composer>>>;
}

impl Composer {
    pub async fn compose(&mut self, input: impl IntoComposeInput) -> Result<()>;
    pub async fn close(&mut self) -> Result<Metadata>;
}
```

`ComposeInput`, `&str`, `String`, and `Entry` implement `IntoComposeInput`.
`Entry` preserves its path and selects its version when present. The API has no
iterator methods on `Composer`; run-to-completion iterator handling belongs to
`Operator::compose`.

`ComposeOptions` contains the destination metadata and precondition fields
applicable from `WriteOptions`, plus `concurrent`. It excludes `append` and
`chunk`. Public option lowering normalizes `concurrent` with `max(1)` and stores
it directly in `OpCompose`.

## Raw API

Add `OpCompose` and a stateful `oio::Compose` operation:

```rust
pub trait Compose: Unpin + Send + Sync {
    fn compose<'a>(
        &'a mut self,
        path: &'a str,
        args: OpRead,
    ) -> impl Future<Output = Result<()>> + MaybeSend + 'a;

    fn close(
        &mut self,
    ) -> impl Future<Output = Result<Metadata>> + MaybeSend;
}

pub trait Service {
    type Composer: oio::Compose;

    fn compose(
        &self,
        ctx: &OperationContext,
        to: &str,
        args: OpCompose,
    ) -> Result<Self::Composer>;
}
```

The public `Composer` converts `IntoComposeInput` into `ComposeInput`,
normalizes its path, resolves `if_not_changed`, and lowers the selected version
and source identity preconditions into `OpRead`. Raw composition reuses
`OpRead`, as `oio::Write::copy_from` does; ranges and reader execution options
remain separate and are never passed to Compose.

`OpCompose` contains destination metadata, destination preconditions, and
`concurrent`. The raw composer owns the in-flight queue, applies backpressure,
records completion state in input order, and commits from `close`. It exposes
neither progress nor cleanup methods.

## Service execution

`Capability::compose` reports that the composed service stack implements
Compose. The raw composer may issue S3 multipart copy requests, stage Azure
blocks, or build an ordered GCS composition tree. It hides provider per-request
source, part, and block limits from callers. Intermediate names must not collide
with caller paths or look like a committed destination.

OpenDAL does not lower Compose to read/write or `Writer::copy_from`. An
unsupported service returns `ErrorKind::Unsupported`, and an execution error
never retries the request through another operation. The service implementation
creates or replaces the public destination only from `close`; its intermediate
work is not part of the destination contract.

Layers may forward composition only when they preserve paths, source bytes,
input identity, and destination write semantics. A layer that changes any of
those clears `Capability::compose`, so calls through that composed stack return
`ErrorKind::Unsupported`.

Destination visibility, replacement, and error behavior follow the service's
ordinary write contract. Composition never deletes caller-provided inputs.

# Drawbacks

The raw `Service` and layer interfaces gain another stateful operation. Service
execution can leave provider-side residual state. Services that cannot
implement the complete contract do not support the operation.

# Rationale and alternatives

`Composer::compose` follows `Deleter::delete`: the operation object accepts one
ordered input at a time and `close` finalizes accepted work. `ComposeInput`
provides the public per-input model, while `Composer` lowers it before crossing
the raw boundary, matching `DeleteInput` and `OpDelete`. The raw operation
reuses `OpRead` because it already represents source revision and identity
requirements. Compose does not reuse public `ReadOptions`, which would expose
ranges and reader execution controls outside its contract.

`concurrent` belongs in `OpCompose` because the service constructs the raw
composer that owns scheduling. A separate `OpComposer` would contain no other
configuration, and a separate scalar argument would split one operation's
construction state across parameters.

Source deletion is a separate destructive operation with independent failure
semantics. Byte ranges would turn Compose into another write stream and prevent
whole-object service planning. Explicit cleanup cannot be portable because the
relevant residual state and lifecycle differ by provider.

OpenDAL does not use `Writer::copy_from` as an automatic substitute. That path
may transfer every source byte through the client and has materially different
cost and performance. Callers can choose it explicitly when they need its
broader streaming semantics.

# Compatibility and migration

The Operator API and input type are additive. Services without support set
`Capability::compose` to `false`, and Compose returns
`ErrorKind::Unsupported`. Raw services and layers add the `Composer` associated
type and forwarding method, using `()` when they do not implement the
operation.

`Writer::copy_from`, `Operator::copy`, and `Copier` keep their contracts. The
blocking API and language bindings do not change in version 1.

# Prior art

[GCS `ComposeObject`](https://cloud.google.com/storage/docs/composing-objects)
concatenates an ordered list of whole objects. S3 multipart uploads accept
[`UploadPartCopy`](https://docs.aws.amazon.com/AmazonS3/latest/API/API_UploadPartCopy.html)
results and commit them in part-number order. Azure Blob stages server-side
copies with
[`Put Block From URL`](https://learn.microsoft.com/en-us/rest/api/storageservices/put-block-from-url)
and commits their order with
[`Put Block List`](https://learn.microsoft.com/en-us/rest/api/storageservices/put-block-list).
