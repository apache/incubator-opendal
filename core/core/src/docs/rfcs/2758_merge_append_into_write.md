- Proposal Name: `merge_append_into_write`
- Start Date: 2023-08-02
- RFC PR: [apache/opendal#2758](https://github.com/apache/opendal/pull/2758)
- Tracking Issue: [apache/opendal#2760](https://github.com/apache/opendal/issues/2760)

# Summary

Merge the `appender` API into `writer` by introducing a new `writer_with.append(true)` method to enable append mode.

# Motivation

Currently OpenDAL has separate `appender` and `writer` APIs:

```rust
let mut appender = op.appender_with("file.txt").await?; 

appender.append(bs).await?;
appender.append(bs).await?;
```

This duplication forces users to learn two different APIs for writing data.

By adding this change, we can:

- Simpler API surface - users only need to learn one writing API.
- Reduce code duplication between append and write implementations.
- Atomic append semantics are handled internally in `writer`.
- Reuse the `sink` api for both `overwrite` and `append` mode.

# Guide-level explanation

The new approach is to enable append mode on `writer`:

```rust 
let mut writer = op.writer_with("file.txt").append(true).await?;

writer.write(bs).await?; // appends to file
writer.write(bs).await?; // appends again
```

Calling `writer_with.append(true)` will start the writer in append mode. Subsequent `write()` calls will append rather than overwrite.

There is no longer a separate `appender` API.

# Reference-level explanation

We will add an `append` flag into `OpWrite`:

```rust
impl OpWrite {   
    pub fn with_append(mut self, append: bool) -> Self {
        self.append = append;
        self
    }
}
```

All services need to check `append` flag and handle append mode accordingly. Services that not support append should return an `Unsupported` error instead.

# Drawbacks

- `writer` API is more complex with the append mode flag.
- Internal implementation must handle both overwrite and append logic.

# Rationale and alternatives

None

# Prior art

Python's file open() supports an `"a"` mode flag to enable append-only writing.

# Unresolved questions

None

# Future possibilities

None
