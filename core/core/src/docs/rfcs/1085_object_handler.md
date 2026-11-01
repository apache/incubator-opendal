- Proposal Name: `object_handler`
- Start Date: 2022-12-19
- RFC PR: [apache/opendal#1085](https://github.com/apache/opendal/pull/1085)
- Tracking Issue: [apache/opendal#1085](https://github.com/apache/opendal/issues/1085)

# Summary

Returning a `file description` to users for native seek support.

# Motivation

OpenDAL's goal is to `access data freely, painlessly, and efficiently`, so we build an operation first API which means we provide operation instead of the file description. Users don't need to call `open` before `read`; OpenDAL will handle all the open and close functions.

However, our users do want to control the complex behavior of that:

- Some storage backends have native `seek` support, but OpenDAL can't fully use them.
- Users want to improve performance by reusing the same file description without `open` and `close` for every read operation.

This RFC will fill this gap.


# Guide-level explanation

Users can get an object handler like:

```rust
let oh: ObjectHandler = op.object("path/to/file").open().await?;
```

`ObjectHandler` will implement `AsyncRead` and `AsyncSeek` so it can be used like `tokio::fs::File`. If the backend supports native seek operation, we will take the native process; otherwise, we will fall back to simulation implementations.

The blocking version will be provided by:

```rust
let boh: BlockingObjectHandler = op.object("path/to/file").blocking_open()?;
```

And `BlockingObjectHandler` will implement `Read` and `Seek` so it can be used like `std::fs::File`. If the backend supports native seek operation, we will take the native process; otherwise, we will fall back to simulation implementations.

# Reference-level explanation

This RFC will add a new API `open` in `Accessor`:

```rust
pub trait Accessor {
    async fn open(&self, path: &str, args: OpOpen) -> Result<(RpOpen, BytesHandler)>;
}
```

Only services that support native `seek` operations can implement this API, like `fs` and `hdfs`. For services that do not support native `seek` operations like `s3` and `azblob`, we will fall back to the simulation implementations: maintaining an in-memory index instead.

# Drawbacks

None

# Rationale and alternatives

## How about writing operations?

Ideally, writing on `ObjectHandler` should also be supported. But we still don't know how this API will be used. Let's apply this API for `read` first.

# Prior art

None

# Unresolved questions

None

# Future possibilities

- Add write support
- Adopt native `pread`
