- Proposal Name: `writer_sink_api`
- Start Date: 2023-04-23
- RFC PR: [apache/opendal#2083](https://github.com/apache/opendal/pull/2083)
- Tracking Issue: [apache/opendal#2084](https://github.com/apache/opendal/issues/2084)

# Summary

Include a `sink` API within the `Writer` to enable streaming writing.

# Motivation

OpenDAL does not support streaming data uploads. Users must first load the data into memory and then send it to the `writer`.

```rust
let bs = balabala();
w.write(bs).await?;
let bs = daladala();
w.write(bs).await?;
...
w.close().await?;
```

There are two main drawbacks to OpenDAL:

- high memory usage, as reported in issue #1821 on GitHub
- low performance due to the need to buffer user data before sending it over the network.

To address this issue, it would be beneficial for OpenDAL to provide an API that allows users to pass a stream or reader directly into the writer.

# Guide-level explanation

I propose to add the following API to `Writer`:

```rust
impl Writer {
    pub async fn copy_from<R>(&mut self, size: u64, r: R) -> Result<()>
    where
        R: futures::AsyncRead + Send + Sync + 'static;

    pub async fn pipe_from<S>(&mut self, size: u64, s: S) -> Result<()>
    where
        S: futures::TryStream + Send + Sync + 'static
        Bytes: From<S::Ok>;
}
```

Users can now upload data in a streaming way:

```rust
// Start writing the 5 TiB file.
let w = op.writer_with(
    OpWrite::default()
        .with_content_length(5 * 1024 * 1024 * 1024 * 1024));

let r = balabala();
// Send to network directly without in-memory buffer.
w.copy_from(size, r).await?;
// repeat...
...

// Close the write once we are ready!
w.close().await?;
```

The underlying services will handle this stream in the most efficient way possible.

# Reference-level explanation

To support `Wrtier::copy_from` and `Writer::pipe_from`, we will add a new API called `sink` inside `oio::Writer`:

```rust
#[async_trait]
pub trait Write: Unpin + Send + Sync {
    async fn sink(&mut self, size: u64, s: Box<dyn futures::TryStream<Ok=Bytes> + Send + Sync>) -> Result<()>;
}
```

OpenDAL converts the user input reader and stream into a byte stream for `oio::Write`. Services that support streaming upload natively can directly pass the stream. If not, they can use `write` repeatedly to write the entire stream.

# Drawbacks

None.

# Rationale and alternatives

## What's the different of `OpWrite::content_length` and `sink` size?

The `OpWrite::content_length` parameter specifies the total length of the file to be written, while the `size` argument in the `sink` API indicates the size of the reader or stream provided. Certain services may optimize by writing all content in a single request if `content_length` is the same with given `size`.

# Prior art

None

# Unresolved questions

None

# Future possibilities

## Retry for the `sink` API

It's impossible to retry the `sink` API itself, but we can provide a wrapper to retry the stream's call of `next`. If we met a retryable error, we can call `next` again by crate like `backon`.

## Blocking support for sink

We will add async support first.
