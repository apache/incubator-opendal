- Proposal Name: `options_api`
- Start Date: 2025-05-22
- RFC PR: [apache/opendal#6213](https://github.com/apache/opendal/pull/6213)
- Tracking Issue: [apache/opendal#6214](https://github.com/apache/opendal/issues/6214)

# Summary

Introduce a new options-based API pattern for OpenDAL to simplify the construction of operations.

# Motivation

OpenDAL currently offers two API patterns. 

The first is a simple pattern that provides the most basic operations, such as `read`, `write`, and `delete`. These operations do not accept any options and serve as quick shortcuts for users to interact with OpenDAL. 

```rust
let data = op.read("example.txt").await?;
let _ = op.write("example.txt", data).await?;
```

The second is the builder pattern API, which includes methods like `read_with`, `write_with`, and `delete_with`. This approach uses a builder pattern to construct operations with customizable options, offering greater flexibility and allowing users to tailor operations to their specific needs.

```rust
let data = op
    .read_with("example.txt")
    .if_match(etag).await?;
let _ = op.write_with("example.txt", data)
    .chunk(1024 * 1024)
    .concurrent(8)
    .await?;
```

The builder pattern is designed to make the OpenDAL API more flexible and extensible, but it can be cumbersome for users who are trying to construct complex operations that require tuning options at runtime.

For examples, we have seen users write code like:

```rust
let mut write = self
    .core
    .write_with(&path, bs)
    .append(kwargs.append.unwrap_or(false));
if let Some(chunk) = kwargs.chunk {
    write = write.chunk(chunk);
}
if let Some(content_type) = &kwargs.content_type {
    write = write.content_type(content_type);
}
if let Some(content_disposition) = &kwargs.content_disposition {
    write = write.content_disposition(content_disposition);
}
if let Some(cache_control) = &kwargs.cache_control {
    write = write.cache_control(cache_control);
}
if let Some(user_metadata) = kwargs.user_metadata.take() {
    write = write.user_metadata(user_metadata);
}
write.call().map(|_| ()).map_err(format_pyerr)
```

This makes the code hard to read and maintain, especially when there are many options to set.

So I propose to introduce an option based API pattern that allows users to set options in a more structured and readable way. 

This new API will allow users to create structs like `ReadOptions`  which contains all the options they want to set for an operation. They can then pass this struct to the operation, making the code cleaner and easier to understand.

# Guide-level explanation

We will have the following new structs:

```rust
pub struct ReadOptions {
  pub if_match: Option<String>,
  pub if_none_match: Option<String>,
}
```

And we will expose APIs like:

```rust
impl Operator {
    pub async fn read_options(&self, path: &str, opts: ReadOptions) -> Result<Buffer> {
        ...
    }
}
```

Users can then use this API like:

```rust
let opts = ReadOptions {
    version: Some(version_id),
    ...ReadOptions::default(),
};
let data = op.read_options(path, opts).await?;
```

# Reference-level explanation

We will introduce an options API by creating new structs for each operation, with each struct containing all configurable options for that specific operation.

Before calling APIs on `oio::Access`, we will convert `ReadOptions` to `OpRead` and `OpReader`.

# Drawbacks

This RFC will add a new API patterns to OpenDAL, which may increase the complexity of the codebase and the learning curve for new users.

# Rationale and alternatives

## Expose `OpXxx` API AS-IS

It’s a valid alternative to expose the `OpXxx` API as is. However, OpenDAL’s public API is not a direct one-to-one mapping from `Operator` to `oio::Access`. For instance, concurrency and chunk handling for read operations are implemented within the public API, which is why we have a separate `OpReader` struct.

Therefore, we need a new struct that can handle all of these aspects simultaneously.

# Prior art

`object_store` has APIs like `read_opts`:

```rust
trait ObjectStore {
    async fn get_opts(&self, path: &Path, options: GetOptions) {..}
}

pub struct GetOptions {
    pub if_match: Option<String>,
    pub if_none_match: Option<String>,
    pub if_modified_since: Option<DateTime<Utc>>,
    pub if_unmodified_since: Option<DateTime<Utc>>,
    pub range: Option<GetRange>,
    pub version: Option<String>,
    pub head: bool,
    pub extensions: Extensions,
}
```

# Unresolved questions

None

# Future possibilities

None
