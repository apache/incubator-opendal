- Proposal Name: `lister_api`
- Start Date: 2023-08-04
- RFC PR: [apache/opendal#2774](https://github.com/apache/opendal/pull/2774)
- Tracking Issue: [apache/opendal#2775](https://github.com/apache/opendal/issues/2775)

# Summary

Add `lister` API to align with other OpenDAL APIs like `read`/`reader`.

# Motivation

Currently OpenDAL has `list` APIs like:

```rust
let lister = op.list().await?;
```

This is inconsistent with APIs like `read`/`reader` and can confuse users.

We should add a new `lister` API and change the `list` to:

- Align with other OpenDAL APIs
- Simplify usage

# Guide-level explanation

The new APIs will be:

```rust
let entries = op.list().await?; // Get entries directly

let lister = op.lister().await?; // Get lister
```

- `op.list()` returns entries directly.
- `op.lister()` returns a lister that users can list entries on demand.

# Reference-level explanation

We will:

- Rename existing `list` to `lister`
- Add new `list` method to call `lister` and return all entries
- Merge `scan` into `list_with` with `delimiter("")`

This keeps the pagination logic encapsulated in `lister`.

# Drawbacks

None

# Rationale and alternatives

None

# Prior art

None

# Unresolved questions

None

# Future possibilities

None
