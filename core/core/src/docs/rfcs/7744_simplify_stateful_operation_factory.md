- Proposal Name: `simplify_stateful_operation_factory`
- Start Date: 2026-06-16
- RFC PR: [apache/opendal#7744](https://github.com/apache/opendal/pull/7744)
- Tracking Issue: [apache/opendal#7745](https://github.com/apache/opendal/issues/7745)

# Summary

Refine the runtime service model from RFC-7740 and PR-7743 by making stateful
operation factories synchronous and reply-free.

In PR-7743, service authors implement the RPITIT `Service` trait, the operator
runtime stores a `Servicer = Arc<dyn ServiceDyn>`, and every operation receives
an explicit `&OperationContext`. This RFC keeps that structure, but changes
`read`, `write`, `list`, `delete`, and `copy` so they synchronously create
`oio::*` bodies and return only `oio::Reader`, `oio::Writer`, `oio::Lister`,
`oio::Deleter`, or `oio::Copier`.

Factory-level `RpRead`, `RpWrite`, `RpList`, `RpDelete`, and `RpCopy` are
removed. Remote I/O and observable operation replies belong to the returned
body. Unary operations such as `stat`, `presign`, `create_dir`, and `rename`
remain async because the `Service` method itself is their execution point.

# Motivation

PR-7743 already gives OpenDAL one runtime service stack: `Service` for
implementors, `ServiceDyn` for object-safe dispatch, and `OperationContext` for
layer-composed runtime resources. However, stateful operations still have the
old unary shape:

```rust,ignore
fn read(&self, ctx: &OperationContext, path: &str, args: OpRead)
    -> impl Future<Output = Result<(RpRead, Self::Reader)>> + MaybeSend;
```

That shape preserves the wrong boundary. A stateful factory creates a
long-lived body; it does not execute the operation. Most factory replies are
empty (`RpWrite`, `RpList`, `RpDelete`, `RpCopy`), so services construct values
with no semantics and layers forward them mechanically.

`RpRead` is the only non-empty stateful reply, but its metadata is observed
while the read body opens or reads a concrete range. RFC-5871 requires services
not to issue an extra request only to fill read metadata, and RFC-7660 makes
`Service::read` create a reusable raw reader while range execution happens in
`oio::Read::open` or `oio::Read::read`. Returning `RpRead` from the factory
therefore reports data from the wrong phase.

# Guide-level explanation

Operator users do not get a new public API. The change is at the raw service
boundary used by PR-7743.

Service authors implement stateful methods as local body factories:

```rust,ignore
impl Service for S3Service {
    fn read(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRead,
    ) -> Result<Self::Reader> {
        Ok(S3Reader::new(
            self.core.clone(),
            ctx.http_client().clone(),
            path,
            args,
        ))
    }

    fn write(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpWrite,
    ) -> Result<Self::Writer> {
        Ok(S3Writer::new(
            self.core.clone(),
            ctx.executor().clone(),
            path,
            args,
        ))
    }
}
```

These methods may validate local arguments, normalize paths, select an
implementation, clone service state, and capture resources from
`OperationContext`. They must not send network requests, open remote handles, or
spawn background work that represents operation progress.

The returned body owns execution:

- `oio::Read` opens range streams and performs range reads.
- `oio::Write` writes data and commits on close.
- `oio::List` fetches pages.
- `oio::Delete` queues and executes batch deletion.
- `oio::Copy` performs one-shot, block, or multipart copy.

One-request operations still use stateful bodies. Server-side copy returns an
`oio::OneShotCopier`; the copy request is sent when the copier is driven, not
when `Service::copy` returns.

# Reference-level explanation

## `Service` and `ServiceDyn`

`Service` keeps PR-7743's implementor-friendly shape: associated body types,
`impl Future + MaybeSend` for unary operations, and `OperationContext` as the
first operation parameter. Only stateful factories become synchronous:

```rust,ignore
pub trait Service: Send + Sync + Debug + Unpin + 'static {
    type Reader: oio::Read;
    type Writer: oio::Write;
    type Lister: oio::List;
    type Deleter: oio::Delete;
    type Copier: oio::Copy;

    fn info(&self) -> ServiceInfo;
    fn capability(&self) -> Capability;

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead)
        -> Result<Self::Reader>;

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite)
        -> Result<Self::Writer>;

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList)
        -> Result<Self::Lister>;

    fn delete(&self, ctx: &OperationContext)
        -> Result<Self::Deleter>;

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
        opts: OpCopier,
    ) -> Result<Self::Copier>;
}
```

`ServiceDyn` mirrors the same boundary. Stateful factories no longer allocate a
boxed future at the service-erasure boundary; the blanket adapter only boxes the
returned body:

```rust,ignore
pub trait ServiceDyn: Send + Sync + Debug + Unpin + 'static {
    fn read_dyn(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRead,
    ) -> Result<oio::Reader>;

    fn write_dyn(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpWrite,
    ) -> Result<oio::Writer>;
}

impl<S: Service + ?Sized> ServiceDyn for S {
    fn read_dyn(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRead,
    ) -> Result<oio::Reader> {
        Ok(Box::new(self.read(ctx, path, args)?) as oio::Reader)
    }
}
```

Unary operations keep PR-7743's async `Service` and boxed-future `ServiceDyn`
shape. Empty unary replies such as `RpCreateDir` and `RpRename` are not changed
by this RFC; they can become `Result<()>` in a separate cleanup.

## Contract

Successful stateful factory return means local construction succeeded. It does
not mean the remote object exists, the destination is writable, listing has
started, or copy/delete has completed.

Factories may fail for local reasons: invalid arguments, unsupported local mode
selection, path normalization failure, or missing local service configuration.
Remote errors are reported by the body method that observes them:

- read errors from `oio::Read::open` or `oio::Read::read`;
- write errors from `oio::Write::write`, `oio::Write::close`, or abort paths;
- list errors from `oio::List::next`;
- delete errors from `oio::Delete::delete` or `oio::Delete::close`;
- copy errors from `oio::Copy::next`, `oio::Copy::close`, or
  `oio::Copy::abort`.

Layer wrappers keep PR-7743's `Layer::apply_service(inner: Servicer) ->
Servicer` model. A wrapper's stateful factory may build a wrapped body, but
retry, timeout, tracing, metrics, and error-context behavior must happen around
body execution.

## Read metadata

`RpRead` remains a read body response and is no longer returned by
`Service::read`:

```rust,ignore
impl oio::Read for S3Reader {
    async fn open(
        &self,
        range: BytesRange,
    ) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let req = self.build_get_object_request(range)?;
        let resp = self.http_client.fetch(req).await?;
        let meta = parse_into_metadata(self.path.as_str(), resp.headers())?;
        Ok((RpRead::new(meta), resp.into_body()))
    }
}
```

This preserves the RFC-5871 contract: metadata is optional, describes complete
object metadata instead of the returned range, and is filled only when the
native read response already returns enough data. Services without native
read-open metadata return `RpRead::default()` from `oio::Read::open` or
`oio::Read::read`.

## Operation classification

New raw operations should be stateful factories only when the caller or a layer
must drive a body with streaming, pagination, chunking, cancellation,
backpressure, retry recovery, progress reporting, or close-time commit
semantics. Otherwise they should be unary async `Service` methods.

After this RFC, raw operations have two execution shapes:

- unary async methods return the final reply directly;
- synchronous stateful factories return a body whose methods perform I/O.

# Drawbacks

This is a raw API breaking change on top of PR-7743. Every service and layer
implementation that still returns `(Rp*, oio::Body)` must be rewritten. The
rewrite is mechanical for most backends, but it is broad.

Some remote errors move later. Services that currently perform I/O in
`Service::read`, `Service::list`, or `Service::copy` must move that work into
the returned body. Callers that treated factory success as remote success must
observe the body result instead.

# Rationale and alternatives

The root cause is that stateful operations are not unary operations. Their
stable product is a body, not a factory reply. Empty replies make the API look
uniform while forcing every service and layer to carry values with no meaning.

Keeping async factories and deleting only empty replies would remove some
boilerplate, but it would keep the wrong execution boundary. Implementations
could still perform remote I/O before returning the body.

Keeping `RpRead` on `Service::read` is rejected for the same reason. Read
metadata is observed at range execution time, not reusable-reader construction
time. Returning it from the factory conflicts with RFC-7660's reader model.

Adding a separate async `prepare` method to every body is unnecessary. Existing
body methods already define the execution points. Lazy initialization can happen
on first `open`, `read`, `next`, `write`, `delete`, `close`, or `abort`.

# Prior art

OpenDAL already uses this model inside `oio`: read owns range execution, write
owns streaming and close-time commit, list owns pagination, delete owns batch
execution, and copy owns one-shot, block, or multipart progress.

# Unresolved questions

- Whether deleting empty unary replies such as `RpCreateDir` and `RpRename`
  should be folded into the implementation or left to a follow-up.

# Future possibilities

Synchronous factories remove boxed futures from stateful body construction at
the `ServiceDyn` boundary and make `Layer::apply_service` wrappers narrower.
They also make future raw operation RFCs choose explicitly between unary async
methods and stateful bodies.
