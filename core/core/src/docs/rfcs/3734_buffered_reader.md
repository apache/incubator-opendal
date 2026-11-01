- Proposal Name: `buffered_reader`
- Start Date: 2023-12-10
- RFC PR: [apache/opendal#3574](https://github.com/apache/opendal/pull/3734)
- Tracking Issue: [apache/opendal#3575](https://github.com/apache/opendal/issues/3735)

# Summary

Allowing the underlying reader to fetch data at the buffer's size to amortize the IO's overhead.

# Motivation

The objective is to mitigate the IO overhead. In certain scenarios, the reader processes the data incrementally, meaning that it utilizes the `seek()` function to navigate to a specific position within the file. Subsequently, it invokes the `read()` to reads `limit` bytes into memory and performs the decoding process.


OpenDAL triggers an IO request upon invoking `read()` if the `seek()` has reset the inner state. For storage services like S3, [research](https://www.vldb.org/pvldb/vol16/p2769-durner.pdf) suggests that an optimal IO size falls within the range of 8MiB to 16MiB. If the IO size is too small, the Time To First Byte (TTFB) dominates the overall runtime, resulting in inefficiency.

Therefore, this RFC proposes the implementation of a buffered reader to amortize the overhead of IO.

# Guide-level explanation

For users who want to buffered reader, they will call the new API `buffer`. And the default behavior remains unchanged, so users using `op.reader_with()` are not affected. The `buffer` function will take a number as input, and this number will represent the maximum buffer capability the reader is able to use. 

```rust
op.reader_with(path).buffer(32 * 1024 * 1024).await
```

# Reference-level explanation

This feature will be implemented in the `CompleteLayer`, with the addition of a `BufferReader` struct in `raw/oio/reader/buffer_reader.rs`. 

The `BufferReader` employs a `tokio::io::ReadBuf` as the inner buffer and uses `offset: Option<u64>` to track the buffered range start of the file, the buffered data should always be `file[offset..offset + buf.len()]`.


```rust
     ...
     async fn read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::Reader)> {
          BufferReader::new(self.complete_read(path, args).await)
     }

     ...

    fn blocking_read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::BlockingReader)> {
          BufferReader::new(self.complete_blocking_read(path, args))
    }
     ...
```

A `buffer` field of type `Option<usize>` will be introduced to `OpRead`. If `buffer` is set to `None`, it functions with default behavior. However, if buffer is set to `Some(n)`, it denotes the maximum buffer capability that the `BufferReader` can utilize. The behavior is similar to [std::io::BufReader](https://doc.rust-lang.org/std/io/struct.BufReader.html), with the difference being that our implementation always provides the `seek_relative` (without discarding the inner buffer) if it's available; And it doesn't buffer trailing reads when the read range is smaller than the buffer capability.

# Drawbacks
None

# Rationale and alternatives
None

# Prior art
None

# Unresolved questions
None

# Future possibilities
- Concurrent fetching.
- Tailing buffering.
