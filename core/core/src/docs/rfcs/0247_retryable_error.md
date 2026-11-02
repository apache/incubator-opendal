- Proposal Name: `retryable_error`
- Start Date: 2022-04-12
- RFC PR: [apache/opendal#247](https://github.com/apache/opendal/pull/247)
- Tracking Issue: [apache/opendal#248](https://github.com/apache/opendal/issues/248)

# Summary

Treat `io::ErrorKind::Interrupt` as retryable error.

# Motivation

Supports retry make our users' lives easier:

> [Feature request: Custom retries for the s3 backend](https://github.com/apache/opendal/issues/196)
>
> While the reading/writing from/to s3, AWS occasionally returns errors that could be retried (at least 5xx?). Currently, in the databend, this will fail the whole execution of the statement (which may have been running for an extended time).

Most users may need this retry feature, like `decompress`. Implementing it in OpenDAL will make users no bother, no backoff logic.

# Guide-level explanation

With the `retry` feature enabled:

```toml
opendal = {version="0.5.2", features=["retry"]}
```

Users can configure the retry behavior easily:

```rust
let backoff = ExponentialBackoff::default();
let op = op.with_backoff(backoff);
```

All requests sent by `op` will be automatically retried.

# Reference-level explanation

We will implement retry features via adding a new `Layer`.

In the retry layer, we will support retrying all operations. To do our best to keep retrying read & write, we will implement `RetryableReader` and `RetryableWriter`, which will support retry while no actual IO happens.

## Retry operations

Most operations are safe to retry, like `list`, `stat`, `delete` and `create`.

We will retry those operations via input backoff.

## Retry IO operations

Retry IO operations are a bit complex because IO operations have side effects, especially for HTTP-based services like s3. We can't resume an operation during the reading process without sending new requests.

This proposal will do the best we can: retry the operation if no actual IO happens.

If we meet an internal error before reading/writing the user's buffer, it's safe and cheap to retry it with precisely the same argument.

## Retryable Error

- Operator MAY retry `io::ErrorKind::Interrupt` errors.
- Services SHOULD return `io::ErrorKind::Interrupt` kind if the error is retryable.

# Drawbacks

## Write operation can't be retried

As we return `Writer` to users, there is no way for OpenDAL to get the input data again.

# Rationale and alternatives

## Implement retry at operator level

We need to implement retry logic for every operator function, and can't address the same problem:

- `Reader` / `Writer` can't be retired.
- Intrusive design that users cannot expand on their own

# Prior art

None

# Unresolved questions

- `read` and `write` can't be retried during IO.

# Future possibilities

None
