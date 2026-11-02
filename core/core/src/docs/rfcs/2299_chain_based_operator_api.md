- Proposal Name: `chain_based_operator_api`
- Start Date: 2023-05-23
- RFC PR: [apache/opendal#2299](https://github.com/apache/opendal/pull/2299)
- Tracking Issue: [apache/opendal#2300](https://github.com/apache/opendal/issues/2300)

# Summary

Add chain based Operator API to replace `OpXxx`.

# Motivation

OpenDAL provides `xxx_with` API for users to add more options for requests:

```rust
let bs = op.read_with("path/to/file", OpRead::new().with_range(0..=1024)).await?;
```

However, the API's usability is hindered as users are required to create a new `OpXxx` struct. The API call can be excessively verbose:

```rust
let bs = op.read_with(
  "path/to/file",
  OpRead::new()
    .with_range(0..=1024)
    .with_if_match("<etag>")
    .with_if_none_match("<etag>")
    .with_override_cache_control("<cache_control>")
    .with_override_content_disposition("<content_disposition>")
  ).await?;
```


# Guide-level explanation

In this proposal, I plan to introduce chain based `Operator` API to make them more friendly to use:

```rust
let bs = op.read_with("path/to/file")
  .range(0..=1024)
  .if_match("<etag>")
  .if_none_match("<etag>")
  .override_cache_control("<cache_control>")
  .override_content_disposition("<content_disposition>")
  .await?;
```

By eliminating the usage of `OpXxx`, our users can write code that is more readable.

# Reference-level explanation

To implement chain based API, we will change `read_with` as following:

```diff
- pub async fn read_with(&self, path: &str, args: OpRead) -> Result<Vec<u8>>
+ pub fn read_with(&self, path: &str) -> FutureRead
```

`FutureRead` will implement `Future<Output=Result<Vec<u8>>>`, so that users can still call `read_with` like the following:

```rust
let bs = op.read_with("path/to/file").await?;
```

For blocking operations, we will change `read_with` as following:

```diff
- pub fn read_with(&self, path: &str, args: OpRead) -> Result<Vec<u8>>
+ pub fn read_with(&self, path: &str) -> FunctionRead
```

`FunctionRead` will implement `call(self) -> Result<Vec<u8>>`, so that users can call `read_with` like the following:

```rust
let bs = op.read_with("path/to/file").call()?;
```

After this change, all `OpXxx` will be moved as raw API.

# Drawbacks

None

# Rationale and alternatives

None

# Prior art

None

# Unresolved questions

None

# Future possibilities

## Change API after fn_traits stabilized

After [fn_traits](https://github.com/rust-lang/rust/issues/29625) get stabilized, we will implement `FnOnce` for `FunctionXxx` instead of `call`.
