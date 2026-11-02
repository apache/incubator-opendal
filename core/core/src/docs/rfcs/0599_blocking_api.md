- Proposal Name: `blocking_api`
- Start Date: 2022-08-30
- RFC PR: [apache/opendal#599](https://github.com/apache/opendal/pull/599)
- Tracking Issue: [apache/opendal#601](https://github.com/apache/opendal/issues/601)

# Summary

We are adding a blocking API for OpenDAL.

# Motivation

Blocking API is the most requested feature inside the OpenDAL community: [Opendal support sync read/write API](https://github.com/apache/opendal/discussions/68)

Our users want blocking API for:

- Higher performance for local IO
- Using OpenDAL in a non-async environment

However, supporting sync and async API in current Rust is a painful job, especially for an IO library like OpenDAL. For example:

```rust
impl Object {
    pub async fn reader(&self) -> Result<impl BytesRead> {}
}
```

Supporting blocking API doesn't mean removing the `async` from the function. We should also handle the returning `Reader`:

```rust
impl Object {
    pub fn reader(&self) -> Result<impl Read> {}
}
```

Until now, I still don't know how to handle them correctly. But we need to have a start: not perfect, but enough for our users to have a try.

So this RFC is an **experiment** try to introduce blocking API support. I expect the OpenDAL community will evaluate those APIs and keep improving them. And finally, we will pick up the best one for stabilizing.

# Guide-level explanation

With this RFC, we can call blocking API with the `blocking_` prefix:

```rust
fn main() -> Result<()> {
    // Init Operator
    let op = Operator::from_env(Scheme::Fs)?;

    // Create object handler.
    let o = op.object("test_file");

    // Write data info object;
    o.blocking_write("Hello, World!")?;

    // Read data from object;
    let bs = o.blocking_read()?;

    // Read range from the object;
    let bs = o.blocking_range_read(1..=11)?;

    // Get the object's path
    let name = o.name();
    let path = o.path();

    // Fetch more meta about the object.
    let meta = o.blocking_metadata()?;
    let mode = meta.mode();
    let length = meta.content_length();
    let content_md5 = meta.content_md5();
    let etag = meta.etag();

    // Delete object.
    o.blocking_delete()?;

    // List dir object.
    let o = op.object("test_dir/");
    let mut ds = o.blocking_list()?;
    while let Some(entry) = ds.try_next()? {
        let path = entry.path();
        let mode = entry.mode();
    }

    Ok(())
}
```

All async public APIs of `Object` and `Operator` will have a sync version with `blocking_` prefix. And they will share precisely the same semantics.

The differences are:

- They will be executed and blocked on the current thread.
- Input and output's `Reader` will become the blocking version like `std::io::Read`.
- Output's `DirStreamer` will become the blocking version like `Iterator`.

Thanks to [RFC-0501: New Builder](./0501-new-builder.md), all our builder-related APIs have been transformed into blocking APIs, so we don't change our initiation logic.

# Reference-level explanation

Under the hood, we will add the following APIs in `Accessor`:

```rust
trait Accessor {
    fn blocking_create(&self, args: &OpCreate) -> Result<()>;
    
    fn blocking_read(&self, args: &OpRead) -> Result<BlockingBytesReader>;
    
    fn blocking_write(&self, args: &OpWrite, r: BlockingBytesReader) -> Result<u64>;
    
    fn blocking_stat(&self, args: &OpStat) -> Result<ObjectMetadata>;
    
    fn blocking_delete(&self, args: &OpDelete) -> Result<()>;
    
    fn blocking_list(&self, args: &OpList) -> Result<DirIterator>;
}
```

Notes:

- `BlockingBytesReader` is a boxed `std::io::Read`.
- All blocking operations are happening on the current thread.
- Blocking operation is implemented natively, no `futures::block_on`.

# Drawbacks

## Two sets of APIs

This RFC will add a new set of APIs, adding complicity for OpenDAL.

And users may misuse them. For example: using `blocking_read` in an async context could block the entire thread.

# Rationale and alternatives

## Use features to switch `async` and `sync`

Some crates provide features to switch the `async` and `sync` versions of API.

In this way:

- We can't provide two kinds of API at the same time.
- Users must decide to use `async` or `sync` at compile time.

## Use blocking IO functions in local fs services

> Can we use blocking IO functions in local fs services to implement Accessor's asynchronous functions directly? What is the drawback of our current non-blocking API?

We can't run blocking IO functions inside the `async` context. We need to let the local thread pool execute them and use `mio` to listen to the events. If we do so, congrats, we are building `tokio::fs` again!

# Prior art

None

# Unresolved questions

None

# Future possibilities

None
