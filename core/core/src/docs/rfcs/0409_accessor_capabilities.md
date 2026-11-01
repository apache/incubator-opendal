- Proposal Name: `accessor_capabilities`
- Start Date: 2022-06-29
- RFC PR: [apache/opendal#409](https://github.com/apache/opendal/pull/409)
- Tracking Issue: [apache/opendal#410](https://github.com/apache/opendal/issues/410)

# Summary

Add support for accessor capabilities so that users can check if a given accessor is capable of a given ability.

# Motivation

Users of OpenDAL are requesting advanced features like the following:

- [Support parallel upload object](https://github.com/apache/opendal/issues/256)
- [Add presign url support](https://github.com/apache/opendal/issues/394)

It's meaningful for OpenDAL to support them in a unified way. Of course, not all storage services have the same feature sets. OpenDAL needs to provide a way for users to check if a given accessor is capable of a given capability.

# Guide-level explanation

Users can check an `Accessor`'s capability via `Operator::metadata()`.

```rust
let meta = op.metadata();
let _: bool = meta.can_presign();
let _: bool = meta.can_multipart(); 
```

`Accessor` will return [`io::ErrorKind::Unsupported`](https://doc.rust-lang.org/stable/std/io/enum.ErrorKind.html#variant.Unsupported) for not supported operations instead of panic as `unimplemented()`.

Users can check before operations or the `Unsupported` error kind after operations.

# Reference-level explanation

We will introduce a new enum called `AccessorCapability`, which includes `AccessorMetadata`.

This enum is private and only accessible inside OpenDAL, so it's not part of our public API. We will expose the check API via `AccessorMetadata`:

```rust
impl AccessorMetadata {
    pub fn can_presign(&self) -> bool { .. }
    pub fn can_multipart(&self) -> bool { .. }
}
```

# Drawbacks

None.

# Rationale and alternatives

None.

# Prior art

## go-storage

- [GSP-109: Redesign Features](https://github.com/beyondstorage/go-storage/blob/master/docs/rfcs/109-redesign-features.md)
- [GSP-837: Support Feature Flag](https://github.com/beyondstorage/go-storage/blob/master/docs/rfcs/837-support-feature-flag.md)

# Unresolved questions

None.

# Future possibilities

None.
