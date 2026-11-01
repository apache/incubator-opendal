- Proposal Name: `dir_entry`
- Start Date: 2022-06-08
- RFC PR: [apache/opendal#337](https://github.com/apache/opendal/pull/337)
- Tracking Issue: [apache/opendal#338](https://github.com/apache/opendal/issues/338)

# Summary

Returning `DirEntry` instead of `Object` in list.

# Motivation

In [Object Stream](./0069-object-stream.md), we introduce read_dir support via:

```rust
pub trait ObjectStream: futures::Stream<Item = Result<Object>> + Unpin + Send {}
impl<T> ObjectStream for T where T: futures::Stream<Item = Result<Object>> + Unpin + Send {}

pub struct Object {
    acc: Arc<dyn Accessor>,
    meta: Metadata,
}
```

However, the `meta` inside `Object` is not well-used:

```rust
pub(crate) fn metadata_ref(&self) -> &Metadata {}
pub(crate) fn metadata_mut(&mut self) -> &mut Metadata {}
pub async fn metadata_cached(&mut self) -> Result<&Metadata> {}
```

Users can't know an object's mode after the list, so they have to send `metadata` every time they get an object:

```rust
let o = op.object("path/to/dir/");
let mut obs = o.list().await?;
// ObjectStream implements `futures::Stream`
while let Some(o) = obs.next().await {
    let mut o = o?;
    // It's highly possible that OpenDAL already did metadata during list.
    // Use `Object::metadata_cached()` to get cached metadata at first.
    let meta = o.metadata_cached().await?;
    match meta.mode() {
        ObjectMode::FILE => {
            println!("Handling file")
        }
        ObjectMode::DIR => {
            println!("Handling dir like start a new list via meta.path()")
        }
        ObjectMode::Unknown => continue,
    }
}
```

This behavior doesn't make sense as we already know the object's mode after the list.

Introducing a separate `DirEntry` could reduce an extra call for metadata most of the time.

```rust
let o = op.object("path/to/dir/");
let mut ds = o.list().await?;
// ObjectStream implements `futures::Stream`
while let Some(de) = ds.try_next().await {
    match de.mode() {
        ObjectMode::FILE => {
            println!("Handling file")
        }
        ObjectMode::DIR => {
            println!("Handling dir like start a new list via meta.path()")
        }
        ObjectMode::Unknown => continue,
    }
}
```

# Guide-level explanation

Within this RFC, `Object::list()` will return `DirStreamer` instead.

```rust
pub trait DirStream: futures::Stream<Item = Result<DirEntry>> + Unpin + Send {}
pub type DirStreamer = Box<dyn DirStream>;
```

`DirStreamer` will stream `DirEntry`, which carries information already known during the list. So we can:

```rust
let id = de.id();
let path = de.path();
let name = de.name();
let mode = de.mode();
let meta = de.metadata().await?;
```

With `DirEntry` support, we can reduce an extra `metadata` call if we only want to know the object's mode:

```rust
let o = op.object("path/to/dir/");
let mut ds = o.list().await?;
// ObjectStream implements `futures::Stream`
while let Some(de) = ds.try_next().await {
    match de.mode() {
        ObjectMode::FILE => {
            println!("Handling file")
        }
        ObjectMode::DIR => {
            println!("Handling dir like start a new list via meta.path()")
        }
        ObjectMode::Unknown => continue,
    }
}
```

We can convert this `DirEntry` into `Object` without overhead:

```rust
let o = de.into();
```

# Reference-level explanation

This proposal will introduce a new struct, `DirEntry`:

```rust
struct DirEntry {}

impl DirEntry {
    pub fn id() -> String {}
    pub fn path() -> &str {}
    pub fn name() -> &str {}
    pub fn mode() -> ObjectMode {}
    pub async fn metadata() -> ObjectMetadata {}
}

impl From<DirEntry> for Object {}
```

And use `DirStream` to replace `ObjectStream`:

```rust
pub trait DirStream: futures::Stream<Item = Result<DirEntry>> + Unpin + Send {}
pub type DirStreamer = Box<dyn DirStream>;
```

With the addition of `DirEntry`, we will remove `meta` from `Object`:

```rust
#[derive(Clone, Debug)]
pub struct Object {
    acc: Arc<dyn Accessor>,
    path: String,
}
```

After this change, `Object` will become a thin wrapper of `Accessor` with path. And metadata related APIs like `metadata_ref()` and `metadata_mut()` will also be removed.

# Drawbacks

We are adding a new concept to our core logic.

# Rationale and alternatives

## Rust fs API design

Rust also provides abstractions like `File` and `DirEntry`:

```rust
use std::fs;

fn main() -> std::io::Result<()> {
    for entry in fs::read_dir(".")? {
        let dir = entry?;
        println!("{:?}", dir.path());
    }
    Ok(())
}
```

Users can open a file with `entry.path()`.

# Prior art

None.

# Unresolved questions

None.

# Future possibilities

None.
