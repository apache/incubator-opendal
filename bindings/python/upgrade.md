# Upgrade to v0.47

## Breaking change: Hugging Face config requires `repo_type`

The `hf` service no longer relies on the backend's implicit `model` default. Set `repo_type` explicitly when creating Hugging Face operators:

```diff
-op = opendal.Operator("hf", repo_id="username/repo")
+op = opendal.Operator("hf", repo_type="model", repo_id="username/repo")
```

Use the appropriate value for the repository: `model`, `dataset`, `space`, or `bucket`.

## Breaking change: Module exports are explicit

`opendal.__init__` now only re-exports the `capability`, `exceptions`, `file`, `layers`, `services`, `types`, `Operator`, and `AsyncOperator` symbols. Imports such as:

```python
from opendal import Metadata, Layer
```

no longer work. Update them to use the dedicated submodules:

```python
from opendal.types import Metadata
from opendal.layers import Layer
```

The legacy helper module `opendal.__base` has also been removed together with `_Base`.

## Breaking change: Capability accessors renamed

Both `Operator.full_capability()` and `AsyncOperator.full_capability()` have been renamed to `capability()`. Adjust your code accordingly:

```diff
-caps = op.full_capability()
+caps = op.capability()
```

## Breaking change: Service identifiers now have typed enums

The constructors for `Operator` / `AsyncOperator` provide overloads that accept `opendal.services.Scheme` members. While plain strings are still accepted at runtime, type checkers (ty/mypy) expect the new enum values. Migrate code bases that relied on importing the old `Scheme` enum from `opendal` to `from opendal import services` and use `services.Scheme.<NAME>`.

# Upgrade to v0.46

## Breaking change: Native blocking API removed

OpenDAL has removed the native blocking API in the core. The Python binding's blocking API now uses an async runtime internally. This is a transparent change and should not affect most users, but:

- The blocking API now requires an async runtime to be available
- Performance characteristics may be slightly different
- All blocking operations are now implemented as async operations running in a tokio runtime

## Breaking change: Removed services

The following services have been removed due to lack of maintainers and users:

- `atomicserver` - This service is no longer supported
- `icloud` - This service is no longer supported
- `nebula-graph` - This service is no longer supported

If you were using any of these services, you'll need to migrate to an alternative storage backend.

## Breaking change: Chainsafe service removed

The Chainsafe service has been sunset and is no longer available.

# Upgrade to v0.44

## Breaking change

Because of [a TLS lib issue](https://github.com/apache/opendal/issues/3650), we temporarily disable the `services-ftp` feature.

# Upgrade to v0.42

## Breaking change for layers

Operator and BlockingOperator won't accept `layers` anymore. Instead, we provide a `layer` API:

```python
op = opendal.Operator("memory").layer(opendal.layers.RetryLayer())
```

We removed not used layers `ConcurrentLimitLayer` and `ImmutableIndexLayer` along with this change.

## File and AsyncFile

OpenDAL removes `Reader` and `AsyncReader` classes, instead, we provide file-like object `File` and `AsyncFile`.

Open a file for reading in blocking way:

```python
with op.open(filename, "rb") as r:
    content = r.read()

# With read options
with op.open(filename, "rb", offset=1024, size=2048) as r:
    content = r.read()
```

Open a file for reading in async way:

```python
async with await op.open(filename, "rb") as r:
    content = await r.read()

# With read options
async with await op.open(filename, "rb", offset=1024, size=2048) as r:
    content = await r.read()
```

Open a file for writing:

```python
# Blocking write
with op.open(filename, "wb") as w:
    w.write(b"hello world")

# With write options
with op.open(filename, "wb", content_type="text/plain", if_not_exists=True) as w:
    w.write(b"hello world")

# Async write
async with await op.open(filename, "wb") as w:
    await w.write(b"hello world")
```

## Breaking change for Errors

We remove the old error classes and provide a couple of Exception based class for the error handling.

1. `opendal.Error` is based class for all the exceptions now.
2. `opendal.exceptions.Unexpected` is added.
3. `opendal.exceptions.Unsupported` is added.
4. `opendal.exceptions.ConfigInvalid` is added.
5. `opendal.exceptions.NotFound` is added.
6. `opendal.exceptions.PermissionDenied` is added.
7. `opendal.exceptions.IsADirectory` is added.
8. `opendal.exceptions.NotADirectory` is added.
9. `opendal.exceptions.AlreadyExists` is added.
10. `opendal.exceptions.IsSameFile` is added.
11. `opendal.exceptions.ConditionNotMatch` is added.
12. `opendal.exceptions.ContentTruncated` is added.
13. `opendal.exceptions.ContentIncomplete` is added.
14. `opendal.exceptions.InvalidInput` is added.
