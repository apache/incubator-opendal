- Proposal Name: `split-capabilities`
- Start Date: 2022-09-04
- RFC PR: [apache/opendal#627](https://github.com/apache/opendal/pull/627)
- Tracking Issue: [apache/opendal#628](https://github.com/apache/opendal/issues/628)

# Summary

Split basic operations into `read`, `write`, and `list` capabilities.

# Motivation

In [RFC-0409: Accessor Capabilities](./0409-accessor-capabilities.md), we introduce the ideas of `Accessor Capabilities`. Services could have different capabilities, and users can check them via:

```rust
let meta = op.metadata();
let _: bool = meta.can_presign();
let _: bool = meta.can_multipart(); 
```

If users call not supported capabilities, OpenDAL will return [`io::ErrorKind::Unsupported`](https://doc.rust-lang.org/stable/std/io/enum.ErrorKind.html#variant.Unsupported) instead.

Along with that RFC, we also introduce an idea about `Basic Operations`: the operations that all services must support, including:

- metadata
- create
- read
- write
- delete
- list

However, not all storage services support them. In our existing services, exception includes:

- HTTP services don't support `write`, `delete`, and `list`.
- IPFS HTTP gateway doesn't support `write` and `delete`.
  - NOTE: ipfs has a writable HTTP gateway, but there is no available instance.
- fs could be read-only if mounted as `RO`.
- object storage like `s3` and `gcs` could not have enough permission.
- cache services may not support `list`.

So in this RFC, we want to remove the idea about `Basic Operations` and convert them into different capabilities:

- `read`: `read` and `stat`
- `write`: `write` and `delete`
- `list`: `list`

# Guide-level explanation

No public API changes.

# Reference-level explanation

This RFC will add three new capabilities:

- `read`: `read` and `stat`
- `write`: `write` and `delete`
- `list`: `list`

After this change, all services must declare the features they support.

Most of this RFCs work is to refactor the tests. This RFC will refactor the behavior tests into several parts based on capabilities.

# Drawbacks

None

# Rationale and alternatives

None

# Prior art

None

# Unresolved questions

None

# Future possibilities

## Read-only Services

OpenDAL can implement read-only services after this change:

- HTTP Service
- IPFS HTTP Gateway

## Add new capabilities with Layers

We can implement a layer that can add `list` capability for underlying storage services. For example, `IndexLayer` for HTTP services.
