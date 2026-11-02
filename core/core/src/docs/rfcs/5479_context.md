- Proposal Name: `context`
- Start Date: 2024-12-30
- RFC PR: [apache/opendal#5480](https://github.com/apache/opendal/pull/5480)
- Tracking Issue: [apache/opendal#5479](https://github.com/apache/opendal/issues/5479)

# Summary

Add `Context` in opendal to distribute global resources like http client, runtime, etc.

# Motivation

OpenDAL now includes two global resources, the `http client` and `runtime`, which are utilized by the specified service across all enabled layers.

However, it's a bit challenging for layers to interact with these global resources.

## For http client

Layers cannot directly access the HTTP client. The only way to interact with the HTTP client is through the service builder, such as [`S3::http_client()`](https://docs.rs/opendal/latest/opendal/services/struct.S3.html#method.http_client). Layers like logging and metrics do not have direct access to the HTTP client.

Users need to implement the `HttpFetcher` trait to interact with the HTTP client. However, the drawback is that users lack context for the given requests; they do not know which service the request originates from or which operation it is performing.

## For runtime

OpenDAL has the [`Execute`](https://docs.rs/opendal/latest/opendal/trait.Execute.html) for users to implement so that they can interact with the runtime. However, the API is difficult to use, as layers need to extract and construct the `Executor` for every request.

For example:

```rust
async fn read(&self, path: &str, mut args: OpRead) -> Result<(RpRead, Self::Reader)> {
    if let Some(exec) = args.executor().cloned() {
        args = args.with_executor(Executor::with(TimeoutExecutor::new(
            exec.into_inner(),
            self.io_timeout,
        )));
    }

    self.io_timeout(Operation::Read, self.inner.read(path, args))
        .await
        .map(|(rp, r)| (rp, TimeoutWrapper::new(r, self.io_timeout)))
}
```

# Guide-level explanation

So I propose to add a `Context` to OpenDAL to distribute global resources like the HTTP client and runtime.

The `Context` is a struct that contains the global resources, such as the HTTP client and runtime. It is passed to the service builder and layers so that they can interact with the global resources.

```rust
let mut ctx = Context::default();
ctx.set_http_client(my_http_client);
ctx.set_executor(my_executor);

let op = op.with_context(ctx);
```

The following API will be added:

- new struct `Context`
- `Context::default()`
- `Context::load_http_client(&self) -> HttpClient`
- `Context::load_executor(&self) -> Executor`
- `Context::update_http_client(&self, f: impl FnOnce(HttpClient) -> HttpClient)`
- `Context::update_executor(&self, f: impl FnOnce(Executor) -> Executor)`
- `Operator::with_context(ctx: Context) -> Operator`

The following API will be deprecated:

- `Operator::default_executor`
- `Operator::with_default_executor`
- `OpRead::with_executor`
- `OpRead::executor`
- `OpWrite::with_executor`
- `OpWrite::executor`
- All services builders' `http_client` API

# Reference-level explanation

We will add `Context` struct in `AccessInfo`. Every service must use `Context::default()` for `AccessInfo` and stores the same instance of `Context` in the service core. All the following usage of http client or runtime should be through the `Context` instead.

The `Context` itself is a struct wrapped by something like `ArcSwap<T>`, allowing us to update it atomically.

The layers will switch to `Context` to get the global resources instead of `OpRead`.

We no longer need to hijack the read operation.

```rust
- async fn read(&self, path: &str, mut args: OpRead) -> Result<(RpRead, Self::Reader)> {
-    if let Some(exec) = args.executor().cloned() {
-        args = args.with_executor(Executor::with(TimeoutExecutor::new(
-            exec.into_inner(),
-            self.io_timeout,
-        )));
-    }
-    
-    ...
- }
```

Instead, we can directly get the executor from the `Context` during `layer`.

```rust
impl<A: Access> Layer<A> for TimeoutLayer {
    type LayeredAccess = TimeoutAccessor<A>;

    fn layer(&self, inner: A) -> Self::LayeredAccess {
        inner
            .info()
            .context()
            .update_executor(|exec| Executor::with(TimeoutExecutor::new(exec, self.io_timeout)));

        TimeoutAccessor {
            inner,

            timeout: self.timeout,
            io_timeout: self.io_timeout,
        }
    }
}
```

# Drawbacks

A bit cost (`50ns`) for every operation that `load_http_client`.

# Rationale and alternatives

None.

# Prior art

None.

# Unresolved questions

None.

# Future possibilities

None.