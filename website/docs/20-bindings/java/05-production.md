---
title: Going to production
sidebar_label: Going to production
description: Make OpenDAL robust in Java — retries, concurrency limits, error handling, and capability checks.
---

# Going to production

The basics read and write data. Production code also has to survive transient
failures, handle errors precisely, and adapt to what a backend supports.

## Layers

A layer wraps an operator to add cross-cutting behavior. `AsyncOperator.layer(...)`
returns a new operator with the layer applied; the original is left unchanged,
so close whichever ones you no longer use:

```java
import org.apache.opendal.AsyncOperator;
import org.apache.opendal.layer.RetryLayer;

AsyncOperator op = AsyncOperator.of("s3", conf)
    .layer(RetryLayer.builder().maxTimes(3).build());
```

The Java binding exposes these layers from `org.apache.opendal.layer`:

| Layer | What it does |
|-------|--------------|
| `RetryLayer` | Retries operations that fail with temporary errors, using exponential backoff. Configure with `jitter`, `factor`, `minDelay`, `maxDelay`, and `maxTimes`. |
| `ConcurrentLimitLayer` | Caps the number of concurrent requests to the given number of permits. |
| `CapabilityOverrideLayer` | Overrides the capability an operator reports for a service. |

```java
import org.apache.opendal.AsyncOperator;
import org.apache.opendal.layer.ConcurrentLimitLayer;
import org.apache.opendal.layer.RetryLayer;

AsyncOperator op = AsyncOperator.of("s3", conf)
    .layer(RetryLayer.builder().maxTimes(3).build())
    .layer(new ConcurrentLimitLayer(16));
```

To run a layered operator synchronously, call `.blocking()` after applying the
layers.

## Error handling

OpenDAL throws `OpenDALException`, a `RuntimeException` that carries a typed
error code. Branch on `getCode()` instead of inspecting messages:

```java
import org.apache.opendal.OpenDALException;

try {
    byte[] data = op.read("maybe-missing.txt");
} catch (OpenDALException e) {
    if (e.getCode() == OpenDALException.Code.NotFound) {
        // handle absence
    } else {
        throw e;
    }
}
```

Codes include `NotFound`, `PermissionDenied`, `AlreadyExists`,
`ConditionNotMatch`, `RateLimited`, and `Unsupported`. Note that `delete` is
idempotent — deleting a missing path succeeds rather than throwing `NotFound`.

With `AsyncOperator`, the `OpenDALException` surfaces as the cause of the
`CompletionException` thrown by `join()` / `get()`.

## Capability checks {#capability-checks}

Not every service supports every operation. Query what a backend can do through
`op.info.capability` before calling optional operations like `copy`, `rename`,
or presign:

```java
if (op.info.capability.copy) {
    op.copy("a.txt", "b.txt");
}
```

Calling an unsupported operation throws an `OpenDALException` with code
`Unsupported`, so capability checks are an optimization, not a requirement for
safety.
</content>
