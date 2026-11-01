- Proposal Name: `remove_native_blocking`
- Start Date: 2025-05-15
- RFC PR: [apache/opendal#6189](https://github.com/apache/opendal/pull/6189)
- Tracking Issue: [apache/opendal#6190](https://github.com/apache/opendal/issues/6190)

# Summary

Remove native blocking support from OpenDAL and use `block_on` in the async runtime instead.

# Motivation

OpenDAL projects offer both native async and blocking support. This means we can provide true blocking functionality when services are capable of it. The advantage is that users who only require blocking operations can enjoy optimal performance. For example, `fs` offers native blocking support, allowing users to utilize `fs` without any additional overhead or performance loss compared to using `tokio_fs`.

However, this approach has several drawbacks:

- Huge maintenance burden: We need to maintain two versions of the same code, which is a significant overhead.
- Increased complexity: The codebase becomes more complex, making it harder to understand and maintain.
- API inconsistency: The API for blocking and async operations may differ, leading to confusion for users, for example, concurrent reading support is not available in blocking APIs.

Apart from the above drawbacks, we also find that the following facts:

- Only a few services have native blocking support, such as `fs`, `memory` and `moka`.
- Only a few users are using `blocking::Operator` (based on [search results from github](https://github.com/search?q=opendal%3A%3Ablocking::Operator&type=code))

So I propose to remove the native blocking support from OpenDAL and use `block_on` in the async runtime instead. This will simplify the codebase, reduce maintenance overhead, and provide a more consistent API for users.

# Guide-level explanation

I plan to move `blocking::Operator` to the `opendal::blocking` module, which will offer a blocking interface for users. The `blocking::Operator` will utilize `block_on` within an async runtime to execute asynchronous operations in a blocking way.

- `opendal::blocking::Operator` will become `opendal::blocking::Operator`
- `opendal::BlockingReader` will become `opendal::blocking::Reader`
- `opendal::BlockingWriter` will become `opendal::blocking::Writer`
- `opendal::BlockingLister` will become `opendal::blocking::Lister`
- `opendal::BlockingDeleter` will become `opendal::blocking::Deleter`

The public API will remain unchanged. The only impacts for users are as follows:

- `blocking::Operator` can no longer be used in an async context
- An async runtime is now required to use the blocking APIs

# Reference-level explanation

I plan to remove all blocking APIs in `oio::Access`:

```diff
pub trait Access: Send + Sync + Debug + Unpin + 'static { 
  ...

  - fn blocking_create_dir(&self, path: &str, args: OpCreateDir) -> Result<RpCreateDir> {
  -     self.as_ref().blocking_create_dir(path, args)
  - }
  - 
  - fn blocking_stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
  -     self.as_ref().blocking_stat(path, args)
  - }
  - 
  - fn blocking_read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::BlockingReader)> {
  -     self.as_ref().blocking_read(path, args)
  - }
  - 
  - fn blocking_write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::BlockingWriter)> {
  -     self.as_ref().blocking_write(path, args)
  - }
  - 
  - fn blocking_delete(&self) -> Result<(RpDelete, Self::BlockingDeleter)> {
  -     self.as_ref().blocking_delete()
  - }
  - 
  - fn blocking_list(&self, path: &str, args: OpList) -> Result<(RpList, Self::BlockingLister)> {
  -     self.as_ref().blocking_list(path, args)
  - }
  - 
  - fn blocking_copy(&self, from: &str, to: &str, args: OpCopy) -> Result<RpCopy> {
  -     self.as_ref().blocking_copy(from, to, args)
  - }
  - 
  - fn blocking_rename(&self, from: &str, to: &str, args: OpRename) -> Result<RpRename> {
  -     self.as_ref().blocking_rename(from, to, args)
  - }
}
```

Due to this change, we will remove all blocking implementations in services and layers. All existing public APIs that start with `Blocking` will be moved into the `blocking` module. For example, `opendal::blocking::Operator` will be relocated to `opendal::blocking::Operator`.

# Drawbacks

Users who rely on blocking APIs will need to adapt their code to use the new `blocking` module. Users might lost the performance benefits of using native blocking APIs, as they will now be using `block_on` in an async runtime.

# Rationale and alternatives

- We could keep the native blocking support, but this would increase the maintenance burden and complexity of the codebase.
- We could provide a separate crate for blocking APIs, but this would lead to API inconsistency and confusion for users.

# Prior art

`reqwest` is using the same approach. It provides a blocking client in `reqwest::blocking` module, which is a wrapper around the async client. This allows users to use blocking APIs without the need for native blocking support.


# Unresolved questions

None

# Future possibilities

None
