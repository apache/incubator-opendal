- Proposal Name: `deleter_api`
- Start Date: 2024-01-04
- RFC PR: [apache/opendal#3911](https://github.com/apache/opendal/pull/3911)
- Tracking Issue: [apache/opendal#3922](https://github.com/apache/opendal/issues/3922)

# Summary

Introduce the `Deleter` API to enhance batch and recursive deletion capabilities.

# Motivation

All OpenDAL's public API follow the same design:

- `read`: Execute a read operation.
- `read_with`: Execute a read operation with additional options, like range and if_match.
- `reader`: Create a reader for streaming data, enabling flexible access.
- `reader_with`: Create a reader with advanced options.

However, `delete` operations vary. OpenDAL offers several methods for file deletion:

- `delete`: Delete a single file or an empty dir.
- `remove`: Remove a list of files.
- `remove_via`: Remove files produced by a stream.
- `remove_all`: Remove all files under a path.

This design is not consistent with the other APIs, and it is not easy to use.

So I propose `Deleter` to address them all at once.

# Guide-level explanation

The following new API will be added to `Operator`:

```diff
impl Operator {
  pub async fn delete(&self, path: &str) -> Result<()>;
+  pub fn delete_with(&self, path: &str) -> FutureDelete;

+  pub async fn deleter(&self) -> Result<Deleter>;
+  pub fn deleter_with(&self) -> FutureDeleter;
}
```

- `delete` is the existing API, which deletes a single file or an empty dir.
- `delete_with` is an extension of the existing `delete` API, which supports additional options, such as `version`.
- `deleter` is a new API that returns a `Deleter` instance.
- `deleter_with` is an extension of the existing `deleter` API, which supports additional options, such as `concurrent`.

The following new options will be available for `delete_with` and `deleter_with`:

- `concurrent`: How many delete tasks can be performed concurrently?
- `buffer`: How many files can be buffered for send in a single batch?

Users can delete multiple files in this way:


```rust
let deleter = op.deleter().await?;

// Add a single file to the deleter.
deleter.delete(path).await?;

// Add a stream of files to the deleter.
deleter.delete_all(&mut lister).await?;

// Close deleter, make sure all input files are deleted.
deleter.close().await?;
```

`Deleter` also implements [`Sink`](https://docs.rs/futures/latest/futures/sink/trait.Sink.html), so all the methods of `Sink` are available for `Deleter`. For example, users can use [`forward`](https://docs.rs/futures/latest/futures/stream/trait.StreamExt.html#method.forward) to forward a stream of files to `Deleter`:

```rust
// Init a deleter to start batch delete tasks.
let deleter = op.deleter().await?;
// List all files that ends with tmp
let lister = op.lister(path).await?
  .filter(|x|future::ready(x.ends_with(".tmp")));

// Forward all paths into deleter.
lister.forward(deleter).await?;

// Send all from a stream into deleter.
deleter.send_all(&mut lister).await?;

// Close the deleter.
deleter.close().await?;
```

Users can control the behavior of `Deleter` by setting the options:

```rust
let deleter = op.deleter_with()
  // Allow up to 8 concurrent delete tasks, default to 1.
  .concurrent(8)
  // Configure the buffer size to 1000, default value provided by services.
  .buffer(1000)
  .await?;

// Add a single file to the deleter.
deleter.delete(path).await?;

// Add a stream of files to the deleter.
deleter.delete_all(&mut lister).await?;

// Close deleter, make sure all input files are deleted.
deleter.close().await?;
```

In response to `Deleter` API, we will remove APIs like `remove`, `remove_via` and `remove_all`.

- `remove` and `remove_via` could be replaced by `Deleter` directly.
- `remove_all` could be replaced by `delete_with(path).recursive(true)`.

# Reference-level explanation

To provide those public APIs, we will add a new associated type in `Accessor`:

```rust
trait Accessor {
    ...
    
    type Deleter = oio::Delete;
    type BlockingDeleter = oio::BlockingDelete;
}
```

And the `delete` API will be changed to return a `oio::Delete` instead:

```diff
trait Accessor {
-  async fn delete(&self) -> Result<(RpDelete, Self::Deleter)>;
+  async fn delete(&self, args: OpDelete) -> Result<(RpDelete, Self::Deleter)>;
}
```

Along with this change, we will remove the `batch` API from `Accessor`:

```rust
trait Accessor {
-   async fn batch(&self, args: OpBatch) -> Result<RpBatch>;
}
```

# Drawbacks

- Big breaking changes.


# Rationale and alternatives

None.

# Prior art

None.

# Unresolved questions

None.

# Future possibilities

## Add API that accepts `IntoIterator`

It's possible to add a new API that accepts `IntoIterator` so users can input `Vec<String>` or `Iter<String>` into `Deleter`.
