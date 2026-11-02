- Proposal Name: `move_read_range_to_reader`
- Start Date: 2026-06-01
- RFC PR: [apache/opendal#7660](https://github.com/apache/opendal/pull/7660)
- Tracking Issue: [apache/opendal#7661](https://github.com/apache/opendal/issues/7661)

# Summary

Change OpenDAL's raw read contract so byte ranges are selected by the raw reader
instead of `Access::read`.

Today `Access::read(path, OpRead { range, .. })` opens one concrete range
stream. This works well for HTTP object stores, where each range read is an
independent request. It is a poor fit for services such as `fs`, `sftp`,
`hdfs`, `hdfs-native`, and `monoiofs`, where the service may keep an opened
handle and execute multiple cursor-independent range reads against it. The
current core reader planner still calls `Access::read` for every planned range,
forcing those services to repeatedly open and close handles.

This RFC keeps the `Access::read` operation, but changes its contract:

- `Access::read(path, OpRead)` creates a reusable raw `oio::Read` for the path
  and read condition set.
- `oio::Read::open(range)` opens a range-scoped byte stream.
- `oio::Read::read(range)` reads one bounded planner range into one `Buffer`.

The public `Operator::read`, `Operator::reader`, `Reader::read`,
`Reader::fetch`, and stream conversion APIs keep their existing user-facing
shape. The change is in the raw contract and the internal execution plan.

# Motivation

OpenDAL's public `Reader` is already range based and cloneable. Users can call:

```rust
let reader = op.reader("path/to/file").await?;

let head = reader.read(0..1024).await?;
let tail = reader.read(4096..8192).await?;
let parts = reader.fetch(vec![0..1024, 4096..8192]).await?;
```

However, the current implementation plans these public calls by generating new
raw range streams. `ReadGenerator::next_reader` and `ChunkedReader` both call
`Access::read(path, args.with_range(range))` for every range. For `s3`, `gcs`,
and `azblob`, this is natural because each range is a new HTTP request. For
handle based services, this means every range repeats open and seek work even
when the target data can be represented by a reusable handle.

The root cause is that `Access::read` is modeled as a transport-level range
request. OpenDAL needs a storage-neutral raw model:

- Object stores can implement a raw reader that issues an independent ranged
  request for each range operation.
- File-like services can implement a raw reader that opens once lazily and uses
  cursor-independent positioned reads.
- Services that cannot perform cursor-independent reads can keep opening one
  independent stream per range and still conform to the contract.
- In-memory or database-backed services can implement a raw reader that slices
  cached values directly.

This keeps the public range-based reader model from RFC-4382 while removing the
S3-specific assumption from the raw API.

# Guide-level explanation

Users do not need a new API.

Single reads still work:

```rust
let data = op.read("path/to/file").await?;
let part = op.read_with("path/to/file").range(1024..2048).await?;
```

Reusable readers still work:

```rust
let reader = op.reader("path/to/file").await?;

let first = reader.read(0..1024).await?;
let second = reader.read(4096..8192).await?;
let ranges = reader.fetch(vec![0..1024, 4096..8192]).await?;
```

The difference is internal. OpenDAL creates one reusable raw reader for
`reader("path/to/file")`. After that, range operations are executed by this raw
reader instead of reopening through `Access::read` for every range.

For HTTP object stores, the raw reader stores the path and read arguments, then
sends a new ranged request for each range. Behavior and cost stay the same.

For handle based services, the raw reader may lazily open the file handle once
and reuse it for subsequent range reads. A reusable handle must not be driven by
a shared seek cursor. It must use cursor-independent reads such as `pread` /
`read_at`, or fall back to independent per-range opens.

# Reference-level explanation

## Raw read contract

`Access::read` remains the raw read operation:

```rust,ignore
pub trait Access: Send + Sync + Debug + Unpin + 'static {
    type Reader: oio::Read;

    fn read(
        &self,
        path: &str,
        args: OpRead,
    ) -> impl Future<Output = Result<(RpRead, Self::Reader)>> + MaybeSend;
}
```

The meaning changes:

- `path` and `OpRead` select a path and read condition set.
- The returned `Self::Reader` is a reusable raw read executor for that path and
  condition set.
- No range is selected by `Access::read`.

`OpRead` should no longer contain `BytesRange`:

```rust,ignore
pub struct OpRead {
    if_match: Option<String>,
    if_none_match: Option<String>,
    if_modified_since: Option<Timestamp>,
    if_unmodified_since: Option<Timestamp>,
    override_content_type: Option<String>,
    override_cache_control: Option<String>,
    override_content_disposition: Option<String>,
    version: Option<String>,
}
```

`OpReader` remains an internal execution policy used by OpenDAL's public
`Reader`. It controls `concurrent`, `chunk`, `gap`, `prefetch`, and
`content_length_hint`. It must not become part of the backend `Access::read`
contract.

`ReadOptions` and `ReaderOptions` should be converted differently:

```rust,ignore
impl From<options::ReadOptions> for (BytesRange, OpRead, OpReader);
impl From<options::ReaderOptions> for (OpRead, OpReader);
```

## Reader and range stream split

The current `oio::Read` is a range-scoped byte stream. It should be split into a
range stream concept and a reusable raw reader concept.

The range stream keeps the current stateful `read(&mut self)` shape:

```rust,ignore
pub type ReadStream = Box<dyn ReadStreamDyn>;

pub trait ReadStream: Unpin + Send + Sync {
    fn read(&mut self) -> impl Future<Output = Result<Buffer>> + MaybeSend;

    fn read_all(&mut self) -> impl Future<Output = Result<Buffer>> + MaybeSend {
        async {
            let mut bufs = vec![];
            loop {
                match self.read().await {
                    Ok(buf) if buf.is_empty() => break,
                    Ok(buf) => bufs.push(buf),
                    Err(err) => return Err(err),
                }
            }
            Ok(bufs.into_iter().flatten().collect())
        }
    }
}
```

The new `oio::Read` becomes the reusable raw reader:

```rust,ignore
pub trait Read: Send + Sync + Debug + Unpin + 'static {
    fn open(
        &self,
        range: BytesRange,
    ) -> impl Future<Output = Result<(RpRead, ReadStream)>> + MaybeSend;

    fn read(
        &self,
        range: BytesRange,
    ) -> impl Future<Output = Result<(RpRead, Buffer)>> + MaybeSend {
        async {
            if range.size().is_none() {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    "read requires a bounded range",
                ));
            }

            let (rp, mut stream) = self.open(range).await?;
            let buffer = stream.read_all().await?;
            Ok((rp, buffer))
        }
    }

}
```

The exact object-safe `ReadDyn` and `ReadStreamDyn` forms should follow the
existing `oio::Read` / `ReadDyn` pattern. This RFC requires `&self` range
operations, but it does not require the type-erased reader itself to be
`Arc<dyn ReadDyn>`. Layers and read contexts may choose the ownership container
they need for wrapping and cloning.

## Range operation semantics

`open` and `read` are the two raw range operations:

- `open(range)` opens a range stream. It may accept bounded or unbounded ranges.
  It is the primitive for streaming and backpressure.
- `read(range)` reads one bounded planner range and returns exactly one
  materialized `Buffer`.

This is the adapter-level `oio::Read` contract. Native services usually
implement either `StreamRead::open` or `PositionRead::read_at`; `StreamReader`
and `PositionReader` then provide the full `oio::Read` contract.

`read` is not a syscall-style partial read. `oio::Read` implementations should
not return a short buffer just because one underlying syscall or network read
returned fewer bytes than requested. For a valid bounded planner range, `read`
should either return the complete range content or return an error.

The public `Reader::read` can still accept a large user range. The core planner
must not blindly pass that large range to raw `read`. It should choose between
`open` and planned bounded `read` calls based on `OpReader` policy, range shape,
and stream conversion needs.

The public `Reader::fetch` can keep its existing behavior:

1. Merge requested ranges according to `OpReader::gap`.
2. Split the merged ranges into bounded execution ranges.
3. Schedule those execution ranges according to `OpReader::concurrent` and
   `OpReader::prefetch`.
4. Execute concurrent `raw_reader.read(range)` calls.
5. Slice merged buffers back into the original requested ranges.

## Cursor-independent concurrency

Raw reader range operations take `&self`, so core and layers may call them
concurrently. A reusable raw reader must therefore be cursor-independent.

Backends must not implement reusable range reads by serializing access to a
shared seek cursor, for example:

```rust,ignore
let mut handle = self.handle.lock().await;
handle.seek(range.offset()).await?;
handle.read_exact(range.size()).await?;
```

This pattern is rejected because it hides a global cursor behind `&self`, makes
timeout and retry semantics harder to reason about, and defeats core
`concurrent`, `prefetch`, and `fetch` planning.

A backend should use one of these implementation models instead:

- independent per-range requests, such as HTTP range GET requests;
- positioned reads on a reusable handle, such as `pread` or `read_at`;
- independent per-range opens when the service cannot provide
  cursor-independent positioned reads.

The last option preserves correctness without gaining handle reuse. This RFC
does not require every service to optimize handle reuse immediately.

## Public operator path

`Operator::read_options` should split public options into read args, the user
range, and execution policy:

```rust,ignore
async fn read_inner(acc: Accessor, path: String, opts: ReadOptions) -> Result<Buffer> {
    let (range, op_read, op_reader) = opts.into();
    let (rp, raw_reader) = acc.read(&path, op_read).await?;
    let reader = Reader::new(acc, path, raw_reader, op_reader, rp);
    reader.read(range.to_range()).await
}
```

`Operator::reader_options` should create the reusable raw reader once:

```rust,ignore
async fn reader_inner(acc: Accessor, path: String, opts: ReaderOptions) -> Result<Reader> {
    let (op_read, op_reader) = opts.into();
    let (rp, raw_reader) = acc.read(&path, op_read).await?;
    Ok(Reader::new(acc, path, raw_reader, op_reader, rp))
}
```

For services where `Access::read` currently performs remote I/O, the returned
raw reader should be lazy. This preserves the lazy reader behavior: creating
`op.reader(path)` should not require opening the underlying data request.

## Read context

`ReadContext` should hold the reusable raw reader:

```rust,ignore
pub struct ReadContext {
    acc: Accessor,
    path: String,
    args: OpRead,
    options: OpReader,
    reader: oio::Reader,
    metadata: OnceLock<Metadata>,
}
```

`acc`, `path`, and `args` are still useful for planning, especially when
OpenDAL needs `stat` to resolve unbounded ranges for chunked reads or
`AsyncSeek` adapters. Actual raw range I/O should go through `reader.open` or
`reader.read`, not through repeated `acc.read` calls.

`Reader::metadata()` remains cache-only. The cache is updated from `RpRead`
returned by `Access::read`, `open`, or `read`. Lazy backends may return
`RpRead::default()` from `Access::read` and fill metadata after the first raw
range operation. Public `Reader::fetch` observes metadata from the planned raw
`read` calls it executes.

## Layers

Existing layers continue to wrap `Access::read`, but the wrapping point moves
from a range stream to a reusable raw reader.

### Type eraser

The type eraser should erase raw readers using the chosen object-safe container:

```rust,ignore
async fn read(&self, path: &str, args: OpRead) -> Result<(RpRead, oio::Reader)> {
    self.inner
        .read(path, args)
        .await
        .map(|(rp, r)| (rp, Box::new(r) as oio::Reader))
}
```

This example uses `Box`; the final container can change if a layer needs shared
ownership for wrapping. The contract is about `&self` range operations, not a
specific type alias.

### Complete layer

`CompleteLayer` currently reads `args.range().size()` in `Access::read` and
wraps the returned range stream. After this RFC, range size is only known when
`open` or `read` is called.

`CompleteLayer` should wrap the raw reader and validate each returned range
body:

- `open(range)` wraps the returned `ReadStream` with `CompleteReadStream`.
- `read(range)` checks the returned buffer length.

### Error context layer

`ErrorContextLayer` should wrap the raw reader and attach operation context to
every range operation:

- service
- path
- range
- already-read bytes for streams

### Retry layer

`RetryLayer` currently stores `OpRead` and advances `args.range_mut()` after
each successful stream read. After this RFC, range progress must move from
`OpRead` into range operation wrappers.

The new retry model should be:

- retry `Access::read(path, OpRead)` while creating the reusable raw reader;
- retry `raw_reader.open(range)` when opening a range stream fails before bytes
  are returned;
- wrap the returned `ReadStream` and track bytes already emitted;
- if a stream read fails with a temporary error, reopen the same raw reader with
  the remaining range and continue;
- retry `read(range)` as an exact bounded range operation.

Because `read` is an exact bounded range operation, retrying it is idempotent
for the same path and read condition set.

### Correctness check layer

`CorrectnessCheckLayer` can keep checking `read_with_*` capabilities in
`Access::read`, because those options still belong to the path and read
condition set.

## Presign read

Presigned reads still describe one concrete request, so they still need a range.
This range should not move back into `OpRead`. Instead,
`PresignOperation::Read` should carry range separately:

```rust,ignore
pub enum PresignOperation {
    Read {
        range: BytesRange,
        args: OpRead,
    },
    // ...
}
```

`presign_read_options` converts `ReadOptions` into `(BytesRange, OpRead,
OpReader)` and discards `OpReader`.

## Backend implementation model

### HTTP object stores

Services like `s3`, `gcs`, `azblob`, `oss`, and `http` should return a raw
reader that stores `core`, `path`, and `OpRead`.

Each `open(range)` sends the same request that the current `Access::read` sends
today. For example, S3's current logic:

```rust,ignore
let resp = self.core.s3_get_object(path, args.range(), &args).await?;
```

becomes reader logic:

```rust,ignore
let resp = self.core.s3_get_object(&self.path, range, &self.args).await?;
```

This keeps object-store behavior unchanged.

### Memory and database-like services

Services that already have the data content in memory can implement
`StreamRead` by opening a stream over the selected slice. Their
`StreamReader` adapter will provide bounded `read(range)`.

### Handle based services

`fs`, `hdfs`, `hdfs-native`, `sftp`, and `monoiofs` can reuse opened handles only when
their implementation can perform cursor-independent reads:

- `fs`: lazily open a file and use platform positioned reads.
- `hdfs`: reuse a handle only if the client API supports cursor-independent
  positioned reads.
- `hdfs-native`: store `FileReader` and use cursor-independent range APIs.
- `monoiofs`: reuse its existing worker model that already performs `read_at`.
- `sftp`: reuse a handle only if the client API supports concurrent positioned
  reads for that handle.

Services that cannot provide cursor-independent positioned reads should keep
opening an independent range stream in `open(range)`. They remain correct and
can still benefit from the new core contract later when the service gains an
appropriate positioned-read primitive.

## Compatibility and migration

This RFC does not change public OpenDAL APIs. Existing users of `Operator` and
public `Reader` keep the same user-facing behavior.

The raw API is breaking:

- third-party `Access` implementations must remove range handling from
  `Access::read`;
- current range-scoped readers must become `ReadStream`;
- services must return reusable raw readers;
- layers must wrap reusable raw readers and range operation results.

Migration can be staged mechanically:

1. Introduce `ReadStream` and the new raw `Read` trait shape.
2. Move `BytesRange` out of `OpRead`.
3. Update type erasure and core read context.
4. Convert object-store services by moving current `Access::read` request logic
   into `open(range)`.
5. Convert memory-like services by slicing in `read(range)`.
6. Convert handle based services first with independent per-range opens, then
   add positioned-read optimization where supported.
7. Rewrite `RetryLayer` around range operation wrappers.

## Validation

The implementation should be considered complete when:

- behavior tests for existing read, reader, stream, seek, and fetch paths still
  pass;
- a service test or unit test proves `op.reader(path)` does not perform data I/O
  before the first range operation;
- a handle based service test proves repeated planned range reads do not reopen
  the handle when positioned reads are supported;
- retry tests cover `open(range)`, `ReadStream` progress retry, and
  `read(range)`;
- complete layer tests cover short buffers from `read`.

# Drawbacks

- This is a breaking raw API change. Third-party `Access` implementations must
  be updated.
- It touches many services because every `Access::read` implementation currently
  receives a range.
- The rename from range-scoped `oio::Read` to `oio::ReadStream` is large but
  necessary to avoid overloading one trait with two meanings.
- `RetryLayer` must be redesigned around range operation progress instead of
  `OpRead.range_mut()`.
- Handle reuse only improves services that can provide cursor-independent
  positioned reads. Other handle based services remain correct but may keep the
  old per-range open cost.

# Rationale and alternatives

## Keep `read(range)` as an exact bounded range operation

`read(range)` is kept as a raw primitive because it allows backends such as `fs`
to perform concurrent positioned reads into buffers. It is intentionally not a
syscall-style `read_at(offset, max_size)` that may return a short buffer.

Partial-read syscall semantics would push fill loops, EOF handling, retry
progress, and request sizing into core and layers. OpenDAL already has
`open(range)` for streaming and backpressure. The raw `read(range)` primitive
should instead express a complete, core-planned, bounded range read.

## Add `Access::open_read_at`

Rejected. It adds a parallel backend operation for an execution optimization.
OpenDAL's raw operation surface should stay centered on semantic operations:
`stat`, `read`, `write`, `list`, `delete`, `copy`, and `rename`.

## Add a public capability for `read_at`

Rejected. Users do not get a new public operation. Whether a backend reuses an
opened handle is an internal execution detail. Public `Capability` should only
describe user-visible behavior.

## Allow shared cursor with internal locking

Rejected. Serializing `seek + read` behind a lock preserves cursor state inside
a supposedly concurrent raw reader. It makes timeout and retry behavior harder
to reason about and hides backend limitations from core planning. Backends that
cannot provide cursor-independent range reads should use independent per-range
opens instead.

## Add optional `read_at` to range streams

Rejected. A range stream is already scoped to one byte range. Deriving other
ranges from it gives one stream two responsibilities and makes layer wrapping
harder. The reusable raw reader should be the place that knows how to execute
additional ranges.

## Add `ReadPlan` beside `Access::read`

Rejected. It improves execution planning but keeps the old range-scoped
`Access::read` as the primary contract. This RFC fixes the raw contract instead
of adding a second planning hook.

# Prior art

## RFC-4382 Range Based Read API

RFC-4382 moved OpenDAL's public `Reader` toward stateless range reads. It also
noted that returning a `Reader` should allow storage services like `fs` to
reduce extra `open` syscalls, and listed future `read_ranges` and native
`read_at` support.

The current raw contract did not fully realize that intent because range
selection stayed in `Access::read`. This RFC keeps RFC-4382's public direction
and moves the raw contract to the place where handle reuse and batch range reads
can be implemented.

## object_store

`object_store` exposes both single-range and multi-range reads through
`get_range` and `get_ranges`. OpenDAL's public `Reader::fetch` serves a similar
user-facing purpose. The accepted raw contract does not expose a batch
primitive; public fetch is planned by core and executed through bounded raw
`read(range)` calls.

# Unresolved questions

## Fetch batching policy

The initial implementation can preserve current public behavior by batching
merged ranges according to `OpReader::concurrent`. We should benchmark whether
the default policy should prefer fewer larger bounded raw reads or one task per
merged range for different backend classes.

## Native preferred read size

This RFC keeps `OpReader` as OpenDAL's execution policy and does not introduce
backend preferred read sizes. A later RFC can revisit service-provided execution
hints if we have enough benchmark evidence.

## Handle based benchmark target

The biggest expected benefit is for services where opening a handle has network
or process coordination cost, such as `sftp` and HDFS-like services. The exact
benchmark target and workload should be selected before implementation starts.

# Future possibilities

- Native multi-range HTTP requests for services that support them.
- Platform-specific positioned read implementations for `fs`.
- Better `fetch` batching heuristics based on workload and service class.
- Integration with future completion-based I/O support.
