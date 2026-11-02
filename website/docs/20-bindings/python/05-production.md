---
title: Going to production
sidebar_label: Going to production
description: Make OpenDAL robust in Python — retries, concurrency limits, error handling, and capability checks.
---

# Going to production

The basics read and write data. Production code also has to survive transient
failures, handle errors precisely, and adapt to what a backend supports.

## Layers

A layer wraps an operator to add cross-cutting behavior. `op.layer(...)` returns
a new operator with the layer applied:

```python
import opendal
from opendal.layers import RetryLayer

op = opendal.Operator("s3", bucket="my-bucket", region="us-east-1")
op = op.layer(RetryLayer(max_times=3))
```

The Python binding exposes these layers:

| Layer | What it does |
|-------|--------------|
| `RetryLayer` | Retries operations that fail with temporary errors, using exponential backoff. |
| `ConcurrentLimitLayer` | Caps the number of concurrent requests. |
| `MimeGuessLayer` | Sets `Content-Type` from the file extension. |

```python
from opendal.layers import ConcurrentLimitLayer, RetryLayer

op = (
    opendal.Operator("s3", bucket="my-bucket", region="us-east-1")
    .layer(RetryLayer(max_times=3))
    .layer(ConcurrentLimitLayer(16))
)
```

## Error handling

OpenDAL raises typed exceptions from `opendal.exceptions`. Catch the specific
class instead of inspecting messages:

```python
import opendal
from opendal.exceptions import NotFound

try:
    data = op.read("maybe-missing.txt")
except NotFound:
    data = None
```

Available exceptions include `NotFound`, `PermissionDenied`, `AlreadyExists`,
`ConditionNotMatch`, `RateLimited`, and `Unsupported`, all subclasses of
`opendal.exceptions.Error`. Note that `delete` is idempotent — deleting a missing
path succeeds rather than raising `NotFound`.

## Capability checks {#capability-checks}

Not every service supports every operation. Query what a backend can do with
`capability()` before calling optional operations like `copy`, `rename`, or
presign:

```python
cap = op.capability()
if cap.copy:
    op.copy("a.txt", "b.txt")
```

Calling an unsupported operation raises `opendal.exceptions.Unsupported`, so
capability checks are an optimization, not a requirement for safety.
