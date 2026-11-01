- Proposal Name: `path-normalization`
- Start Date: 2022-03-08
- RFC PR: [apache/opendal#112](https://github.com/apache/opendal/pull/112)
- Tracking Issue: [apache/opendal#112](https://github.com/apache/opendal/issues/112)

# Summary

Implement path normalization to enhance user experience.

# Motivation

OpenDAL's current path behavior makes users confused:

- [operator.object("/admin/data/") error](https://github.com/apache/opendal/issues/107)
- [Read /admin/data//ontime_200.csv return empty](https://github.com/apache/opendal/issues/109)

They are different bugs that reflect the exact root cause: the path is not well normalized.

On local fs, we can read the same path with different path: `abc/def/../def`, `abc/def`, `abc//def`, `abc/./def`.

There is no magic here: our stdlib does the dirty job. For example:

- [std::path::PathBuf::canonicalize](https://doc.rust-lang.org/std/path/struct.PathBuf.html#method.canonicalize): Returns the canonical, absolute form of the path with all intermediate components normalized and symbolic links resolved.
- [std::path::PathBuf::components](https://doc.rust-lang.org/std/path/struct.PathBuf.html#method.components): Produces an iterator over the Components of the path. When parsing the path, there is a small amount of normalization...

But for s3 alike storage system, there's no such helpers: `abc/def/../def`, `abc/def`, `abc//def`, `abc/./def` refers entirely different objects. So users may confuse why I can't get the object with this path.

So OpenDAL needs to implement path normalization to enhance the user experience.

# Guide-level explanation

We will do path normalization automatically.

The following rules will be applied (so far):

- Remove `//` inside path: `op.object("abc/def")` and `op.object("abc//def")` will resolve to the same object.
- Make sure path under `root`: `op.object("/abc")` and `op.object("abc")` will resolve to the same object.

Other rules still need more consideration to leave them for the future.

# Reference-level explanation

We will build the absolute path via `{root}/{path}` and replace all `//` into `/` instead.

# Drawbacks

None

# Rationale and alternatives

## How about the link?

If we build an actual path via `{root}/{path}`, the link object may be inaccessible.

I don't have good ideas so far. Maybe we can add a new flag to control the link behavior. For now, there's no feature request for link support.

Let's leave for the future to resolve.

## S3 URI Clean

For s3, `abc//def` is different from `abc/def` indeed. To make it possible to access not normalized path, we can provide a new flag for the builder:

```rust
let builder = Backend::build().disable_path_normalization()
```

In this way, the user can control the path more precisely.

# Prior art

None

# Unresolved questions

None

# Future possibilities

None
