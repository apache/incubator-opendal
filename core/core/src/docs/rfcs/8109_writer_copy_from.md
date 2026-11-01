- Proposal Name: `writer_copy_from`
- Start Date: 2026-08-19
- RFC PR: [apache/opendal#8109](https://github.com/apache/opendal/pull/8109)
- Tracking Issue: [apache/opendal#8134](https://github.com/apache/opendal/issues/8134)

# Summary

Add `Writer::copy_from` to append a range of an existing object to an open
writer. The source path is resolved by the same `Operator` that created the
writer. Calls to `write` and `copy_from` may be interleaved in any order, and
their call order defines the destination byte stream.

For every input sequence, the result matches reading each source through the
same `Operator` with its supplied `ReadOptions` and writing those bytes in call
order. OpenDAL may use `UploadPartCopy`, `PutBlockFromURL`, or another native
operation when it preserves source conditions, destination options, ordering,
and writer completion semantics. Native execution keeps unchanged bytes inside
the storage service, while fallback streams bounded buffers through the client.
Applications avoid client staging, backend-specific multipart copy, and custom
boundary assembly.

# Motivation

OpenDAL currently has two separate data paths. `Writer` accepts bytes held by
the client, while `Operator::copy` and `Copier` create a destination from one
complete source object. Neither API can insert an existing object range into an
object that is still being constructed:

```text
write(new_header)
copy_from(old_object, body_range)
write(new_footer)
copy_from(another_object, payload_range)
close()
```

This pattern occurs when a file format rewrites its header, footer, or index
while retaining an encoded payload; when a database compacts immutable
segments into a new snapshot; when an application replaces a range by joining
an old prefix, a new patch, and an old suffix; and when distributed workers
assemble uploaded results with client-generated boundary data.

Without this operation, an application must download unchanged remote bytes
and upload them to the same storage system. For large objects, that adds client
bandwidth and memory pressure, latency, and another long data path that can
fail. Applications may instead call S3 or Azure APIs directly, but doing so
loses OpenDAL's portability and its common retry, condition, layer, and
observability semantics.

This proposal makes existing remote bytes a writer input without making a
cloud-specific copy primitive part of the public contract. Higher-level
systems describe the ordered bytes that form the destination. OpenDAL decides
which bytes can move inside the storage service and which boundary bytes must
pass through the client. The same application therefore avoids most data
round trips on a capable backend and retains correct read-to-write behavior on
every other backend.

OpenDAL also becomes the single place that handles multipart minimum sizes,
physical range splitting, source conditions, concurrent part ordering, retry,
and abort. File formats and databases do not need to reimplement those protocol
details.

# Guide-level explanation

Use `copy_from` together with `write` to construct a destination from local and
remote bytes:

```rust
let mut writer = op.writer("target").await?;

writer.write(header).await?;
writer.copy_from("source", body_start..body_end).await?;
writer.write(footer).await?;
writer.close().await?;
```

Use `copy_from_options` when the source must be read from a fixed version or
under a condition:

```rust
use opendal::options::ReadOptions;

writer
    .copy_from_options(
        "source",
        ReadOptions {
            range: (body_start..body_end).into(),
            if_match: Some(source_etag),
            ..Default::default()
        },
    )
    .await?;
```

The source path belongs to the same `Operator` as the writer. The source and
destination may still resolve to different services through a routing layer;
OpenDAL streams the source through the client when the composed service stack
cannot preserve the operation as a native copy.

`copy_from` copies content only. Source metadata does not replace destination
metadata, which remains controlled by the `WriteOptions` used to create the
writer. An empty source range is a no-op.

`copy_from` shares the writer's chunk and concurrency budgets with `write` and
applies backpressure while it schedules the range. Successful return means that
all work produced by the call has been scheduled except for the writer's single
retained logical suffix. It does not mean that the destination has been
committed. As with `write`, `close` schedules the suffix, performs the final
commit, and reports any deferred failure.

`copy_from` does not expose progress within one call. Applications split a
large logical range into multiple bounded, ordered calls of at most 5 GiB each.
Each successful return provides a checkpoint for recording progress, aborting
the writer, or attributing an error to the next subrange. OpenDAL may still
split a subrange into physical requests to satisfy service limits; those
internal part boundaries do not become public progress events.

# Reference-level explanation

## Public API

Add two methods to the asynchronous `Writer`:

```rust
impl Writer {
    pub async fn copy_from(
        &mut self,
        path: &str,
        range: impl Into<BytesRange>,
    ) -> Result<()>;

    pub async fn copy_from_options(
        &mut self,
        path: &str,
        options: ReadOptions,
    ) -> Result<()>;
}
```

`copy_from` constructs default `ReadOptions` with the supplied range.
`copy_from_options` preserves the range, version, conditions, and execution
options defined by `Operator::read_options`. `content_length_hint` remains an
execution hint and does not identify a source object.

The source path is normalized and resolved by the `WriteContext`'s composed
`Servicer` and `OperationContext`. This RFC does not accept a `Reader` or a
source `Operator` and does not attempt to determine whether two independently
constructed operators address compatible storage.

The public operation requires the existing read and write capabilities. An
unbounded range also requires OpenDAL to discover its length before writer
mutation. `Capability::write_can_copy_from` reports whether the composed
service can execute eligible ranges natively. The public operation remains
available through streaming fallback when this capability is false.

Version 1 limits each `copy_from` call to 5 GiB. A bounded or suffix range whose
declared length exceeds that limit returns an input error before writer
mutation. OpenDAL resolves the length of an unbounded range before accepting
it and applies the same limit. Applications assemble larger logical ranges with
multiple calls.

## Read equivalence and source consistency

For every accepted input, the destination bytes must equal the result of
opening the source through the same `Operator` with the supplied `ReadOptions`
and streaming that range into the writer. Native copy, boundary
materialization, and complete streaming fallback must preserve this
equivalence.

OpenDAL must not discard a source version or condition to use a native path.
Every metadata lookup, split native request, and fallback read uses the same
applicable values. Without `version` or `if_match`, `copy_from` has the same
weak consistency as an ordinary chunked read and does not add snapshot
isolation.

Native execution requires an absolute bounded range. OpenDAL may resolve a
suffix or unbounded public range with a stat that uses the same version and
applicable conditions. If it cannot safely resolve the range for native
execution, it uses streaming fallback. For an unbounded range, the metadata
lookup or the composed reader's open response must also establish the length
before destination mutation so OpenDAL can enforce the per-call limit;
otherwise the operation returns `Unsupported`. Resolution does not add snapshot
isolation: every subsequent request still uses the supplied conditions, and an
unconditional copy retains ordinary read consistency.

## Lifecycle and observability

The `Writer` owns the complete destination transaction across every `write` and
`copy_from` call. `close` commits that transaction, and `abort` cancels it.
`copy_from` does not create an independently closable or abortable copy handle.

Tracing and metrics report logical, native, boundary-materialized, and streamed
bytes so operators can distinguish accepted input from its physical transfer
path.

## Online assembly

The public `Writer` and `WriteGenerator` assemble an ordered sequence of buffers
and bounded source ranges under one chunk and concurrency budget. A
`copy_from` call drives its range until every eligible physical part has been
scheduled, so arbitrary input is not deferred without bound until `close`.

The generator retains one unscheduled logical suffix so that every scheduled
physical part is legal as a non-final part. `close` schedules the suffix as the
final part. Adjacent buffers may be combined, and contiguous ranges may be
combined when their path and read arguments match.

This retained suffix is required for arbitrary interleaving. For example, S3
requires every non-final multipart part to be at least 5 MiB. Given a 3 MiB
buffer followed by a remote range, OpenDAL reads only enough of the range to
form a legal upload part, then uses `UploadPartCopy` for the remaining legal
ranges. A backend without native range copy streams the entire range through
the normal reader and writer paths.

## Native writer contract

Extend `oio::Write` with an optional internal operation that accepts a source
path, `OpRead`, and exactly one bounded physical `BytesRange`. Following
RFC-7660, `OpRead` contains the source conditions but not the range. The raw
operation rejects ranges that are not absolute and bounded. `Ok(())` means the
writer accepted the range. Every error is an execution failure because the
native operation may already have accepted a part. The writer enters its
normal error state and remains abortable. Execution errors never trigger
streaming fallback.

`WriteGenerator` reads `Capability::write_can_copy_from` when it creates the
writer. When the capability is false or the writer appends, it uses streaming
for the complete public call. Otherwise, it uses native execution for every
eligible physical range. A service sets the capability only when every
supported `OpRead` argument can be represented by its native operation.

Multipart and block helpers schedule local uploads and remote copies in one
ordered part queue under the same upload ID, part or block numbers, completion,
and abort state. S3 and TOS map remote parts to `UploadPartCopy`; Azure Blob
maps remote blocks to `PutBlockFromURL`. A writer containing only one remote
range still creates and completes the multipart or block transaction unless a
service provides a semantically equivalent specialized path.

## Layer contract

The native operation passes through the complete `oio::Write` wrapper stack. A
writer wrapper may forward it only when the layer is an identity transform for
the source path and byte content and does not change the selected route target.
Every other layer clears `Capability::write_can_copy_from`. The public `Writer`
then opens the source through the complete composed reader stack and uses
streaming fallback, preserving the layer's read-to-write behavior.

Assembly and fallback belong to the public `Writer` and `WriteGenerator`.
Service writers only execute bounded native ranges inside their existing upload
transaction. Retry repeats the same immutable range and part number, while
tracing, metrics, and completion wrappers account for accepted logical, native,
boundary-materialized, and streamed bytes.

# Compatibility and migration

This proposal adds methods to the asynchronous `Writer`, an optional raw
writer method, and `Capability::write_can_copy_from`. It does not change the
blocking API, `write`, `write_from(Buf)`, `Operator::copy`, or `Copier`.

The capability defaults to false, so existing services remain correct through
read-to-write fallback. Layers that affect paths, routing, or bytes clear the
capability. Services can then add native support incrementally. The raw method
defaults to a terminal `ErrorKind::Unsupported` error so a false capability
claim fails visibly instead of changing execution paths.

Append writers accept `copy_from`. A service writer without a native append
transaction uses streaming fallback for the complete public call.

# Drawbacks

The writer retains one logical suffix descriptor and sometimes boundary bytes
until a later input or `close` determines the final part layout. Scheduled I/O
and its errors may remain pending until a later call or `close`.

Automatic fallback can transfer more data through the client than a user
expects from the word "copy". Metrics and tracing make that behavior visible,
but the API deliberately does not promise zero client transfer.

Every writer wrapper gains another method to forward or reject. Path and
content-transforming layers require particular care before native service
support is enabled.

# Rationale and alternatives

A closed `compose` plan cannot represent an open writer in which local and
remote inputs are discovered incrementally. Keeping the operation on `Writer`
preserves streaming and backpressure.

A public `CopySource` would duplicate path, range, version, and condition data
already represented by `ReadOptions`. A public copy policy would expose a
backend execution decision and still could not make arbitrary part boundaries
native. Path plus `ReadOptions` is the smallest source contract needed by the
same Operator.

Accepting a `Reader` would implicitly broaden the operation to cross-Operator
sources and make native compatibility depend on opaque reader and layer
identity. This RFC keeps that problem out of the initial contract.

`Access` and `Copier` operate outside an already-open destination writer and
cannot reuse its upload transaction. A separate raw subtrait or downcast would
bypass writer wrappers that enforce retry, transformation, routing, and
observability semantics.

A native-only method would fail valid input sequences that a normal read and
write can handle. Defining read-to-write equivalence makes the operation
portable while retaining native copy as an optimization.

# Prior art

S3 multipart uploads allow `UploadPart` and `UploadPartCopy` results in the same
completion list. Azure Blob supports staging blocks from a URL before
committing a block list. These APIs demonstrate the optimization but also
expose different size, condition, and transaction rules. The proposed Writer
contract normalizes those differences behind ordered byte-stream semantics.

RFC-3017 removed the previous `oio::Write::copy_from(Reader)` because owning a
stateful reader conflicted with buffering and retry. This proposal passes an
immutable path and read arguments instead, so fallback reads and native
requests can be recreated safely.

# Unresolved questions

None.

# Future possibilities

Cross-Operator sources and a blocking API can be considered separately after
the same-Operator asynchronous contract is implemented and validated. A future
service capability may permit source and destination path equality when it
guarantees that an open writer cannot affect reads until commit.
