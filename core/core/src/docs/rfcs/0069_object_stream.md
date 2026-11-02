- Proposal Name: `object_stream`
- Start Date: 2022-02-25
- RFC PR: [apache/opendal#69](https://github.com/apache/opendal/pull/69)
- Tracking Issue: [apache/opendal#69](https://github.com/apache/opendal/issues/69)

# Summary

Allow user to read dir via `ObjectStream`.

# Motivation

Users need `readdir` support in `OpenDAL`: [Implement List support](https://github.com/apache/opendal/issues/12). Take [databend] for example, with `List` support, we can implement copy from `s3://bucket/path/to/dir` instead of only `s3://bucket/path/to/file`.

# Guide-level explanation

`Operator` supports new action called `objects("path/to/dir")` which returns a `ObjectStream`, we can iterator current dir like `std::fs::ReadDir`:

```rust
let mut obs = op.objects("").map(|o| o.expect("list object"));
while let Some(o) = obs.next().await {
    // Do something upon `Object`.
}
```

To better support different file modes, there is a new object meta called `ObjectMode`:

```rust
let meta = o.metadata().await?;
let mode = meta.mode();
if mode.contains(ObjectMode::FILE) {
    // Do something on a file object.
} else if mode.contains(ObjectMode::DIR) {
    // Do something on a dir object.
}
```

We will try to cache some object metadata so that users can reduce `stat` calls:

```rust
let meta = o.metadata_cached().await?;
```

`o.metadata_cached()` will return local cached metadata if available.

# Reference-level explanation

First, we will add a new API in `Accessor`:

```rust
pub type BoxedObjectStream = Box<dyn futures::Stream<Item = Result<Object>> + Unpin + Send>;

async fn list(&self, args: &OpList) -> Result<BoxedObjectStream> {
    let _ = args;
    unimplemented!()
}
```

To support options in the future, we will wrap this call via `ObjectStream`:

```rust
pub struct ObjectStream {
    acc: Arc<dyn Accessor>,
    path: String,

    state: State,
}

enum State {
    Idle,
    Sending(BoxFuture<'static, Result<BoxedObjectStream>>),
    Listing(BoxedObjectStream),
}
```

So the public API to end-users will be:

```rust
impl Operator {
    pub fn objects(&self, path: &str) -> ObjectStream {
        ObjectStream::new(self.inner(), path)
    }
}
```

For cached metadata support, we will add a flag in `Metadata`:

```rust
#[derive(Debug, Clone, Default)]
pub struct Metadata {
    complete: bool,

    path: String,
    mode: Option<ObjectMode>,

    content_length: Option<u64>,
}
```

And add new API `Objbct::metadata_cached()`:

```rust
pub async fn metadata_cached(&mut self) -> Result<&Metadata> {
    if self.meta.complete() {
        return Ok(&self.meta);
    }

    let op = &OpStat::new(self.meta.path());
    self.meta = self.acc.stat(op).await?;

    Ok(&self.meta)
}
```

The backend implementer must make sure `complete` is correctly set.

`Metadata` will be immutable outsides, so all `set_xxx` APIs will be set to crate public only:

```rust
pub(crate) fn set_content_length(&mut self, content_length: u64) -> &mut Self {
    self.content_length = Some(content_length);
    self
}
```

# Drawbacks

None

# Rationale and alternatives

None

# Prior art

None

# Unresolved questions

None

# Future possibilities

- More precise field-level metadata cache so that user can send `stat` only when needed.

[databend]: https://github.com/datafuselabs/databend
