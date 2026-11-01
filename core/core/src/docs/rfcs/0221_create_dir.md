- Proposal Name: `create-dir`
- Start Date: 2022-04-06
- RFC PR: [apache/opendal#221](https://github.com/apache/opendal/pull/221)
- Tracking Issue: [apache/opendal#222](https://github.com/apache/opendal/issues/222)

# Summary

Add creating dir support for OpenDAL.

# Motivation

Interoperability between OpenDAL services requires dir support. The object storage system will simulate dir operations with `/` via object ends. But we can't share the same behavior with `fs`, as `mkdir` is a separate syscall.

So we need to unify the behavior about dir across different services.

# Guide-level explanation

After this proposal got merged, we will treat all paths that end with `/` as a dir.

For example:

- `read("abc/")` will return an `IsDir` error.
- `write("abc/")` will return an `IsDir` error.
- `stat("abc/")` will be guaranteed to return a dir or a `NotDir` error.
- `delete("abc/")` will be guaranteed to delete a dir or `NotDir` / `NotEmpty` error.
- `list("abc/")` will be guaranteed to list a dir or a `NotDir` error.

And we will support create an empty object:

```rust
// create a dir object "abc/"
let _ = op.object("abc/").create().await?;
// create a file object "abc"
let _ = op.object("abc").create().await?;
```

# Reference-level explanation

And we will add a new API called `create` to create an empty object.

```rust
struct OpCreate {
    path: String,
    mode: ObjectMode,
}

pub trait Accessor: Send + Sync + Debug {
    async fn create(&self, args: &OpCreate) -> Result<Metadata>;
}
```

`Object` will expose API like `create` which will call `Accessor::create()` internally.

# Drawbacks

None

# Rationale and alternatives

None

# Prior art

None

# Unresolved questions

When writing this proposal, [io_error_more](https://github.com/rust-lang/rust/issues/86442) is not stabilized yet. We can't use `NotADirectory` nor `IsADirectory` directly.

Using `from_raw_os_error` is unacceptable because we can't carry our error context.

```rust
use std::io;

let error = io::Error::from_raw_os_error(22);
assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
```

So we will use `ErrorKind::Other` for now, which means our users can't check the following errors:

- `IsADirectory`
- `DirectoryNotEmpty`
- `NotADirectory`

Until they get stabilized.

# Future possibilities

None
