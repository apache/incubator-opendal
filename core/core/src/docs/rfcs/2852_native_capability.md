- Proposal Name: `native_capability`
- Start Date: 2023-08-11
- RFC PR: [apache/opendal#2852](https://github.com/apache/opendal/pull/2852)
- Tracking Issue: [apache/opendal#2859](https://github.com/apache/opendal/issues/2859)

# Summary

Add `native_capability` and `full_capability` to `Operator` so that users can make more informed decisions.

# Motivation

OpenDAL adds `Capability` to inform users whether a service supports a specific feature. However, this is not enough for users to make decisions. OpenDAL doesn't simply expose the services' API directly; instead, it simulates the behavior to make it more useful.

For example, `s3` doesn't support seek operations like a local file system. But it's a quite common operation for users. So OpenDAL will try to simulate the behavior by calculating the correct offset and reading the data from that offset instead. After this simulation, the `s3` service has the `read_can_seek` capability now.

As another example, most services like `s3` don't support blocking operations. OpenDAL implements a `BlockingLayer` to make it possible. After this implementation, the `s3` service has the `blocking` capability now.

However, these capabilities alone are insufficient for users to make informed decisions. Take the `s3` service's `blocking` capability as an example. Users are unable to determine whether it is a native capability or not, which may result in them unknowingly utilizing this feature in performance-sensitive scenarios, leading to significantly poor performance.

So this proposal intends to address this issue by adding `native_capability` and `full_capability` to `OperatorInfo`. Users can use `native_capability` to determine whether a capability is native or not.

# Guide-level explanation

We will add two new APIs `native_capability()` and `full_capability()` in `OperatorInfo`, and remove the `capability()` and related `can_xxx()` API.

```diff
+ pub fn native_capability(&self) -> Capability
+ pub fn full_capability(&self) -> Capability
- pub fn capability(&self) -> Capability
```

# Reference-level explanation

We will add two new fields `native_capability` and `full_capability` in `AccessorInfo`:

- Services SHOULD only set `native_capability`, and `full_capability` will be the same as `native_capability`.
- Layers MAY change `full_capability` and MUST NOT modify `native_capability`.
- `OperatorInfo` should forward `native_capability()` and `full_capability()` to `AccessorInfo`.

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
