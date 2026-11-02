- Proposal Name: `refactor-error`
- Start Date: 2022-11-21
- RFC PR: [apache/opendal#977](https://github.com/apache/opendal/pull/977)
- Tracking Issue: [apache/opendal#976](https://github.com/apache/opendal/pull/976)

# Summary

Use a separate error instead of `std::io::Error`.

# Motivation

OpenDAL is used to use `std::io::Error` for all functions. This design is natural and easy to use. But there are many problems with the usage:

## Not friendly for retry

`io::Error` can't carry retry-related information. In [RFC-0247: Retryable Error](./0247-retryable-error.md), we use `io::ErrorKind::Interrupt` to indicate this error is retryable. But this change will hide the real error kind from the underlying. To mark this error has been retried, we have to add another new error wrapper:

```rust
#[derive(thiserror::Error, Debug)]
#[error("permanent error: still failing after retry, source: {source}")]
struct PermanentError {
    source: Error,
}
```

## ErrorKind is inaccurate

`std::io::ErrorKind` is used to represent errors returned from system io, which is unsuitable for mistakes that have business semantics. For example, users can't distinguish `ObjectNotFound` or `BucketNotFound` from `ErrorKind::NotFound`.

## ErrorKind is incomplete

OpenDAL has been waiting for features [`io_error_more`](https://github.com/rust-lang/rust/issues/86442) to be stabilized for a long time. But there is no progress so far, which makes it impossible to return `ErrorKind::IsADirectory` or `ErrorKind::NotADirectory` on stable rust.

## Error is not easy to carry context

To carry context inside `std::io::Error`, we have to check and make sure all functions are constructed `ObjectError` or `BackendError`:

```rust
#[derive(Error, Debug)]
#[error("object error: (op: {op}, path: {path}, source: {source})")]
pub struct ObjectError {
    op: Operation,
    path: String,
    source: anyhow::Error,
}
```

To make everything worse, we can't prevent opendal returns raw io errors directly. For example, in `Object::range_read`:

```rust
pub async fn range_read(&self, range: impl RangeBounds<u64>) -> Result<Vec<u8>> {
    ...

    io::copy(s, &mut bs).await?;

    Ok(bs.into_inner())
}
```

We leaked the `io::Error` without any context.

# Guide-level explanation

Thus, I propose to add `opendal::Error` back with everything improved.

Users will have similar usage as before:

```rust
if let Err(e) = op.object("test_file").metadata().await {
    if e.kind() == ErrorKind::ObjectNotFound {
        println!("object not exist")
    }
}
```

Users can check if this error a `temporary`:

```rust
if err.is_temporary() {
    // retry the operation
}
```

Users can print error messages via `Display`:

```rust
> println!("{}", err);

ObjectNotFound (permanent) at read, context: { service: S3, path: path/to/file } => status code: 404, headers: {"x-amz-request-id": "GCYDTQX51YRSF4ZF", "x-amz-id-2": "EH0vV6lTwWk+lFXqCMCBSk1oovqhG4bzALU9+sUudyw7TEVrfWm2o/AFJKhYKpdGqOoBZGgMTC0=", "content-type": "application/xml", "date": "Mon, 21 Nov 2022 05:26:37 GMT", "server": "AmazonS3"}, body: ""
```

Also, users can choose to print the more verbose message via `Debug`:

```rust
> println!("{:?}", err);

ObjectNotFound (permanent) at read => status code: 404, headers: {"x-amz-request-id": "GCYDTQX51YRSF4ZF", "x-amz-id-2": "EH0vV6lTwWk+lFXqCMCBSk1oovqhG4bzALU9+sUudyw7TEVrfWm2o/AFJKhYKpdGqOoBZGgMTC0=", "content-type": "application/xml", "date": "Mon, 21 Nov 2022 05:26:37 GMT", "server": "AmazonS3"}, body: ""

Context:
    service: S3
    path: path/to/file

Source: <source error>

Backtrace: <backtrace if we have>
```

# Reference-level explanation

We will add new `Error` and `ErrorKind` in opendal:

```rust
pub struct Error {
    kind: ErrorKind,
    message: String,

    status: ErrorStatus,
    operation: &'static str,
    context: Vec<(&'static str, String)>,
    source: Option<anyhow::Error>,
}
```

- status: the status of this error, which indicates if this error is temporary
- operation: the operation which generates this error
- context: the context related to this error
- source: the underlying source error

# Drawbacks

## Breaking changes

This RFC will lead to a breaking at user side.

# Rationale and alternatives

None.

# Prior art

None.

# Unresolved questions

None.

# Future possibilities

## More ErrorKind

We can add more error kinds to make it possible for users to check.
