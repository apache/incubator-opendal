- Proposal Name: `merge_range_read_into_read`
- Start Date: 2023-08-20
- RFC PR: [apache/opendal#2884](https://github.com/apache/opendal/pull/2884)
- Tracking Issue: [apache/opendal#2885](https://github.com/apache/opendal/issues/2885)

# Summary

Merge the `range_read` API into `read` by deleting the `op.range_reader(path, range)` and `op.range_read(path, range)` method.

# Motivation

Currently OpenDAL has separate `range_read` and `read` APIs:

```rust
let bs = op.range_read("path/to/file", 1024..2048).await?;

let bs = op.read("path/to/file").await?;
```

As same as `range_reader` and `reader` APIs:
```rust
let reader = op.range_reader("path/to/file", 1024..2048).await?;

let reader = op.reader("path/to/file").await?;
```


This duplication forces users to learn two different APIs for reading data.

By adding this change, we can:

- Simpler API surface - users only need to learn one writing API.
- Reduce code duplication between read and range_read implementations.
- Atomic read semantics are handled internally in `reader`.

# Guide-level explanation

There is no new approach to read data from file. The `read` and `reader` API supported range read by default.

Calling `read_with("path/to/file").range(range)` will return a `reader` that supports range read.

```rust
let bs = op.read_with("path/to/file").range(1024..2048).await?;
```

Calling `reader_with("path/to/file").range(range)` will return a `reader` that supports range read.

```rust
let rs = op.reader_with("path/to/file").range(1024..2048).await?;
```

There is no longer a separate `range_read` and `range_reader` API.

# Reference-level explanation

None

# Drawbacks

None

## Breaking Changes

This RFC  has removed the `range_read` and `range_reader` APIs. If you have been using these APIs, you will need to reimplement your code by `op.read("path/to/file").range(1024..2048)` or `op.reader("path/to/file").range(1024..2048)`.

# Rationale and alternatives

None

# Prior art

None

# Unresolved questions

None

# Future possibilities

None
