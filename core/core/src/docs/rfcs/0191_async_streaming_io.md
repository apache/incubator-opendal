- Proposal Name: `async_streaming_io`
- Start Date: 2022-03-28
- RFC PR: [apache/opendal#191](https://github.com/apache/opendal/pull/191)
- Tracking Issue: [apache/opendal#190](https://github.com/apache/opendal/issues/190)

**Reverted**

# Summary

Use `Stream`/`Sink` instead of `AsyncRead` in `Accessor`.

# Motivation

`Accessor` intends to be the `underlying trait of all backends for implementers`. However, it's not so underlying enough.

## Over-wrapped

`Accessor` returns a `BoxedAsyncReader` for `read` operation:

```rust
pub type BoxedAsyncReader = Box<dyn AsyncRead + Unpin + Send>;

pub trait Accessor {
    async fn read(&self, args: &OpRead) -> Result<BoxedAsyncReader> {
        let _ = args;
        unimplemented!()
    }
}
```

And we are exposing `Reader`, which implements `AsyncRead` and `AsyncSeek` to end-users. For every call to `Reader::poll_read()`, we need:

- `Reader::poll_read()`
- `BoxedAsyncReader::poll_read()`
- `IntoAsyncRead<ByteStream>::poll_read()`
- `ByteStream::poll_next()`

If we could return a `Stream` directly, we can transform the call stack into:

- `Reader::poll_read()`
- `ByteStream::poll_next()`

In this way, we operate on the underlying IO stream, and the caller must keep track of the reading states.

## Inconsistent

OpenDAL's `read` and `write` behavior is not consistent.

```rust
pub type BoxedAsyncReader = Box<dyn AsyncRead + Unpin + Send>;

pub trait Accessor: Send + Sync + Debug {
    async fn read(&self, args: &OpRead) -> Result<BoxedAsyncReader> {
        let _ = args;
        unimplemented!()
    }
    async fn write(&self, r: BoxedAsyncReader, args: &OpWrite) -> Result<usize> {
        let (_, _) = (r, args);
        unimplemented!()
    }
}
```

For `read`, OpenDAL returns a `BoxedAsyncReader` which users can decide when and how to read data. But for `write`, OpenDAL accepts a `BoxedAsyncReader` instead, in which users can't control the writing logic. How large will the writing buffer size be? When to call `flush`?

## Service native optimization

OpenDAL knows more about the service detail, but returning `BoxedAsyncReader` makes it can't fully use the advantage.

For example, most object storage services use HTTP to transfer data which is TCP stream-based. The most efficient way is to return a full TCP buffer, but users don't know about that. First, users could have continuous small reads on stream. To overcome the poor performance, they have to use `BufReader`, which adds a new buffering between reading. Then, users don't know the correct (best) buffer size to set.

Via returning a `Stream`, users could benefit from it in both ways:

- Users who want underlying control can operate on the `Stream` directly.
- Users who don't care about the behavior can use OpenDAL provided Reader, which always adopts the best optimization.

# Guide-level explanation

Within the `async_streaming_io` feature, we will add the following new APIs to `Object`:

```rust
impl Object {
    pub async fn stream(&self, offset: Option<u64>, size: Option<u64>) -> Result<BytesStream> {}
    pub async fn sink(&self, size: u64) -> Result<BytesSink> {}
}
```

Users can control the underlying logic of those bytes, streams, and sinks.

For example, they can:

- Read data on demand: `stream.next().await`
- Write data on demand: `sink.feed(bs).await; sink.close().await;`

Based on `stream` and `sink`, `Object` will provide more optimized helper functions like:

- `async read(offset: Option<u64>, size: Option<u64>) -> Result<bytes::Bytes>`
- `async write(bs: bytes::Bytes) -> Result<()>`

# Reference-level explanation

`read` and `write` in `Accessor` will be refactored into streaming-based:

```rust
pub type BytesStream =  Box<dyn Stream + Unpin + Send>;
pub type BytesSink =  Box<dyn Sink + Unpin + Send>;

pub trait Accessor: Send + Sync + Debug {
    async fn read(&self, args: &OpRead) -> Result<BytesStream> {
        let _ = args;
        unimplemented!()
    }
    async fn write(&self, args: &OpWrite) -> Result<BytesSink> {
        let _ = args;
        unimplemented!()
    }
}
```

All other IO functions will be adapted to fit these changes.

For fs, it's simple to implement `Stream` and `Sink` for `tokio::fs::File`.

We will return a `BodySinker` instead for all HTTP-based storage services. In which we maintain a `put_object` `ResponseFuture` that construct by `hyper` and a `sender` part of the channel. All data sent by users will be passed to `ResponseFuture` via the unbuffered channel.

```rust
struct BodySinker {
    fut: ResponseFuture,
    sender: Sender<bytes::Bytes>
}
```

# Drawbacks

## Performance regression on fs

`fs` is not stream based backend, and convert from `Reader` to `Stream` is not zero cost. Based on benchmark over `IntoStream`, we can get nearly 70% performance drawback (pure memory):

```rust
into_stream/into_stream time:   [1.3046 ms 1.3056 ms 1.3068 ms]
                        thrpt:  [2.9891 GiB/s 2.9919 GiB/s 2.9942 GiB/s]
into_stream/raw_reader  time:   [382.10 us 383.52 us 385.16 us]
                        thrpt:  [10.142 GiB/s 10.185 GiB/s 10.223 GiB/s]
```

However, real fs is not as fast as memory and most overhead will happen at disk side, so that performance regression is allowed (at least at this time).

# Rationale and alternatives

## Performance for switching from Reader to Stream

Before

```rust
read_full/4.00 KiB      time:   [455.70 us 466.18 us 476.93 us]
                        thrpt:  [8.1904 MiB/s 8.3794 MiB/s 8.5719 MiB/s]
read_full/256 KiB       time:   [530.63 us 544.30 us 557.84 us]
                        thrpt:  [448.16 MiB/s 459.30 MiB/s 471.14 MiB/s]
read_full/4.00 MiB      time:   [1.5569 ms 1.6152 ms 1.6743 ms]
                        thrpt:  [2.3330 GiB/s 2.4184 GiB/s 2.5090 GiB/s]
read_full/16.0 MiB      time:   [5.7337 ms 5.9087 ms 6.0813 ms]
                        thrpt:  [2.5693 GiB/s 2.6444 GiB/s 2.7251 GiB/s]
```

After

```rust
read_full/4.00 KiB      time:   [455.67 us 466.03 us 476.21 us]
                        thrpt:  [8.2027 MiB/s 8.3819 MiB/s 8.5725 MiB/s]
                 change:
                        time:   [-2.1168% +0.6241% +3.8735%] (p = 0.68 > 0.05)
                        thrpt:  [-3.7291% -0.6203% +2.1625%]
                        No change in performance detected.
read_full/256 KiB       time:   [521.04 us 535.20 us 548.74 us]
                        thrpt:  [455.59 MiB/s 467.11 MiB/s 479.81 MiB/s]
                 change:
                        time:   [-7.8470% -4.7987% -1.4955%] (p = 0.01 < 0.05)
                        thrpt:  [+1.5182% +5.0406% +8.5152%]
                        Performance has improved.
read_full/4.00 MiB      time:   [1.4571 ms 1.5184 ms 1.5843 ms]
                        thrpt:  [2.4655 GiB/s 2.5725 GiB/s 2.6808 GiB/s]
                 change:
                        time:   [-5.4403% -1.5696% +2.3719%] (p = 0.44 > 0.05)
                        thrpt:  [-2.3170% +1.5946% +5.7533%]
                        No change in performance detected.
read_full/16.0 MiB      time:   [5.0201 ms 5.2105 ms 5.3986 ms]
                        thrpt:  [2.8943 GiB/s 2.9988 GiB/s 3.1125 GiB/s]
                 change:
                        time:   [-15.917% -11.816% -7.5219%] (p = 0.00 < 0.05)
                        thrpt:  [+8.1337% +13.400% +18.930%]
                        Performance has improved.
```

## Performance for the extra channel in `write`

Based on the benchmark during research, the **unbuffered** channel does improve the performance a bit in some cases:

Before:

```rust
write_once/4.00 KiB     time:   [564.11 us 575.17 us 586.15 us]
                        thrpt:  [6.6642 MiB/s 6.7914 MiB/s 6.9246 MiB/s]
write_once/256 KiB      time:   [1.3600 ms 1.3896 ms 1.4168 ms]
                        thrpt:  [176.46 MiB/s 179.90 MiB/s 183.82 MiB/s]
write_once/4.00 MiB     time:   [11.394 ms 11.555 ms 11.717 ms]
                        thrpt:  [341.39 MiB/s 346.18 MiB/s 351.07 MiB/s]
write_once/16.0 MiB     time:   [41.829 ms 42.645 ms 43.454 ms]
                        thrpt:  [368.20 MiB/s 375.19 MiB/s 382.51 MiB/s]
```

After:

```rust
write_once/4.00 KiB     time:   [572.20 us 583.62 us 595.21 us]
                        thrpt:  [6.5628 MiB/s 6.6932 MiB/s 6.8267 MiB/s]
                 change:
                        time:   [-6.3126% -3.8179% -1.0733%] (p = 0.00 < 0.05)
                        thrpt:  [+1.0849% +3.9695% +6.7380%]
                        Performance has improved.
write_once/256 KiB      time:   [1.3192 ms 1.3456 ms 1.3738 ms]
                        thrpt:  [181.98 MiB/s 185.79 MiB/s 189.50 MiB/s]
                 change:
                        time:   [-0.5899% +1.7476% +4.1037%] (p = 0.15 > 0.05)
                        thrpt:  [-3.9420% -1.7176% +0.5934%]
                        No change in performance detected.
write_once/4.00 MiB     time:   [10.855 ms 11.039 ms 11.228 ms]
                        thrpt:  [356.25 MiB/s 362.34 MiB/s 368.51 MiB/s]
                 change:
                        time:   [-6.9651% -4.8176% -2.5681%] (p = 0.00 < 0.05)
                        thrpt:  [+2.6358% +5.0614% +7.4866%]
                        Performance has improved.
write_once/16.0 MiB     time:   [38.706 ms 39.577 ms 40.457 ms]
                        thrpt:  [395.48 MiB/s 404.27 MiB/s 413.37 MiB/s]
                 change:
                        time:   [-10.829% -8.3611% -5.8702%] (p = 0.00 < 0.05)
                        thrpt:  [+6.2363% +9.1240% +12.145%]
                        Performance has improved.
```

## Add complexity on the services side

Returning `Stream` and `Sink` make it complex to implement. At first glance, it does. But in reality, it's not.

Note: HTTP (especially for hyper) is stream-oriented.

- Returning a `stream` is more straightforward than `reader`.
- Returning `Sink` is covered by the global shared `BodySinker` struct.

Other helper functions will be covered at the Object-level which services don't need to bother.

# Prior art

## Returning a `Writer`

The most natural extending is to return `BoxedAsyncWriter`:

```rust
pub trait Accessor: Send + Sync + Debug {
    /// Read data from the underlying storage into input writer.
    async fn read(&self, args: &OpRead) -> Result<BoxedAsyncReader> {
        let _ = args;
        unimplemented!()
    }
    /// Write data from input reader to the underlying storage.
    async fn write(&self, args: &OpWrite) -> Result<BoxedAsyncWriter> {
        let _ = args;
        unimplemented!()
    }
}
```

But it only fixes the `Inconsistent` concern and can't help with other issues.

## Slice based API

Most rust IO APIs are based on slice:

```rust
pub trait Accessor: Send + Sync + Debug {
    /// Read data from the underlying storage into input writer.
    async fn read(&self, args: &OpRead, bs: &mut [u8]) -> Result<usize> {
        let _ = args;
        unimplemented!()
    }
    /// Write data from input reader to the underlying storage.
    async fn write(&self, args: &OpWrite, bs: &[u8]) -> Result<usize> {
        let _ = args;
        unimplemented!()
    }
}
```

The problem is `Accessor` doesn't have states:

- If we require all data must be passed at one time, we can't support large files read & write
- If we allow users to call `read`/`write` multiple times, we need to implement another `Reader` and `Writer` alike logic.

## Accept `Reader` and `Writer`

It's also possible to accept `Reader` and `Writer` instead.

```rust
pub trait Accessor: Send + Sync + Debug {
    /// Read data from the underlying storage into input writer.
    async fn read(&self, args: &OpRead, w: BoxedAsyncWriter) -> Result<usize> {
        let _ = args;
        unimplemented!()
    }
    /// Write data from input reader to the underlying storage.
    async fn write(&self, args: &OpWrite, r: BoxedAsyncReader) -> Result<usize> {
        let _ = args;
        unimplemented!()
    }
}
```

This API design addressed all concerns but made it hard for users to use. Primarily, we can't support `futures::AsyncRead` and `tokio::AsyncRead` simultaneously.

For example, we can't accept a `Box::new(Vec::new())`, user can't get this vec from OpenDAL.

# Unresolved questions

None.

# Future possibilities

- Implement `Object::read_into(w: BoxedAsyncWriter)`
- Implement `Object::write_from(r: BoxedAsyncReader)`
